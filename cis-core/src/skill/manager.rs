//! Skill 热插拔管理器
//!
//! 管理 Skill 的完整生命周期：安装、加载、激活、暂停、卸载、移除。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot};

use super::permission_checker::{CheckContext, PermissionChecker, PermissionScope, ResourcePattern};
use super::registry::SkillRegistry;
use super::types::{LoadOptions, SkillConfig, SkillInfo, SkillMeta, SkillState, SkillType};
use super::{Event, Skill, SkillContext};
use crate::error::{CisError, Result};
use crate::storage::db::DbManager;
use crate::storage::paths::Paths;

#[cfg(feature = "wasm")]
use crate::wasm::WasmRuntime;

/// Skill 事件循环命令
#[derive(Debug)]
enum SkillEventCommand {
    /// 处理事件
    HandleEvent(Event),
    /// 停止事件循环
    Stop,
}

/// 活跃的 Skill 实例
struct ActiveSkill {
    /// Skill 元数据
    _info: SkillInfo,
    /// 配置
    config: SkillConfig,
    /// Skill 实例（用于事件处理）
    skill: Arc<dyn Skill>,
    /// 事件发送通道
    event_sender: Option<mpsc::UnboundedSender<SkillEventCommand>>,
    /// 停止信号
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ActiveSkill {
    /// 检查 Skill 是否处于活跃状态
    fn is_active(&self) -> bool {
        self.event_sender.is_some()
    }

    /// 安全关闭 Skill 事件循环
    async fn shutdown(&mut self) {
        // 发送停止命令到事件循环
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(SkillEventCommand::Stop);
        }
        
        // 发送 shutdown 信号（备用）
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        
        // 清除通道引用
        self.event_sender = None;
        
        // 小延迟确保事件循环有时间处理停止命令
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }
}

impl Drop for ActiveSkill {
    fn drop(&mut self) {
        // 尝试同步关闭（仅清理资源，不阻塞）
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(SkillEventCommand::Stop);
        }
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

/// 简单的 SkillContext 实现
struct SimpleSkillContext {
    config: SkillConfig,
}

impl SimpleSkillContext {
    fn new(config: SkillConfig) -> Self {
        Self { config }
    }
}

impl SkillContext for SimpleSkillContext {
    fn log_info(&self, message: &str) {
        tracing::info!("[Skill] {}", message);
    }
    
    fn log_debug(&self, message: &str) {
        tracing::debug!("[Skill] {}", message);
    }
    
    fn log_warn(&self, message: &str) {
        tracing::warn!("[Skill] {}", message);
    }
    
    fn log_error(&self, message: &str) {
        tracing::error!("[Skill] {}", message);
    }
    
    fn memory_get(&self, _key: &str) -> Option<Vec<u8>> {
        None
    }
    
    fn memory_set(&self, _key: &str, _value: &[u8]) -> Result<()> {
        Ok(())
    }
    
    fn memory_delete(&self, _key: &str) -> Result<()> {
        Ok(())
    }
    
    fn config(&self) -> &SkillConfig {
        &self.config
    }
}

/// 占位 Skill 实现（用于未提供具体实例的情况）
struct DummySkill {
    name: String,
}

impl DummySkill {
    fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl Skill for DummySkill {
    fn name(&self) -> &str {
        &self.name
    }
    
    async fn handle_event(&self, _ctx: &dyn SkillContext, _event: Event) -> Result<()> {
        // 占位实现，不处理任何事件
        Ok(())
    }
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
    /// Permission checker
    permission_checker: Arc<PermissionChecker>,
}

impl SkillManager {
    /// 创建新的 Skill 管理器
    pub fn new(db_manager: Arc<DbManager>) -> Result<Self> {
        let registry = Arc::new(Mutex::new(SkillRegistry::load()?));

        #[cfg(feature = "wasm")]
        let wasm_runtime = Arc::new(Mutex::new(WasmRuntime::new()?));

        let permission_checker = Arc::new(PermissionChecker::new()?);

        Ok(Self {
            db_manager,
            registry,
            active_skills: Arc::new(Mutex::new(HashMap::new())),
            #[cfg(feature = "wasm")]
            wasm_runtime,
            permission_checker,
        })
    }

    /// Get permission checker
    pub fn permission_checker(&self) -> Arc<PermissionChecker> {
        self.permission_checker.clone()
    }

