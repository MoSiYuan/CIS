//! # Matrix Receipts API
//!
//! 实现已读回执功能,遵循 Matrix Client-Server API v1.11 规范。
//!
//! ## 端点
//!
//! - `POST /_matrix/client/v3/rooms/{roomId}/receipt/{receiptType}/{eventId}` - 发送已读回执
//! - `GET /_matrix/client/v3/rooms/{roomId}/receipts` - 获取回执列表
//!
//! ## Receipt 类型
//!
//! - `m.read` - 已读回执
//! - `m.read.private` - 私有已读回执 (不显示给其他用户)
//! - `m.fully_read` - 完全已读标记 (用户阅读到此点之前的所有消息)
//!
//! ## 使用示例
//!
//! ```ignore
//! use cis_core::matrix::receipts::{ReceiptService, ReceiptType};
//!
//! // 发送已读回执
//! service.set_receipt(
//!     "@user:cis.local",
//!     "!room:cis.local",
//!     "$event_id",
//!     ReceiptType::Read,
//! ).await?;
//! ```

use crate::matrix::error::{MatrixError, MatrixResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Receipt 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReceiptType {
    /// 已读回执
    Read,

    /// 私有已读回执
    ReadPrivate,

    /// 完全已读标记
    FullyRead,
}

impl ReceiptType {
    /// 获取类型字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            ReceiptType::Read => "m.read",
            ReceiptType::ReadPrivate => "m.read.private",
            ReceiptType::FullyRead => "m.fully_read",
        }
    }

    /// 从字符串解析
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "m.read" => Some(ReceiptType::Read),
            "m.read.private" => Some(ReceiptType::ReadPrivate),
            "m.fully_read" => Some(ReceiptType::FullyRead),
            _ => None,
        }
    }
}

/// Receipt 事件内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptEventContent {
    /// 已读回执映射 (user_id -> receipt info)
    #[serde(rename = "m.read")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<HashMap<String, ReceiptInfo>>,
}

/// Receipt 信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptInfo {
    /// 附加数据
    pub data: HashMap<String, serde_json::Value>,

    /// 已读事件 ID 列表
    #[serde(rename = "event_ids")]
    pub event_ids: Vec<String>,
}

/// Receipt 记录
#[derive(Debug, Clone)]
pub struct Receipt {
    /// 房间 ID
    pub room_id: String,

    /// 用户 ID
    pub user_id: String,

    /// 事件 ID
    pub event_id: String,

    /// Receipt 类型
    pub receipt_type: ReceiptType,

    /// 时间戳 (毫秒)
    pub timestamp: i64,
}

/// Receipt 服务
#[derive(Clone)]
pub struct ReceiptService {
    /// Receipt 存储 (room_id -> (user_id -> (receipt_type -> receipt)))
    store: Arc<RwLock<HashMap<String, HashMap<String, HashMap<ReceiptType, Receipt>>>>>,
}

impl ReceiptService {
    /// 创建新的 Receipt 服务
    pub fn new() -> Self {
        Self {
            store: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 设置已读回执
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    /// - `room_id`: 房间 ID
    /// - `event_id`: 事件 ID
    /// - `receipt_type`: Receipt 类型
    pub async fn set_receipt(
        &self,
        user_id: &str,
        room_id: &str,
        event_id: &str,
        receipt_type: ReceiptType,
    ) -> MatrixResult<()> {
        let receipt = Receipt {
            room_id: room_id.to_string(),
            user_id: user_id.to_string(),
            event_id: event_id.to_string(),
            receipt_type,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        let mut store = self.store.write().await;

        // 获取或创建房间存储
        let room_receipts = store
            .entry(room_id.to_string())
            .or_insert_with(HashMap::new);

        // 获取或创建用户存储
        let user_receipts = room_receipts
            .entry(user_id.to_string())
            .or_insert_with(HashMap::new);

        // 设置 Receipt
        user_receipts.insert(receipt_type, receipt);

        tracing::debug!(
            "Set receipt for user {} in room {}: event_id={}, type={:?}",
            user_id,
            room_id,
            event_id,
            receipt_type
        );

        Ok(())
    }

    /// 获取用户的已读回执
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    /// - `room_id`: 房间 ID
    /// - `receipt_type`: Receipt 类型
    pub async fn get_receipt(
        &self,
        user_id: &str,
        room_id: &str,
        receipt_type: ReceiptType,
    ) -> Option<Receipt> {
        let store = self.store.read().await;

        store
            .get(room_id)?
            .get(user_id)?
            .get(&receipt_type)
            .cloned()
    }

    /// 获取房间所有用户的已读回执
    ///
    /// # 参数
    ///
    /// - `room_id`: 房间 ID
    /// - `receipt_type`: Receipt 类型
    pub async fn get_room_receipts(
        &self,
        room_id: &str,
        receipt_type: ReceiptType,
    ) -> HashMap<String, Receipt> {
        let store = self.store.read().await;

        if let Some(room_receipts) = store.get(room_id) {
            let mut result = HashMap::new();

            for (user_id, user_receipts) in room_receipts.iter() {
                if let Some(receipt) = user_receipts.get(&receipt_type) {
                    result.insert(user_id.clone(), receipt.clone());
                }
            }

            result
        } else {
            HashMap::new()
        }
    }

    /// 构建 Receipt 事件 (用于同步响应)
    ///
    /// # 参数
    ///
    /// - `room_id`: 房间 ID
    /// - `receipt_type`: Receipt 类型
    pub async fn build_receipt_event(
        &self,
        room_id: &str,
        receipt_type: ReceiptType,
    ) -> Option<ReceiptEventContent> {
        let receipts = self.get_room_receipts(room_id, receipt_type).await;

        if receipts.is_empty() {
            return None;
        }

        let mut read_map = HashMap::new();

        for (user_id, receipt) in receipts {
            let info = ReceiptInfo {
                data: HashMap::new(),
                event_ids: vec![receipt.event_id],
            };
            read_map.insert(user_id, info);
        }

        Some(ReceiptEventContent {
            read: Some(read_map),
        })
    }

    /// 删除房间的所有 Receipt
    ///
    /// 当用户离开房间时调用
    ///
    /// # 参数
    ///
    /// - `user_id`: 用户 ID
    /// - `room_id`: 房间 ID
    pub async fn remove_user_from_room(&self, user_id: &str, room_id: &str) {
        let mut store = self.store.write().await;

        if let Some(room_receipts) = store.get_mut(room_id) {
            room_receipts.remove(user_id);
        }
    }

    /// 清理过期的 Receipt (例如 30 天前)
    pub async fn cleanup_old_receipts(&self, days: u64) {
        let mut store = self.store.write().await;
        let cutoff_ts = chrono::Utc::now().timestamp_millis() - (days * 24 * 60 * 60 * 1000) as i64;
        let mut total_cleaned = 0;

        for (_room_id, room_receipts) in store.iter_mut() {
            for (_user_id, user_receipts) in room_receipts.iter_mut() {
                let before = user_receipts.len();
                user_receipts.retain(|_, receipt| receipt.timestamp > cutoff_ts);
                total_cleaned += before - user_receipts.len();
            }
        }

        if total_cleaned > 0 {
            tracing::debug!(
                "Cleaned up {} old receipts (older than {} days)",
                total_cleaned,
                days
            );
        }
    }

    /// 启动自动清理任务
    pub fn spawn_cleanup_task(&self) {
        let service = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600)); // 每小时清理一次
            loop {
                interval.tick().await;
                // 清理 30 天前的回执
                service.cleanup_old_receipts(30).await;
            }
        });
    }
}

