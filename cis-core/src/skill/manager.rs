//! Skill 热插拔管理器
//!
//! 管理 Skill 的完整生命周期：安装、加载、激活、暂停、卸载、移除。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::registry::SkillRegistry;
use super::types::{LoadOptions, SkillConfig, SkillInfo, SkillMeta, SkillState, SkillType};
use crate::error::{CisError, Result};
use crate::storage::db::{DbManager, SkillDb};
use crate::storage::paths::Paths;

#[cfg(feature = "wasm")]
use crate::wasm::WasmRuntime;

/// 活跃的 Skill 实例
struct ActiveSkill {
    /// Skill 元数据
    _info: SkillInfo,
    /// Skill 数据库连接
    db: Arc<Mutex<SkillDb>>,
    /// 配置
    config: SkillConfig,
}

/// Skill 管理器
pub struct SkillManager {
    /// 数据库管理器
    db_manager: Arc<DbManager>,
    /// Skill 注册表
    registry: Arc<Mutex<SkillRegistry>>,
    /// 活跃的 Skills
    active_skills: Arc<Mutex<HashMap<String, ActiveSkill>>>,
    /// WASM 运行时（仅在 wasm 特性启用时）
    #[cfg(feature = "wasm")]
    wasm_runtime: Arc<Mutex<WasmRuntime>>,
}

