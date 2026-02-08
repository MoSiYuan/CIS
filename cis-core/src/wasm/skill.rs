//! # WASM Skill 实现
//!
//! 将 WASM 模块包装为 Skill 接口的实现。

use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use wasmer::{Instance, Memory, MemoryType};

use crate::error::{CisError, Result};
use crate::memory::MemoryServiceTrait;
use crate::skill::{Event, Skill, SkillConfig, SkillContext};

use super::host::{HostEnv, create_host_imports};
use super::{WasmInstance, WasmSkillConfig};

/// WASM 内存页大小（64KB）
const WASM_PAGE_SIZE: usize = 64 * 1024;

/// 默认最大内存（512MB）
const DEFAULT_MAX_MEMORY_MB: usize = 512;

/// WASM Skill 实现
pub struct WasmSkill {
    /// Skill 名称
    name: String,
    /// Skill 版本
    version: String,
    /// Skill 描述
    description: String,
    /// WASM 实例
    instance: Arc<Mutex<WasmInstance>>,
    /// Host 环境
    host_env: HostEnv,
    /// Skill 配置
    config: WasmSkillConfig,
    /// 实例化状态
    instantiated: bool,
}

impl WasmSkill {
    /// 创建新的 WASM Skill
    ///
    /// # 参数
    ///
    /// - `name`: Skill 名称
    /// - `version`: Skill 版本
    /// - `description`: Skill 描述
    /// - `wasm_instance`: WASM 实例
    /// - `memory_service`: 记忆服务
    /// - `config`: 可选的 WASM 配置
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        wasm_instance: WasmInstance,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        config: Option<WasmSkillConfig>,
    ) -> Result<Self> {
        let name = name.into();
        let version = version.into();
        let description = description.into();
        let config = config.unwrap_or_default();

        // 验证配置
        config.validate()?;

        // 创建 AI 回调（简化实现）
        #[allow(clippy::type_complexity)]
        let ai_callback: Arc<Mutex<dyn Fn(&str) -> String + Send + 'static>> = 
            Arc::new(Mutex::new(|prompt: &str| {
                format!("AI response to: {}", prompt)
            }));

        let host_env = HostEnv::new(memory_service, ai_callback);

        Ok(Self {
            name,
            version,
            description,
            instance: Arc::new(Mutex::new(wasm_instance)),
            host_env,
            config,
            instantiated: false,
        })
    }

    /// 计算内存页数限制
    fn get_max_memory_pages(&self) -> u32 {
        let max_memory_mb = self.config.memory_limit
            .map(|bytes| bytes / (1024 * 1024))
            .unwrap_or(DEFAULT_MAX_MEMORY_MB);
        
        // 512MB = 8192 页（每页 64KB）
        let max_pages = (max_memory_mb * 1024 * 1024) / WASM_PAGE_SIZE;
        max_pages.min(65536) as u32 // WebAssembly 最大支持 65536 页（4GB）
    }

    /// 实例化 WASM 模块
    ///
    /// 创建 WASM 实例并链接 Host 函数。
    pub fn instantiate(&mut self) -> Result<()> {
        if self.instantiated {
            tracing::warn!("WASM Skill '{}' already instantiated", self.name);
            return Ok(());
        }

        let mut instance_guard = self.instance.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        
        // 获取 store Arc，保留它以避免临时值被释放
        let binding = instance_guard.store();
        let store_arc = binding.clone();
        
        // 获取 store 的锁
        let mut store = store_arc.lock()
            .map_err(|e| CisError::skill(format!("Store lock failed: {}", e)))?;

        // 创建线性内存，应用内存限制
        let max_pages = self.get_max_memory_pages();
        let memory_type = MemoryType::new(1, Some(max_pages), false);
        let memory = Memory::new(&mut *store, memory_type)
            .map_err(|e| CisError::skill(format!("Failed to create memory: {}", e)))?;

        // 设置 Host 环境内存
        self.host_env.set_memory(memory.clone());

        // 创建 FunctionEnv
        let function_env = wasmer::FunctionEnv::new(&mut *store, self.host_env.clone());

        // 创建 Host 函数导入
        let imports = create_host_imports(&mut store, &function_env);

        // 实例化模块
        let instance = Instance::new(&mut *store, instance_guard.module(), &imports)
            .map_err(|e| CisError::skill(format!("Failed to instantiate WASM module: {}", e)))?;

        // 设置实例
        instance_guard.set_instance(instance);
        self.instantiated = true;

        tracing::info!(
            "WASM Skill '{}' instantiated successfully (max_memory: {} pages)", 
            self.name, max_pages
        );
        Ok(())
    }

    /// 调用 WASM 导出函数
    pub fn call_export(&self, func_name: &str, args: &[wasmer::Value]) -> Result<Vec<wasmer::Value>> {
        if !self.instantiated {
            return Err(CisError::skill("WASM Skill not instantiated"));
        }

        let instance_guard = self.instance.lock()
            .map_err(|e| CisError::skill(format!("Lock failed: {}", e)))?;
        
        let instance = instance_guard.instance()
            .ok_or_else(|| CisError::skill("WASM instance not instantiated"))?;

        // 获取 store Arc，保留它以避免临时值被释放
        let binding = instance_guard.store();
        let store_arc = binding.clone();
        
        let mut store = store_arc.lock()
            .map_err(|e| CisError::skill(format!("Store lock failed: {}", e)))?;

        let func = instance.exports.get_function(func_name)
            .map_err(|e| CisError::skill(format!("Function '{}' not found: {}", func_name, e)))?;

        let result = func.call(&mut *store, args)
            .map_err(|e| CisError::skill(format!("Failed to call function '{}': {}", func_name, e)))?;

        Ok(result.to_vec())
    }

    /// 调用 Skill 初始化函数
    pub fn call_init(&self, config: &SkillConfig) -> Result<()> {
        if !self.instantiated {
            return Err(CisError::skill("WASM Skill not instantiated"));
        }

        // 将配置序列化为 JSON
        let config_json = serde_json::to_string(&config.values)
            .map_err(|e| CisError::skill(format!("Failed to serialize config: {}", e)))?;

        // 注意：实际实现需要将 config_json 写入 WASM 内存
        // 这里简化处理
        tracing::info!("Calling init with config: {}", config_json);

        // 尝试调用 init 函数（如果存在）
        match self.call_export("skill_init", &[]) {
            Ok(_) => Ok(()),
            Err(e) => {
                // 如果函数不存在，记录警告但继续
                tracing::warn!("Skill init function not found or failed: {}", e);
                Ok(())
            }
        }
    }

    /// 调用 Skill 事件处理函数
    pub fn call_handle_event(&self, event: &Event) -> Result<()> {
        if !self.instantiated {
            return Err(CisError::skill("WASM Skill not instantiated"));
        }

        // 将事件序列化为 JSON
        let event_json = serde_json::to_string(event)
            .map_err(|e| CisError::skill(format!("Failed to serialize event: {}", e)))?;

        // 注意：实际实现需要将 event_json 写入 WASM 内存
        tracing::info!("Calling handle_event with: {}", event_json);

        // 尝试调用 handle_event 函数（如果存在）
        match self.call_export("skill_handle_event", &[]) {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::warn!("Skill handle_event function not found or failed: {}", e);
                Ok(())
            }
        }
    }

    /// 调用 Skill 关闭函数
    pub fn call_shutdown(&self) -> Result<()> {
        if !self.instantiated {
            return Ok(());
        }

        // 尝试调用 shutdown 函数（如果存在）
        match self.call_export("skill_shutdown", &[]) {
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("Skill shutdown function not found or failed: {}", e);
            }
        }
        
        tracing::info!("WASM Skill '{}' shutdown", self.name);
        Ok(())
    }

    /// 获取配置
    pub fn config(&self) -> &WasmSkillConfig {
        &self.config
    }

    /// 是否已实例化
    pub fn is_instantiated(&self) -> bool {
        self.instantiated
    }
}

