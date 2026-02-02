//! # Conversation Context
//!
//! 对话上下文管理，维护当前对话状态和历史。
//! 支持向量检索历史消息和跨项目会话恢复。

use chrono::{DateTime, Utc};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

use crate::error::{CisError, Result};
use crate::storage::conversation_db::ConversationDb;
use crate::vector::VectorStorage;

/// 对话上下文
///
/// 管理单个对话的完整上下文，包括历史消息和元数据
/// 支持向量存储集成用于语义检索
#[derive(Debug, Clone)]
pub struct ConversationContext {
    /// 对话ID
    pub conversation_id: String,
    /// 会话ID
    pub session_id: String,
    /// 对话标题
    pub title: Option<String>,
    /// 项目路径
    pub project_path: Option<PathBuf>,
    /// 对话摘要
    pub summary: Option<String>,
    /// 话题标签
    pub topics: Vec<String>,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后更新时间
    pub last_updated: DateTime<Utc>,
    /// 消息历史
    pub messages: Vec<ContextMessage>,
    /// 最大历史消息数
    max_history: usize,
    /// 向量存储（可选）
    vector_storage: Option<Arc<VectorStorage>>,
}

/// 上下文消息
#[derive(Debug, Clone)]
pub struct ContextMessage {
    /// 消息ID
    pub id: String,
    /// 角色
    pub role: MessageRole,
    /// 内容
    pub content: String,
    /// 时间戳
    pub timestamp: DateTime<Utc>,
    /// 元数据（如token数、模型信息等）
    pub metadata: Option<serde_json::Value>,
}

/// 消息角色
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    /// 用户消息
    User,
    /// AI助手消息
    Assistant,
    /// 系统消息
    System,
    /// 工具调用结果
    Tool,
}

impl ToString for MessageRole {
    fn to_string(&self) -> String {
        match self {
            MessageRole::User => "user".to_string(),
            MessageRole::Assistant => "assistant".to_string(),
            MessageRole::System => "system".to_string(),
            MessageRole::Tool => "tool".to_string(),
        }
    }
}

impl ConversationContext {
    /// 创建新的对话上下文
    pub fn new(conversation_id: String, session_id: String) -> Self {
        let now = Utc::now();
        Self {
            conversation_id,
            session_id,
            title: None,
            project_path: None,
            summary: None,
            topics: Vec::new(),
            created_at: now,
            last_updated: now,
            messages: Vec::new(),
            max_history: 100,
            vector_storage: None,
        }
    }

    /// 创建新上下文（含向量存储集成）
    pub fn with_vector_storage(
        conversation_id: String,
        session_id: String,
        project_path: Option<PathBuf>,
        storage: Arc<VectorStorage>,
    ) -> Self {
        let now = Utc::now();
        Self {
            conversation_id,
            session_id,
            title: None,
            project_path,
            summary: None,
            topics: Vec::new(),
            created_at: now,
            last_updated: now,
            messages: Vec::new(),
            max_history: 100,
            vector_storage: Some(storage),
        }
    }

    /// 添加用户消息
    pub fn add_user_message(&mut self, content: impl Into<String>) -> String {
        let msg_id = format!("msg-{}-user", self.messages.len());
        let msg = ContextMessage {
            id: msg_id.clone(),
            role: MessageRole::User,
            content: content.into(),
            timestamp: Utc::now(),
            metadata: None,
        };
        self.add_message(msg);
        msg_id
    }

    /// 添加用户消息并建立向量索引（异步版本）
    pub async fn add_user_message_with_index(
        &mut self,
        content: impl Into<String>,
    ) -> Result<String> {
        let content = content.into();
        let id = Uuid::new_v4().to_string();
        let msg = ContextMessage {
            id: id.clone(),
            role: MessageRole::User,
            content: content.clone(),
            timestamp: Utc::now(),
            metadata: None,
        };

        // 向量索引（如果可用）
        if let Some(storage) = &self.vector_storage {
            let conv_msg = crate::vector::storage::ConversationMessage {
                message_id: id.clone(),
                room_id: self.conversation_id.clone(),
                sender: "user".to_string(),
                content: content.clone(),
                timestamp: Utc::now().timestamp(),
                message_type: "text".to_string(),
            };
            storage.index_message(&conv_msg).await?;
        }

        self.add_message(msg);
        Ok(id)
    }

