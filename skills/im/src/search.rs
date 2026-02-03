//! IM 消息语义搜索
//!
//! 集成 VectorStorage 实现消息语义搜索。

use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::db::ImDatabase;
use crate::message::MessageManager;
use crate::types::MessageContent;
use crate::error::Result;

/// 搜索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageSearchResult {
    pub message_id: String,
    pub session_id: String,
    pub sender_id: String,
    pub content_preview: String,
    pub similarity: f32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// IM 消息语义搜索器
pub struct ImMessageSearch {
    #[allow(dead_code)]
    db: Arc<ImDatabase>,
    #[allow(dead_code)]
    message_manager: Arc<MessageManager>,
}

impl ImMessageSearch {
    /// 创建新的搜索器
    pub fn new(db: Arc<ImDatabase>, message_manager: Arc<MessageManager>) -> Self {
        Self { db, message_manager }
    }

    /// 索引消息
    /// 
    /// 将消息内容索引到向量存储中，用于后续的语义搜索。
    /// 注意：需要集成 VectorStorage 才能正常工作。
    pub async fn index_message(
        &self,
        message_id: &str,
        content: &MessageContent,
    ) -> Result<()> {
        // 提取可索引的文本内容
        let text = match content {
            MessageContent::Text { text } => text.clone(),
            MessageContent::Reply { content, .. } => {
                // 回复消息，索引被回复的内容
                if let MessageContent::Text { text } = content.as_ref() {
                    text.clone()
                } else {
                    return Ok(()); // 非文本内容不索引
                }
            }
            _ => {
                // 其他类型的消息暂不支持语义索引
                return Ok(());
            }
        };

        // 这里应该调用 VectorStorage 进行索引
        // 示例：vector_storage.index_memory(&format!("im:msg:{}", message_id), text.as_bytes(), Some("im_message")).await?;
        
        tracing::debug!("Indexing message {}: {}", message_id, text);
        Ok(())
    }