impl SkillManager {
    /// 创建新的 Skill 管理器
    pub fn new(db_manager: Arc<DbManager>) -> Result<Self> {
        let registry = Arc::new(Mutex::new(SkillRegistry::load()?));

        #[cfg(feature = "wasm")]
        let wasm_runtime = Arc::new(Mutex::new(WasmRuntime::new()?));

        Ok(Self {
            db_manager,
            registry,
            active_skills: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "wasm")]
            wasm_runtime,
        })
    }

    /// 加载 WASM Skill
    ///
    /// 从 WASM 字节码加载并实例化 Skill。
    #[cfg(feature = "wasm")]
    pub fn load_wasm(&self, name: &str, wasm_bytes: &[u8], options: LoadOptions) -> Result<()> {
        use crate::memory::MemoryService;
        use std::sync::Mutex as StdMutex;

        tracing::info!("Loading WASM skill '{}'...", name);

        // 获取或创建记忆服务
        let core_db = self.db_manager.core();
        let memory_service: Arc<StdMutex<dyn crate::memory::MemoryServiceTrait>> = 
            Arc::new(StdMutex::new(MemoryService::new(core_db)));

        // 使用 WasmSkillBuilder 构建 WASM Skill
        let mut wasm_skill = crate::wasm::WasmSkillBuilder::new()
            .name(name)
            .version("1.0.0")
            .description("WASM Skill")
            .wasm_bytes(wasm_bytes.to_vec())
            .memory_service(memory_service)
            .build()?;

        // 实例化 WASM 模块
        wasm_skill.instantiate()?;

        // 初始化 Skill
        let config = options.config.unwrap_or_default();
        wasm_skill.call_init(&config)?;

        tracing::info!("WASM skill '{}' loaded successfully", name);

        // 自动激活（如果启用）
        if options.auto_activate {
            tracing::info!("Activating WASM skill '{}'...", name);
        }

        Ok(())
    }

    // ==================== 注册/注销 ====================

    /// 注册 Skill
    ///
    /// 将已安装的 Skill 注册到系统中，但不加载。
    pub fn register(&self, meta: SkillMeta) -> Result<()> {
        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        registry.register(meta)?;
        tracing::info!("Skill registered successfully");

        Ok(())
    }

    /// 注销 Skill
    ///
    /// 从系统中移除 Skill 注册信息。
    pub fn unregister(&self, name: &str) -> Result<()> {
        // 先卸载（如果已加载）
        if self.is_loaded(name)? {
            self.unload(name)?;
        }

        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        registry.unregister(name)?;
        tracing::info!("Skill '{}' unregistered", name);

        Ok(())
    }

    // ==================== 加载/卸载（热插拔核心）====================

    /// 加载 Skill
    ///
    /// 创建数据库连接，初始化 Skill，但不激活。
    pub fn load(&self, name: &str, options: LoadOptions) -> Result<()> {
        // 检查是否已加载
        if self.is_loaded(name)? && !options.force_reload {
            return Ok(());
        }

        // 先卸载（如果已加载且强制重载）
        if self.is_loaded(name)? && options.force_reload {
            self.unload(name)?;
        }

        // 获取 Skill 信息
        let info = {
            let registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            registry
                .get(name)
                .cloned()
                .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?
        };

        // 检查状态
        if !info.runtime.state.can_load() {
            return Err(CisError::skill(format!(
                "Skill '{}' cannot be loaded from state {:?}",
                name, info.runtime.state
            )));
        }

        tracing::info!("Loading skill '{}'...", name);

        // 1. 加载 Skill 数据库
        let skill_db = self.db_manager.load_skill_db(name)?;
        
        // 1.5 挂载到多库连接（支持跨库查询）
        if let Err(e) = self.db_manager.attach_skill_db(name) {
            tracing::warn!("Failed to attach skill db to multi-connection: {}", e);
            // 不中断加载流程，只是警告
        }

        // 2. 确保 Skill 数据目录存在
        let skill_data_dir = Paths::skill_data_dir(name);
        std::fs::create_dir_all(&skill_data_dir)?;

        // 3. 初始化配置
        let config = options.config.unwrap_or_default();

        // 4. 创建活跃 Skill 记录
        let active_skill = ActiveSkill {
            _info: info.clone(),
            db: skill_db,
            config,
        };

        // 5. 添加到活跃列表
        {
            let mut active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            active_skills.insert(name.to_string(), active_skill);
        }

        // 6. 更新注册表状态
        {
            let mut registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            registry.update_state(name, SkillState::Loaded)?;
        }

        tracing::info!("Skill '{}' loaded successfully", name);

        // 7. 自动激活（如果启用）
        if options.auto_activate {
            self.activate(name)?;
        }

        Ok(())
    }

    /// 卸载 Skill
    ///
    /// 关闭数据库连接，释放资源，支持热插拔。
    pub fn unload(&self, name: &str) -> Result<()> {
        if !self.is_loaded(name)? {
            return Ok(());
        }

        tracing::info!("Unloading skill '{}'...", name);

        // 1. 先停用（如果处于活跃状态）
        if self.is_active(name)? {
            self.deactivate(name)?;
        }

        // 2. 从活跃列表移除
        {
            let mut active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            active_skills.remove(name);
        }

        // 3. 从多库连接卸载
        if let Err(e) = self.db_manager.detach_skill_db(name) {
            tracing::warn!("Failed to detach skill db from multi-connection: {}", e);
        }

        // 4. 关闭数据库连接
        self.db_manager.unload_skill_db(name)?;

        // 4. 更新注册表状态
        {
            let mut registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            registry.update_state(name, SkillState::Unloaded)?;
        }

        tracing::info!("Skill '{}' unloaded successfully", name);

        Ok(())
    }

    /// 检查 Skill 是否已加载
    pub fn is_loaded(&self, name: &str) -> Result<bool> {
        let active_skills = self.active_skills.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        Ok(active_skills.contains_key(name))
    }

    /// 检查 Skill 是否处于活跃状态
    pub fn is_active(&self, name: &str) -> Result<bool> {
        let registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        
        if let Some(info) = registry.get(name) {
            Ok(info.runtime.state.is_active())
        } else {
            Ok(false)
        }
    }

    // ==================== 激活/停用 ====================

    /// 激活 Skill
    ///
    /// 使 Skill 开始处理事件。
    pub fn activate(&self, name: &str) -> Result<()> {
        if !self.is_loaded(name)? {
            return Err(CisError::skill(format!(
                "Skill '{}' is not loaded, load first",
                name
            )));
        }

        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        let info = registry
            .get(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        if !info.runtime.state.can_pause() && info.runtime.state != SkillState::Loaded {
            return Err(CisError::skill(format!(
                "Skill '{}' cannot be activated from state {:?}",
                name, info.runtime.state
            )));
        }

        tracing::info!("Activating skill '{}'...", name);

        // TODO: 启动 Skill 的事件循环

        registry.update_state(name, SkillState::Active)?;

        tracing::info!("Skill '{}' activated", name);

        Ok(())
    }

    /// 停用 Skill
    ///
    /// 停止 Skill 处理事件，但保持加载状态。
    pub fn deactivate(&self, name: &str) -> Result<()> {
        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        let info = registry
            .get(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        if !info.runtime.state.is_active() {
            return Ok(());
        }

        tracing::info!("Deactivating skill '{}'...", name);

        // TODO: 停止 Skill 的事件循环

        registry.update_state(name, SkillState::Loaded)?;

        tracing::info!("Skill '{}' deactivated", name);

        Ok(())
    }

    // ==================== 暂停/恢复 ====================

    /// 暂停 Skill
    pub fn pause(&self, name: &str) -> Result<()> {
        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        let info = registry
            .get(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        if !info.runtime.state.can_pause() {
            return Err(CisError::skill(format!(
                "Skill '{}' cannot be paused from state {:?}",
                name, info.runtime.state
            )));
        }

        registry.update_state(name, SkillState::Paused)?;
        tracing::info!("Skill '{}' paused", name);

        Ok(())
    }

    /// 恢复 Skill
    pub fn resume(&self, name: &str) -> Result<()> {
        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        let info = registry
            .get(name)
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;

        if !info.runtime.state.can_resume() {
            return Err(CisError::skill(format!(
                "Skill '{}' cannot be resumed from state {:?}",
                name, info.runtime.state
            )));
        }

        registry.update_state(name, SkillState::Active)?;
        tracing::info!("Skill '{}' resumed", name);

        Ok(())
    }

    // ==================== 安装/移除 ====================

    /// 安装 Skill
    ///
    /// 从路径安装 Skill 到系统。
    pub fn install(&self, source_path: &std::path::Path, skill_type: SkillType) -> Result<SkillMeta> {
        // 读取 Skill 元数据
        let manifest_path = source_path.join("skill.toml");
        if !manifest_path.exists() {
            return Err(CisError::skill(
                "skill.toml not found in source path"
            ));
        }

        let manifest_content = std::fs::read_to_string(&manifest_path)?;
        let meta: SkillMeta = toml::from_str(&manifest_content)
            .map_err(|e| CisError::skill(format!("Failed to parse skill.toml: {}", e)))?;

        let name = &meta.name;
        tracing::info!("Installing skill '{}' from {:?}...", name, source_path);

        // 确定目标路径
        let target_dir = match skill_type {
            SkillType::Native => Paths::skills_native_dir().join(name),
            SkillType::Wasm => Paths::skills_wasm_dir().join(format!("{}.wasm", name)),
            _ => return Err(CisError::skill("Unsupported skill type")),
        };

        // 复制文件
        if target_dir.exists() {
            std::fs::remove_dir_all(&target_dir)?;
        }

        if skill_type == SkillType::Native {
            // 复制整个目录
            Self::copy_dir_all(source_path, &target_dir)?;
        } else {
            // 复制单个 WASM 文件
            std::fs::copy(source_path, &target_dir)?;
        }

        // 创建数据目录
        let data_dir = Paths::skill_data_dir(name);
        std::fs::create_dir_all(&data_dir)?;

        // 注册到系统
        self.register(meta.clone())?;

        tracing::info!("Skill '{}' installed successfully", name);

        Ok(meta)
    }

    /// 移除 Skill
    ///
    /// 完全卸载并删除 Skill。
    pub fn remove(&self, name: &str) -> Result<()> {
        tracing::info!("Removing skill '{}'...", name);

        // 1. 注销（会先卸载）
        self.unregister(name)?;

        // 2. 删除代码
        let native_path = Paths::skills_native_dir().join(name);
        if native_path.exists() {
            std::fs::remove_dir_all(&native_path)?;
        }

        let wasm_path = Paths::skills_wasm_dir().join(format!("{}.wasm", name));
        if wasm_path.exists() {
            std::fs::remove_file(&wasm_path)?;
        }

        // 3. 询问是否删除数据
        let data_dir = Paths::skill_data_dir(name);
        if data_dir.exists() {
            tracing::warn!(
                "Skill data directory still exists at {:?}. Remove manually if needed.",
                data_dir
            );
        }

        tracing::info!("Skill '{}' removed successfully", name);

        Ok(())
    }

    // ==================== 查询 ====================

    /// 获取 Skill 信息
    pub fn get_info(&self, name: &str) -> Result<Option<SkillInfo>> {
        let registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        Ok(registry.get(name).cloned())
    }

    /// 列出所有 Skills
    pub fn list_all(&self) -> Result<Vec<SkillInfo>> {
        let registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        Ok(registry.list_all().into_iter().cloned().collect())
    }

    /// 列出活跃 Skills
    pub fn list_active(&self) -> Result<Vec<SkillInfo>> {
        let registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        Ok(registry.list_active().into_iter().cloned().collect())
    }

    /// 获取已加载的 Skill 名称列表
    pub fn list_loaded(&self) -> Result<Vec<String>> {
        let active_skills = self.active_skills.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        Ok(active_skills.keys().cloned().collect())
    }
    
    /// 获取 Skill Registry（用于直接访问 registry 操作）
    pub fn get_registry(&self) -> Result<std::sync::MutexGuard<'_, SkillRegistry>> {
        self.registry.lock()
            .map_err(|e| CisError::skill(format!("Registry lock failed: {}", e)))
    }
    
    /// 获取 WASM Runtime（用于执行 WASM skills）
    #[cfg(feature = "wasm")]
    pub fn get_wasm_runtime(&self) -> Result<Arc<std::sync::Mutex<WasmRuntime>>> {
        Ok(self.wasm_runtime.clone())
    }

    // ==================== 工具方法 ====================

    /// 递归复制目录
    fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
        std::fs::create_dir_all(dst)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = path.file_name()
                .ok_or_else(|| CisError::skill("Invalid file name"))?;
            let dest_path = dst.join(file_name);

            if path.is_dir() {
                Self::copy_dir_all(&path, &dest_path)?;
            } else {
                std::fs::copy(&path, &dest_path)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_env() {
        let temp_dir = std::env::temp_dir().join("cis_test_skill_manager");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::env::set_var("CIS_DATA_DIR", &temp_dir);
        crate::storage::paths::Paths::ensure_dirs().unwrap();
    }

    fn cleanup_test_env() {
        std::env::remove_var("CIS_DATA_DIR");
    }

    #[test]
    fn test_skill_lifecycle() {
        setup_test_env();

        let db_manager = Arc::new(DbManager::new().unwrap());
        let manager = SkillManager::new(db_manager).unwrap();

        // 注册
        let meta = SkillMeta {
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            description: "Test".to_string(),
            author: "Test".to_string(),
            skill_type: SkillType::Native,
            path: "/test".to_string(),
            db_path: "/test/db".to_string(),
            permissions: vec![],
            subscriptions: vec![],
            config_schema: None,
            room_config: None,
        };

        manager.register(meta).unwrap();
        assert!(manager.get_info("test-skill").unwrap().is_some());

        // 加载
        manager.load("test-skill", LoadOptions::default()).unwrap();
        assert!(manager.is_loaded("test-skill").unwrap());

        // 激活
        manager.activate("test-skill").unwrap();
        assert!(manager.is_active("test-skill").unwrap());

        // 停用
        manager.deactivate("test-skill").unwrap();
        assert!(!manager.is_active("test-skill").unwrap());

        // 卸载
        manager.unload("test-skill").unwrap();
        assert!(!manager.is_loaded("test-skill").unwrap());

        // 注销
        manager.unregister("test-skill").unwrap();
        assert!(manager.get_info("test-skill").unwrap().is_none());

        cleanup_test_env();
    }
}