    /// 添加助手消息
    pub fn add_assistant_message(
        &mut self,
        content: impl Into<String>,
        metadata: Option<serde_json::Value>,
    ) -> String {
        let msg_id = format!("msg-{}-assistant", self.messages.len());
        let msg = ContextMessage {
            id: msg_id.clone(),
            role: MessageRole::Assistant,
            content: content.into(),
            timestamp: Utc::now(),
            metadata,
        };
        self.add_message(msg);
        msg_id
    }

    /// 添加助手消息并建立向量索引（异步版本）
    pub async fn add_assistant_message_with_index(
        &mut self,
        content: impl Into<String>,
        metadata: Option<serde_json::Value>,
    ) -> Result<String> {
        let content = content.into();
        let id = Uuid::new_v4().to_string();
        let msg = ContextMessage {
            id: id.clone(),
            role: MessageRole::Assistant,
            content: content.clone(),
            timestamp: Utc::now(),
            metadata: metadata.clone(),
        };

        // 向量索引（如果可用）
        if let Some(storage) = &self.vector_storage {
            let conv_msg = crate::vector::storage::ConversationMessage {
                message_id: id.clone(),
                room_id: self.conversation_id.clone(),
                sender: "assistant".to_string(),
                content: content.clone(),
                timestamp: Utc::now().timestamp(),
                message_type: "text".to_string(),
            };
            storage.index_message(&conv_msg).await?;
        }

        self.add_message(msg);
        Ok(id)
    }

    /// 添加系统消息
    pub fn add_system_message(&mut self, content: impl Into<String>) -> String {
        let msg_id = format!("msg-{}-system", self.messages.len());
        let msg = ContextMessage {
            id: msg_id.clone(),
            role: MessageRole::System,
            content: content.into(),
            timestamp: Utc::now(),
            metadata: None,
        };
        self.add_message(msg);
        msg_id
    }

    /// 添加消息到历史
    fn add_message(&mut self, msg: ContextMessage) {
        self.messages.push(msg);
        self.last_updated = Utc::now();

        // 限制历史长度
        if self.messages.len() > self.max_history {
            self.messages.remove(0);
        }
    }

    /// 获取最近的消息
    pub fn recent_messages(&self, count: usize) -> &[ContextMessage] {
        let start = self.messages.len().saturating_sub(count);
        &self.messages[start..]
    }