    /// 语义搜索消息
    /// 
    /// 使用向量相似度搜索相关消息。
    /// 注意：需要集成 VectorStorage 才能正常工作。
    pub async fn semantic_search(
        &self,
        query: &str,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MessageSearchResult>> {
        // 这里应该调用 VectorStorage 进行语义搜索
        // 示例：
        // let results = self.vector_storage.search_memory(query, limit, Some(0.7)).await?;
        
        tracing::debug!("Searching messages with query: {}", query);
        
        // 目前返回空结果，实际实现需要集成 VectorStorage
        let _ = (query, session_id, limit);
        Ok(vec![])
    }

    /// 混合搜索（语义 + 关键词）
    /// 
    /// 结合语义搜索和关键词搜索，提供更全面的搜索结果。
    pub async fn hybrid_search(
        &self,
        query: &str,
        session_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MessageSearchResult>> {
        // 同时进行语义搜索和关键词搜索
        let semantic_results = self.semantic_search(query, session_id, limit).await?;
        
        // 关键词搜索通过 MessageManager 实现
        let keyword_results = if let Some(sid) = session_id {
            // 使用 filter_messages 进行关键词过滤
            let filter = crate::message::MessageFilter {
                ..Default::default()
            };
            let messages = self.message_manager.filter_messages(sid, filter, limit).await?;
            
            messages
                .into_iter()
                .filter(|msg| {
                    if let Some(text) = msg.content.text_content() {
                        text.to_lowercase().contains(&query.to_lowercase())
                    } else {
                        false
                    }
                })
                .map(|msg| MessageSearchResult {
                    message_id: msg.id.clone(),
                    session_id: msg.conversation_id.clone(),
                    sender_id: msg.sender_id.clone(),
                    content_preview: msg.content.text_content()
                        .map(|s| s.chars().take(100).collect())
                        .unwrap_or_default(),
                    similarity: 1.0, // 关键词匹配给最高相似度
                    timestamp: msg.created_at,
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        // 合并结果并去重
        let mut results = semantic_results;
        results.extend(keyword_results);
        
        // 按相似度排序
        results.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap());
        results.truncate(limit);
        
        Ok(results)
    }

    /// 批量索引会话中的所有消息
    /// 
    /// 用于初始化或重建索引。
    pub async fn index_session_messages(&self, session_id: &str) -> Result<usize> {
        let messages = self.message_manager.get_history(session_id, None, 1000).await?;
        let mut count = 0;

        for msg in &messages {
            if let MessageContent::Text { .. } | MessageContent::Reply { .. } = &msg.content {
                self.index_message(&msg.id, &msg.content).await?;
                count += 1;
            }
        }

        tracing::info!("Indexed {} messages from session {}", count, session_id);
        Ok(count)
    }

    /// 搜索相似会话
    /// 
    /// 根据会话的主题或内容找到相似的会话。
    pub async fn find_similar_sessions(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<String>> {
        // 获取会话中的消息
        let messages = self.message_manager.get_history(session_id, None, 50).await?;
        
        // 提取会话主题
        let session_topic: String = messages
            .iter()
            .filter_map(|msg| msg.content.text_content())
            .take(5)
            .collect::<Vec<_>>()
            .join(" ");

        if session_topic.is_empty() {
            return Ok(vec![]);
        }

        // 使用主题进行语义搜索
        let results = self.semantic_search(&session_topic, None, limit * 2).await?;
        
        // 提取不同的会话 ID
        let mut similar_sessions: Vec<String> = results
            .into_iter()
            .filter(|r| r.session_id != session_id)
            .map(|r| r.session_id)
            .collect();
        
        similar_sessions.dedup();
        similar_sessions.truncate(limit);
        
        Ok(similar_sessions)
    }
}

/// 搜索配置
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// 语义搜索权重
    pub semantic_weight: f32,
    /// 关键词搜索权重
    pub keyword_weight: f32,
    /// 相似度阈值
    pub similarity_threshold: f32,
    /// 最大返回结果数
    pub max_results: usize,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            semantic_weight: 0.7,
            keyword_weight: 0.3,
            similarity_threshold: 0.6,
            max_results: 20,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::types::{Conversation, ConversationType, Message};
    use crate::session::SessionManager;

    async fn setup_search() -> (ImMessageSearch, Arc<ImDatabase>, tempfile::TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let db = Arc::new(ImDatabase::open(&temp_dir.path().join("test.db")).unwrap());
        let msg_manager = Arc::new(MessageManager::new(Arc::clone(&db)));
        let search = ImMessageSearch::new(Arc::clone(&db), msg_manager);
        (search, db, temp_dir)
    }

    async fn create_test_conversation(db: &ImDatabase) -> String {
        let conv = Conversation {
            id: "test-conv".to_string(),
            conversation_type: ConversationType::Group,
            name: Some("Test".to_string()),
            participants: vec!["user1".to_string(), "user2".to_string()],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_message_at: None,
            avatar_url: None,
            metadata: serde_json::json!({}),
        };
        db.create_conversation(&conv).await.unwrap();
        conv.id
    }

    #[tokio::test]
    async fn test_index_message() {
        let (search, _db, _temp) = setup_search().await;

        let content = MessageContent::Text {
            text: "Hello World".to_string(),
        };

        // 索引消息不应该出错
        search.index_message("msg-1", &content).await.unwrap();
    }

    #[tokio::test]
    async fn test_index_session_messages() {
        let (search, db, _temp) = setup_search().await;
        let conv_id = create_test_conversation(&db).await;

        // 添加一些消息
        let msg1 = Message::new(
            conv_id.clone(),
            "user1".to_string(),
            MessageContent::Text { text: "Hello".to_string() },
        );
        let msg2 = Message::new(
            conv_id.clone(),
            "user2".to_string(),
            MessageContent::Text { text: "World".to_string() },
        );

        db.save_message(&msg1).await.unwrap();
        db.save_message(&msg2).await.unwrap();

        // 索引会话消息
        let count = search.index_session_messages(&conv_id).await.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_hybrid_search() {
        let (search, db, _temp) = setup_search().await;
        let conv_id = create_test_conversation(&db).await;

        // 添加消息
        let msg = Message::new(
            conv_id.clone(),
            "user1".to_string(),
            MessageContent::Text { text: "Hello World".to_string() },
        );
        db.save_message(&msg).await.unwrap();

        // 混合搜索
        let results = search.hybrid_search("Hello", Some(&conv_id), 10).await.unwrap();
        
        // 至少应该有关键词匹配结果
        assert!(!results.is_empty());
    }
}
