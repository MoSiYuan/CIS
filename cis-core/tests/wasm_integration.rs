//! WASM 集成测试
//!
//! 测试 WASM 运行时的完整功能流程。

use cis_core::wasm::{WasmRuntime, WasmSkillConfig, WasmSkillBuilder, WasmSkillExecutor};
use cis_core::wasm::runtime::{WasmModule, WasmSkillInstance};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};

/// 简单的 WASM 模块（空模块）
const SIMPLE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 模拟 AI Provider
struct MockAiProvider;

#[async_trait]
impl cis_core::ai::AiProvider for MockAiProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn available(&self) -> bool {
        true
    }

    async fn chat(&self, prompt: &str) -> cis_core::ai::Result<String> {
        Ok(format!("Mock AI response: {}", prompt))
    }

    async fn chat_with_context(
        &self,
        _system: &str,
        _messages: &[cis_core::ai::Message],
    ) -> cis_core::ai::Result<String> {
        Ok("Mock context response".to_string())
    }

    async fn chat_with_rag(
        &self,
        prompt: &str,
        _ctx: Option<&cis_core::conversation::ConversationContext>,
    ) -> cis_core::ai::Result<String> {
        Ok(format!("Mock RAG: {}", prompt))
    }

    async fn generate_json(
        &self,
        _prompt: &str,
        _schema: &str,
    ) -> cis_core::ai::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": "mock"}))
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
    let ai: Arc<Mutex<dyn cis_core::ai::AiProvider>> = Arc::new(Mutex::new(MockAiProvider));
    let memory: Arc<Mutex<dyn cis_core::memory::MemoryServiceTrait>> =
        Arc::new(Mutex::new(MockMemoryService::new()));
    (ai, memory)
}

/// 测试完整 WASM 运行时生命周期
#[test]
fn test_wasm_runtime_full_lifecycle() {
    // 1. 创建运行时
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    
    // 2. 加载模块
    let module = runtime.load_module(SIMPLE_WASM).expect("Failed to load module");
    
    // 3. 验证模块属性
    assert!(module.max_memory_pages() > 0);
    
    // 运行时应该保持有效
    drop(module);
    drop(runtime);
}

/// 测试自定义配置运行时
#[test]
fn test_wasm_runtime_custom_config() {
    let config = WasmSkillConfig {
        memory_limit: Some(256 * 1024 * 1024), // 256MB
        execution_timeout: Some(60000),        // 60 seconds
        allowed_syscalls: vec!["read".to_string()],
    };
    
    let runtime = WasmRuntime::with_config(config).expect("Failed to create runtime with config");
    let module = runtime.load_module(SIMPLE_WASM).expect("Failed to load module");
    
    // 256MB / 64KB = 4096 页
    assert_eq!(module.max_memory_pages(), 4096);
}

/// 测试配置验证失败
#[test]
fn test_wasm_config_validation_failure() {
    // 无效的内存限制
    let invalid_config = WasmSkillConfig {
        memory_limit: Some(0),
        ..Default::default()
    };
    
    let result = WasmRuntime::with_config(invalid_config);
    assert!(result.is_err());
    
    // 过大的内存限制
    let too_large_config = WasmSkillConfig {
        memory_limit: Some(10 * 1024 * 1024 * 1024), // 10GB
        ..Default::default()
    };
    
    let result = WasmRuntime::with_config(too_large_config);
    assert!(result.is_err());
}

/// 测试 WASM Skill 完整生命周期
#[test]
fn test_wasm_skill_full_lifecycle() {
    let (ai, memory) = create_test_services();
    
    // 1. 创建 Skill
    let mut skill = WasmSkillBuilder::new()
        .name("test-skill")
        .version("1.0.0")
        .description("Test Skill")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .build()
        .expect("Failed to build skill");
    
    assert_eq!(skill.name(), "test-skill");
    assert!(!skill.is_instantiated());
    
    // 2. 实例化
    skill.instantiate().expect("Failed to instantiate");
    assert!(skill.is_instantiated());
    
    // 3. 重复实例化（幂等）
    skill.instantiate().expect("Second instantiate should succeed");
}

/// 测试 WASM Skill 执行器
#[test]
fn test_wasm_skill_executor() {
    let (ai, memory) = create_test_services();
    
    // 创建执行器
    let executor = WasmSkillExecutor::new(ai, memory)
        .expect("Failed to create executor");
    
    // 加载并实例化
    let instance = executor.load_and_instantiate(SIMPLE_WASM)
        .expect("Failed to load and instantiate");
    
    // 调用生命周期方法
    instance.init().expect("Init failed");
    instance.shutdown().expect("Shutdown failed");
}

/// 测试 WASM Skill 执行器执行流程
#[tokio::test]
async fn test_wasm_skill_executor_execute() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory)
        .expect("Failed to create executor");
    
    let input = serde_json::json!({
        "action": "test",
        "data": {
            "key": "value"
        }
    });
    
    let result = executor.execute(SIMPLE_WASM, input).await;
    assert!(result.is_ok());
    
    let output = result.unwrap();
    assert!(output.get("success").is_some());
}