    /// 获取最近的对话历史（用户和助手消息）
    pub fn recent_dialog(&self, count: usize) -> Vec<&ContextMessage> {
        self.messages
            .iter()
            .filter(|m| matches!(m.role, MessageRole::User | MessageRole::Assistant))
            .rev()
            .take(count)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// 向量检索相关历史（RAG支持）
    pub async fn retrieve_relevant_history(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ContextMessage>> {
        if let Some(storage) = &self.vector_storage {
            let results = storage
                .search_messages(query, Some(&self.conversation_id), limit, Some(0.7))
                .await?;

            // 将搜索结果转为ContextMessage
            let messages: Vec<ContextMessage> = results
                .into_iter()
                .map(|r| ContextMessage {
                    id: r.message_id,
                    role: if r.sender == "user" {
                        MessageRole::User
                    } else {
                        MessageRole::Assistant
                    },
                    content: r.content,
                    timestamp: DateTime::from_timestamp(r.timestamp, 0).unwrap_or_else(Utc::now),
                    metadata: None,
                })
                .collect();

            Ok(messages)
        } else {
            // 回退到最近N条
            Ok(self.recent_messages(limit).to_vec())
        }
    }

    /// 设置对话标题
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = Some(title.into());
        self.last_updated = Utc::now();
    }

    /// 设置对话摘要
    pub fn set_summary(&mut self, summary: impl Into<String>) {
        self.summary = Some(summary.into());
        self.last_updated = Utc::now();
    }

    /// 添加话题标签
    pub fn add_topic(&mut self, topic: impl Into<String>) {
        let topic = topic.into();
        if !self.topics.contains(&topic) {
            self.topics.push(topic);
            self.last_updated = Utc::now();
        }
    }

    /// 获取消息总数
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// 获取对话时长（分钟）
    pub fn duration_minutes(&self) -> i64 {
        (self.last_updated - self.created_at).num_minutes()
    }

    /// 清空历史（保留系统消息）
    pub fn clear_history(&mut self) {
        self.messages.retain(|m| matches!(m.role, MessageRole::System));
        self.last_updated = Utc::now();
    }

    /// 设置最大历史消息数
    pub fn set_max_history(&mut self, max: usize) {
        self.max_history = max;
        // 立即应用限制
        while self.messages.len() > self.max_history {
            self.messages.remove(0);
        }
    }

    /// 生成项目上下文注入提示
    pub fn project_context_prompt(&self) -> Option<String> {
        self.project_path.as_ref().map(|path| {
            format!(
                "当前工作目录: {}\n项目上下文已加载。",
                path.display()
            )
        })
    }

    /// 检测是否需要跨项目恢复
    pub fn needs_cross_project_recovery(&self) -> bool {
        // 如果对话中包含"回到之前的项目"等意图
        false // 由上层根据用户输入判断
    }

    /// 获取向量存储引用
    pub fn vector_storage(&self) -> Option<&Arc<VectorStorage>> {
        self.vector_storage.as_ref()
    }

    /// 设置向量存储
    pub fn set_vector_storage(&mut self, storage: Arc<VectorStorage>) {
        self.vector_storage = Some(storage);
    }

    /// 设置项目路径
    pub fn set_project_path(&mut self, path: impl Into<Option<PathBuf>>) {
        self.project_path = path.into();
        self.last_updated = Utc::now();
    }
}

/// 可恢复会话信息
#[derive(Debug, Clone)]
pub struct RecoverableSession {
    /// 项目路径
    pub project_path: String,
    /// 对话ID
    pub conversation_id: String,
    /// 会话摘要
    pub summary: Option<String>,
    /// 最后活跃时间
    pub last_active: DateTime<Utc>,
}

/// 跨项目会话恢复器
pub struct SessionRecovery {
    conversation_db: Arc<ConversationDb>,
    vector_storage: Arc<VectorStorage>,
}

impl SessionRecovery {
    /// 创建新的会话恢复器
    pub fn new(
        conversation_db: Arc<ConversationDb>,
        vector_storage: Arc<VectorStorage>,
    ) -> Self {
        Self {
            conversation_db,
            vector_storage,
        }
    }

    /// 搜索可恢复的历史会话
    pub fn find_recoverable_sessions(
        &self,
        session_id: &str,
        current_project: &str,
        limit: usize,
    ) -> Result<Vec<RecoverableSession>> {
        // 1. 获取会话历史中的所有项目
        let mut stmt = self.conversation_db.conn().prepare(
            "SELECT DISTINCT project_path FROM conversations 
             WHERE session_id = ?1 AND project_path IS NOT NULL AND project_path != ?2
             ORDER BY updated_at DESC LIMIT ?3",
        )?;

        let projects: Vec<String> = stmt
            .query_map(
                rusqlite::params![session_id, current_project, limit as i32],
                |row| row.get(0),
            )?
            .filter_map(|r| r.ok())
            .collect();

        // 2. 为每个项目获取最新会话摘要
        let mut sessions = Vec::new();
        for project in projects {
            if let Some(conv) = self
                .conversation_db
                .list_conversations_by_project(&project, 1)?
                .into_iter()
                .next()
            {
                sessions.push(RecoverableSession {
                    project_path: project,
                    conversation_id: conv.id,
                    summary: conv.summary,
                    last_active: conv.updated_at,
                });
            }
        }

        Ok(sessions)
    }

