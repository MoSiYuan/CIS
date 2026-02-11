//! E2E Skill 执行测试
//!
//! 端到端测试 Skill 的完整执行流程。

use cis_core::wasm::{WasmRuntime, WasmSkillConfig, WasmSkillBuilder, WasmSkillExecutor};
use cis_core::skill::{Skill, SkillConfig};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// 简单的 WASM 模块
const SIMPLE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 模拟 AI Provider
struct MockAiProvider {
    responses: Mutex<Vec<String>>,
}

impl MockAiProvider {
    fn new() -> Self {
        Self {
            responses: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl cis_core::ai::AiProvider for MockAiProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn available(&self) -> bool {
        true
    }

    async fn chat(&self, prompt: &str) -> cis_core::ai::Result<String> {
        let response = format!("AI Response: {}", prompt);
        self.responses.lock().unwrap().push(prompt.to_string());
        Ok(response)
    }

    async fn chat_with_context(
        &self,
        _system: &str,
        _messages: &[cis_core::ai::Message],
    ) -> cis_core::ai::Result<String> {
        Ok("Context response".to_string())
    }

    async fn chat_with_rag(
        &self,
        prompt: &str,
        _ctx: Option<&cis_core::conversation::ConversationContext>,
    ) -> cis_core::ai::Result<String> {
        Ok(format!("RAG: {}", prompt))
    }

    async fn generate_json(
        &self,
        _prompt: &str,
        _schema: &str,
    ) -> cis_core::ai::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": "success"}))
    }
}

/// 模拟记忆服务
struct MockMemoryService {
    data: Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl MockMemoryService {
    fn new() -> Self {
        Self {
            data: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn stored_count(&self) -> usize {
        self.data.lock().unwrap().len()
    }
}

impl cis_core::memory::MemoryServiceTrait for MockMemoryService {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.data.lock().unwrap().get(key).cloned()
    }

    fn set(&self, key: &str, value: &[u8]) -> cis_core::error::Result<()> {
        self.data.lock().unwrap().insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn delete(&self, key: &str) -> cis_core::error::Result<()> {
        self.data.lock().unwrap().remove(key);
        Ok(())
    }

    fn search(&self, _query: &str, _limit: usize) -> cis_core::error::Result<Vec<cis_core::memory::MemorySearchItem>> {
        Ok(vec![])
    }
}

fn create_test_services() -> (
    Arc<Mutex<dyn cis_core::ai::AiProvider>>,
    Arc<Mutex<dyn cis_core::memory::MemoryServiceTrait>>,
) {
    let ai: Arc<Mutex<dyn cis_core::ai::AiProvider>> = Arc::new(Mutex::new(MockAiProvider::new()));
    let memory: Arc<Mutex<dyn cis_core::memory::MemoryServiceTrait>> =
        Arc::new(Mutex::new(MockMemoryService::new()));
    (ai, memory)
}

/// E2E 测试：完整的 Skill 创建和执行流程
#[tokio::test]
async fn test_e2e_skill_creation_and_execution() {
    // 1. 创建服务
    let (ai, memory) = create_test_services();
    
    // 2. 创建执行器
    let executor = WasmSkillExecutor::new(ai, memory)
        .expect("Failed to create executor");
    
    // 3. 准备输入
    let input = serde_json::json!({
        "action": "process",
        "data": {
            "items": ["item1", "item2", "item3"]
        }
    });
    
    // 4. 执行 WASM Skill
    let result = executor.execute(SIMPLE_WASM, input).await;
    
    // 5. 验证结果
    assert!(result.is_ok(), "Execution failed: {:?}", result.err());
    
    let output = result.unwrap();
    assert!(output.get("success").is_some());
}

/// E2E 测试：使用 Builder 模式的完整流程
#[tokio::test]
async fn test_e2e_builder_pattern() {
    let (ai, memory) = create_test_services();
    
    // 1. 使用 Builder 创建 Skill
    let mut skill = WasmSkillBuilder::new()
        .name("e2e-test-skill")
        .version("1.0.0")
        .description("E2E Test Skill")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .config(
            WasmSkillConfig::new()
                .with_memory_limit_mb(256)
                .with_timeout_ms(30000)
        )
        .build()
        .expect("Failed to build skill");
    
    // 2. 验证 Skill 属性
    assert_eq!(skill.name(), "e2e-test-skill");
    assert_eq!(skill.version(), "1.0.0");
    
    // 3. 实例化
    skill.instantiate().expect("Failed to instantiate");
    assert!(skill.is_instantiated());
    
    // 4. 使用 Skill trait 方法
    let config = SkillConfig::default();
    skill.init(config).await.expect("Failed to init");
}

/// E2E 测试：自定义配置完整流程
#[tokio::test]
async fn test_e2e_custom_config() {
    let (ai, memory) = create_test_services();
    
    // 自定义配置
    let config = WasmSkillConfig {
        memory_limit: Some(128 * 1024 * 1024), // 128MB
        execution_timeout: Some(10000),        // 10 seconds
        allowed_syscalls: vec!["read".to_string()],
    };
    
    // 创建带自定义配置的 Skill
    let skill = WasmSkillBuilder::new()
        .name("custom-config-skill")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .config(config)
        .build()
        .expect("Failed to build skill");
    
    // 验证配置
    assert_eq!(skill.config().memory_limit, Some(128 * 1024 * 1024));
    assert_eq!(skill.config().execution_timeout, Some(10000));
}

/// E2E 测试：运行时创建和模块加载
#[test]
fn test_e2e_runtime_and_module() {
    // 1. 创建运行时
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    
    // 2. 加载模块
    let module = runtime.load_module(SIMPLE_WASM).expect("Failed to load module");
    
    // 3. 验证模块属性
    assert_eq!(module.max_memory_pages(), 8192); // 默认 512MB
    
    // 4. 验证运行时状态
    let _store = runtime.store();
    let _engine = runtime.engine();
}

/// E2E 测试：多 Skill 并发执行
#[tokio::test]
async fn test_e2e_concurrent_skill_execution() {
    let (ai, memory) = create_test_services();
    
    let executor = Arc::new(
        WasmSkillExecutor::new(ai, memory).expect("Failed to create executor")
    );
    
    let mut handles = vec![];
    
    // 并发执行 10 个 Skill
    for i in 0..10 {
        let executor = executor.clone();
        let handle = tokio::spawn(async move {
            let input = serde_json::json!({
                "task_id": i,
                "data": format!("task {}", i)
            });
            
            executor.execute(SIMPLE_WASM, input).await
        });
        handles.push(handle);
    }
    
    // 等待所有执行完成
    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 10);
}

/// E2E 测试：Skill 生命周期（创建 -> 实例化 -> 调用 -> 关闭）
#[test]
fn test_e2e_skill_lifecycle() {
    let (ai, memory) = create_test_services();
    
    // 1. 创建
    let mut skill = WasmSkillBuilder::new()
        .name("lifecycle-skill")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .build()
        .expect("Failed to build");
    
    // 2. 验证初始状态
    assert!(!skill.is_instantiated());
    
    // 3. 实例化
    skill.instantiate().expect("Failed to instantiate");
    assert!(skill.is_instantiated());
    
    // 4. 调用初始化
    let config = cis_core::skill::SkillConfig::default();
    skill.call_init(&config).expect("Failed to init");
    
    // 5. 调用关闭
    skill.call_shutdown().expect("Failed to shutdown");
}

/// E2E 测试：错误处理 - 无效 WASM
#[tokio::test]
async fn test_e2e_invalid_wasm_error() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).expect("Failed to create executor");
    