#[async_trait]
impl Skill for WasmSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        &self.version
    }

    fn description(&self) -> &str {
        &self.description
    }

    async fn init(&mut self, config: SkillConfig) -> Result<()> {
        // 实例化 WASM 模块
        self.instantiate()?;
        
        // 调用 WASM init 函数
        self.call_init(&config)?;
        
        tracing::info!("WASM Skill '{}' initialized", self.name);
        Ok(())
    }

    async fn handle_event(&self, _ctx: &dyn SkillContext, event: Event) -> Result<()> {
        self.call_handle_event(&event)?;
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        self.call_shutdown()?;
        Ok(())
    }
}

impl Drop for WasmSkill {
    fn drop(&mut self) {
        // 确保资源被正确释放
        if self.instantiated {
            tracing::debug!("WASM Skill '{}' being dropped, cleaning up resources", self.name);
            
            // 尝试调用 shutdown
            if let Err(e) = self.call_shutdown() {
                tracing::warn!("Error during WASM Skill shutdown: {}", e);
            }
        }
    }
}

/// WASM Skill 构建器
pub struct WasmSkillBuilder {
    name: Option<String>,
    version: Option<String>,
    description: Option<String>,
    wasm_bytes: Option<Vec<u8>>,
    memory_service: Option<Arc<Mutex<dyn MemoryServiceTrait>>>,
    ai_provider: Option<Arc<Mutex<dyn crate::ai::AiProvider>>>,
    config: Option<WasmSkillConfig>,
}