    /// 恢复指定项目的上下文
    pub fn recover_context(&self, conversation_id: &str) -> Result<ConversationContext> {
        // 1. 加载对话
        let conv = self
            .conversation_db
            .get_conversation(conversation_id)?
            .ok_or_else(|| CisError::conversation("Conversation not found".to_string()))?;

        // 2. 加载消息
        let db_messages = self.conversation_db.get_messages(conversation_id)?;

        // 3. 重建上下文
        let mut context = ConversationContext::with_vector_storage(
            conv.id,
            conv.session_id,
            conv.project_path.map(PathBuf::from),
            Arc::clone(&self.vector_storage),
        );

        // 4. 恢复标题
        if let Some(summary) = &conv.summary {
            context.set_summary(summary.clone());
        }

        // 5. 恢复话题
        for topic in &conv.topics {
            context.add_topic(topic.clone());
        }

        // 6. 恢复消息
        for db_msg in db_messages {
            let role = match db_msg.role.as_str() {
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "system" => MessageRole::System,
                "tool" => MessageRole::Tool,
                _ => MessageRole::User,
            };
            let msg = ContextMessage {
                id: db_msg.id,
                role,
                content: db_msg.content,
                timestamp: db_msg.timestamp,
                metadata: None,
            };
            context.messages.push(msg);
        }

        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_context() {
        let mut ctx =
            ConversationContext::new("conv-001".to_string(), "session-001".to_string());

        // 添加系统消息
        ctx.add_system_message("You are a helpful assistant.");
        assert_eq!(ctx.message_count(), 1);

        // 添加用户消息
        ctx.add_user_message("Hello!");
        assert_eq!(ctx.message_count(), 2);

        // 添加助手消息
        ctx.add_assistant_message("Hi there!", None);
        assert_eq!(ctx.message_count(), 3);

        // 测试获取最近消息
        let recent = ctx.recent_messages(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].content, "Hello!");

        // 测试获取对话历史
        let dialog = ctx.recent_dialog(10);
        assert_eq!(dialog.len(), 2); // 不包括系统消息

        // 测试话题标签
        ctx.add_topic("greeting");
        ctx.add_topic("test");
        assert_eq!(ctx.topics.len(), 2);

        // 测试重复标签不会重复添加
        ctx.add_topic("greeting");
        assert_eq!(ctx.topics.len(), 2);

        // 测试清空历史
        ctx.clear_history();
        assert_eq!(ctx.message_count(), 1); // 保留系统消息
    }

    #[test]
    fn test_max_history_limit() {
        let mut ctx =
            ConversationContext::new("conv-002".to_string(), "session-001".to_string());
        ctx.set_max_history(5);

        // 添加超过限制的消息
        for i in 0..10 {
            ctx.add_user_message(format!("Message {}", i));
        }

        assert_eq!(ctx.message_count(), 5);
    }

    #[test]
    fn test_project_context_prompt() {
        let mut ctx =
            ConversationContext::new("conv-003".to_string(), "session-001".to_string());

        // 无项目路径时返回None
        assert!(ctx.project_context_prompt().is_none());

        // 设置项目路径
        ctx.set_project_path(Some(PathBuf::from("/home/user/myproject")));
        let prompt = ctx.project_context_prompt();
        assert!(prompt.is_some());
        assert!(prompt.unwrap().contains("/home/user/myproject"));
    }

    #[test]
    fn test_with_vector_storage() {
        // 注意：此测试需要一个模拟的VectorStorage
        // 在实际测试中需要使用mock或测试数据库
    }
}
