//! 消息管理器
//!
//! 提供完整的消息发送、接收、查询和管理功能。

use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Utc;

use crate::db::ImDatabase;
use crate::types::*;
use crate::error::{ImError, Result};
use crate::search::ImMessageSearch;

/// 消息管理器
pub struct MessageManager {
    db: Arc<ImDatabase>,
    /// 语义搜索（可选）
    search: Option<Arc<ImMessageSearch>>,
    /// 内存中的消息缓存（最近消息）
    cache: Arc<RwLock<std::collections::HashMap<String, Message>>>,
    /// 配置
    config: MessageConfig,
}

/// 消息管理器配置
#[derive(Debug, Clone)]
pub struct MessageConfig {
    /// 缓存大小限制
    pub cache_size: usize,
    /// 最大消息长度
    pub max_message_length: usize,
    /// 是否启用搜索索引
    pub enable_search_index: bool,
}

impl Default for MessageConfig {
    fn default() -> Self {
        Self {
            cache_size: 1000,
            max_message_length: 4096,
            enable_search_index: true,
        }
    }
}

/// 发送消息选项
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// 是否需要送达确认
    pub require_delivery_confirmation: bool,
    /// 重试次数
    pub retry_count: u32,
    /// 消息引用（回复）
    pub reply_to: Option<String>,
    /// 是否需要持久化存储
    pub persist: bool,
}

/// 消息查询过滤器
#[derive(Debug, Clone, Default)]
pub struct MessageFilter {
    /// 发送者过滤
    pub sender_id: Option<String>,
    /// 内容类型过滤
    pub content_type: Option<String>,
    /// 时间范围开始
    pub after: Option<chrono::DateTime<Utc>>,
    /// 时间范围结束
    pub before: Option<chrono::DateTime<Utc>>,
}

/// 接收消息结果
#[derive(Debug, Clone)]
pub struct ReceiveResult {
    /// 消息是否为新消息
    pub is_new: bool,
    /// 是否需要通知
    pub should_notify: bool,
    /// 消息ID
    pub message_id: String,
}

impl MessageManager {
    /// 创建新的消息管理器
    pub fn new(db: Arc<ImDatabase>) -> Self {
        Self {
            db,
            search: None,
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
            config: MessageConfig::default(),
        }
    }
    
    /// 使用自定义配置创建
    pub fn with_config(mut self, config: MessageConfig) -> Self {
        self.config = config;
        self
    }
    
    /// 设置搜索器
    pub fn with_search(mut self, search: Arc<ImMessageSearch>) -> Self {
        self.search = Some(search);
        self
    }
    
    // ===== 消息发送 =====
    
    /// 发送文本消息
    pub async fn send_text(
        &self,
        session_id: &str,
        sender_id: &str,
        text: impl Into<String>,
        options: SendOptions,
    ) -> Result<Message> {
        let text = text.into();
        
        // 验证消息长度
        if text.len() > self.config.max_message_length {
            return Err(ImError::MessageTooLarge {
                size: text.len(),
                max: self.config.max_message_length,
            });
        }
        
        let content = if let Some(reply_to) = options.reply_to.clone() {
            MessageContent::Reply {
                reply_to,
                content: Box::new(MessageContent::Text { text }),
            }
        } else {
            MessageContent::Text { text }
        };
        
        self.send_message(session_id, sender_id, content, options).await
    }
    
    /// 发送图片消息
    pub async fn send_image(
        &self,
        session_id: &str,
        sender_id: &str,
        url: impl Into<String>,
        width: Option<u32>,
        height: Option<u32>,
        alt_text: Option<String>,
    ) -> Result<Message> {
        let content = MessageContent::Image {
            url: url.into(),
            width,
            height,
            alt_text,
        };
        
        self.send_message(session_id, sender_id, content, SendOptions::default()).await
    }
    
    /// 发送文件消息
    pub async fn send_file(
        &self,
        session_id: &str,
        sender_id: &str,
        name: impl Into<String>,
        url: impl Into<String>,
        size: u64,
        mime_type: Option<String>,
    ) -> Result<Message> {
        let content = MessageContent::File {
            name: name.into(),
            url: url.into(),
            size,
            mime_type,
        };
        
        self.send_message(session_id, sender_id, content, SendOptions::default()).await
    }
    