impl Default for ReceiptService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_receipt_type_from_str() {
        assert_eq!(ReceiptType::from_str("m.read"), Some(ReceiptType::Read));
        assert_eq!(
            ReceiptType::from_str("m.read.private"),
            Some(ReceiptType::ReadPrivate)
        );
        assert_eq!(
            ReceiptType::from_str("m.fully_read"),
            Some(ReceiptType::FullyRead)
        );
        assert_eq!(ReceiptType::from_str("unknown"), None);
    }

    #[tokio::test]
    async fn test_set_get_receipt() {
        let service = ReceiptService::new();

        service
            .set_receipt(
                "@user1:cis.local",
                "!room1:cis.local",
                "$event1",
                ReceiptType::Read,
            )
            .await
            .unwrap();

        let receipt = service
            .get_receipt("@user1:cis.local", "!room1:cis.local", ReceiptType::Read)
            .await;

        assert!(receipt.is_some());
        let receipt = receipt.unwrap();
        assert_eq!(receipt.event_id, "$event1");
        assert_eq!(receipt.receipt_type, ReceiptType::Read);
    }

    #[tokio::test]
    async fn test_multiple_users_receipts() {
        let service = ReceiptService::new();

        // 用户 1 设置已读
        service
            .set_receipt(
                "@user1:cis.local",
                "!room1:cis.local",
                "$event1",
                ReceiptType::Read,
            )
            .await
            .unwrap();

        // 用户 2 设置已读
        service
            .set_receipt(
                "@user2:cis.local",
                "!room1:cis.local",
                "$event2",
                ReceiptType::Read,
            )
            .await
            .unwrap();

        // 获取房间所有回执
        let receipts = service
            .get_room_receipts("!room1:cis.local", ReceiptType::Read)
            .await;

        assert_eq!(receipts.len(), 2);
        assert!(receipts.contains_key("@user1:cis.local"));
        assert!(receipts.contains_key("@user2:cis.local"));
    }

    #[tokio::test]
    async fn test_build_receipt_event() {
        let service = ReceiptService::new();

        service
            .set_receipt(
                "@user1:cis.local",
                "!room1:cis.local",
                "$event1",
                ReceiptType::Read,
            )
            .await
            .unwrap();

        let event = service
            .build_receipt_event("!room1:cis.local", ReceiptType::Read)
            .await;

        assert!(event.is_some());
        let event = event.unwrap();
        assert!(event.read.is_some());

        let read_map = event.read.unwrap();
        assert_eq!(read_map.len(), 1);
        assert!(read_map.contains_key("@user1:cis.local"));

        let receipt_info = read_map.get("@user1:cis.local").unwrap();
        assert_eq!(receipt_info.event_ids, vec!["$event1"]);
    }

    #[tokio::test]
    async fn test_remove_user_from_room() {
        let service = ReceiptService::new();

        service
            .set_receipt(
                "@user1:cis.local",
                "!room1:cis.local",
                "$event1",
                ReceiptType::Read,
            )
            .await
            .unwrap();

        // 移除用户
        service
            .remove_user_from_room("@user1:cis.local", "!room1:cis.local")
            .await;

        // 验证回执已删除
        let receipt = service
            .get_receipt("@user1:cis.local", "!room1:cis.local", ReceiptType::Read)
            .await;

        assert!(receipt.is_none());
    }
}
