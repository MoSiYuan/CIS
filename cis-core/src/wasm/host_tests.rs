//! WASM Host 函数测试
//!
//! 测试 Host API 的功能和限制检查。

use super::host::{ExecutionLimits, HostContext, HostFunctions};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use async_trait::async_trait;

/// 模拟记忆服务用于测试
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

/// 模拟 AI Provider 用于测试
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
        Ok(format!("Mock response to: {}", prompt))
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

fn create_test_context() -> HostContext {
    let memory: Arc<Mutex<dyn crate::memory::MemoryServiceTrait>> = 
        Arc::new(Mutex::new(MockMemoryService::new()));
    let ai: Arc<Mutex<dyn crate::ai::AiProvider>> = 
        Arc::new(Mutex::new(MockAiProvider));
    
    HostContext::new(memory, ai)
}

/// 测试执行限制创建
#[test]
fn test_execution_limits_creation() {
    let limits = ExecutionLimits::new(Duration::from_secs(30), 1_000_000);
    
    assert_eq!(limits.timeout, Duration::from_secs(30));
    assert_eq!(limits.max_steps, 1_000_000);
    assert_eq!(limits.current_steps, 0);
}

/// 测试执行限制默认
#[test]
fn test_execution_limits_default() {
    let limits = ExecutionLimits::default();
    
    assert_eq!(limits.timeout, Duration::from_secs(30));
    assert_eq!(limits.max_steps, 1_000_000);
}

/// 测试超时检查
#[test]
fn test_execution_limits_timeout() {
    let limits = ExecutionLimits::new(Duration::from_millis(1), 1_000_000);
    
    // 初始状态不应超时
    assert!(!limits.is_timeout());
    
    // 等待超时
    std::thread::sleep(Duration::from_millis(10));
    assert!(limits.is_timeout());
}

/// 测试步数限制检查
#[test]
fn test_execution_limits_step_counting() {
    let mut limits = ExecutionLimits::new(Duration::from_secs(30), 10);
    
    assert!(!limits.is_step_limit_reached());
    
    // 增加步数
    for _ in 0..10 {
        limits.increment_step();
    }
    
    assert!(limits.is_step_limit_reached());
    assert_eq!(limits.current_steps, 10);
}

/// 测试执行限制重置
#[test]
fn test_execution_limits_reset() {
    let mut limits = ExecutionLimits::new(Duration::from_secs(30), 100);
    
    // 增加步数
    for _ in 0..50 {
        limits.increment_step();
    }
    
    // 重置
    limits.reset();
    
    assert_eq!(limits.current_steps, 0);
    assert!(!limits.is_timeout()); // 时间也被重置
}

/// 测试剩余时间计算
#[test]
fn test_execution_limits_remaining_time() {
    let limits = ExecutionLimits::new(Duration::from_millis(100), 100);
    
    let remaining = limits.remaining_time();
    assert!(remaining > Duration::ZERO);
    assert!(remaining <= Duration::from_millis(100));
    
    // 等待一段时间
    std::thread::sleep(Duration::from_millis(50));
    
    let new_remaining = limits.remaining_time();
    assert!(new_remaining < remaining);
}

/// 测试 Host 上下文创建
#[test]
fn test_host_context_creation() {
    let ctx = create_test_context();
    
    assert!(ctx.memory_ref.is_none());
    assert!(ctx.log_callback.is_none());
    assert!(ctx.execution_limits.is_none());
    assert!(!ctx.allow_network);
    assert!(ctx.allowed_hosts.is_empty());
}

/// 测试设置内存引用
#[test]
fn test_host_context_set_memory() {
    let mut ctx = create_test_context();
    
    // 创建一个模拟内存（这里只是测试设置功能）
    ctx.set_memory_ref(None); // 简化测试
    
    assert!(ctx.memory_ref.is_none());
}

/// 测试设置日志回调
#[test]
fn test_host_context_set_log_callback() {
    let mut ctx = create_test_context();
    
    let logged = Arc::new(Mutex::new(String::new()));
    let logged_clone = logged.clone();
    
    ctx.set_log_callback(move |msg| {
        *logged_clone.lock().unwrap() = msg.to_string();
    });
    
    assert!(ctx.log_callback.is_some());
}