    /// 发送语音消息
    pub async fn send_voice(
        &self,
        session_id: &str,
        sender_id: &str,
        url: impl Into<String>,
        duration_secs: u32,
    ) -> Result<Message> {
        let content = MessageContent::Voice {
            url: url.into(),
            duration_secs,
        };
        
        self.send_message(session_id, sender_id, content, SendOptions::default()).await
    }
    
    /// 发送消息（通用方法）
    pub async fn send_message(
        &self,
        session_id: &str,
        sender_id: &str,
        content: MessageContent,
        options: SendOptions,
    ) -> Result<Message> {
        // 验证会话存在
        let session = self.db.get_session(session_id).await?
            .ok_or_else(|| ImError::ConversationNotFound(session_id.to_string()))?;
        
        // 验证发送者是参与者
        if !session.participants.contains(&sender_id.to_string()) {
            return Err(ImError::Unauthorized);
        }
        
        // 创建消息
        let message = Message::new(
            session_id.to_string(),
            sender_id.to_string(),
            content.clone(),
        );
        
        // 保存到数据库
        if options.persist {
            self.db.save_message(&message).await?;
        }
        
        // 添加到缓存
        self.add_to_cache(message.clone()).await;
        
        // 索引到向量存储（如果是文本消息）
        if self.config.enable_search_index {
            if let Some(search) = &self.search {
                if let Err(e) = search.index_message(&message.id, &content).await {
                    tracing::warn!("Failed to index message: {}", e);
                }
            }
        }
        
        // TODO: 发送到远程节点（如果是联邦会话）
        if options.require_delivery_confirmation {
            // 等待送达确认逻辑
            tracing::debug!("Waiting for delivery confirmation for message {}", message.id);
        }
        
        Ok(message)
    }
    
    // ===== 消息接收 =====
    
    /// 接收消息（来自远程节点）
    pub async fn receive_message(&self, message: Message) -> Result<ReceiveResult> {
        // 检查消息是否已存在
        let existing = self.get_message(&message.id).await?;
        let is_new = existing.is_none();
        
        if is_new {
            // 保存到数据库
            self.db.save_message(&message).await?;
            
            // 添加到缓存
            self.add_to_cache(message.clone()).await;
            
            // 索引到向量存储
            if self.config.enable_search_index {
                if let Some(search) = &self.search {
                    if let Err(e) = search.index_message(&message.id, &message.content).await {
                        tracing::warn!("Failed to index received message: {}", e);
                    }
                }
            }
        }
        
        Ok(ReceiveResult {
            is_new,
            should_notify: is_new,
            message_id: message.id.clone(),
        })
    }
    
    /// 批量接收消息
    pub async fn receive_messages(&self, messages: Vec<Message>) -> Result<Vec<ReceiveResult>> {
        let mut results = Vec::with_capacity(messages.len());
        
        for message in messages {
            match self.receive_message(message).await {
                Ok(result) => results.push(result),
                Err(e) => {
                    tracing::error!("Failed to receive message: {}", e);
                    // 继续处理其他消息
                }
            }
        }
        
        Ok(results)
    }
    
    // ===== 消息查询 =====
    
    /// 获取单条消息
    pub async fn get_message(&self, message_id: &str) -> Result<Option<Message>> {
        // 先查缓存
        {
            let cache = self.cache.read().await;
            if let Some(msg) = cache.get(message_id) {
                return Ok(Some(msg.clone()));
            }
        }
        
        // 查数据库
        self.db.get_message(message_id).await
    }
    
    /// 获取会话的消息历史
    pub async fn get_history(
        &self,
        session_id: &str,
        before: Option<chrono::DateTime<Utc>>,
        limit: usize,
    ) -> Result<Vec<Message>> {
        self.db.get_messages(session_id, before, limit).await
    }
    
