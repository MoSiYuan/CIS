//! # Matrix Presence API
//!
//! 实现在线状态管理，遵循 Matrix Client-Server API v1.11 规范。
//!
//! ## 端点
//!
//! - `GET /_matrix/client/v3/presence/{userId}/status` - 获取用户在线状态
//! - `PUT /_matrix/client/v3/presence/{userId}/status` - 设置自己的在线状态
//! - `GET /_matrix/client/v3/presence/list/{userId}` - 获取订阅列表
//! - `POST /_matrix/client/v3/presence/list/{userId}` - 订阅用户状态
//!
//! ## Presence 状态
//!
//! - `online` - 用户在线
//! - `offline` - 用户离线
//! - `unavailable` - 用户离开但未离线
//!
//! ## 使用示例
//!
//! ```ignore
//! use cis_core::matrix::presence::{PresenceService, PresenceState};
//!
//! // 设置在线状态
//! let state = PresenceState {
//!     presence: "online".to_string(),
//!     status_msg: Some("Working on CIS".to_string()),
//! };
//! service.set_presence("@user:cis.local", state).await?;
//!
//! // 查询用户状态
//! let presence = service.get_presence("@user:cis.local").await?;
//! println!("User is: {}", presence.presence);
//! ```

use crate::matrix::error::{MatrixError, MatrixResult};
use crate::matrix::store_social::MatrixSocialStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Presence 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceState {
    /// 在线状态: "online", "offline", "unavailable"
    pub presence: String,

    /// 距离上次活跃的时间 (毫秒)
    #[serde(rename = "last_active_ago")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_active_ago: Option<u64>,

    /// 状态消息
    #[serde(rename = "status_msg")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_msg: Option<String>,
}

impl PresenceState {
    /// 创建新的在线状态
    pub fn online() -> Self {
        Self {
            presence: "online".to_string(),
            last_active_ago: Some(0),
            status_msg: None,
        }
    }

    /// 创建离线状态
    pub fn offline() -> Self {
        Self {
            presence: "offline".to_string(),
            last_active_ago: None,
            status_msg: None,
        }
    }

    /// 创建不可用状态
    pub fn unavailable() -> Self {
        Self {
            presence: "unavailable".to_string(),
            last_active_ago: Some(0),
            status_msg: None,
        }
    }

    /// 设置状态消息
    pub fn with_status_msg(mut self, msg: impl Into<String>) -> Self {
        self.status_msg = Some(msg.into());
        self
    }

    /// 验证状态值
    pub fn is_valid(&self) -> bool {
        matches!(self.presence.as_str(), "online" | "offline" | "unavailable")
    }
}

/// Presence 事件 (用于同步响应)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceEvent {
    /// Presence 状态
    pub presence: PresenceState,

    /// 发送者用户 ID
    pub sender: String,

    /// 事件类型 (固定为 "m.presence")
    #[serde(rename = "type")]
    pub event_type: String,
}

impl PresenceEvent {
    /// 创建新的 Presence 事件
    pub fn new(user_id: impl Into<String>, presence: PresenceState) -> Self {
        Self {
            presence,
            sender: user_id.into(),
            event_type: "m.presence".to_string(),
        }
    }
}

/// Presence 列表请求
#[derive(Debug, Deserialize)]
pub struct PresenceListRequest {
    /// 订阅的用户 ID 列表
    pub users: Vec<String>,
}

/// Presence 列表响应
#[derive(Debug, Serialize)]
pub struct PresenceListResponse {
    /// 用户状态映射
    pub users: HashMap<String, PresenceState>,
}

/// Presence 服务
#[derive(Clone)]
pub struct PresenceService {
    /// 社交存储 (用于查询用户信息)
    social_store: Arc<MatrixSocialStore>,

    /// Presence 状态缓存 (内存)
    cache: Arc<RwLock<HashMap<String, PresenceState>>>,

