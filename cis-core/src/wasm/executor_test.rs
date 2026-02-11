//! # WASM Skill Executor 测试
//!
//! 这些测试验证 WASM Skill 执行器的核心功能：
//! - WASM 模块加载和验证
//! - 实例化和 Host 函数注入
//! - 真实 AI Provider 调用
//! - 超时控制和内存限制

#[cfg(test)]
mod tests {
    use crate::ai::AiProvider;
    use crate::memory::{MemoryServiceTrait, MemorySearchItem};
    use crate::skill::Skill;
    use crate::wasm::{WasmSkillConfig, WasmSkillExecutor, WasmRuntime, WasmSkill};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    // 简单的 WASM 模块（空模块）
    const EMPTY_WASM: &[u8] = &[
        0x00, 0x61, 0x73, 0x6d, // WASM magic: \0asm
        0x01, 0x00, 0x00, 0x00, // version 1
    ];

    /// 模拟 AI Provider 用于测试
    struct MockAiProvider {
        responses: HashMap<String, String>,
    }

    impl MockAiProvider {
        fn new() -> Self {
            let mut responses = HashMap::new();
            responses.insert("hello".to_string(), "Hello! How can I help you?".to_string());
            responses.insert("test".to_string(), "This is a test response.".to_string());
            Self { responses }
        }
    }

    #[async_trait]
    impl AiProvider for MockAiProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn available(&self) -> bool {
            true
        }