    /// 获取最新消息
    pub async fn get_latest_message(&self, session_id: &str) -> Result<Option<Message>> {
        let messages = self.db.get_messages(session_id, None, 1).await?;
        Ok(messages.into_iter().next())
    }
    
    /// 搜索消息
    pub async fn search_messages(
        &self,
        query: &str,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Message>> {
        // 先尝试语义搜索
        if self.config.enable_search_index {
            if let Some(search) = &self.search {
                match search.semantic_search(query, session_id, limit).await {
                    Ok(results) => {
                        // 加载完整消息
                        let mut messages = Vec::new();
                        for result in results {
                            if let Ok(Some(msg)) = self.get_message(&result.message_id).await {
                                messages.push(msg);
                            }
                        }
                        if !messages.is_empty() {
                            return Ok(messages);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Semantic search failed: {}, falling back to keyword search", e);
                    }
                }
            }
        }
        
        // 回退到关键词搜索
        self.db.search_messages(query, session_id, limit).await
    }
    
    /// 基于过滤器的消息搜索
    pub async fn filter_messages(
        &self,
        session_id: &str,
        filter: MessageFilter,
        limit: usize,
    ) -> Result<Vec<Message>> {
        // 获取该会话的所有消息
        let messages = self.db.get_messages(session_id, filter.before, 1000).await?;
        
        // 应用过滤器
        let filtered: Vec<Message> = messages
            .into_iter()
            .filter(|msg| {
                // 发送者过滤
                if let Some(ref sender) = filter.sender_id {
                    if msg.sender_id != *sender {
                        return false;
                    }
                }
                
                // 时间范围过滤
                if let Some(ref after) = filter.after {
                    if msg.created_at < *after {
                        return false;
                    }
                }
                
                // 内容类型过滤
                if let Some(ref content_type) = filter.content_type {
                    if msg.content.content_type() != content_type {
                        return false;
                    }
                }
                
                true
            })
            .take(limit)
            .collect();
        
        Ok(filtered)
    }
    
    // ===== 消息状态管理 =====
    
    /// 标记消息已读
    pub async fn mark_as_read(
        &self,
        session_id: &str,
        user_id: &str,
        message_id: &str,
    ) -> Result<()> {
        self.db.mark_as_read(session_id, user_id, message_id).await
    }
    
    /// 批量标记已读（某个时间点之前的所有消息）
    pub async fn mark_all_as_read(
        &self,
        session_id: &str,
        user_id: &str,
        before: chrono::DateTime<Utc>,
    ) -> Result<usize> {
        let messages = self.db.get_messages(session_id, Some(before), 1000).await?;
        let mut count = 0;
        
        for msg in messages {
            if !msg.read_by.contains(&user_id.to_string()) {
                self.db.mark_as_read(session_id, user_id, &msg.id).await?;
                count += 1;
            }
        }
        
        Ok(count)
    }
    
    /// 获取未读消息数
    pub async fn get_unread_count(&self, session_id: &str, user_id: &str) -> Result<u64> {
        self.db.get_unread_count(session_id, user_id).await
    }
    
    // ===== 消息编辑与删除 =====
    
    /// 编辑消息
    pub async fn edit_message(
        &self,
        message_id: &str,
        sender_id: &str,
        new_content: MessageContent,
    ) -> Result<Message> {
        let mut message = self.db.get_message(message_id).await?
            .ok_or_else(|| ImError::InvalidMessage(format!("Message not found: {}", message_id)))?;
        
        // 验证发送者
        if message.sender_id != sender_id {
            return Err(ImError::Unauthorized);
        }
        
        message.content = new_content;
        message.updated_at = Some(Utc::now());
        
        self.db.save_message(&message).await?;
        
        // 更新缓存
        self.add_to_cache(message.clone()).await;
        
        Ok(message)
    }
    
    /// 删除消息
    pub async fn delete_message(&self, message_id: &str, sender_id: &str) -> Result<()> {
        // 验证消息存在且发送者匹配
        if let Some(message) = self.db.get_message(message_id).await? {
            if message.sender_id != sender_id {
                return Err(ImError::Unauthorized);
            }
        }
        
        // 从数据库删除
        self.db.delete_message(message_id).await?;
        
        // 从缓存删除
        {
            let mut cache = self.cache.write().await;
            cache.remove(message_id);
        }
        
        Ok(())
    }
    
    /// 撤回消息（保留占位符）
    pub async fn recall_message(&self, message_id: &str, sender_id: &str) -> Result<()> {
        let mut message = self.db.get_message(message_id).await?
            .ok_or_else(|| ImError::InvalidMessage(format!("Message not found: {}", message_id)))?;
        
        // 验证发送者
        if message.sender_id != sender_id {
            return Err(ImError::Unauthorized);
        }
        
        // 将内容替换为撤回标记
        message.content = MessageContent::Text { 
            text: "[消息已撤回]".to_string() 
        };
        message.updated_at = Some(Utc::now());
        
        self.db.save_message(&message).await?;
        self.add_to_cache(message).await;
        
        Ok(())
    }
    
    // ===== 缓存管理 =====
    
    /// 添加消息到缓存
    async fn add_to_cache(&self, message: Message) {
        let mut cache = self.cache.write().await;
        
        // 检查缓存大小
        if cache.len() >= self.config.cache_size {
            // 移除最旧的消息（简单策略：随机移除一个）
            if let Some(key) = cache.keys().next().cloned() {
                cache.remove(&key);
            }
        }
        
        cache.insert(message.id.clone(), message);
    }
    
    /// 清空缓存
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
    
    /// 获取缓存统计
    pub async fn cache_stats(&self) -> (usize, usize) {
        let cache = self.cache.read().await;
        (cache.len(), self.config.cache_size)
    }
    
    // ===== 批量操作 =====
    
    /// 批量获取消息
    pub async fn get_messages_batch(&self, message_ids: &[String]) -> Result<Vec<Message>> {
        let mut messages = Vec::with_capacity(message_ids.len());
        
        for id in message_ids {
            if let Some(msg) = self.get_message(id).await? {
                messages.push(msg);
            }
        }
        
        Ok(messages)
    }
    
    /// 批量删除消息（仅限发送者）
    pub async fn delete_messages_batch(
        &self, 
        message_ids: &[String], 
        sender_id: &str
    ) -> Result<usize> {
        let mut deleted = 0;
        
        for id in message_ids {
            if self.delete_message(id, sender_id).await.is_ok() {
                deleted += 1;
            }
        }
        
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    async fn setup_manager() -> (MessageManager, Arc<ImDatabase>, tempfile::TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(ImDatabase::open(temp_dir.path()).unwrap());
        let manager = MessageManager::new(Arc::clone(&db));
        (manager, db, temp_dir)
    }
    
    async fn create_test_session(db: &ImDatabase) -> String {
        let session = Conversation {
            id: Uuid::new_v4().to_string(),
            conversation_type: ConversationType::Group,
            name: Some("Test Session".to_string()),
            participants: vec!["user1".to_string(), "user2".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({}),
        };
        db.create_session(&session).await.unwrap();
        session.id
    }
    
    #[tokio::test]
    async fn test_send_text_message() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        let msg = manager
            .send_text(&session_id, "user1", "Hello World", SendOptions::default())
            .await
            .unwrap();
        
        assert_eq!(msg.sender_id, "user1");
        assert!(matches!(msg.content, MessageContent::Text { .. }));
    }
    
    #[tokio::test]
    async fn test_send_reply_message() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        let original = manager
            .send_text(&session_id, "user1", "Original message", SendOptions::default())
            .await
            .unwrap();
        
        let reply = manager
            .send_text(
                &session_id,
                "user2",
                "This is a reply",
                SendOptions {
                    reply_to: Some(original.id.clone()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        
        assert!(matches!(reply.content, MessageContent::Reply { .. }));
    }
    
    #[tokio::test]
    async fn test_receive_message() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        // 创建一条消息
        let message = Message::new(
            session_id.clone(),
            "remote_user".to_string(),
            MessageContent::Text { text: "Remote message".to_string() },
        );
        
        // 接收消息
        let result = manager.receive_message(message.clone()).await.unwrap();
        assert!(result.is_new);
        assert!(result.should_notify);
        
        // 验证消息已保存
        let retrieved = manager.get_message(&message.id).await.unwrap();
        assert!(retrieved.is_some());
    }
    
    #[tokio::test]
    async fn test_mark_as_read() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        let msg = manager
            .send_text(&session_id, "user1", "Test message", SendOptions::default())
            .await
            .unwrap();
        
        // 标记已读
        manager.mark_as_read(&session_id, "user2", &msg.id).await.unwrap();
        
        // 验证未读数为0
        let count = manager.get_unread_count(&session_id, "user2").await.unwrap();
        assert_eq!(count, 0);
    }
    
    #[tokio::test]
    async fn test_get_unread_count() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        // 发送两条消息
        manager
            .send_text(&session_id, "user1", "Message 1", SendOptions::default())
            .await
            .unwrap();
        manager
            .send_text(&session_id, "user1", "Message 2", SendOptions::default())
            .await
            .unwrap();
        
        // 检查未读数
        let count = manager.get_unread_count(&session_id, "user2").await.unwrap();
        assert_eq!(count, 2);
        
        // 标记所有已读
        manager.mark_all_as_read(&session_id, "user2", Utc::now()).await.unwrap();
        
        // 再次检查
        let count = manager.get_unread_count(&session_id, "user2").await.unwrap();
        assert_eq!(count, 0);
    }
    
    #[tokio::test]
    async fn test_edit_message() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        let msg = manager
            .send_text(&session_id, "user1", "Original", SendOptions::default())
            .await
            .unwrap();
        
        let edited = manager
            .edit_message(
                &msg.id,
                "user1",
                MessageContent::Text { text: "Edited".to_string() }
            )
            .await
            .unwrap();
        
        assert!(edited.updated_at.is_some());
        if let MessageContent::Text { text } = edited.content {
            assert_eq!(text, "Edited");
        }
    }
    
    #[tokio::test]
    async fn test_delete_message() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        let msg = manager
            .send_text(&session_id, "user1", "To be deleted", SendOptions::default())
            .await
            .unwrap();
        
        // 删除消息
        manager.delete_message(&msg.id, "user1").await.unwrap();
        
        // 验证消息已删除
        let retrieved = manager.get_message(&msg.id).await.unwrap();
        assert!(retrieved.is_none());
    }
    