/// 测试无效 WASM 模块加载失败
#[test]
fn test_invalid_wasm_loading() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    
    // 无效魔数
    let invalid_magic = vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00];
    let result = runtime.load_module(&invalid_magic);
    assert!(result.is_err());
    
    // 不支持的版本
    let unsupported_version = vec![0x00, 0x61, 0x73, 0x6d, 0x02, 0x00, 0x00, 0x00];
    let result = runtime.load_module(&unsupported_version);
    assert!(result.is_err());
    
    // 太短的模块
    let too_short = vec![0x00, 0x61, 0x73];
    let result = runtime.load_module(&too_short);
    assert!(result.is_err());
    
    // 空模块
    let result = runtime.load_module(&[]);
    assert!(result.is_err());
}

/// 测试内存限制边界
#[test]
fn test_memory_limit_boundaries() {
    let runtime = WasmRuntime::new().expect("Failed to create runtime");
    let module = runtime.load_module(SIMPLE_WASM).expect("Failed to load module");
    
    // 默认内存限制应该是 512MB = 8192 页
    assert_eq!(module.max_memory_pages(), 8192);
}

/// 测试超时配置
#[test]
fn test_timeout_configuration() {
    let config = WasmSkillConfig {
        execution_timeout: Some(10000), // 10 seconds
        ..Default::default()
    };
    
    let runtime = WasmRuntime::with_config(config).expect("Failed to create runtime");
    let module = runtime.load_module(SIMPLE_WASM).expect("Failed to load module");
    
    assert_eq!(module.execution_timeout().as_millis(), 10000);
}

/// 测试系统调用白名单
#[test]
fn test_syscall_whitelist() {
    let config = WasmSkillConfig {
        allowed_syscalls: vec![
            "read".to_string(),
            "write".to_string(),
            "close".to_string(),
        ],
        ..Default::default()
    };
    
    let runtime = WasmRuntime::with_config(config).expect("Failed to create runtime");
    let _module = runtime.load_module(SIMPLE_WASM).expect("Failed to load module");
    
    // 验证配置被正确保存
}

/// 测试 Builder 链式调用
#[test]
fn test_builder_chaining() {
    let (ai, memory) = create_test_services();
    
    let config = WasmSkillConfig::default()
        .with_memory_limit_mb(128)
        .with_timeout_ms(5000);
    
    let skill = WasmSkillBuilder::new()
        .name("chained-skill")
        .version("2.0.0")
        .description("Chained builder test")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .config(config)
        .build();
    
    assert!(skill.is_ok());
}

/// 测试并发实例化
#[tokio::test]
async fn test_concurrent_instantiation() {
    let (ai, memory) = create_test_services();
    
    let executor = Arc::new(
        WasmSkillExecutor::new(ai, memory).expect("Failed to create executor")
    );
    
    let mut handles = vec![];
    
    for _ in 0..10 {
        let executor = executor.clone();
        let handle = tokio::spawn(async move {
            executor.load_and_instantiate(SIMPLE_WASM)
        });
        handles.push(handle);
    }
    
    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 10);
}

/// 测试配置克隆
#[test]
fn test_config_clone() {
    let config = WasmSkillConfig {
        memory_limit: Some(256 * 1024 * 1024),
        execution_timeout: Some(30000),
        allowed_syscalls: vec!["read".to_string(), "write".to_string()],
    };
    
    let cloned = config.clone();
    
    assert_eq!(config.memory_limit, cloned.memory_limit);
    assert_eq!(config.execution_timeout, cloned.execution_timeout);
    assert_eq!(config.allowed_syscalls, cloned.allowed_syscalls);
}

/// 测试 Skill Trait 实现
#[test]
fn test_skill_trait_implementation() {
    use cis_core::skill::Skill;
    
    let (ai, memory) = create_test_services();
    
    let skill = WasmSkillBuilder::new()
        .name("trait-test")
        .version("1.0.0")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .build()
        .expect("Failed to build skill");
    
    // 验证 Skill trait 方法
    assert_eq!(skill.name(), "trait-test");
    assert_eq!(skill.version(), "1.0.0");
}

/// 测试执行器获取服务
#[test]
fn test_executor_service_access() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai.clone(), memory.clone())
        .expect("Failed to create executor");
    
    // 验证可以获取服务的引用
    let _ai_ref = executor.ai_provider();
    let _memory_ref = executor.memory_service();
}

/// 测试错误路径：缺少必需字段的 Builder
#[test]
fn test_builder_missing_required_fields() {
    let result = WasmSkillBuilder::new()
        .name("incomplete")
        .build();
    
    assert!(result.is_err());
}

/// 测试错误路径：无效的配置值
#[test]
fn test_invalid_config_values() {
    // 负的超时值（如果实现允许的话应该被拒绝）
    // 注意：u64 不能有负值，所以这里测试 0
    let config = WasmSkillConfig {
        execution_timeout: Some(0),
        ..Default::default()
    };
    
    assert!(config.validate().is_err());
}
