//! # CIS Test Framework
//!
//! 完整的测试框架，提供 Mock 实现、测试工具和覆盖率支持。
//!
//! ## 模块结构
//!
//! - `mocks`: Mock 实现，用于隔离测试
//!   - `MockNetworkService`: 网络服务 Mock
//!   - `MockStorageService`: 存储服务 Mock
//!   - `MockEventBus`: 事件总线 Mock
//!   - `MockAiProvider`: AI Provider Mock
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::test::mocks::{MockNetworkService, MockStorageService, MockCallTracker};
//!
//! # async fn example() {
//! let mock_network = MockNetworkService::new();
//! let mock_storage = MockStorageService::new();
//!
//! // 配置 Mock 行为
//! mock_network.when_connect("ws://localhost:8080").return_ok(()).await;
//! mock_storage.when_get("key").return_ok(Some("value".to_string())).await;
//!
//! // 执行测试
//! // ...
//!
//! // 验证调用
//! mock_network.assert_called("connect", 1).await;
//! # }
//! ```

pub mod mocks;

#[cfg(test)]
pub mod examples;

/// 测试工具函数
pub mod utils {
    use std::time::Duration;
    use tokio::time::timeout;

    /// 带超时的异步测试包装器
    pub async fn with_timeout<T, F>(duration: Duration, f: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T>,
    {
        timeout(duration, f)
            .await
            .map_err(|_| format!("Test timed out after {:?}", duration))
    }

    /// 默认测试超时（5秒）
    pub async fn with_default_timeout<T, F>(f: F) -> Result<T, String>
    where
        F: std::future::Future<Output = T>,
    {
        with_timeout(Duration::from_secs(5), f).await
    }

    /// 创建临时测试目录
    pub fn temp_test_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("cis_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("Failed to create test directory");
        dir
    }

    /// 清理测试目录
    pub fn cleanup_test_dir(dir: &std::path::Path) {
        if dir.exists() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_dir_creation() {
        let dir = utils::temp_test_dir();
        assert!(dir.exists());
        utils::cleanup_test_dir(&dir);
        assert!(!dir.exists());
    }
}
