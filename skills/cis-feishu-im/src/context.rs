//! 对话上下文管理
//!
//! 维护对话历史，支持多轮对话
//! 数据存储在 IM 数据库（feishu_im.db），严格分离于记忆数据库

use cis_core::ai::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

use crate::config::{expand_path, ContextConfig};

/// 对话会话
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSession {
    /// 会话 ID（通常是 chat_id）
    pub session_id: String,

    /// 对话历史
    pub messages: Vec<Message>,

    /// 创建时间（Unix 时间戳）
    pub created_at: u64,

    /// 最后活跃时间（Unix 时间戳）
    pub last_active: u64,
}

/// 内存中的会话（使用 Instant 用于超时检查）
#[derive(Debug, Clone)]
struct SessionCache {
    session: ConversationSession,
    created_instant: Instant,
    active_instant: Instant,
}

/// 对话上下文管理器
///
/// 负责管理对话历史，数据存储在 IM 数据库
pub struct ConversationContext {
    /// 内存中的会话缓存
    sessions: RwLock<HashMap<String, SessionCache>>,

    /// 配置
    config: ContextConfig,

    /// 数据库路径
    db_path: PathBuf,
}

impl ConversationContext {
    /// 创建新的对话上下文管理器
    pub fn new(config: ContextConfig) -> Self {
        let db_path = expand_path(&PathBuf::from("~/.cis/data/feishu_im.db"));

        Self {
            sessions: RwLock::new(HashMap::new()),
            config,
            db_path,
        }
    }

    /// 添加消息到会话
    pub async fn add_message(&self, session_id: &str, message: Message) {
        let mut sessions = self.sessions.write().await;
        let now_ts = chrono::Utc::now().timestamp() as u64;
        let now = Instant::now();

        let cache = sessions
            .entry(session_id.to_string())
            .or_insert_with(|| {
                let session = ConversationSession {
                    session_id: session_id.to_string(),
                    messages: Vec::new(),
                    created_at: now_ts,
                    last_active: now_ts,
                };
                SessionCache {
                    session,
                    created_instant: now,
                    active_instant: now,
                }
            });

        cache.session.messages.push(message);
        cache.session.last_active = now_ts;
        cache.active_instant = now;

        // 持久化到数据库（如果启用）
        let session_clone = cache.session.clone();
        if self.config.persist_context {
            if let Err(e) = self.persist_session(&session_clone).await {
                tracing::error!("持久化会话失败: {}", e);
            }
        }

        // 清理过期会话（在 drop cache 引用后）
        drop(cache);
        self.cleanup_expired_sessions(&mut sessions).await;
    }

    /// 获取会话历史（用于 AI 上下文）
    pub async fn get_history(&self, session_id: &str) -> Vec<Message> {
        let sessions = self.sessions.read().await;

        if let Some(cache) = sessions.get(session_id) {
            return cache.session.messages.clone();
        }

        Vec::new()
    }

    /// 清空会话历史
    pub async fn clear_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
    }

    /// 获取所有活跃会话
    pub async fn get_active_sessions(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// 清理过期会话
    async fn cleanup_expired_sessions(&self, sessions: &mut HashMap<String, SessionCache>) {
        let timeout = Duration::from_secs(self.config.context_timeout_secs);
        let now = Instant::now();

        // 移除超时的会话
        sessions.retain(|_, cache| {
            now.duration_since(cache.active_instant) < timeout
        });

        // 如果会话消息过多，保留最近的 N 条
        for cache in sessions.values_mut() {
            if cache.session.messages.len() > self.config.max_turns {
                let start = cache.session.messages.len() - self.config.max_turns;
                cache.session.messages = cache.session.messages.split_off(start);
            }
        }
    }

    /// 持久化会话到数据库
    async fn persist_session(&self, session: &ConversationSession) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: 实现 SQLite 持久化
        // 1. 连接到 feishu_im.db
        // 2. 插入或更新 sessions 表
        // 3. 插入或更新 messages 表

        tracing::debug!(
            "持久化会话: session_id={}, messages={}",
            session.session_id,
            session.messages.len()
        );

        Ok(())
    }

    /// 从数据库加载会话
    pub async fn load_session(&self, _session_id: &str) -> Result<Option<ConversationSession>, Box<dyn std::error::Error>> {
        // TODO: 实现 SQLite 加载
        // 1. 连接到 feishu_im.db
        // 2. 查询 sessions 表
        // 3. 查询 messages 表
        // 4. 构建 ConversationSession

        Ok(None)
    }

    /// 获取会话统计信息
    pub async fn get_stats(&self) -> ContextStats {
        let sessions = self.sessions.read().await;

        let total_sessions = sessions.len();
        let total_messages: usize = sessions.values().map(|s| s.session.messages.len()).sum();

        ContextStats {
            total_sessions,
            total_messages,
            active_sessions: sessions
                .values()
                .filter(|s| Instant::now().duration_since(s.active_instant).as_secs() < 300)
                .count(),
        }
    }
}

impl Clone for ConversationContext {
    fn clone(&self) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()), // 不克隆内部状态
            config: self.config.clone(),
            db_path: self.db_path.clone(),
        }
    }
}

/// 对话上下文统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    /// 总会话数
    pub total_sessions: usize,

    /// 总消息数
    pub total_messages: usize,

    /// 活跃会话数（5分钟内有活动）
    pub active_sessions: usize,
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new(ContextConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_add_message() {
        let ctx = ConversationContext::default();
        let session_id = "test_session";

        ctx.add_message(session_id, Message::user("Hello")).await;
        ctx.add_message(session_id, Message::assistant("Hi there!")).await;

        let history = ctx.get_history(session_id).await;
        assert_eq!(history.len(), 2);
    }

    #[tokio::test]
    async fn test_clear_session() {
        let ctx = ConversationContext::default();
        let session_id = "test_session";

        ctx.add_message(session_id, Message::user("Hello")).await;
        assert_eq!(ctx.get_history(session_id).await.len(), 1);

        ctx.clear_session(session_id).await;
        assert_eq!(ctx.get_history(session_id).await.len(), 0);
    }

    #[tokio::test]
    async fn test_max_turns_limit() {
        let mut config = ContextConfig::default();
        config.max_turns = 3;

        let ctx = ConversationContext::new(config);
        let session_id = "test_session";

        // 添加 5 条消息，但只保留最后 3 条
        for i in 0..5 {
            ctx.add_message(session_id, Message::user(&format!("Message {}", i))).await;
        }

        let history = ctx.get_history(session_id).await;
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_get_stats() {
        let ctx = ConversationContext::default();

        ctx.add_message("session1", Message::user("Hello")).await;
        ctx.add_message("session2", Message::user("World")).await;

        let stats = ctx.get_stats().await;
        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.total_messages, 2);
    }
}
