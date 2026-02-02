//! 飞书对话会话管理
//!
//! 连接飞书对话与 CIS 核心 Session 系统

use crate::context::ConversationContext;
use cis_core::ai::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

/// 飞书对话会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSession {
    /// 会话 ID（CIS 内部使用）
    pub id: String,

    /// 飞书 chat_id
    pub chat_id: String,

    /// 会话名称（飞书群名/用户名）
    pub name: String,

    /// 会话类型
    pub session_type: FeishuSessionType,

    /// 创建时间
    pub created_at: i64,

    /// 最后活跃时间
    pub last_active: i64,

    /// 消息数量
    pub message_count: usize,

    /// 会话状态
    pub status: FeishuSessionStatus,
}

/// 会话类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeishuSessionType {
    /// 私聊
    Private,
    /// 群聊
    Group,
}

/// 会话状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FeishuSessionStatus {
    /// 活跃
    Active,
    /// 归档
    Archived,
    /// 已删除
    Deleted,
}

/// 飞书会话管理器
pub struct FeishuSessionManager {
    /// 会话存储
    sessions: Arc<RwLock<HashMap<String, FeishuSession>>>,

    /// chat_id 到 session_id 的映射
    chat_to_session: Arc<RwLock<HashMap<String, String>>>,

    /// 数据库路径
    db_path: PathBuf,

    /// 对话上下文
    context: Arc<ConversationContext>,
}

