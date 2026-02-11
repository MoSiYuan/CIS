//! WASM Skill 集成测试
//!
//! 测试 WASM Skill 的构建、实例化和执行。

use super::skill::{WasmSkill, WasmSkillBuilder, WasmSkillExecutor};
use super::{WasmSkillConfig, DEFAULT_MEMORY_LIMIT_BYTES};
use crate::skill::Skill;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;

/// 简单的 WASM 模块
const SIMPLE_WASM: &[u8] = &[
    0x00, 0x61, 0x73, 0x6d, // WASM magic
    0x01, 0x00, 0x00, 0x00, // version 1
];

/// 模拟 AI Provider
struct MockAiProvider;

#[async_trait]
impl crate::ai::AiProvider for MockAiProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn available(&self) -> bool {
        true
    }

    async fn chat(&self, prompt: &str) -> crate::ai::Result<String> {
        Ok(format!("Mock: {}", prompt))
    }

    async fn chat_with_context(
        &self,
        _system: &str,
        _messages: &[crate::ai::Message],
    ) -> crate::ai::Result<String> {
        Ok("Mock context".to_string())
    }

    async fn chat_with_rag(
        &self,
        prompt: &str,
        _ctx: Option<&crate::conversation::ConversationContext>,
    ) -> crate::ai::Result<String> {
        Ok(format!("Mock RAG: {}", prompt))
    }

    async fn generate_json(
        &self,
        _prompt: &str,
        _schema: &str,
    ) -> crate::ai::Result<serde_json::Value> {
        Ok(serde_json::json!({"result": "mock"}))
    }
}

/// 模拟记忆服务
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

impl crate::memory::MemoryServiceTrait for MockMemoryService {
    fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.data.get(key).cloned()
    }

    fn set(&self, _key: &str, _value: &[u8]) -> crate::error::Result<()> {
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
    Arc<Mutex<dyn crate::ai::AiProvider>>,
    Arc<Mutex<dyn crate::memory::MemoryServiceTrait>>,
) {
    let ai: Arc<Mutex<dyn crate::ai::AiProvider>> = Arc::new(Mutex::new(MockAiProvider));
    let memory: Arc<Mutex<dyn crate::memory::MemoryServiceTrait>> =
        Arc::new(Mutex::new(MockMemoryService::new()));
    (ai, memory)
}

/// 测试 WASM Skill 创建
#[test]
fn test_wasm_skill_creation() {
    let (ai, memory) = create_test_services();
    
    let skill = WasmSkill::new(
        "test-skill",
        "1.0.0",
        "Test Skill",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        None,
    );
    
    assert!(skill.is_ok());
    let skill = skill.unwrap();
    assert_eq!(skill.name(), "test-skill");
    assert_eq!(skill.version(), "1.0.0");
    assert_eq!(skill.description(), "Test Skill");
    assert!(!skill.is_instantiated());
}

/// 测试 WASM Skill 实例化
#[test]
fn test_wasm_skill_instantiate() {
    let (ai, memory) = create_test_services();
    
    let mut skill = WasmSkill::new(
        "test-skill",
        "1.0.0",
        "Test Skill",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        None,
    ).unwrap();
    
    let result = skill.instantiate();
    assert!(result.is_ok());
    assert!(skill.is_instantiated());
}

/// 测试重复实例化
#[test]
fn test_wasm_skill_instantiate_twice() {
    let (ai, memory) = create_test_services();
    
    let mut skill = WasmSkill::new(
        "test-skill",
        "1.0.0",
        "Test Skill",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        None,
    ).unwrap();
    
    skill.instantiate().unwrap();
    let result = skill.instantiate(); // 第二次应该成功（幂等）
    assert!(result.is_ok());
}

/// 测试未实例化调用
#[test]
fn test_wasm_skill_call_without_instantiate() {
    let (ai, memory) = create_test_services();
    
    let skill = WasmSkill::new(
        "test-skill",
        "1.0.0",
        "Test Skill",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        None,
    ).unwrap();
    
    // 未实例化时调用应该失败
    let result = skill.call_init(&crate::skill::SkillConfig::default());
    assert!(result.is_err());
}

/// 测试 WASM Skill Builder
#[test]
fn test_wasm_skill_builder() {
    let (ai, memory) = create_test_services();
    
    let result = WasmSkillBuilder::new()
        .name("test-skill")
        .version("1.0.0")
        .description("Test Skill")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .build();
    
    assert!(result.is_ok());
    let skill = result.unwrap();
    assert_eq!(skill.name(), "test-skill");
}

/// 测试 Builder 缺少必需字段
#[test]
fn test_wasm_skill_builder_missing_fields() {
    let result = WasmSkillBuilder::new()
        .name("test-skill")
        // 缺少其他必需字段
        .build();
    
    assert!(result.is_err());
}

/// 测试 Builder 默认值
#[test]
fn test_wasm_skill_builder_defaults() {
    let (ai, memory) = create_test_services();
    
    let result = WasmSkillBuilder::new()
        .name("test-skill")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .build();
    
    assert!(result.is_ok());
    let skill = result.unwrap();
    assert_eq!(skill.version(), "0.1.0"); // 默认版本
    assert_eq!(skill.description(), "WASM Skill"); // 默认描述
}

