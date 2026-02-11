//! # WASM Skill 实现
//!
//! 将 WASM 模块包装为 Skill 接口的实现。

use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use wasmer::{Instance, Memory, MemoryType};

use crate::ai::AiProvider;
use crate::error::{CisError, Result};
use crate::memory::MemoryServiceTrait;
use crate::skill::{Event, Skill, SkillConfig, SkillContext};
use crate::storage::DbManager;

use super::runtime::{WasmRuntime, WasmSkillInstance};
use super::WasmSkillConfig;

/// WASM Skill 执行器
/// 
/// 提供 WASM Skill 的完整执行能力，包括：
/// - 模块加载和验证
/// - 实例化和 Host 函数注入
/// - 带超时和内存限制的执行
/// - 真实 AI Provider 调用
pub struct WasmSkillExecutor {
    runtime: Arc<WasmRuntime>,
    ai_provider: Arc<Mutex<dyn AiProvider>>,
    memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    db_manager: Option<Arc<DbManager>>,
}

/// WASM 内存页大小（64KB）
const WASM_PAGE_SIZE: usize = 64 * 1024;

/// 默认最大内存（512MB）
const DEFAULT_MAX_MEMORY_MB: usize = 512;

/// WASM Skill 实现（完整版）
/// 
/// 实现真实的 Skill trait，使用 WasmSkillExecutor 执行 WASM 代码
pub struct WasmSkill {
    /// Skill 名称
    name: String,
    /// Skill 版本
    version: String,
    /// Skill 描述
    description: String,
    /// WASM 字节码
    wasm_bytes: Vec<u8>,
    /// 运行时实例
    runtime_instance: Option<WasmSkillInstance>,
    /// 记忆服务
    memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    /// AI Provider
    ai_provider: Arc<Mutex<dyn AiProvider>>,
    /// Skill 配置
    config: WasmSkillConfig,
    /// 实例化状态
    instantiated: bool,
}