    let invalid_wasm = vec![0x00, 0x00, 0x00, 0x00]; // 无效 WASM
    
    let input = serde_json::json!({"test": true});
    let result = executor.execute(&invalid_wasm, input).await;
    
    // 应该失败
    assert!(result.is_err());
}

/// E2E 测试：错误处理 - 配置验证失败
#[test]
fn test_e2e_config_validation_error() {
    let invalid_config = WasmSkillConfig {
        memory_limit: Some(0), // 无效：0 内存
        ..Default::default()
    };
    
    let result = WasmSkillConfig::validate(&invalid_config);
    assert!(result.is_err());
}

/// E2E 测试：边界条件 - 大输入数据
#[tokio::test]
async fn test_e2e_large_input_data() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).expect("Failed to create executor");
    
    // 创建大输入
    let large_data = "x".repeat(10000);
    let input = serde_json::json!({
        "large_data": large_data
    });
    
    let result = executor.execute(SIMPLE_WASM, input).await;
    assert!(result.is_ok());
}

/// E2E 测试：边界条件 - 复杂嵌套 JSON
#[tokio::test]
async fn test_e2e_complex_nested_json() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).expect("Failed to create executor");
    
    // 创建复杂嵌套 JSON
    let input = serde_json::json!({
        "level1": {
            "level2": {
                "level3": {
                    "array": [1, 2, 3, 4, 5],
                    "nested_object": {
                        "key1": "value1",
                        "key2": ["a", "b", "c"],
                        "key3": {
                            "deep": "value"
                        }
                    }
                }
            }
        },
        "metadata": {
            "timestamp": "2024-01-01T00:00:00Z",
            "version": "1.0.0"
        }
    });
    
    let result = executor.execute(SIMPLE_WASM, input).await;
    assert!(result.is_ok());
}

/// E2E 测试：性能 - 快速连续执行
#[tokio::test]
async fn test_e2e_rapid_execution() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).expect("Failed to create executor");
    
    // 快速连续执行 50 次
    for i in 0..50 {
        let input = serde_json::json!({"iteration": i});
        let result = executor.execute(SIMPLE_WASM, input).await;
        assert!(result.is_ok(), "Failed at iteration {}", i);
    }
}

/// E2E 测试：服务集成 - AI Provider 调用
#[tokio::test]
async fn test_e2e_ai_provider_integration() {
    let ai = Arc::new(Mutex::new(MockAiProvider::new())) as Arc<Mutex<dyn cis_core::ai::AiProvider>>;
    let memory: Arc<Mutex<dyn cis_core::memory::MemoryServiceTrait>> =
        Arc::new(Mutex::new(MockMemoryService::new()));
    
    let executor = WasmSkillExecutor::new(ai.clone(), memory).expect("Failed to create executor");
    
    let input = serde_json::json!({
        "prompt": "Hello, AI!"
    });
    
    let result = executor.execute(SIMPLE_WASM, input).await;
    assert!(result.is_ok());
}

/// E2E 测试：服务集成 - 记忆服务调用
#[tokio::test]
async fn test_e2e_memory_service_integration() {
    let ai: Arc<Mutex<dyn cis_core::ai::AiProvider>> = Arc::new(Mutex::new(MockAiProvider::new()));
    let memory = Arc::new(Mutex::new(MockMemoryService::new())) as Arc<Mutex<dyn cis_core::memory::MemoryServiceTrait>>;
    
    // 预填充记忆
    {
        let mem = memory.lock().unwrap();
        mem.set("key1", b"value1").expect("Failed to set");
        mem.set("key2", b"value2").expect("Failed to set");
    }
    
    let executor = WasmSkillExecutor::new(ai, memory.clone()).expect("Failed to create executor");
    
    let input = serde_json::json!({
        "action": "read_memory"
    });
    
    let result = executor.execute(SIMPLE_WASM, input).await;
    assert!(result.is_ok());
}