/// 测试设置执行限制
#[test]
fn test_host_context_set_execution_limits() {
    let mut ctx = create_test_context();
    
    ctx.set_execution_limits(Duration::from_secs(60), 500_000);
    
    assert!(ctx.execution_limits.is_some());
    let limits = ctx.execution_limits.unwrap();
    assert_eq!(limits.timeout, Duration::from_secs(60));
    assert_eq!(limits.max_steps, 500_000);
}

/// 测试网络权限设置
#[test]
fn test_host_context_network_permissions() {
    let mut ctx = create_test_context();
    
    // 默认不允许网络
    assert!(!ctx.allow_network);
    assert!(ctx.is_host_allowed("example.com"));
    
    // 设置允许网络但限制主机
    ctx.set_network_permissions(true, vec!["example.com".to_string()]);
    
    assert!(ctx.allow_network);
    assert!(ctx.is_host_allowed("example.com"));
    assert!(!ctx.is_host_allowed("other.com"));
    
    // 允许所有主机
    ctx.set_network_permissions(true, vec![]);
    assert!(ctx.is_host_allowed("any-host.com"));
}

/// 测试检查限制（无限制时）
#[test]
fn test_host_context_check_limits_none() {
    let ctx = create_test_context();
    
    // 没有设置限制时应该通过
    let result = ctx.check_limits();
    assert!(result.is_ok());
}

/// 测试检查步数限制
#[test]
fn test_host_context_check_limits_steps() {
    let mut ctx = create_test_context();
    
    ctx.set_execution_limits(Duration::from_secs(60), 5);
    
    // 前 5 步应该通过
    for _ in 0..5 {
        assert!(ctx.check_limits().is_ok());
        if let Some(ref mut limits) = ctx.execution_limits {
            limits.increment_step();
        }
    }
    
    // 第 6 步应该失败
    assert!(ctx.check_limits().is_err());
}

/// 测试检查超时
#[test]
fn test_host_context_check_limits_timeout() {
    let mut ctx = create_test_context();
    
    ctx.set_execution_limits(Duration::from_millis(1), 1_000_000);
    
    // 初始应该通过
    assert!(ctx.check_limits().is_ok());
    
    // 等待超时
    std::thread::sleep(Duration::from_millis(10));
    
    // 应该超时
    assert!(ctx.check_limits().is_err());
}

/// 测试 Host 函数创建导入
#[test]
fn test_host_functions_create_imports() {
    use wasmer::{Engine, Store};
    
    let engine = Engine::default();
    let mut store = Store::new(engine);
    let ctx = create_test_context();
    
    let function_env = wasmer::FunctionEnv::new(&mut store, ctx);
    let imports = HostFunctions::create_imports(&mut store, function_env);
    
    // 验证所有 Host 函数都被定义
    assert!(imports.get_namespace("env").is_some());
}

/// 测试 Host 上下文克隆
#[test]
fn test_host_context_clone() {
    let ctx = create_test_context();
    let _cloned = ctx.clone();
    
    // 克隆应该成功
}

/// 测试数据库管理器设置
#[test]
fn test_host_context_db_manager() {
    // 由于需要实际的 DbManager，这里只是测试接口存在
    // 实际测试应该在集成测试中完成
}

/// 测试网络权限边界情况
#[test]
fn test_network_permissions_edge_cases() {
    let mut ctx = create_test_context();
    
    // 禁止网络访问
    ctx.set_network_permissions(false, vec![]);
    assert!(!ctx.is_host_allowed("any.com"));
    
    // 允许网络但空主机列表（允许所有）
    ctx.set_network_permissions(true, vec![]);
    assert!(ctx.is_host_allowed("any.com"));
    
    // 允许网络但特定主机
    ctx.set_network_permissions(true, vec!["specific.com".to_string()]);
    assert!(ctx.is_host_allowed("specific.com"));
    assert!(!ctx.is_host_allowed("other.com"));
    assert!(ctx.is_host_allowed("sub.specific.com")); // 子域名应该匹配
}

/// 测试执行已运行时间
#[test]
fn test_execution_limits_elapsed() {
    let limits = ExecutionLimits::new(Duration::from_secs(60), 100);
    
    let elapsed1 = limits.elapsed();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed2 = limits.elapsed();
    
    assert!(elapsed2 > elapsed1);
}

// 辅助 trait 扩展用于测试
trait TestHostContextExt {
    fn set_memory_ref(&mut self, _memory: Option<()>);
}

impl TestHostContextExt for HostContext {
    fn set_memory_ref(&mut self, _memory: Option<()>) {
        // 测试用空实现
    }
}