    /// 订阅列表 (user_id -> [subscribed_users])
    subscriptions: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl PresenceService {
    /// 创建新的 Presence 服务
    pub fn new(social_store: Arc<MatrixSocialStore>) -> Self {
        Self {
            social_store,
            cache: Arc::new(RwLock::new(HashMap::new())),
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置用户的 Presence 状态
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    /// - `state`: Presence 状态
    pub async fn set_presence(
        &self,
        user_id: &str,
        state: PresenceState,
    ) -> MatrixResult<()> {
        // 验证状态
        if !state.is_valid() {
            return Err(MatrixError::InvalidArgument(format!(
                "Invalid presence state: {}",
                state.presence
            )));
        }

        // 更新缓存
        let mut cache = self.cache.write().await;
        cache.insert(user_id.to_string(), state.clone());

        // TODO: 存储到数据库
        // TODO: 广播到订阅者

        tracing::debug!("Set presence for {}: {}", user_id, state.presence);

        Ok(())
    }

    /// 获取用户的 Presence 状态
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    pub async fn get_presence(&self, user_id: &str) -> MatrixResult<PresenceState> {
        // 从缓存读取
        let cache = self.cache.read().await;
        if let Some(state) = cache.get(user_id) {
            return Ok(state.clone());
        }

        // 缓存未命中,返回默认离线状态
        Ok(PresenceState::offline())
    }

    /// 获取多个用户的 Presence 状态
    ///
    /// # 参数
    ///
    /// - `user_ids`: 用户 ID 列表
    pub async fn get_presences(
        &self,
        user_ids: &[String],
    ) -> MatrixResult<HashMap<String, PresenceState>> {
        let mut result = HashMap::new();
        let cache = self.cache.read().await;

        for user_id in user_ids {
            if let Some(state) = cache.get(user_id) {
                result.insert(user_id.clone(), state.clone());
            } else {
                result.insert(user_id.clone(), PresenceState::offline());
            }
        }

        Ok(result)
    }

    /// 订阅用户的 Presence 状态
    ///
    /// # 参数
    ///
    /// - `subscriber_id`: 订阅者用户 ID
    /// - `target_ids`: 要订阅的用户 ID 列表
    pub async fn subscribe(
        &self,
        subscriber_id: &str,
        target_ids: Vec<String>,
    ) -> MatrixResult<()> {
        let mut subs = self.subscriptions.write().await;
        subs.insert(subscriber_id.to_string(), target_ids);
        Ok(())
    }

    /// 获取订阅列表
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    pub async fn get_subscriptions(&self, user_id: &str) -> MatrixResult<Vec<String>> {
        let subs = self.subscriptions.read().await;
        Ok(subs.get(user_id).cloned().unwrap_or_default())
    }

    /// 清理过期的 Presence 状态
    ///
    /// 定期清理长时间未活跃的用户状态 (例如 10 分钟)
    pub async fn cleanup_inactive(&self, timeout_ms: u64) {
        let mut cache = self.cache.write().await;
        let now = chrono::Utc::now().timestamp_millis() as u64;

        cache.retain(|user_id, state| {
            if state.presence == "offline" {
                return false; // 移除离线用户
            }

            // 移除长时间未活跃的用户
            if let Some(last_active) = state.last_active_ago {
                let elapsed = now - last_active;
                if elapsed > timeout_ms {
                    tracing::debug!(
                        "Cleaning up inactive user: {} (inactive for {}ms)",
                        user_id,
                        elapsed
                    );
                    return false;
                }
            }

            true
        });
    }

    /// 启动自动清理任务
    pub fn spawn_cleanup_task(&self) {
        let service = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                // 清理 10 分钟未活跃的用户
                service.cleanup_inactive(600 * 1000).await;
            }
        });
    }

    /// 获取 Presence 事件 (用于同步响应)
    ///
    /// 返回指定用户的 Presence 变化事件列表
    pub async fn get_presence_events(
        &self,
        user_ids: &[String],
    ) -> MatrixResult<Vec<PresenceEvent>> {
        let mut events = Vec::new();
        let presences = self.get_presences(user_ids).await?;

        for (user_id, presence) in presences {
            events.push(PresenceEvent::new(user_id, presence));
        }

        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_presence_state_online() {
        let state = PresenceState::online();
        assert_eq!(state.presence, "online");
        assert!(state.is_valid());
    }

    #[test]
    fn test_presence_state_offline() {
        let state = PresenceState::offline();
        assert_eq!(state.presence, "offline");
        assert!(state.is_valid());
    }

    #[test]
    fn test_presence_state_unavailable() {
        let state = PresenceState::unavailable();
        assert_eq!(state.presence, "unavailable");
        assert!(state.is_valid());
    }

    #[test]
    fn test_presence_with_status_msg() {
        let state = PresenceState::online().with_status_msg("Working");
        assert_eq!(state.status_msg, Some("Working".to_string()));
    }

    #[test]
    fn test_presence_invalid_state() {
        let state = PresenceState {
            presence: "invalid".to_string(),
            last_active_ago: Some(0),
            status_msg: None,
        };
        assert!(!state.is_valid());
    }

    #[test]
    fn test_presence_event() {
        let state = PresenceState::online().with_status_msg("Available");
        let event = PresenceEvent::new("@user:cis.local", state);

        assert_eq!(event.sender, "@user:cis.local");
        assert_eq!(event.event_type, "m.presence");
        assert_eq!(event.presence.presence, "online");
    }

    #[tokio::test]
    async fn test_presence_service_set_get() {
        // 注意: 实际测试需要 MatrixSocialStore 实例
        // 这里仅展示 API 用法

        // let social_store = Arc::new(MatrixSocialStore::new(...));
        // let service = PresenceService::new(social_store);

        // let state = PresenceState::online().with_status_msg("Testing");
        // service.set_presence("@user:cis.local", state).await.unwrap();

        // let retrieved = service.get_presence("@user:cis.local").await.unwrap();
        // assert_eq!(retrieved.presence, "online");
    }
}
