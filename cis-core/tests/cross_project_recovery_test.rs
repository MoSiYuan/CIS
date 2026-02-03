//! 跨项目上下文恢复端到端测试
//!
//! 测试场景:
//! 1. 在项目A创建对话
//! 2. 切换到项目B
//! 3. 询问"回到之前的项目"
//! 4. 验证能正确恢复项目A的上下文

use std::env;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use cis_core::conversation::context::{ConversationContext, SessionRecovery};
use cis_core::storage::conversation_db::ConversationDb;
use cis_core::vector::storage::VectorStorage;

/// 模拟 embedding service（用于测试）
use async_trait::async_trait;
use cis_core::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
use cis_core::error::Result;

struct MockEmbeddingService;

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 简单的确定性模拟：根据文本哈希生成向量
        let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
        let hash = text.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        for i in 0..DEFAULT_EMBEDDING_DIM {
            let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
            vec[i] = val;
        }
        // 归一化
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
}

#[tokio::test]
async fn test_cross_project_context_recovery() {
    let temp_dir = TempDir::new().unwrap();
    let project_a = temp_dir.path().join("project-a");
    let project_b = temp_dir.path().join("project-b");
    
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b).unwrap();
    
    // 创建共享的 conversation_db
    let conv_db_path = temp_dir.path().join("conversations.db");
    let conversation_db = Arc::new(ConversationDb::open(&conv_db_path).unwrap());
    
    // 创建向量存储（使用 mock embedding）
    let vector_db_path_a = temp_dir.path().join("vector-a.db");
    let vector_storage_a = Arc::new(
        VectorStorage::open_with_service(&vector_db_path_a, Arc::new(MockEmbeddingService)).unwrap()
    );
    
    // 初始化项目A
    let mut ctx_a = ConversationContext::with_vector_storage(
        "conv-a".to_string(),
        "session-1".to_string(),
        Some(project_a.clone()),
        vector_storage_a.clone(),
    );
    
    // 模拟开始对话
    ctx_a.start_conversation("session-1", Some(project_a.clone())).await.unwrap();
    
    // 添加对话内容
    ctx_a.add_user_message_async("项目A的问题: 如何设置导航？").await.unwrap();
    ctx_a.add_assistant_message_async("导航设置方法...").await.unwrap();
    
    // 保存对话（不生成摘要向量索引，因为我们使用不同的存储）
    let conv_a = cis_core::storage::conversation_db::Conversation {
        id: "conv-a".to_string(),
        session_id: "session-1".to_string(),
        project_path: Some(project_a.to_string_lossy().to_string()),
        summary: Some("关于导航设置的讨论".to_string()),
        topics: vec!["导航".to_string(), "配置".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    conversation_db.save_conversation(&conv_a).unwrap();
    
    // 保存消息
    for msg in &ctx_a.messages {
        let db_msg = cis_core::storage::conversation_db::ConversationMessage {
            id: msg.id.clone(),
            conversation_id: ctx_a.conversation_id.clone(),
            role: msg.role.to_string(),
            content: msg.content.clone(),
            timestamp: msg.timestamp,
        };
        conversation_db.save_message(&db_msg).unwrap();
    }
    
    // 切换到项目B
    let vector_db_path_b = temp_dir.path().join("vector-b.db");
    let vector_storage_b = Arc::new(
        VectorStorage::open_with_service(&vector_db_path_b, Arc::new(MockEmbeddingService)).unwrap()
    );
    
    let mut ctx_b = ConversationContext::with_vector_storage(
        "conv-b".to_string(),
        "session-1".to_string(),
        Some(project_b.clone()),
        vector_storage_b.clone(),
    );
    
    ctx_b.start_conversation("session-1", Some(project_b.clone())).await.unwrap();
    ctx_b.add_user_message_async("项目B的问题").await.unwrap();
    
    // 保存项目B的对话
    let conv_b = cis_core::storage::conversation_db::Conversation {
        id: "conv-b".to_string(),
        session_id: "session-1".to_string(),
        project_path: Some(project_b.to_string_lossy().to_string()),
        summary: Some("项目B的讨论".to_string()),
        topics: vec!["项目B".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    conversation_db.save_conversation(&conv_b).unwrap();
    
    // 询问"回到之前的项目" - 使用 SessionRecovery 查找可恢复会话
    let recovery = SessionRecovery::new(
        conversation_db.clone(),
        vector_storage_b.clone(),
    );
    
    let current_project_str = project_b.to_str().unwrap();
    let recoverable = recovery.find_recoverable_sessions("session-1", current_project_str, 10).unwrap();
    
    // 验证能找到项目A的会话
    assert!(!recoverable.is_empty(), "应该找到可恢复的会话");
    let found_project_a = recoverable.iter().any(|r| {
        r.project_path == project_a.to_string_lossy().to_string()
    });
    assert!(found_project_a, "应该能找到项目A的会话");
    
    // 找到项目A的会话并恢复
    let project_a_session = recoverable.iter()
        .find(|r| r.project_path == project_a.to_string_lossy().to_string())
        .expect("应该找到项目A的会话");
    
    // 恢复项目A的上下文
    let recovered_ctx = recovery.recover_context(&project_a_session.conversation_id).unwrap();
    
    // 验证上下文已恢复
    assert!(recovered_ctx.messages.iter().any(|m| m.content.contains("导航")));
    assert_eq!(recovered_ctx.project_path, Some(project_a.clone()));
}

#[tokio::test]
async fn test_cross_project_context_recovery_multiple_projects() {
    let temp_dir = TempDir::new().unwrap();
    
    // 创建三个项目
    let projects: Vec<PathBuf> = (0..3)
        .map(|i| temp_dir.path().join(format!("project-{}", i)))
        .collect();
    
    for project in &projects {
        std::fs::create_dir_all(project).unwrap();
    }
    
    // 创建共享的 conversation_db
    let conv_db_path = temp_dir.path().join("conversations.db");
    let conversation_db = Arc::new(ConversationDb::open(&conv_db_path).unwrap());
    
    // 为每个项目创建对话
    for (i, project) in projects.iter().enumerate() {
        let vector_db_path = temp_dir.path().join(format!("vector-{}.db", i));
        let vector_storage = Arc::new(
            VectorStorage::open_with_service(&vector_db_path, Arc::new(MockEmbeddingService)).unwrap()
        );
        
        let mut ctx = ConversationContext::with_vector_storage(
            format!("conv-{}", i),
            "session-multi".to_string(),
            Some(project.clone()),
            vector_storage,
        );
        
        ctx.start_conversation("session-multi", Some(project.clone())).await.unwrap();
        ctx.add_user_message_async(&format!("项目{}的问题", i)).await.unwrap();
        ctx.add_assistant_message_async(&format!("项目{}的答案", i)).await.unwrap();
        
        // 保存对话
        let conv = cis_core::storage::conversation_db::Conversation {
            id: format!("conv-{}", i),
            session_id: "session-multi".to_string(),
            project_path: Some(project.to_string_lossy().to_string()),
            summary: Some(format!("项目{}的讨论", i)),
            topics: vec![format!("topic-{}", i)],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        conversation_db.save_conversation(&conv).unwrap();
        
        // 保存消息
        for msg in &ctx.messages {
            let db_msg = cis_core::storage::conversation_db::ConversationMessage {
                id: msg.id.clone(),
                conversation_id: ctx.conversation_id.clone(),
                role: msg.role.to_string(),
                content: msg.content.clone(),
                timestamp: msg.timestamp,
            };
            conversation_db.save_message(&db_msg).unwrap();
        }
    }
    
    // 使用 SessionRecovery 查找所有可恢复会话
    let last_vector_path = temp_dir.path().join("vector-last.db");
    let last_vector_storage = Arc::new(
        VectorStorage::open_with_service(&last_vector_path, Arc::new(MockEmbeddingService)).unwrap()
    );
    
    let recovery = SessionRecovery::new(
        conversation_db.clone(),
        last_vector_storage,
    );
    
    let current_project = projects[2].to_str().unwrap();
    let recoverable = recovery.find_recoverable_sessions("session-multi", current_project, 10).unwrap();
    
    // 应该能找到其他两个项目
    assert_eq!(recoverable.len(), 2, "应该找到另外两个项目的会话");
    
    // 验证按时间排序（最新的在前）
    for i in 0..recoverable.len() - 1 {
        assert!(
            recoverable[i].last_active >= recoverable[i + 1].last_active,
            "应该按时间降序排列"
        );
    }
}
