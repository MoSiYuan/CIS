//! # Conversation Recovery Integration Tests
//!
//! 测试跨项目会话恢复功能

use std::sync::Arc;
use tempfile::tempdir;

use cis_core::conversation::{ConversationContext, SessionRecovery, RecoverableSession};
use cis_core::storage::conversation_db::{ConversationDb, Conversation, ConversationMessage};
use cis_core::vector::VectorStorage;
use cis_core::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
use async_trait::async_trait;

/// 模拟 embedding service（用于测试）
struct MockEmbeddingService;

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> cis_core::error::Result<Vec<f32>> {
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

    async fn batch_embed(&self, texts: &[&str]) -> cis_core::error::Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

/// 创建测试存储
fn create_test_storage(path: &std::path::Path) -> VectorStorage {
    let embedding: Arc<dyn EmbeddingService> = Arc::new(MockEmbeddingService);
    VectorStorage::open_with_service(path, embedding).expect("Failed to open vector storage")
}

/// 测试跨项目会话恢复基础功能
#[test]
fn test_cross_project_recovery() {
    // 创建临时目录
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_recovery.db");
    let vec_db_path = temp_dir.path().join("test_vec_recovery.db");
    
    // 创建数据库和存储
    let db = Arc::new(ConversationDb::open(&db_path).expect("Failed to open conversation DB"));
    let storage = Arc::new(create_test_storage(&vec_db_path));
    
    let recovery = SessionRecovery::new(Arc::clone(&db), Arc::clone(&storage));
    
    // 创建测试会话数据
    let session_id = "test-session-001";
    let project1 = "/home/user/project1";
    let project2 = "/home/user/project2";
    
    // 为项目1创建对话
    let conv1 = Conversation {
        id: "conv-001".to_string(),
        session_id: session_id.to_string(),
        project_path: Some(project1.to_string()),
        summary: Some("Project 1 discussion".to_string()),
        topics: vec!["rust".to_string(), "testing".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    db.save_conversation(&conv1).expect("Failed to save conversation 1");
    
    // 为对话添加消息
    let msg1 = ConversationMessage {
        id: "msg-001".to_string(),
        conversation_id: "conv-001".to_string(),
        role: "user".to_string(),
        content: "How do I write tests?".to_string(),
        timestamp: chrono::Utc::now(),
    };
    
    db.save_message(&msg1).expect("Failed to save message 1");
    
    // 为项目2创建对话
    let conv2 = Conversation {
        id: "conv-002".to_string(),
        session_id: session_id.to_string(),
        project_path: Some(project2.to_string()),
        summary: Some("Project 2 planning".to_string()),
        topics: vec!["architecture".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    db.save_conversation(&conv2).expect("Failed to save conversation 2");
    
    // 测试：查找可恢复会话
    let recoverable = recovery.find_recoverable_sessions(session_id, project1, 10)
        .expect("Failed to find recoverable sessions");
    
    // 应该找到项目2的会话（排除当前项目1）
    assert_eq!(recoverable.len(), 1, "Should find 1 recoverable session");
    assert_eq!(recoverable[0].project_path, project2, "Should be project2");
    assert_eq!(recoverable[0].conversation_id, "conv-002", "Should reference conv-002");
}

/// 测试恢复会话上下文
#[test]
fn test_recover_context() {
    // 创建临时目录
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_recover.db");
    let vec_db_path = temp_dir.path().join("test_vec_recover.db");
    
    // 创建数据库和存储
    let db = Arc::new(ConversationDb::open(&db_path).expect("Failed to open conversation DB"));
    let storage = Arc::new(create_test_storage(&vec_db_path));
    
    let recovery = SessionRecovery::new(Arc::clone(&db), Arc::clone(&storage));
    
    // 创建测试对话
    let conv = Conversation {
        id: "conv-recover-001".to_string(),
        session_id: "session-recover".to_string(),
        project_path: Some("/test/project".to_string()),
        summary: Some("Test recovery".to_string()),
        topics: vec!["topic1".to_string(), "topic2".to_string()],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    
    db.save_conversation(&conv).expect("Failed to save conversation");
    
    // 添加消息
    let messages = vec![
        ConversationMessage {
            id: "msg-r1".to_string(),
            conversation_id: "conv-recover-001".to_string(),
            role: "system".to_string(),
            content: "You are a helpful assistant.".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ConversationMessage {
            id: "msg-r2".to_string(),
            conversation_id: "conv-recover-001".to_string(),
            role: "user".to_string(),
            content: "Hello!".to_string(),
            timestamp: chrono::Utc::now(),
        },
        ConversationMessage {
            id: "msg-r3".to_string(),
            conversation_id: "conv-recover-001".to_string(),
            role: "assistant".to_string(),
            content: "Hi there!".to_string(),
            timestamp: chrono::Utc::now(),
        },
    ];
    
    for msg in &messages {
        db.save_message(msg).expect("Failed to save message");
    }
    
    // 恢复上下文
    let context = recovery.recover_context("conv-recover-001")
        .expect("Failed to recover context");
    
    // 验证恢复的上下文
    assert_eq!(context.conversation_id, "conv-recover-001");
    assert_eq!(context.session_id, "session-recover");
    assert_eq!(context.project_path, Some(std::path::PathBuf::from("/test/project")));
    assert_eq!(context.message_count(), 3);
    
    // 验证话题被恢复
    // 注意：当前实现中话题恢复可能不完整，这里只验证基本结构
}

/// 测试多项目会话恢复场景
#[test]
fn test_multi_project_session_recovery() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_multi.db");
    let vec_db_path = temp_dir.path().join("test_vec_multi.db");
    
    let db = Arc::new(ConversationDb::open(&db_path).expect("Failed to open conversation DB"));
    let storage = Arc::new(create_test_storage(&vec_db_path));
    
    let recovery = SessionRecovery::new(Arc::clone(&db), Arc::clone(&storage));
    
    let session_id = "multi-session";
    let current_project = "/current/project";
    
    // 创建多个项目的历史对话
    let projects = vec![
        "/project/a",
        "/project/b",
        "/project/c",
        current_project,
    ];
    
    for (i, project) in projects.iter().enumerate() {
        let conv = Conversation {
            id: format!("multi-conv-{}", i),
            session_id: session_id.to_string(),
            project_path: Some(project.to_string()),
            summary: Some(format!("Discussion for {}", project)),
            topics: vec![format!("topic{}", i)],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        
        db.save_conversation(&conv).expect("Failed to save conversation");
    }
    
    // 查找可恢复会话
    let recoverable = recovery.find_recoverable_sessions(session_id, current_project, 10)
        .expect("Failed to find recoverable sessions");
    
    // 应该找到3个其他项目的会话
    assert_eq!(recoverable.len(), 3, "Should find 3 recoverable sessions from other projects");
    
    // 验证没有当前项目的会话
    for session in &recoverable {
        assert_ne!(session.project_path, current_project, "Should not include current project");
    }
}

/// 测试限制返回数量
#[test]
fn test_recovery_limit() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_limit.db");
    let vec_db_path = temp_dir.path().join("test_vec_limit.db");
    
    let db = Arc::new(ConversationDb::open(&db_path).expect("Failed to open conversation DB"));
    let storage = Arc::new(create_test_storage(&vec_db_path));
    
    let recovery = SessionRecovery::new(Arc::clone(&db), Arc::clone(&storage));
    
    let session_id = "limit-session";
    
    // 创建10个不同项目的对话
    for i in 0..10 {
        let conv = Conversation {
            id: format!("limit-conv-{}", i),
            session_id: session_id.to_string(),
            project_path: Some(format!("/project/{}", i)),
            summary: Some(format!("Project {} discussion", i)),
            topics: vec![],
            created_at: chrono::Utc::now() - chrono::Duration::seconds((10 - i) as i64),
            updated_at: chrono::Utc::now() - chrono::Duration::seconds((10 - i) as i64),
        };
        
        db.save_conversation(&conv).expect("Failed to save conversation");
    }
    
    // 限制只返回2个
    let recoverable = recovery.find_recoverable_sessions(session_id, "/project/5", 2)
        .expect("Failed to find recoverable sessions");
    
    assert_eq!(recoverable.len(), 2, "Should respect the limit");
}

/// 测试 RecoverableSession 结构
#[test]
fn test_recoverable_session_structure() {
    let session = RecoverableSession {
        project_path: "/test/path".to_string(),
        conversation_id: "conv-test".to_string(),
        summary: Some("Test summary".to_string()),
        last_active: chrono::Utc::now(),
    };
    
    assert_eq!(session.project_path, "/test/path");
    assert_eq!(session.conversation_id, "conv-test");
    assert!(session.summary.is_some());
    assert_eq!(session.summary.unwrap(), "Test summary");
}

/// 测试空会话恢复场景
#[test]
fn test_empty_session_recovery() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_empty.db");
    let vec_db_path = temp_dir.path().join("test_vec_empty.db");
    
    let db = Arc::new(ConversationDb::open(&db_path).expect("Failed to open conversation DB"));
    let storage = Arc::new(create_test_storage(&vec_db_path));
    
    let recovery = SessionRecovery::new(Arc::clone(&db), Arc::clone(&storage));
    
    // 查询一个没有历史记录的会话
    let recoverable = recovery.find_recoverable_sessions("empty-session", "/any/project", 10)
        .expect("Should not fail on empty session");
    
    assert!(recoverable.is_empty(), "Should return empty list for new session");
}
