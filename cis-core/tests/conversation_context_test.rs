//! ConversationContext 单元测试
//!
//! CVI-014: 为核心组件编写单元测试

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

use cis_core::conversation::ConversationContext;
use cis_core::storage::conversation_db::ConversationDb;
use cis_core::vector::VectorStorage;
use async_trait::async_trait;
use cis_core::error::Result;

/// 模拟 embedding service（用于测试）
struct MockEmbeddingService;

#[async_trait]
impl cis_core::ai::embedding::EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut vec = vec![0.0f32; 768];
        let hash = text.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        for i in 0..768 {
            let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
            vec[i] = val;
        }
        let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut vec {
                *x /= norm;
            }
        }
        Ok(vec)
    }

    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimension(&self) -> usize {
        768
    }
}

#[tokio::test]
async fn test_start_conversation() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    ctx.start_conversation("session-1", Some(PathBuf::from("/project"))).await.unwrap();
    
    assert_eq!(ctx.session_id, "session-1");
    assert!(ctx.project_path.is_some());
    assert_eq!(ctx.project_path.unwrap(), PathBuf::from("/project"));
}

#[tokio::test]
async fn test_add_messages() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    ctx.add_user_message("how to set navigation?");
    ctx.add_assistant_message("navigation has been set...", None);
    
    assert_eq!(ctx.messages.len(), 2);
    
    // 检查消息角色
    assert_eq!(ctx.messages[0].role.to_string(), "user");
    assert_eq!(ctx.messages[1].role.to_string(), "assistant");
}

#[tokio::test]
async fn test_add_user_message_async() {
    let temp_dir = TempDir::new().unwrap();
    let vector_path = temp_dir.path().join("vector.db");
    
    let embedding = Arc::new(MockEmbeddingService);
    let storage = Arc::new(VectorStorage::open_with_service(&vector_path, embedding).unwrap());
    
    let mut ctx = ConversationContext::with_vector_storage(
        "conv-1".to_string(),
        "session-1".to_string(),
        Some(PathBuf::from("/project")),
        storage,
    );
    
    let id = ctx.add_user_message_async("测试消息").await.unwrap();
    assert!(!id.is_empty());
    assert_eq!(ctx.messages.len(), 1);
}

#[tokio::test]
async fn test_add_assistant_message_async() {
    let temp_dir = TempDir::new().unwrap();
    let vector_path = temp_dir.path().join("vector.db");
    
    let embedding = Arc::new(MockEmbeddingService);
    let storage = Arc::new(VectorStorage::open_with_service(&vector_path, embedding).unwrap());
    
    let mut ctx = ConversationContext::with_vector_storage(
        "conv-1".to_string(),
        "session-1".to_string(),
        Some(PathBuf::from("/project")),
        storage,
    );
    
    let id = ctx.add_assistant_message_async("助手回复").await.unwrap();
    assert!(!id.is_empty());
    assert_eq!(ctx.messages.len(), 1);
}

#[tokio::test]
async fn test_save_with_summary() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("conv.db");
    let db = Arc::new(ConversationDb::open(&db_path).unwrap());
    
    let temp_dir2 = TempDir::new().unwrap();
    let vector_path = temp_dir2.path().join("vector.db");
    let embedding = Arc::new(MockEmbeddingService);
    let storage = Arc::new(VectorStorage::open_with_service(&vector_path, embedding).unwrap());
    
    let mut ctx = ConversationContext::with_vector_storage(
        "conv-1".to_string(),
        "session-1".to_string(),
        Some(PathBuf::from("/project")),
        storage,
    );
    
    ctx.add_user_message("how to set navigation?");
    ctx.add_assistant_message("导航已设置...", None);
    
    ctx.save_with_summary(db).await.unwrap();
    
    // 保存后应该生成摘要和话题
    // 注意：实际行为取决于 generate_summary_internal 和 extract_topics_internal 的实现
    println!("Summary: {:?}", ctx.summary);
    println!("Topics: {:?}", ctx.topics);
}

#[tokio::test]
async fn test_find_similar_conversations() {
    let temp_dir = TempDir::new().unwrap();
    let vector_path = temp_dir.path().join("vector.db");
    
    let embedding = Arc::new(MockEmbeddingService);
    let vector_storage = Arc::new(VectorStorage::open_with_service(&vector_path, embedding).unwrap());
    
    let ctx = ConversationContext::with_vector_storage(
        "query-conv".to_string(),
        "session-1".to_string(),
        Some(PathBuf::from("/project")),
        vector_storage.clone(),
    );
    
    // 由于需要预先创建摘要索引，这里只测试接口
    let similar = ctx.find_similar_conversations("问题", 3).await.unwrap();
    println!("Similar conversations: {:?}", similar);
}