impl WasmSkill {
    /// 创建新的 WASM Skill（完整版）
    ///
    /// # 参数
    ///
    /// - `name`: Skill 名称
    /// - `version`: Skill 版本
    /// - `description`: Skill 描述
    /// - `wasm_bytes`: WASM 字节码
    /// - `memory_service`: 记忆服务
    /// - `ai_provider`: AI Provider（真实调用）
    /// - `config`: 可选的 WASM 配置
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        description: impl Into<String>,
        wasm_bytes: Vec<u8>,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
        ai_provider: Arc<Mutex<dyn AiProvider>>,
        config: Option<WasmSkillConfig>,
    ) -> Result<Self> {
        let name = name.into();
        let version = version.into();
        let description = description.into();
        let config = config.unwrap_or_default();

        // 验证配置
        config.validate()?;

        tracing::info!(
            "Creating WASM Skill '{}' ({} bytes)",
            name,
            wasm_bytes.len()
        );

        Ok(Self {
            name,
            version,
            description,
            wasm_bytes,
            runtime_instance: None,
            memory_service,
            ai_provider,
            config,
            instantiated: false,
        })
    }

    /// 获取 Skill 名称
    pub fn name(&self) -> &str {
        &self.name
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
    /// 创建 WASM 实例并链接 Host 函数，使用真实的 AI Provider。
    pub fn instantiate(&mut self) -> Result<()> {
        if self.instantiated {
            tracing::warn!("WASM Skill '{}' already instantiated", self.name);
            return Ok(());
        }

        // 创建运行时
        let runtime = WasmRuntime::with_config(self.config.clone())?;
        
        // 加载并实例化模块，注入真实 AI Provider 和 Memory Service
        let instance = runtime.load_skill(
            &self.wasm_bytes,
            Arc::clone(&self.memory_service),
            Arc::clone(&self.ai_provider),
        )?;

        self.runtime_instance = Some(instance);
        self.instantiated = true;

        tracing::info!(
            "WASM Skill '{}' instantiated successfully (max_memory: {} pages)", 
            self.name, 
            self.get_max_memory_pages()
        );
        Ok(())
    }

    /// 调用 Skill 初始化函数
    pub fn call_init(&self, _config: &SkillConfig) -> Result<()> {
        if !self.instantiated {
            return Err(CisError::skill("WASM Skill not instantiated"));
        }

        if let Some(ref instance) = self.runtime_instance {
            instance.init()?;
        }

        Ok(())
    }

    /// 调用 Skill 事件处理函数
    pub fn call_handle_event(&self, event: &Event) -> Result<()> {
        if !self.instantiated {
            return Err(CisError::skill("WASM Skill not instantiated"));
        }

        if let Some(ref instance) = self.runtime_instance {
            // 序列化事件
            let event_json = serde_json::to_string(event)
                .map_err(|e| CisError::skill(format!("Failed to serialize event: {}", e)))?;
            
            // 调用 on_event
            instance.on_event("custom", event_json.as_bytes())?;
        }

        Ok(())
    }

    /// 调用 Skill 关闭函数
    pub fn call_shutdown(&self) -> Result<()> {
        if !self.instantiated {
            return Ok(());
        }

        if let Some(ref instance) = self.runtime_instance {
            instance.shutdown()?;
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

    /// 获取内存使用量（字节）
    pub fn memory_usage(&self) -> Option<usize> {
        self.runtime_instance.as_ref().map(|inst| {
            // 这里需要通过其他方式获取内存使用量
            // 暂时返回 0
            0
        })
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
        // 实例化 WASM 模块（带真实 AI Provider）
        self.instantiate()?;
        
        // 调用 WASM init 函数
        self.call_init(&config)?;
        
        tracing::info!(
            "WASM Skill '{}' initialized (memory_limit: {:?} MB, timeout: {:?} ms)",
            self.name,
            self.config.memory_limit.map(|b| b / (1024 * 1024)),
            self.config.execution_timeout
        );
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

    /// 构建 WASM Skill（完整版，使用真实 AI Provider）
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

        // 创建 WASM Skill（此时不实例化，在 init 时实例化）
        WasmSkill::new(
            name, 
            version, 
            description, 
            wasm_bytes, 
            memory_service, 
            ai_provider, 
            Some(config)
        )
    }
}

/// WASM Skill 执行器实现
/// 
/// 提供低级别的 WASM 执行能力
impl WasmSkillExecutor {
    /// 创建新的执行器
    pub fn new(
        ai_provider: Arc<Mutex<dyn AiProvider>>,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    ) -> Result<Self> {
        let runtime = Arc::new(WasmRuntime::new()?);
        
        Ok(Self {
            runtime,
            ai_provider,
            memory_service,
            db_manager: None,
        })
    }

    /// 使用自定义配置创建执行器
    pub fn with_config(
        config: WasmSkillConfig,
        ai_provider: Arc<Mutex<dyn AiProvider>>,
        memory_service: Arc<Mutex<dyn MemoryServiceTrait>>,
    ) -> Result<Self> {
        let runtime = Arc::new(WasmRuntime::with_config(config)?);
        
        Ok(Self {
            runtime,
            ai_provider,
            memory_service,
            db_manager: None,
        })
    }

    /// 设置数据库管理器
    pub fn set_db_manager(&mut self, db_manager: Arc<DbManager>) {
        self.db_manager = Some(db_manager);
    }

    /// 加载并实例化 WASM 模块
    pub fn load_and_instantiate(&self, wasm_bytes: &[u8]) -> Result<WasmSkillInstance> {
        // 加载模块
        let module = self.runtime.load_module(wasm_bytes)?;
        
        // 实例化（带数据库管理器）
        let instance = if let Some(ref db) = self.db_manager {
            module.instantiate_with_db(
                Arc::clone(&self.memory_service),
                Arc::clone(&self.ai_provider),
                Some(Arc::clone(db)),
            )?
        } else {
            module.instantiate(
                Arc::clone(&self.memory_service),
                Arc::clone(&self.ai_provider),
            )?
        };
        
        Ok(instance)
    }

    /// 执行 WASM Skill（完整流程）
    pub async fn execute(
        &self,
        wasm_bytes: &[u8],
        input: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // 1. 加载并实例化
        let instance = self.load_and_instantiate(wasm_bytes)?;
        
        // 2. 初始化
        instance.init()?;
        
        // 3. 准备输入数据
        let input_json = serde_json::to_string(&input)
            .map_err(|e| CisError::skill(format!("Failed to serialize input: {}", e)))?;
        
        // 4. 调用执行函数（如果存在）
        let result = match instance.on_event("execute", input_json.as_bytes()) {
            Ok(code) => {
                serde_json::json!({
                    "success": code >= 0,
                    "code": code,
                })
            }
            Err(e) => {
                tracing::warn!("Execute function not found or failed: {}", e);
                serde_json::json!({
                    "success": false,
                    "error": e.to_string(),
                })
            }
        };
        
        // 5. 关闭
        instance.shutdown()?;
        
        Ok(result)
    }

    /// 获取运行时引用
    pub fn runtime(&self) -> &WasmRuntime {
        &self.runtime
    }

    /// 获取 AI Provider
    pub fn ai_provider(&self) -> Arc<Mutex<dyn AiProvider>> {
        Arc::clone(&self.ai_provider)
    }

    /// 获取 Memory Service
    pub fn memory_service(&self) -> Arc<Mutex<dyn MemoryServiceTrait>> {
        Arc::clone(&self.memory_service)
    }
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
    use crate::ai::AiProvider;
    use crate::memory::MemoryServiceTrait;
    use async_trait::async_trait;

    // 简单的 WASM 模块：空模块
    const SIMPLE_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // WASM magic
        0x01, 0x00, 0x00, 0x00, // version 1
    ];

    /// 模拟 AI Provider 用于测试
    struct MockAiProvider;

    #[async_trait]
    impl AiProvider for MockAiProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn available(&self) -> bool {
            true
        }

        async fn chat(&self, prompt: &str) -> crate::ai::Result<String> {
            Ok(format!("Mock AI response to: {}", prompt))
        }

        async fn chat_with_context(
            &self,
            _system: &str,
            _messages: &[crate::ai::Message],
        ) -> crate::ai::Result<String> {
            Ok("Mock context response".to_string())
        }

        async fn chat_with_rag(
            &self,
            prompt: &str,
            _ctx: Option<&crate::conversation::ConversationContext>,
        ) -> crate::ai::Result<String> {
            Ok(format!("Mock RAG response to: {}", prompt))
        }

        async fn generate_json(
            &self,
            _prompt: &str,
            _schema: &str,
        ) -> crate::ai::Result<serde_json::Value> {
            Ok(serde_json::json!({"mock": true}))
        }
    }

    /// 模拟 Memory Service 用于测试
    struct MockMemoryService {
        data: std::collections::HashMap<String, Vec<u8>>,
    }

    impl MockMemoryService {
        fn new() -> Self {
            Self {
                data: std::collections::HashMap::new(),
            }
        }
    }

    impl MemoryServiceTrait for MockMemoryService {
        fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.data.get(key).cloned()
        }

        fn set(&self, _key: &str, _value: &[u8]) -> crate::error::Result<()> {
            // 注意：由于 trait 使用 &self，这里无法修改 data
            // 实际测试中可以使用 Mutex 包装
            Ok(())
        }

        fn delete(&self, _key: &str) -> crate::error::Result<()> {
            Ok(())
        }

        fn search(&self, _query: &str, _limit: usize) -> crate::error::Result<Vec<crate::memory::MemorySearchItem>> {
            Ok(vec![])
        }
    }

    fn create_test_services() -> (
        Arc<Mutex<dyn AiProvider>>,
        Arc<Mutex<dyn MemoryServiceTrait>>,
    ) {
        let ai: Arc<Mutex<dyn AiProvider>> = Arc::new(Mutex::new(MockAiProvider));
        let memory: Arc<Mutex<dyn MemoryServiceTrait>> = 
            Arc::new(Mutex::new(MockMemoryService::new()));
        (ai, memory)
    }

    #[test]
    fn test_wasm_skill_builder() {
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

    #[test]
    fn test_wasm_skill_executor_creation() {
        let (ai, memory) = create_test_services();
        
        let executor = WasmSkillExecutor::new(ai, memory);
        assert!(executor.is_ok());
    }

    #[test]
    fn test_wasm_skill_executor_with_config() {
        let (ai, memory) = create_test_services();
        
        let config = WasmSkillConfig {
            memory_limit: Some(128 * 1024 * 1024), // 128MB
            execution_timeout: Some(10000),         // 10 seconds
            allowed_syscalls: vec![],
        };
        
        let executor = WasmSkillExecutor::with_config(config, ai, memory);
        assert!(executor.is_ok());
    }

    #[test]
    fn test_wasm_skill_creation() {
        let (ai, memory) = create_test_services();
        
        let skill = WasmSkill::new(
            "test-skill",
            "1.0.0",
            "Test WASM Skill",
            SIMPLE_WASM.to_vec(),
            memory,
            ai,
            None,
        );
        
        assert!(skill.is_ok());
        
        let skill = skill.unwrap();
        assert_eq!(skill.name(), "test-skill");
        assert_eq!(skill.version(), "1.0.0");
        assert_eq!(skill.description(), "Test WASM Skill");
        assert!(!skill.is_instantiated());
    }

    #[test]
    fn test_wasm_skill_with_custom_config() {
        let (ai, memory) = create_test_services();
        
        let config = WasmSkillConfig {
            memory_limit: Some(256 * 1024 * 1024),
            execution_timeout: Some(5000),
            allowed_syscalls: vec!["read".to_string()],
        };
        
        let skill = WasmSkill::new(
            "test-skill",
            "1.0.0",
            "Test WASM Skill",
            SIMPLE_WASM.to_vec(),
            memory,
            ai,
            Some(config),
        );
        
        assert!(skill.is_ok());
        let skill = skill.unwrap();
        assert_eq!(skill.config().memory_limit, Some(256 * 1024 * 1024));
        assert_eq!(skill.config().execution_timeout, Some(5000));
    }

    #[test]
    fn test_wasm_skill_instantiate() {
        let (ai, memory) = create_test_services();
        
        let mut skill = WasmSkill::new(
            "test-skill",
            "1.0.0",
            "Test WASM Skill",
            SIMPLE_WASM.to_vec(),
            memory,
            ai,
            None,
        ).unwrap();
        
        // 实例化
        let result = skill.instantiate();
        assert!(result.is_ok());
        assert!(skill.is_instantiated());
        
        // 再次实例化应该直接返回成功
        let result = skill.instantiate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_wasm_skill_builder_full() {
        let (ai, memory) = create_test_services();
        
        let result = WasmSkillBuilder::new()
            .name("test-skill")
            .version("1.0.0")
            .description("Test WASM Skill")
            .wasm_bytes(SIMPLE_WASM.to_vec())
            .memory_service(memory)
            .ai_provider(ai)
            .build();
        
        assert!(result.is_ok());
        let skill = result.unwrap();
        assert_eq!(skill.name(), "test-skill");
    }

    #[test]
    fn test_wasm_skill_executor_load_simple_wasm() {
        let (ai, memory) = create_test_services();
        
        let executor = WasmSkillExecutor::new(ai, memory).unwrap();
        
        // 加载并实例化简单 WASM 模块
        let result = executor.load_and_instantiate(SIMPLE_WASM);
        assert!(result.is_ok());
        
        let instance = result.unwrap();
        
        // 测试初始化
        let result = instance.init();
        assert!(result.is_ok());
        
        // 测试事件处理
        let result = instance.on_event("test", b"{}");
        assert!(result.is_ok());
        
        // 测试关闭
        let result = instance.shutdown();
        assert!(result.is_ok());
    }

    #[test]
    fn test_wasm_skill_config_validation() {
        // 无效：内存限制为 0
        let config = WasmSkillConfig {
            memory_limit: Some(0),
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // 无效：内存限制超过 4GB
        let config = WasmSkillConfig {
            memory_limit: Some(5 * 1024 * 1024 * 1024), // 5GB
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // 无效：超时为 0
        let config = WasmSkillConfig {
            execution_timeout: Some(0),
            ..Default::default()
        };
        assert!(config.validate().is_err());
        
        // 无效：超时超过 5 分钟
        let config = WasmSkillConfig {
            execution_timeout: Some(400_000), // > 5 分钟
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
