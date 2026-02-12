//! # Matrix Typing API
//!
//! 实现输入指示器功能,遵循 Matrix Client-Server API v1.11 规范。
//!
//! ## 端点
//!
//! - `PUT /_matrix/client/v3/rooms/{roomId}/typing/{userId}` - 发送输入状态
//!
//! ## Typing 事件
//!
//! Typing 是瞬态事件,表示用户正在输入消息。
//!
//! ## 使用示例
//!
//! ```ignore
//! use cis_core::matrix::typing::{TypingService, TypingRequest};
//!
//! // 发送输入状态
//! let request = TypingRequest {
//!     typing: true,
//!     timeout: Some(30000),
//! };
//! service.set_typing("@user:cis.local", "!room:cis.local", request).await?;
//! ```

use crate::matrix::error::{MatrixError, MatrixResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Typing 请求
#[derive(Debug, Clone, Deserialize)]
pub struct TypingRequest {
    /// 是否正在输入
    pub typing: bool,

    /// 超时时间 (毫秒),默认 30 秒
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
}

/// Typing 事件 (用于同步响应)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingEvent {
    /// 正在输入的用户 ID 列表
    pub user_ids: Vec<String>,
}

/// Typing 服务
#[derive(Clone)]
pub struct TypingService {
    /// Typing 状态缓存 (room_id -> user_ids + expire_time)
    cache: Arc<RwLock<HashMap<String, RoomTypingState>>>,
}

/// 房间 Typing 状态
#[derive(Debug, Clone)]
struct RoomTypingState {
    /// 正在输入的用户及过期时间
    typing_users: HashMap<String, i64>, // user_id -> expire_ts
}

impl RoomTypingState {
    fn new() -> Self {
        Self {
            typing_users: HashMap::new(),
        }
    }

    /// 添加正在输入的用户
    fn add_user(&mut self, user_id: &str, timeout_ms: u64) {
        let now = chrono::Utc::now().timestamp_millis();
        let expire_ts = now + timeout_ms as i64;
        self.typing_users.insert(user_id.to_string(), expire_ts);
    }

    /// 移除输入状态
    fn remove_user(&mut self, user_id: &str) {
        self.typing_users.remove(user_id);
    }

    /// 获取未过期的用户列表
    fn get_active_users(&self) -> Vec<String> {
        let now = chrono::Utc::now().timestamp_millis();
        self.typing_users
            .iter()
            .filter(|(_, expire_ts)| **expire_ts > now)
            .map(|(user_id, _)| user_id.clone())
            .collect()
    }

    /// 清理过期用户
    fn cleanup_expired(&mut self) -> usize {
        let now = chrono::Utc::now().timestamp_millis();
        let before = self.typing_users.len();

        self.typing_users
            .retain(|_, expire_ts| *expire_ts > now);

        before - self.typing_users.len()
    }
}

impl TypingService {
    /// 创建新的 Typing 服务
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置用户的 Typing 状态
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    /// - `room_id`: 房间 ID
    /// - `request`: Typing 请求
    pub async fn set_typing(
        &self,
        user_id: &str,
        room_id: &str,
        request: TypingRequest,
    ) -> MatrixResult<()> {
        let mut cache = self.cache.write().await;

        // 获取或创建房间状态
        let room_state = cache
            .entry(room_id.to_string())
            .or_insert_with(RoomTypingState::new);

        if request.typing {
            // 设置输入状态
            let timeout = request.timeout.unwrap_or(30000); // 默认 30 秒
            room_state.add_user(user_id, timeout);

            tracing::debug!(
                "User {} started typing in room {} (timeout: {}ms)",
                user_id,
                room_id,
                timeout
            );
        } else {
            // 取消输入状态
            room_state.remove_user(user_id);

            tracing::debug!(
                "User {} stopped typing in room {}",
                user_id,
                room_id
            );
        }

        Ok(())
    }

    /// 获取房间的 Typing 状态
    ///
    /// # 参数
    ///
    /// - `room_id`: 房间 ID
    pub async fn get_typing(&self, room_id: &str) -> TypingEvent {
        let cache = self.cache.read().await;

        if let Some(room_state) = cache.get(room_id) {
            let user_ids = room_state.get_active_users();
            TypingEvent { user_ids }
        } else {
            TypingEvent {
                user_ids: Vec::new(),
            }
        }
    }

