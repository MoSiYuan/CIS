//! # Mock Implementations
//!
//! 可验证的 Mock 实现，用于单元测试中的依赖隔离。
//!
//! ## 核心特性
//!
//! - **调用追踪**: 自动记录所有方法调用
//! - **参数验证**: 验证调用时的参数
//! - **行为配置**: 预设返回值或错误
//! - **并发安全**: 使用内部锁保证线程安全
//!
//! ## Mock 列表
//!
//! | Mock | 用途 | 关键能力 |
//! |------|------|----------|
//! | `MockNetworkService` | 网络服务 | 连接模拟、消息收发 |
//! | `MockStorageService` | 存储服务 | KV 操作、查询验证 |
//! | `MockEventBus` | 事件总线 | 事件发布订阅 |
//! | `MockAiProvider` | AI Provider | 响应模拟、流式输出 |
//! | `MockEmbeddingService` | 嵌入服务 | 向量生成、相似度计算 |
//! | `MockSkillExecutor` | Skill 执行器 | 执行模拟、日志输出 |
//!
//! ## 使用模式
//!
//! ### 1. 基本使用
//!
//! ```rust
//! let mock = MockStorageService::new();
//! mock.when_get("key").return_ok(Some("value")).await;
//! let result = mock.get("key").await.unwrap();
//! assert_eq!(result, Some("value".to_string()));
//! ```
//!
//! ### 2. 调用验证
//!
//! ```rust
//! mock.assert_called_with("get", vec!["key"]).await;
//! mock.assert_call_count("set", 2).await;
//! ```
//!
//! ### 3. 错误模拟
//!
//! ```rust
//! mock.when_get("error_key").return_err(StorageError::NotFound).await;
//! ```

pub mod network_service;
pub mod storage_service;
pub mod event_bus;
pub mod ai_provider;
pub mod embedding_service;
pub mod skill_executor;

pub use network_service::MockNetworkService;
pub use storage_service::MockStorageService;
pub use event_bus::MockEventBus;
pub use ai_provider::MockAiProvider;
pub use embedding_service::MockEmbeddingService;
pub use skill_executor::MockSkillExecutor;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// 调用记录
#[derive(Debug, Clone)]
pub struct CallRecord {
    /// 方法名
    pub method: String,
    /// 参数列表（序列化后）
    pub args: Vec<String>,
    /// 调用时间
    pub timestamp: std::time::Instant,
}

impl CallRecord {
    pub fn new(method: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            method: method.into(),
            args,
            timestamp: std::time::Instant::now(),
        }
    }
}

/// Mock 调用追踪器
#[derive(Debug, Default, Clone)]
pub struct MockCallTracker {
    calls: Arc<Mutex<Vec<CallRecord>>>,
}

impl MockCallTracker {
    /// 创建新的追踪器
    pub fn new() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// 记录调用
    pub fn record(&self, method: impl Into<String>, args: Vec<String>) {
        let record = CallRecord::new(method, args);
        let mut calls = self.calls.lock().unwrap();
        calls.push(record);
    }

    /// 获取所有调用记录
    pub fn get_calls(&self) -> Vec<CallRecord> {
        self.calls.lock().unwrap().clone()
    }

    /// 获取指定方法的调用记录
    pub fn get_calls_for(&self, method: &str) -> Vec<CallRecord> {
        self.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.method == method)
            .cloned()
            .collect()
    }

    /// 获取调用次数
    pub fn call_count(&self, method: &str) -> usize {
        self.get_calls_for(method).len()
    }

    /// 断言：方法被调用指定次数
    pub fn assert_call_count(&self, method: &str, expected: usize) {
        let actual = self.call_count(method);
        assert_eq!(
            actual, expected,
            "Expected method '{}' to be called {} times, but was called {} times",
            method, expected, actual
        );
    }

    /// 断言：方法至少被调用一次
    pub fn assert_called(&self, method: &str) {
        let count = self.call_count(method);
        assert!(
            count > 0,
            "Expected method '{}' to be called at least once, but was never called",
            method
        );
    }

    /// 断言：方法从未被调用
    pub fn assert_not_called(&self, method: &str) {
        let count = self.call_count(method);
        assert_eq!(
            count, 0,
            "Expected method '{}' to never be called, but was called {} times",
            method, count
        );
    }

    /// 断言：最后一次调用的参数
    pub fn assert_last_call_args(&self, method: &str, expected_args: Vec<&str>) {
        let calls = self.get_calls_for(method);
        let last_call = calls.last().expect(&format!(
            "Expected method '{}' to have been called",
            method
        ));
        
        let expected: Vec<String> = expected_args.iter().map(|s| s.to_string()).collect();
        assert_eq!(
            last_call.args, expected,
            "Method '{}' was called with unexpected arguments",
            method
        );
    }

    /// 清空调用记录
    pub fn clear(&self) {
        self.calls.lock().unwrap().clear();
    }
}

/// Mock 行为预设
pub struct MockBehavior<T, E> {
    result: core::result::Result<T, E>,
}

impl<T, E> MockBehavior<T, E> {
    pub fn return_ok(value: T) -> Self {
        Self { result: core::result::Result::Ok(value) }
    }

    pub fn return_err(error: E) -> Self {
        Self { result: core::result::Result::Err(error) }
    }

    pub fn into_result(self) -> core::result::Result<T, E> {
        self.result
    }
}

/// 参数匹配器
#[derive(Debug, Clone)]
pub enum ArgMatcher {
    /// 精确匹配
    Exact(String),
    /// 包含匹配
    Contains(String),
    /// 任意值
    Any,
    /// 正则匹配
    Regex(String),
}

impl ArgMatcher {
    pub fn matches(&self, value: &str) -> bool {
        match self {
            ArgMatcher::Exact(expected) => value == expected,
            ArgMatcher::Contains(substr) => value.contains(substr),
            ArgMatcher::Any => true,
            ArgMatcher::Regex(pattern) => {
                regex::Regex::new(pattern)
                    .map(|re| re.is_match(value))
                    .unwrap_or(false)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_tracker_basic() {
        let tracker = MockCallTracker::new();
        
        tracker.record("test", vec!["arg1".to_string(), "arg2".to_string()]);
        tracker.record("test", vec!["arg3".to_string()]);
        
        assert_eq!(tracker.call_count("test"), 2);
        assert_eq!(tracker.call_count("other"), 0);
    }

    #[test]
    fn test_call_tracker_assertions() {
        let tracker = MockCallTracker::new();
        
        tracker.record("method", vec!["a".to_string()]);
        tracker.record("method", vec!["b".to_string()]);
        
        tracker.assert_call_count("method", 2);
        tracker.assert_called("method");
        tracker.assert_not_called("other");
    }

    #[test]
    fn test_arg_matcher() {
        assert!(ArgMatcher::Exact("test".to_string()).matches("test"));
        assert!(!ArgMatcher::Exact("test".to_string()).matches("other"));
        
        assert!(ArgMatcher::Contains("foo".to_string()).matches("foobar"));
        assert!(!ArgMatcher::Contains("baz".to_string()).matches("foobar"));
        
        assert!(ArgMatcher::Any.matches("anything"));
        
        assert!(ArgMatcher::Regex(r"^test_\d+$".to_string()).matches("test_123"));
        assert!(!ArgMatcher::Regex(r"^test_\d+$".to_string()).matches("test_abc"));
    }
}