    #[tokio::test]
    async fn test_recall_message() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        let msg = manager
            .send_text(&session_id, "user1", "To be recalled", SendOptions::default())
            .await
            .unwrap();
        
        // 撤回消息
        manager.recall_message(&msg.id, "user1").await.unwrap();
        
        // 验证消息内容已变更
        let recalled = manager.get_message(&msg.id).await.unwrap().unwrap();
        if let MessageContent::Text { text } = recalled.content {
            assert_eq!(text, "[消息已撤回]");
        }
    }
    
    #[tokio::test]
    async fn test_filter_messages() {
        let (manager, db, _temp) = setup_manager().await;
        let session_id = create_test_session(&db).await;
        
        // 发送多条消息
        manager
            .send_text(&session_id, "user1", "From user1", SendOptions::default())
            .await
            .unwrap();
        manager
            .send_text(&session_id, "user2", "From user2", SendOptions::default())
            .await
            .unwrap();
        
        // 按发送者过滤
        let filter = MessageFilter {
            sender_id: Some("user1".to_string()),
            ..Default::default()
        };
        
        let results = manager.filter_messages(&session_id, filter, 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].sender_id, "user1");
    }
    
    #[tokio::test]
    async fn test_cache_operations() {
        let (manager, _db, _temp) = setup_manager().await;
        
        // 检查初始缓存状态
        let (size, capacity) = manager.cache_stats().await;
        assert_eq!(size, 0);
        assert_eq!(capacity, 1000);
        
        // 清空缓存
        manager.clear_cache().await;
        
        let (size, _) = manager.cache_stats().await;
        assert_eq!(size, 0);
    }
}