    /// 清理过期的 Typing 状态
    ///
    /// 应该定期调用 (例如每 10 秒)
    pub async fn cleanup_expired(&self) -> usize {
        let mut cache = self.cache.write().await;
        let mut total_cleaned = 0;

        for room_state in cache.values_mut() {
            total_cleaned += room_state.cleanup_expired();
        }

        if total_cleaned > 0 {
            tracing::debug!("Cleaned up {} expired typing indicators", total_cleaned);
        }

        total_cleaned
    }

    /// 启动自动清理任务
    pub fn spawn_cleanup_task(&self) {
        let service = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
            loop {
                interval.tick().await;
                service.cleanup_expired().await;
            }
        });
    }

    /// 移除房间的所有 Typing 状态
    ///
    /// 当用户离开房间时调用
    pub async fn remove_user_from_room(&self, user_id: &str, room_id: &str) {
        let mut cache = self.cache.write().await;

        if let Some(room_state) = cache.get_mut(room_id) {
            room_state.remove_user(user_id);
        }
    }

    /// 获取所有房间的 Typing 状态
    ///
    /// 用于批量同步
    pub async fn get_all_typing_states(&self) -> HashMap<String, TypingEvent> {
        let cache = self.cache.read().await;
        let mut result = HashMap::new();

        for (room_id, _) in cache.iter() {
            let typing_event = self.get_typing(room_id).await;
            result.insert(room_id.clone(), typing_event);
        }

        result
    }
}

impl Default for TypingService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_typing_state_add_user() {
        let mut state = RoomTypingState::new();
        state.add_user("@user1:cis.local", 30000);

        let users = state.get_active_users();
        assert_eq!(users.len(), 1);
        assert!(users.contains(&"@user1:cis.local".to_string()));
    }

    #[test]
    fn test_room_typing_state_remove_user() {
        let mut state = RoomTypingState::new();
        state.add_user("@user1:cis.local", 30000);
        state.remove_user("@user1:cis.local");

        let users = state.get_active_users();
        assert_eq!(users.len(), 0);
    }

    #[test]
    fn test_room_typing_state_multiple_users() {
        let mut state = RoomTypingState::new();
        state.add_user("@user1:cis.local", 30000);
        state.add_user("@user2:cis.local", 30000);
        state.add_user("@user3:cis.local", 30000);

        let users = state.get_active_users();
        assert_eq!(users.len(), 3);
    }

    #[tokio::test]
    async fn test_typing_service_set_typing() {
        let service = TypingService::new();

        let request = TypingRequest {
            typing: true,
            timeout: Some(30000),
        };

        service
            .set_typing("@user1:cis.local", "!room1:cis.local", request)
            .await
            .unwrap();

        let typing = service.get_typing("!room1:cis.local").await;
        assert_eq!(typing.user_ids.len(), 1);
        assert!(typing.user_ids.contains(&"@user1:cis.local".to_string()));
    }

    #[tokio::test]
    async fn test_typing_service_stop_typing() {
        let service = TypingService::new();

        // 开始输入
        let request_start = TypingRequest {
            typing: true,
            timeout: Some(30000),
        };
        service
            .set_typing("@user1:cis.local", "!room1:cis.local", request_start)
            .await
            .unwrap();

        // 停止输入
        let request_stop = TypingRequest {
            typing: false,
            timeout: None,
        };
        service
            .set_typing("@user1:cis.local", "!room1:cis.local", request_stop)
            .await
            .unwrap();

        let typing = service.get_typing("!room1:cis.local").await;
        assert_eq!(typing.user_ids.len(), 0);
    }

    #[tokio::test]
    async fn test_typing_service_multiple_rooms() {
        let service = TypingService::new();

        let request = TypingRequest {
            typing: true,
            timeout: Some(30000),
        };

        // 在不同房间输入
        service
            .set_typing("@user1:cis.local", "!room1:cis.local", request.clone())
            .await
            .unwrap();
        service
            .set_typing("@user1:cis.local", "!room2:cis.local", request)
            .await
            .unwrap();

        // 验证两个房间都有输入状态
        let typing1 = service.get_typing("!room1:cis.local").await;
        let typing2 = service.get_typing("!room2:cis.local").await;

        assert_eq!(typing1.user_ids.len(), 1);
        assert_eq!(typing2.user_ids.len(), 1);
    }
}