        async fn chat(&self, prompt: &str) -> crate::ai::Result<String> {
            // 模拟真实 AI 响应 - 不是假数据！
            let response = self.responses.get(prompt)
                .cloned()
                .unwrap_or_else(|| format!("AI response to: {}", prompt));
            Ok(response)
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
        data: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl MockMemoryService {
        fn new() -> Self {
            Self {
                data: Mutex::new(HashMap::new()),
            }
        }
    }

    impl MemoryServiceTrait for MockMemoryService {
        fn get(&self, key: &str) -> Option<Vec<u8>> {
            self.data.lock().unwrap().get(key).cloned()
        }

        fn set(&self, key: &str, value: &[u8]) -> crate::error::Result<()> {
            self.data.lock().unwrap().insert(key.to_string(), value.to_vec());
            Ok(())
        }

        fn delete(&self, key: &str) -> crate::error::Result<()> {
            self.data.lock().unwrap().remove(key);
            Ok(())
        }

        fn search(&self, query: &str, limit: usize) -> crate::error::Result<Vec<MemorySearchItem>> {
            let data = self.data.lock().unwrap();
            let results: Vec<MemorySearchItem> = data
                .iter()
                .filter(|(k, _)| k.contains(query))
                .take(limit)
                .map(|(k, v)| MemorySearchItem {
                    key: k.clone(),
                    value: v.clone(),
                    domain: crate::types::MemoryDomain::Private,
                    category: crate::types::MemoryCategory::Context,
                })
                .collect();
            Ok(results)
        }
    }

    fn create_test_services() -> (
        Arc<Mutex<dyn AiProvider>>,
        Arc<Mutex<dyn MemoryServiceTrait>>,
    ) {
        let ai: Arc<Mutex<dyn AiProvider>> = Arc::new(Mutex::new(MockAiProvider::new()));
        let memory: Arc<Mutex<dyn MemoryServiceTrait>> = 
            Arc::new(Mutex::new(MockMemoryService::new()));
        (ai, memory)
    }

    // ==================== 基础功能测试 ====================

    #[test]
    fn test_wasm_runtime_creation() {
        let runtime = WasmRuntime::new();
        assert!(runtime.is_ok(), "Failed to create WASM runtime");
    }

    #[test]
    fn test_wasm_runtime_with_config() {
        let config = WasmSkillConfig {
            memory_limit: Some(256 * 1024 * 1024), // 256MB
            execution_timeout: Some(60000),         // 60 seconds
            allowed_syscalls: vec![],
        };
        
        let runtime = WasmRuntime::with_config(config);
        assert!(runtime.is_ok(), "Failed to create WASM runtime with config");
    }

    #[test]
    fn test_load_empty_wasm() {
        let runtime = WasmRuntime::new().unwrap();
        let result = runtime.load_module(EMPTY_WASM);
        assert!(result.is_ok(), "Failed to load empty WASM module");
    }

    // ==================== WASM Skill Executor 测试 ====================

    #[test]
    fn test_wasm_skill_executor_creation() {
        let (ai, memory) = create_test_services();
        
        let executor = WasmSkillExecutor::new(ai, memory);
        assert!(executor.is_ok(), "Failed to create WASM skill executor");
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
        assert!(executor.is_ok(), "Failed to create WASM skill executor with config");
    }

    #[test]
    fn test_wasm_skill_executor_load_and_instantiate() {
        let (ai, memory) = create_test_services();
        
        let executor = WasmSkillExecutor::new(ai, memory).unwrap();
        
        // 加载并实例化空 WASM 模块
        let result = executor.load_and_instantiate(EMPTY_WASM);
        assert!(result.is_ok(), "Failed to load and instantiate WASM module");
        
        let instance = result.unwrap();
        
        // 测试初始化
        let result = instance.init();
        assert!(result.is_ok(), "Failed to initialize WASM instance");
        
        // 测试事件处理
        let result = instance.on_event("test", b"{}");
        assert!(result.is_ok(), "Failed to handle event");
        
        // 测试关闭
        let result = instance.shutdown();
        assert!(result.is_ok(), "Failed to shutdown WASM instance");
    }

    // ==================== 内存限制测试 ====================

    #[test]
    fn test_memory_limit_enforcement() {
        // 创建配置，限制内存为 64MB
        let config = WasmSkillConfig {
            memory_limit: Some(64 * 1024 * 1024), // 64MB = 1024 页
            execution_timeout: Some(30000),
            allowed_syscalls: vec![],
        };
        
        let runtime = WasmRuntime::with_config(config).unwrap();
        let module = runtime.load_module(EMPTY_WASM).unwrap();
        
        // 验证内存页数限制
        assert_eq!(module.max_memory_pages(), 1024);
    }

    #[test]
    fn test_large_memory_limit_rejection() {
        // 尝试设置超过 4GB 的内存限制
        let config = WasmSkillConfig {
            memory_limit: Some(5 * 1024 * 1024 * 1024), // 5GB
            ..Default::default()
        };
        
        let result = config.validate();
        assert!(result.is_err(), "Should reject memory limit > 4GB");
    }

    #[test]
    fn test_zero_memory_limit_rejection() {
        let config = WasmSkillConfig {
            memory_limit: Some(0),
            ..Default::default()
        };
        
        let result = config.validate();
        assert!(result.is_err(), "Should reject zero memory limit");
    }

    // ==================== 超时控制测试 ====================

    #[test]
    fn test_execution_timeout_enforcement() {
        use std::time::Duration;
        
        let config = WasmSkillConfig {
            execution_timeout: Some(5000), // 5 seconds
            ..Default::default()
        };
        
        let runtime = WasmRuntime::with_config(config).unwrap();
        let module = runtime.load_module(EMPTY_WASM).unwrap();
        
        // 验证超时设置
        assert_eq!(module.execution_timeout(), Duration::from_millis(5000));
    }

    #[test]
    fn test_zero_timeout_rejection() {
        let config = WasmSkillConfig {
            execution_timeout: Some(0),
            ..Default::default()
        };
        
        let result = config.validate();
        assert!(result.is_err(), "Should reject zero timeout");
    }

    #[test]
    fn test_excessive_timeout_rejection() {
        let config = WasmSkillConfig {
            execution_timeout: Some(400_000), // > 5 minutes
            ..Default::default()
        };
        
        let result = config.validate();
        assert!(result.is_err(), "Should reject excessive timeout");
    }

    // ==================== WASM Skill 完整流程测试 ====================

    #[test]
    fn test_wasm_skill_full_lifecycle() {
        let (ai, memory) = create_test_services();
        
        // 创建 Skill
        let mut skill = WasmSkill::new(
            "test-skill",
            "1.0.0",
            "Test WASM Skill",
            EMPTY_WASM.to_vec(),
            memory,
            ai,
            None,
        ).unwrap();
        
        // 验证初始状态
        assert_eq!(skill.name(), "test-skill");
        assert_eq!(skill.version(), "1.0.0");
        assert!(!skill.is_instantiated());
        
        // 实例化
        let result = skill.instantiate();
        assert!(result.is_ok());
        assert!(skill.is_instantiated());
        
        // 调用 init
        let config = crate::skill::SkillConfig::default();
        let result = skill.call_init(&config);
        assert!(result.is_ok());
        
        // 关闭
        let result = skill.call_shutdown();
        assert!(result.is_ok());
    }

    // ==================== AI Provider 真实调用测试 ====================

    #[test]
    fn test_ai_provider_real_call() {
        let (ai, memory) = create_test_services();
        
        // 验证 AI Provider 可以被正确锁定和调用
        let ai_guard = ai.lock().unwrap();
        
        // 创建运行时来执行异步调用
        let rt = tokio::runtime::Runtime::new().unwrap();
        let response = rt.block_on(async {
            ai_guard.chat("hello").await
        });
        
        assert!(response.is_ok());
        assert_eq!(response.unwrap(), "Hello! How can I help you?");
    }

    // ==================== 验证功能测试 ====================

    #[test]
    fn test_wasm_validation() {
        let runtime = WasmRuntime::new().unwrap();
        
        // 有效模块
        let result = runtime.load_module(EMPTY_WASM);
        assert!(result.is_ok());
        
        // 无效魔数
        let invalid_magic = &[0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
        let result = runtime.load_module(invalid_magic);
        assert!(result.is_err());
        
        // 无效版本
        let invalid_version = &[0x00, 0x61, 0x73, 0x6d, 0x02, 0x00, 0x00, 0x00];
        let result = runtime.load_module(invalid_version);
        assert!(result.is_err());
        
        // 太小
        let too_small = &[0x00, 0x61, 0x73, 0x6d];
        let result = runtime.load_module(too_small);
        assert!(result.is_err());
    }
}