impl FeishuSessionManager {
    /// 创建新的会话管理器
    pub fn new(
        db_path: PathBuf,
        context: Arc<ConversationContext>,
    ) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            chat_to_session: Arc::new(RwLock::new(HashMap::new())),
            db_path,
            context,
        }
    }

    /// 创建或获取会话
    pub async fn get_or_create_session(
        &self,
        chat_id: &str,
        name: &str,
        session_type: FeishuSessionType,
    ) -> FeishuSession {
        // 检查是否已存在
        {
            let chat_map = self.chat_to_session.read().await;
            if let Some(session_id) = chat_map.get(chat_id) {
                let sessions = self.sessions.read().await;
                if let Some(session) = sessions.get(session_id) {
                    return session.clone();
                }
            }
        }

        // 创建新会话
        let session_id = format!("feishu_{}", chat_id);
        let now = Utc::now().timestamp_millis();

        let session = FeishuSession {
            id: session_id.clone(),
            chat_id: chat_id.to_string(),
            name: name.to_string(),
            session_type,
            created_at: now,
            last_active: now,
            message_count: 0,
            status: FeishuSessionStatus::Active,
        };

        // 保存会话
        {
            let mut sessions = self.sessions.write().await;
            let mut chat_map = self.chat_to_session.write().await;

            sessions.insert(session_id.clone(), session.clone());
            chat_map.insert(chat_id.to_string(), session_id);
        }

        // 持久化到数据库
        self.save_session(&session).await;

        session
    }

    /// 更新会话活跃时间
    pub async fn update_activity(&self, chat_id: &str) {
        let chat_map = self.chat_to_session.read().await;
        if let Some(session_id) = chat_map.get(chat_id) {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.last_active = Utc::now().timestamp_millis();
            }
        }
    }

    /// 增加消息计数
    pub async fn increment_message_count(&self, chat_id: &str) {
        let chat_map = self.chat_to_session.read().await;
        if let Some(session_id) = chat_map.get(chat_id) {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.message_count += 1;
            }
        }
    }

    /// 列出所有会话
    pub async fn list_sessions(&self) -> Vec<FeishuSession> {
        let sessions = self.sessions.read().await;
        sessions.values().cloned().collect()
    }

    /// 列出活跃会话
    pub async fn list_active_sessions(&self) -> Vec<FeishuSession> {
        let sessions = self.sessions.read().await;
        sessions
            .values()
            .filter(|s| s.status == FeishuSessionStatus::Active)
            .cloned()
            .collect()
    }

    /// 获取会话详情
    pub async fn get_session(&self, session_id: &str) -> Option<FeishuSession> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).cloned()
    }

    /// 根据 chat_id 获取会话
    pub async fn get_session_by_chat(&self, chat_id: &str) -> Option<FeishuSession> {
        let chat_map = self.chat_to_session.read().await;
        if let Some(session_id) = chat_map.get(chat_id) {
            let sessions = self.sessions.read().await;
            sessions.get(session_id).cloned()
        } else {
            None
        }
    }

    /// 获取会话对话历史
    pub async fn get_session_history(&self, session_id: &str) -> Vec<Message> {
        let session = self.get_session(session_id).await;
        if let Some(session) = session {
            self.context.get_history(&session.chat_id).await
        } else {
            Vec::new()
        }
    }

    /// 归档会话
    pub async fn archive_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.status = FeishuSessionStatus::Archived;
            true
        } else {
            false
        }
    }

    /// 删除会话
    pub async fn delete_session(&self, session_id: &str) -> bool {
        let mut sessions = self.sessions.write().await;
        let mut chat_map = self.chat_to_session.write().await;

        if let Some(session) = sessions.remove(session_id) {
            chat_map.remove(&session.chat_id);
            true
        } else {
            false
        }
    }

    /// 搜索会话（按名称）
    pub async fn search_sessions(&self, query: &str) -> Vec<FeishuSession> {
        let sessions = self.sessions.read().await;
        let query_lower = query.to_lowercase();

        sessions
            .values()
            .filter(|s| {
                s.name.to_lowercase().contains(&query_lower)
                    || s.chat_id.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// 保存会话到数据库
    async fn save_session(&self, session: &FeishuSession) {
        // TODO: 实现数据库持久化
        // 目前使用内存存储，后期可以集成到 CIS 的数据库系统
        tracing::debug!("保存会话: {} ({})", session.name, session.id);
    }

    /// 从数据库加载所有会话
    pub async fn load_sessions(&self) {
        // TODO: 从数据库加载会话
        // 目前会话在运行时动态创建
        tracing::debug!("加载会话历史");
    }

    /// 生成会话摘要（用于显示）
    pub fn format_session_summary(session: &FeishuSession) -> String {
        let type_str = match session.session_type {
            FeishuSessionType::Private => "私聊",
            FeishuSessionType::Group => "群聊",
        };

        let status_str = match session.status {
            FeishuSessionStatus::Active => "活跃",
            FeishuSessionStatus::Archived => "归档",
            FeishuSessionStatus::Deleted => "已删除",
        };

        let last_active = if session.last_active > 0 {
            let dt = DateTime::from_timestamp(session.last_active / 1000, 0)
                .unwrap_or_default();
            format!("{}", dt.format("%Y-%m-%d %H:%M"))
        } else {
            "从未活跃".to_string()
        };

        format!(
            "📱 {} [{}] {}\n   ID: {}\n   消息数: {}\n   最后活跃: {}",
            type_str,
            status_str,
            session.name,
            session.id,
            session.message_count,
            last_active
        )
    }

    /// 生成会话详情
    pub fn format_session_detail(session: &FeishuSession, history: &[Message]) -> String {
        let mut output = Self::format_session_summary(session);
        output.push_str("\n\n📝 对话历史:\n");

        for (i, msg) in history.iter().enumerate() {
            let role = match msg.role {
                cis_core::ai::Role::User => "👤 用户",
                cis_core::ai::Role::Assistant => "🤖 AI",
                cis_core::ai::Role::System => "⚙️ 系统",
            };

            let content = if msg.content.len() > 100 {
                format!("{}...", &msg.content[..97])
            } else {
                msg.content.clone()
            };

            output.push_str(&format!("  {}. {}: {}\n", i + 1, role, content));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_type_display() {
        let session = FeishuSession {
            id: "test".to_string(),
            chat_id: "oc_test".to_string(),
            name: "测试群".to_string(),
            session_type: FeishuSessionType::Group,
            created_at: 0,
            last_active: 0,
            message_count: 10,
            status: FeishuSessionStatus::Active,
        };

        let summary = FeishuSessionManager::format_session_summary(&session);
        assert!(summary.contains("群聊"));
        assert!(summary.contains("测试群"));
        assert!(summary.contains("活跃"));
    }
}