    /// 加载 WASM Skill
    ///
    /// 从 WASM 字节码加载并实例化 Skill。
    #[cfg(feature = "wasm")]
    pub fn load_wasm(&self, name: &str, wasm_bytes: &[u8], options: LoadOptions) -> Result<()> {
        use crate::memory::MemoryService;
        use std::sync::Mutex as StdMutex;

        // 验证 WASM 魔术数字
        crate::validate_wasm_magic(wasm_bytes)?;

        // 检查 WASM 字节码大小
        crate::check_allocation_size(wasm_bytes.len(), 128 * 1024 * 1024)?; // 128MB 限制

        tracing::info!("Loading WASM skill '{}'...", name);

        // 获取或创建记忆服务
        let memory_service: Arc<StdMutex<dyn crate::memory::MemoryServiceTrait>> = 
            Arc::new(StdMutex::new(MemoryService::open_default("default")?));

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
        // 验证 Skill 名称
        crate::check_string_length(&meta.name, 256)?;
        
        let mut registry = self.registry.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;

        registry.register(meta)?;
        tracing::info!("Skill registered successfully");

        Ok(())
    }

    /// 注销 Skill
    ///
    /// 从系统中移除 Skill 注册信息。
    pub async fn unregister(&self, name: &str) -> Result<()> {
        // 先卸载（如果已加载）
        if self.is_loaded(name)? {
            self.unload(name).await?;
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
    pub async fn load(&self, name: &str, options: LoadOptions) -> Result<()> {
        // 验证 Skill 名称长度
        crate::check_string_length(name, 256)?;

        // 检查是否已加载
        if self.is_loaded(name)? && !options.force_reload {
            return Ok(());
        }

        // 先卸载（如果已加载且强制重载）
        if self.is_loaded(name)? && options.force_reload {
            self.unload(name).await?;
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
        // 使用 DummySkill 作为占位，实际的 skill 实例应该通过 register_skill_instance 注册
        let active_skill = ActiveSkill {
            _info: info.clone(),
            db: skill_db,
            config,
            skill: Arc::new(DummySkill::new(name.to_string())),
            event_sender: None,
            shutdown_tx: None,
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
        // Note: auto_activate 需要调用者显式调用 activate()，因为 activate 现在是 async
        if options.auto_activate {
            tracing::info!("Skill '{}' marked for auto-activation. Call activate() to start event loop.", name);
        }

        Ok(())
    }

    /// 卸载 Skill
    ///
    /// 关闭数据库连接，释放资源，支持热插拔。
    pub async fn unload(&self, name: &str) -> Result<()> {
        if !self.is_loaded(name)? {
            return Ok(());
        }

        tracing::info!("Unloading skill '{}'...", name);

        // 1. 先停用（如果处于活跃状态）
        if self.is_active(name)? {
            self.deactivate(name).await?;
        }

        // 2. 从活跃列表移除（先取出以触发 Drop）
        let active_skill_to_shutdown = {
            let mut active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            
            // 取出以触发 Drop 执行
            active_skills.remove(name)
        };
        
        // 确保事件循环已停止（在锁外执行 await）
        if let Some(mut active_skill) = active_skill_to_shutdown {
            active_skill.shutdown().await;
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
    pub async fn activate(&self, name: &str) -> Result<()> {
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

        // 获取 skill 实例以启动事件循环
        let skill_to_spawn = {
            let active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            
            if let Some(active_skill) = active_skills.get(name) {
                active_skill.skill.clone()
            } else {
                return Err(CisError::skill(format!("Skill '{}' not found in active skills", name)));
            }
        };

        // 创建事件通道
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SkillEventCommand>();
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // 启动事件处理循环
        let skill_name = name.to_string();
        let _active_skills = self.active_skills.clone();
        
        tokio::spawn(async move {
            tracing::info!("Skill '{}' event loop started", skill_name);
            
            loop {
                tokio::select! {
                    // 接收事件命令
                    Some(cmd) = event_rx.recv() => {
                        match cmd {
                            SkillEventCommand::HandleEvent(event) => {
                                let ctx = SimpleSkillContext::new(SkillConfig::default());
                                if let Err(e) = skill_to_spawn.handle_event(&ctx, event).await {
                                    tracing::error!("Skill '{}' event handler error: {}", skill_name, e);
                                }
                            }
                            SkillEventCommand::Stop => {
                                tracing::info!("Skill '{}' event loop stopping", skill_name);
                                break;
                            }
                        }
                    }
                    // 接收停止信号
                    _ = &mut shutdown_rx => {
                        tracing::info!("Skill '{}' received shutdown signal", skill_name);
                        break;
                    }
                    // 优雅退出：当所有发送者都关闭时
                    else => {
                        tracing::info!("Skill '{}' event loop ended (all senders closed)", skill_name);
                        break;
                    }
                }
            }
            
            tracing::info!("Skill '{}' event loop stopped", skill_name);
        });

        // 保存通道到 ActiveSkill
        {
            let mut active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            
            if let Some(active_skill) = active_skills.get_mut(name) {
                active_skill.event_sender = Some(event_tx);
                active_skill.shutdown_tx = Some(shutdown_tx);
            }
        }

        registry.update_state(name, SkillState::Active)?;

        tracing::info!("Skill '{}' activated", name);

        Ok(())
    }

    /// 停用 Skill
    ///
    /// 停止 Skill 处理事件，但保持加载状态。
    pub async fn deactivate(&self, name: &str) -> Result<()> {
        let is_active = {
            let registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            
            let info = registry
                .get(name)
                .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", name)))?;
            
            info.runtime.state.is_active()
        };

        if !is_active {
            return Ok(());
        }

        tracing::info!("Deactivating skill '{}'...", name);

        // 停止事件循环
        {
            let mut active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            
            if let Some(active_skill) = active_skills.get_mut(name) {
                // 发送停止命令
                if let Some(ref sender) = active_skill.event_sender {
                    let _ = sender.send(SkillEventCommand::Stop);
                }
                // 或者发送 shutdown 信号
                if let Some(shutdown_tx) = active_skill.shutdown_tx.take() {
                    let _ = shutdown_tx.send(());
                }
                // 清除通道
                active_skill.event_sender = None;
            }
        }

        // 等待一小段时间确保事件循环停止
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // 重新获取锁并更新状态
        {
            let mut registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            registry.update_state(name, SkillState::Loaded)?;
        }

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
        
        // 验证 manifest 大小
        crate::check_string_length(&manifest_content, 1024 * 1024)?; // 1MB 限制
        
        let meta: SkillMeta = toml::from_str(&manifest_content)
            .map_err(|e| CisError::skill(format!("Failed to parse skill.toml: {}", e)))?;

        // 验证 Skill 名称
        crate::check_string_length(&meta.name, 256)?;
        
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
    pub async fn remove(&self, name: &str) -> Result<()> {
        tracing::info!("Removing skill '{}'...", name);

        // 1. 注销（会先卸载）
        self.unregister(name).await?;

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

    /// 发送事件到指定 Skill
    ///
    /// 用于外部模块（如 GLM API）触发 Skill 的事件处理
    pub async fn send_event(&self, skill_name: &str, event: Event) -> Result<()> {
        // 验证 Skill 名称
        crate::check_string_length(skill_name, 256)?;
        
        // 检查 skill 是否活跃，并获取事件发送通道
        let event_sender = {
            let active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            
            if let Some(active) = active_skills.get(skill_name) {
                active.event_sender.clone()
            } else {
                None
            }
        };

        if let Some(sender) = event_sender {
            // 通过通道发送事件
            sender.send(SkillEventCommand::HandleEvent(event))
                .map_err(|_| CisError::skill(format!("Skill '{}' event channel closed", skill_name)))?;
            
            Ok(())
        } else {
            // Skill 没有事件循环（可能未激活），尝试直接调用
            let skill = {
                let active_skills = self.active_skills.lock()
                    .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
                
                active_skills.get(skill_name).map(|active| (active.skill.clone(), active.config.clone()))
            };

            if let Some((skill, config)) = skill {
                let ctx = SimpleSkillContext::new(config);
                skill.handle_event(&ctx, event).await
                    .map_err(|e| CisError::skill(format!("Event handling failed: {}", e)))?;
                Ok(())
            } else {
                Err(CisError::skill(format!("Skill '{}' is not loaded", skill_name)))
            }
        }
    }

    /// 注册 Skill 实例（用于原生 Skill，如 DagExecutorSkill）
    ///
    /// 允许外部模块注册一个具体的 Skill trait 对象，使其可以接收事件
    pub async fn register_skill_instance(&self, skill: Arc<dyn Skill>, options: LoadOptions) -> Result<()> {
        let name = skill.name().to_string();
        
        // 验证 Skill 名称
        crate::check_string_length(&name, 256)?;
        
        // 检查是否已加载
        if self.is_loaded(&name)? && !options.force_reload {
            return Ok(());
        }
        
        // 先卸载（如果已加载且强制重载）
        if self.is_loaded(&name)? && options.force_reload {
            self.unload(&name).await?;
        }
        
        // 获取或创建注册信息
        let info = {
            let registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            registry.get(&name).cloned()
        };
        
        // 如果未注册，自动注册
        let info = if let Some(info) = info {
            info
        } else {
            // 创建默认的 SkillInfo
            SkillInfo {
                meta: SkillMeta {
                    name: name.clone(),
                    version: skill.version().to_string(),
                    description: skill.description().to_string(),
                    author: "system".to_string(),
                    skill_type: SkillType::Native,
                    path: String::new(),
                    db_path: String::new(),
                    permissions: vec![],
                    subscriptions: vec![],
                    config_schema: None,
                    room_config: None,
                },
                runtime: super::types::SkillRuntime {
                    state: SkillState::Registered,
                    loaded_at: None,
                    last_active_at: None,
                    error: None,
                    pid: None,
                },
            }
        };
        
        // 加载 Skill 数据库
        let skill_db = self.db_manager.load_skill_db(&name)?;
        
        // 确保 Skill 数据目录存在
        let skill_data_dir = Paths::skill_data_dir(&name);
        std::fs::create_dir_all(&skill_data_dir)?;
        
        // 初始化配置
        let config = options.config.unwrap_or_default();
        
        // 创建活跃 Skill 记录
        let active_skill = ActiveSkill {
            _info: info.clone(),
            db: skill_db,
            config,
            skill,
            event_sender: None,
            shutdown_tx: None,
        };
        
        // 添加到活跃列表
        {
            let mut active_skills = self.active_skills.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            active_skills.insert(name.clone(), active_skill);
        }
        
        // 更新注册表状态
        {
            let mut registry = self.registry.lock()
                .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
            registry.update_state(&name, SkillState::Active)?;
        }
        
        tracing::info!("Skill instance '{}' registered successfully", name);
        Ok(())
    }

    /// 安全关闭所有 Skills
    ///
    /// 在应用关闭时调用，确保所有 Skill 资源被正确释放
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down SkillManager...");
        
        // 获取所有已加载的 Skill 名称
        let skill_names = self.list_loaded()?;
        
        // 逐个卸载
        for name in skill_names {
            if let Err(e) = self.unload(&name).await {
                tracing::warn!("Failed to unload skill '{}': {}", name, e);
            }
        }
        
        tracing::info!("SkillManager shutdown completed");
        Ok(())
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

    fn setup_test_env() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static TEST_ID: AtomicU64 = AtomicU64::new(0);
        let test_id = TEST_ID.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir()
            .join("cis_test_skill_manager")
            .join(format!("pid_{}_test_{}", std::process::id(), test_id));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::env::set_var("CIS_DATA_DIR", &temp_dir);
        crate::storage::paths::Paths::ensure_dirs().unwrap();
        temp_dir
    }

    fn cleanup_test_env(temp_dir: &std::path::Path) {
        std::env::remove_var("CIS_DATA_DIR");
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_skill_lifecycle() {
        let temp_dir = setup_test_env();

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
        manager.load("test-skill", LoadOptions::default()).await.unwrap();
        assert!(manager.is_loaded("test-skill").unwrap());

        // 激活
        manager.activate("test-skill").await.unwrap();
        assert!(manager.is_active("test-skill").unwrap());

        // 停用
        manager.deactivate("test-skill").await.unwrap();
        assert!(!manager.is_active("test-skill").unwrap());

        // 卸载
        manager.unload("test-skill").await.unwrap();
        assert!(!manager.is_loaded("test-skill").unwrap());

        // 注销
        manager.unregister("test-skill").await.unwrap();
        assert!(manager.get_info("test-skill").unwrap().is_none());

        cleanup_test_env(&temp_dir);
    }

    #[test]
    fn test_skill_name_validation() {
        // 测试名称长度验证
        let long_name = "a".repeat(300);
        assert!(crate::check_string_length(&long_name, 256).is_err());
        
        let valid_name = "a".repeat(255);
        assert!(crate::check_string_length(&valid_name, 256).is_ok());
    }
}