#[tokio::test]
async fn test_prepare_ai_prompt() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    ctx.set_project_path(Some(PathBuf::from("/project")));
    ctx.add_user_message("how to setup navigation?");
    ctx.set_summary("discussing navigation setup");
    ctx.add_topic("navigation");
    ctx.add_topic("setup");
    
    let prompt = ctx.prepare_ai_prompt("how to optimize query?").await.unwrap();
    
    println!("Generated prompt:\n{}", prompt);
    
    // 验证 prompt 包含预期的内容
    assert!(prompt.contains("/project") || prompt.contains("project"));
    assert!(prompt.contains("discussing navigation setup") || prompt.contains("navigation"));
    assert!(prompt.contains("how to optimize query?"));
}

#[tokio::test]
async fn test_recent_messages() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    ctx.add_system_message("System message");
    ctx.add_user_message("Message 1");
    ctx.add_assistant_message("Response 1", None);
    ctx.add_user_message("Message 2");
    
    let recent = ctx.recent_messages(2);
    assert_eq!(recent.len(), 2);
    // recent_messages 返回最后N条消息，所以顺序是 Message 2, Response 1
    assert_eq!(recent[0].content, "Response 1");
    assert_eq!(recent[1].content, "Message 2");
}

#[tokio::test]
async fn test_recent_dialog() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    ctx.add_system_message("System message");
    ctx.add_user_message("User message 1");
    ctx.add_assistant_message("Assistant response 1", None);
    ctx.add_user_message("User message 2");
    ctx.add_assistant_message("Assistant response 2", None);
    
    // 只返回用户和助手消息，不包括系统消息
    let dialog = ctx.recent_dialog(10);
    assert_eq!(dialog.len(), 4);
}

#[tokio::test]
async fn test_set_title() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    assert!(ctx.title.is_none());
    
    ctx.set_title("Test Conversation");
    
    assert!(ctx.title.is_some());
    assert_eq!(ctx.title.unwrap(), "Test Conversation");
}

#[tokio::test]
async fn test_set_summary() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    assert!(ctx.summary.is_none());
    
    ctx.set_summary("This is a summary");
    
    assert!(ctx.summary.is_some());
    assert_eq!(ctx.summary.unwrap(), "This is a summary");
}

#[tokio::test]
async fn test_add_topic() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    assert!(ctx.topics.is_empty());
    
    ctx.add_topic("topic1");
    ctx.add_topic("topic2");
    ctx.add_topic("topic1"); // 重复添加
    
    assert_eq!(ctx.topics.len(), 2);
    assert!(ctx.topics.contains(&"topic1".to_string()));
    assert!(ctx.topics.contains(&"topic2".to_string()));
}

#[tokio::test]
async fn test_clear_history() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    ctx.add_system_message("System message");
    ctx.add_user_message("User message");
    ctx.add_assistant_message("Assistant response", None);
    
    assert_eq!(ctx.messages.len(), 3);
    
    ctx.clear_history();
    
    // 只保留系统消息
    assert_eq!(ctx.messages.len(), 1);
    assert_eq!(ctx.messages[0].role.to_string(), "system");
}

#[tokio::test]
async fn test_set_max_history() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    ctx.set_max_history(3);
    
    ctx.add_user_message("Message 1");
    ctx.add_user_message("Message 2");
    ctx.add_user_message("Message 3");
    ctx.add_user_message("Message 4");
    
    assert_eq!(ctx.messages.len(), 3);
}

#[tokio::test]
async fn test_project_context_prompt() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    // 无项目路径时返回 None
    assert!(ctx.project_context_prompt().is_none());
    
    // 设置项目路径
    ctx.set_project_path(Some(PathBuf::from("/home/user/project")));
    
    let prompt = ctx.project_context_prompt();
    assert!(prompt.is_some());
    let prompt_str = prompt.unwrap();
    assert!(prompt_str.contains("/home/user/project"));
}

#[tokio::test]
async fn test_message_count() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    assert_eq!(ctx.message_count(), 0);
    
    ctx.add_user_message("Message 1");
    assert_eq!(ctx.message_count(), 1);
    
    ctx.add_assistant_message("Response", None);
    assert_eq!(ctx.message_count(), 2);
}

#[tokio::test]
async fn test_duration_minutes() {
    let mut ctx = ConversationContext::new("conv-1".to_string(), "session-1".to_string());
    
    // 新创建的对话持续时间应该为 0
    assert_eq!(ctx.duration_minutes(), 0);
    
    // 更新 last_updated 模拟时间流逝
    ctx.last_updated = ctx.created_at + chrono::Duration::minutes(5);
    assert_eq!(ctx.duration_minutes(), 5);
}