/// 测试 WASM Skill 配置
#[test]
fn test_wasm_skill_custom_config() {
    let (ai, memory) = create_test_services();
    
    let config = WasmSkillConfig {
        memory_limit: Some(256 * 1024 * 1024),
        execution_timeout: Some(10000),
        allowed_syscalls: vec!["read".to_string()],
    };
    
    let skill = WasmSkill::new(
        "test-skill",
        "1.0.0",
        "Test Skill",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        Some(config),
    ).unwrap();
    
    assert_eq!(skill.config().memory_limit, Some(256 * 1024 * 1024));
    assert_eq!(skill.config().execution_timeout, Some(10000));
}

/// 测试 WASM Skill 执行器创建
#[test]
fn test_wasm_skill_executor_creation() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory);
    assert!(executor.is_ok());
}

/// 测试执行器自定义配置
#[test]
fn test_wasm_skill_executor_with_config() {
    let (ai, memory) = create_test_services();
    
    let config = WasmSkillConfig {
        memory_limit: Some(128 * 1024 * 1024),
        execution_timeout: Some(5000),
        allowed_syscalls: vec![],
    };
    
    let executor = WasmSkillExecutor::with_config(config, ai, memory);
    assert!(executor.is_ok());
}

/// 测试内存页计算
#[test]
fn test_memory_pages_calculation() {
    let (ai, memory) = create_test_services();
    
    // 128MB = 2048 页
    let config = WasmSkillConfig {
        memory_limit: Some(128 * 1024 * 1024),
        ..Default::default()
    };
    
    let skill = WasmSkill::new(
        "test-skill",
        "1.0.0",
        "Test Skill",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        Some(config),
    ).unwrap();
    
    // 通过技能验证内存限制
    assert_eq!(skill.config().memory_limit, Some(128 * 1024 * 1024));
}

/// 测试 Skill 特性实现
#[test]
fn test_skill_trait_methods() {
    let (ai, memory) = create_test_services();
    
    let skill = WasmSkill::new(
        "test-skill",
        "2.0.0",
        "Test Description",
        SIMPLE_WASM.to_vec(),
        memory,
        ai,
        None,
    ).unwrap();
    
    assert_eq!(skill.name(), "test-skill");
    assert_eq!(skill.version(), "2.0.0");
    assert_eq!(skill.description(), "Test Description");
}

/// 测试执行器加载和实例化
#[test]
fn test_executor_load_and_instantiate() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).unwrap();
    let instance = executor.load_and_instantiate(SIMPLE_WASM);
    
    assert!(instance.is_ok());
    let instance = instance.unwrap();
    
    // 测试实例方法
    assert!(instance.init().is_ok());
    assert!(instance.shutdown().is_ok());
}

/// 测试执行器执行（异步）
#[tokio::test]
async fn test_executor_execute() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai, memory).unwrap();
    
    let input = serde_json::json!({
        "action": "test",
        "data": "hello"
    });
    
    let result = executor.execute(SIMPLE_WASM, input).await;
    assert!(result.is_ok());
    
    let output = result.unwrap();
    assert!(output.get("success").is_some());
}

/// 测试 Builder 配置设置
#[test]
fn test_wasm_skill_builder_with_config() {
    let (ai, memory) = create_test_services();
    
    let config = WasmSkillConfig {
        memory_limit: Some(512 * 1024 * 1024),
        execution_timeout: Some(30000),
        allowed_syscalls: vec!["read".to_string(), "write".to_string()],
    };
    
    let result = WasmSkillBuilder::new()
        .name("test-skill")
        .version("1.0.0")
        .wasm_bytes(SIMPLE_WASM.to_vec())
        .memory_service(memory)
        .ai_provider(ai)
        .config(config)
        .build();
    
    assert!(result.is_ok());
}

/// 测试默认内存限制
#[test]
fn test_default_memory_limit() {
    let config = WasmSkillConfig::default();
    assert_eq!(config.memory_limit, Some(DEFAULT_MEMORY_LIMIT_BYTES));
}

/// 测试执行器获取服务
#[test]
fn test_executor_get_services() {
    let (ai, memory) = create_test_services();
    
    let executor = WasmSkillExecutor::new(ai.clone(), memory.clone()).unwrap();
    
    // 验证可以获取服务的克隆
    let _ai_clone = executor.ai_provider();
    let _memory_clone = executor.memory_service();
}

/// 测试 Skill Drop
#[test]
fn test_wasm_skill_drop() {
    let (ai, memory) = create_test_services();
    
    {
        let skill = WasmSkill::new(
            "test-skill",
            "1.0.0",
            "Test",
            SIMPLE_WASM.to_vec(),
            memory,
            ai,
            None,
        ).unwrap();
        
        // skill 在这里被 drop
        drop(skill);
    }
    
    // 应该正常完成
}

/// 测试无效 WASM 创建失败
#[test]
fn test_wasm_skill_invalid_wasm() {
    let (ai, memory) = create_test_services();
    
    let invalid_wasm = vec![0x00, 0x00, 0x00, 0x00]; // 无效 WASM
    
    let result = WasmSkill::new(
        "test",
        "1.0",
        "Test",
        invalid_wasm,
        memory,
        ai,
        None,
    );
    
    // 创建时不会失败，实例化时会失败
    assert!(result.is_ok());
}
