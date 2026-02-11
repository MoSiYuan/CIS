//! # WASM 模块测试
//!
//! WASM Runtime 的完整测试套件。

use super::*;
use super::skill::{WasmSkill, WasmSkillBuilder, WasmSkillExecutor};
use crate::ai::AiProvider;
use crate::memory::{MemoryServiceTrait, MemorySearchItem};
use crate::skill::Skill;
use async_trait::async_trait;
use std::collections::HashMap;

/// 最小的有效 WASM 模块（空模块）
/// 
/// 这个模块只包含 WASM 魔数和版本号。
const EMPTY_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic: \0asm
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 简单的 WASM 模块，导出一个 `add` 函数
/// 
/// 这个模块是用 WAT (WebAssembly Text Format) 编写的：
/// ```wat
/// (module
///   (func $add (param i32 i32) (result i32)
///     local.get 0
///     local.get 1
///     i32.add)
///   (export "add" (func $add))
/// )
/// ```
const ADD_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // header
    0x01, 0x07, 0x01,                               // type section
    0x60, 0x02, 0x7f, 0x7f, 0x01, 0x7f,             // type: (i32, i32) -> i32
    0x03, 0x02, 0x01, 0x00,                         // func section
    0x07, 0x07, 0x01,                               // export section
    0x03, 0x61, 0x64, 0x64, 0x00, 0x00,             // export "add" as func 0
    0x0a, 0x09, 0x01,                               // code section
    0x07, 0x00,                                     // body size 7, 0 locals
    0x20, 0x00,                                     // local.get 0
    0x20, 0x01,                                     // local.get 1
    0x6a,                                           // i32.add
    0x0b,                                           // end
];

/// 无效 WASM 魔数
const INVALID_MAGIC: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, // Invalid magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 无效 WASM 版本
const INVALID_VERSION: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x02, 0x00, 0x00, 0x00, // Invalid version 2
];

/// 太小的 WASM 模块
const TOO_SMALL: &[u8] = &[0x00, 0x61, 0x73, 0x6d];

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
        // 模拟真实 AI 响应
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

#[test]
fn test_wasm_runtime_creation() {
    let runtime = WasmRuntime::new();
    assert!(runtime.is_ok(), "Failed to create WASM runtime");
}

#[test]
fn test_load_empty_wasm() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(EMPTY_WASM);
    assert!(result.is_ok(), "Failed to load empty WASM module");
}

#[test]
fn test_load_add_wasm() {
    let runtime = WasmRuntime::new().unwrap();
    let result = runtime.load_module(ADD_WASM);
    assert!(result.is_ok(), "Failed to load add WASM module");
}

#[test]
fn test_invalid_wasm_rejection() {
    let runtime = WasmRuntime::new().unwrap();
    
    // 测试无效魔数
    let result = runtime.load_module(INVALID_MAGIC);
    assert!(result.is_err(), "Should reject invalid WASM magic");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("magic"), "Error should mention magic number");
    
    // 测试无效版本
    let result = runtime.load_module(INVALID_VERSION);
    assert!(result.is_err(), "Should reject invalid WASM version");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("version"), "Error should mention version");
    
    // 测试太小的模块
    let result = runtime.load_module(TOO_SMALL);
    assert!(result.is_err(), "Should reject too small WASM module");
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("too small"), "Error should mention size");
}

#[test]
fn test_wasm_instance_creation() {
    let runtime = WasmRuntime::new().unwrap();
    let module = runtime.load_module(EMPTY_WASM).unwrap();
    
    // 验证模块已加载 - module() 返回 &Module
    let _module_ref = module.module();
}

#[test]
fn test_host_api_encode_decode() {
    use super::host::{encode_result, decode_ptr, decode_len};

    let ptr = 0x1234i32;
    let len = 0x5678i32;
    
    let encoded = encode_result(ptr, len);
    assert_eq!(decode_ptr(encoded), ptr);
    assert_eq!(decode_len(encoded), len);
}

#[test]
fn test_log_level_conversion() {
    use super::host::LogLevel;

    assert_eq!(LogLevel::from_i32(0), Some(LogLevel::Debug));
    assert_eq!(LogLevel::from_i32(1), Some(LogLevel::Info));
    assert_eq!(LogLevel::from_i32(2), Some(LogLevel::Warn));
    assert_eq!(LogLevel::from_i32(3), Some(LogLevel::Error));
    assert_eq!(LogLevel::from_i32(99), None);
}

#[test]
fn test_skill_builder() {
    let _builder = WasmSkillBuilder::new()
        .name("test-skill")
        .version("1.0.0")
        .description("Test WASM Skill")
        .wasm_bytes(EMPTY_WASM.to_vec());

    // Builder created successfully
}

/// 内存使用测试
#[test]
fn test_memory_page_calculation() {
    // 64KB 每页
    const PAGE_SIZE: usize = 64 * 1024;
    
    // 512MB = 8192 页
    let mb_512 = 512 * 1024 * 1024;
    assert_eq!(mb_512 / PAGE_SIZE, 8192);
    
    // 1GB = 16384 页
    let gb_1 = 1024 * 1024 * 1024;
    assert_eq!(gb_1 / PAGE_SIZE, 16384);
    
    // 4GB = 65536 页（WebAssembly 最大）
    let gb_4 = 4 * 1024 * 1024 * 1024;
    assert_eq!(gb_4 / PAGE_SIZE, 65536);
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

#[test]
fn test_wasm_skill_executor_with_add_module() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).unwrap();
    
    // 加载 add 模块
    let result = executor.load_and_instantiate(ADD_WASM);
    assert!(result.is_ok(), "Failed to load add WASM module");
    
    let instance = result.unwrap();
    
    // 初始化
    let result = instance.init();
    assert!(result.is_ok());
    
    // 关闭
    let result = instance.shutdown();
    assert!(result.is_ok());
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

// ==================== Host 函数测试 ====================

#[test]
fn test_execution_limits() {
    use super::host::ExecutionLimits;
    use std::time::Duration;
    
    let limits = ExecutionLimits::new(
        Duration::from_secs(30),
        1_000_000
    );
    
    assert!(!limits.is_timeout());
    assert!(!limits.is_step_limit_reached());
    assert!(limits.remaining_time() > Duration::ZERO);
    
    std::thread::sleep(Duration::from_millis(10));
    assert!(limits.elapsed() >= Duration::from_millis(10));
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

#[test]
fn test_wasm_skill_builder_full() {
    let (ai, memory) = create_test_services();
    
    let result = WasmSkillBuilder::new()
        .name("test-skill")
        .version("1.0.0")
        .description("Test WASM Skill")
        .wasm_bytes(EMPTY_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .build();
    
    assert!(result.is_ok(), "Failed to build WASM skill");
    let skill = result.unwrap();
    assert_eq!(skill.name(), "test-skill");
    assert_eq!(skill.version(), "1.0.0");
}

// ==================== 性能测试 ====================

#[test]
fn test_wasm_runtime_performance() {
    let runtime = WasmRuntime::new().unwrap();
    
    // 多次加载模块测试性能
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _ = runtime.load_module(EMPTY_WASM).unwrap();
    }
    let elapsed = start.elapsed();
    
    // 100 次加载应该在 1 秒内完成
    assert!(elapsed < std::time::Duration::from_secs(1), 
        "WASM loading too slow: {:?}", elapsed);
}