impl WasmSkillBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            name: None,
            version: None,
            description: None,
            wasm_bytes: None,
            memory_service: None,
            ai_provider: None,
            config: None,
        }
    }

    /// 设置名称
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// 设置版本
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// 设置描述
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// 设置 WASM 字节码
    pub fn wasm_bytes(mut self, bytes: Vec<u8>) -> Self {
        self.wasm_bytes = Some(bytes);
        self
    }

    /// 设置记忆服务
    pub fn memory_service(mut self, service: Arc<Mutex<dyn MemoryServiceTrait>>) -> Self {
        self.memory_service = Some(service);
        self
    }

    /// 设置 AI Provider
    pub fn ai_provider(mut self, provider: Arc<Mutex<dyn crate::ai::AiProvider>>) -> Self {
        self.ai_provider = Some(provider);
        self
    }

    /// 设置 WASM 配置
    pub fn config(mut self, config: WasmSkillConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// 构建 WASM Skill
    pub fn build(self) -> Result<WasmSkill> {
        let name = self.name.ok_or_else(|| CisError::skill("Name is required"))?;
        let version = self.version.unwrap_or_else(|| "0.1.0".to_string());
        let description = self.description.unwrap_or_else(|| "WASM Skill".to_string());
        let wasm_bytes = self.wasm_bytes.ok_or_else(|| CisError::skill("WASM bytes are required"))?;
        let memory_service = self.memory_service.ok_or_else(|| CisError::skill("Memory service is required"))?;
        let ai_provider = self.ai_provider.ok_or_else(|| CisError::skill("AI provider is required"))?;
        let config = self.config.unwrap_or_default();

        // 验证配置
        config.validate()?;

        // 克隆服务以便后续使用
        let memory_service_clone = Arc::clone(&memory_service);
        
        // 创建运行时和实例（新版 API）
        let runtime = super::runtime::WasmRuntime::with_config(config.clone())?;
        let skill_instance = runtime.load_skill(&wasm_bytes, memory_service, ai_provider)?;
        
        // 初始化
        skill_instance.init()?;

        // 创建兼容的 WasmInstance
        let module = runtime.load_module(&wasm_bytes)?;
        let (wasm_instance, _store) = create_compatible_instance(module, skill_instance)?;

        WasmSkill::new(name, version, description, wasm_instance, memory_service_clone, Some(config))
    }
}

/// 创建兼容的 WasmInstance（内部函数）
fn create_compatible_instance(
    _module: super::runtime::WasmModule,
    skill_instance: super::runtime::WasmSkillInstance,
) -> Result<(super::WasmInstance, Arc<Mutex<wasmer::Store>>)> {
    use super::WasmInstance;
    use wasmer::{Engine, Module, Store};
    
    // 创建一个新的引擎和存储用于兼容层
    let engine = Engine::default();
    let store = Store::new(engine.clone());
    let store_arc = Arc::new(Mutex::new(store));
    
    // 创建一个空模块（我们不实际使用它，只是为了满足 API）
    let empty_wasm: &[u8] = &[0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
    let compat_module = Module::from_binary(&engine, empty_wasm)
        .map_err(|e| CisError::wasm(format!("Failed to create compat module: {}", e)))?;
    
    let mut instance = WasmInstance::from_module(compat_module, Arc::clone(&store_arc));
    instance.set_runtime_instance(skill_instance);
    
    Ok((instance, store_arc))
}

impl Default for WasmSkillBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm::DEFAULT_MEMORY_LIMIT_BYTES;

    // 简单的 WASM 模块：空模块
    const SIMPLE_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // WASM magic
        0x01, 0x00, 0x00, 0x00, // version 1
    ];

    #[test]
    fn test_wasm_skill_builder() {
        // 这个测试需要一个有效的 WASM 文件和 MemoryService
        // 实际测试需要在有完整环境的情况下运行
        let builder = WasmSkillBuilder::new()
            .name("test-skill")
            .version("1.0.0")
            .description("Test WASM Skill")
            .wasm_bytes(SIMPLE_WASM.to_vec());

        assert_eq!(builder.name, Some("test-skill".to_string()));
        assert_eq!(builder.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_wasm_skill_config() {
        let config = WasmSkillConfig {
            memory_limit: Some(256 * 1024 * 1024), // 256MB
            execution_timeout: Some(60000),         // 60 seconds
            allowed_syscalls: vec!["read".to_string()],
        };

        // 验证配置
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_memory_pages_calculation() {
        // 模拟 WasmSkill 的内存页计算
        let config = WasmSkillConfig {
            memory_limit: Some(512 * 1024 * 1024), // 512MB
            ..Default::default()
        };

        let max_memory_mb = config.memory_limit.unwrap() / (1024 * 1024);
        let max_pages = (max_memory_mb * 1024 * 1024) / WASM_PAGE_SIZE;
        
        assert_eq!(max_pages, 8192); // 512MB / 64KB = 8192 pages
    }

    #[test]
    fn test_default_memory_limit() {
        let config = WasmSkillConfig::default();
        assert_eq!(config.memory_limit, Some(DEFAULT_MEMORY_LIMIT_BYTES));
    }
}
