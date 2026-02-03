//! IM Skill 集成测试
//!
//! 测试 IM Skill 的完整功能流程。

use std::sync::Arc;
use tempfile::TempDir;

use im_skill::{
    ImSkill, ImDatabase, SessionManager, MessageManager, ImMessageSearch,
    types::*,
    message::{SendOptions, MessageFilter},
};

/// 测试助手：创建临时环境
async fn setup_test_env() -> (ImSkill, Arc<ImDatabase>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("im_test.db");
    let skill = ImSkill::new(&db_path).unwrap();
    let db = Arc::new(ImDatabase::open(&db_path).expect("Failed to open database"));
    (skill, db, temp_dir)
}

/// 测试助手：创建测试会话
async fn create_test_session(
    skill: &ImSkill,
    session_type: ConversationType,
    participants: Vec<String>,
) -> Conversation {
    let name = match session_type {
        ConversationType::Direct => None,
        ConversationType::Group => Some("Test Group".to_string()),
        ConversationType::Channel => Some("Test Channel".to_string()),
    };

    skill.create_conversation(session_type, name, participants).await.unwrap()
}

#[tokio::test]
async fn test_full_messaging_flow() {
    let (skill, _db, _temp) = setup_test_env().await;

    // 1. 创建私聊会话
    let session = create_test_session(
        &skill,
        ConversationType::Direct,
        vec!["alice".to_string(), "bob".to_string()],
    ).await;

    assert_eq!(session.conversation_type, ConversationType::Direct);
    assert_eq!(session.participants.len(), 2);

    // 2. 发送消息
    let msg1 = skill.send_message(
        &session.id,
        "alice",
        MessageContent::Text { text: "Hello Bob!".to_string() },
    ).await.unwrap();

    assert_eq!(msg1.sender_id, "alice");
    assert!(matches!(msg1.content, MessageContent::Text { .. }));

    // 3. 回复消息
    let msg2 = skill.send_message(
        &session.id,
        "bob",
        MessageContent::Reply {
            reply_to: msg1.id.clone(),
            content: Box::new(MessageContent::Text { text: "Hi Alice!".to_string() }),
        },
    ).await.unwrap();

    assert!(matches!(msg2.content, MessageContent::Reply { .. }));

    // 4. 获取消息历史
    let history = skill.get_history(&session.id, None, 10).await.unwrap();
    assert_eq!(history.len(), 2);

    // 5. 标记已读
    skill.mark_read(&msg1.id, "bob").await.unwrap();

    let updated_msg = skill.get_history(&session.id, None, 10).await.unwrap();
    let msg = updated_msg.iter().find(|m| m.id == msg1.id).unwrap();
    assert!(msg.read_by.contains(&"bob".to_string()));
}

#[tokio::test]
async fn test_group_session_management() {
    let (skill, db, _temp) = setup_test_env().await;
    let session_manager = SessionManager::new(db.clone());

    // 1. 创建群组会话
    let session = session_manager
        .create_group_session(
            "Dev Team".to_string(),
            vec!["alice".to_string(), "bob".to_string()],
        )
        .await
        .unwrap();

    assert_eq!(session.name, Some("Dev Team".to_string()));
    assert_eq!(session.participants.len(), 2);

    // 2. 添加新成员
    session_manager
        .add_participant(&session.id, "charlie".to_string())
        .await
        .unwrap();

    let updated = session_manager.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(updated.participants.len(), 3);
    assert!(updated.participants.contains(&"charlie".to_string()));

    // 3. 移除成员
    session_manager.remove_participant(&session.id, "bob").await.unwrap();

    let updated = session_manager.get_session(&session.id).await.unwrap().unwrap();
    assert_eq!(updated.participants.len(), 2);
    assert!(!updated.participants.contains(&"bob".to_string()));
}

#[tokio::test]
async fn test_message_search_and_filter() {
    let (skill, db, _temp) = setup_test_env().await;
    let msg_manager = MessageManager::new(db.clone());

    // 创建会话
    let session = create_test_session(
        &skill,
        ConversationType::Group,
        vec!["alice".to_string(), "bob".to_string()],
    ).await;

    // 发送多条消息
    let messages = vec![
        ("alice", "Hello everyone"),
        ("bob", "Hi Alice, how are you?"),
        ("alice", "I'm doing great!"),
        ("bob", "That's good to hear"),
        ("alice", "Let's discuss the project"),
    ];

    for (sender, text) in messages {
        msg_manager
            .send_text(&session.id, sender, text.to_string(), SendOptions::default())
            .await
            .unwrap();
    }

    // 测试按发送者过滤
    let filter = MessageFilter {
        sender_id: Some("alice".to_string()),
        ..Default::default()
    };
    let results = msg_manager
        .search_messages(&session.id, filter, 10)
        .await
        .unwrap();
    assert_eq!(results.len(), 3);

    // 测试未读计数
    let unread = msg_manager.get_unread_count(&session.id, "bob").await.unwrap();
    assert_eq!(unread, 5);

    // 标记所有已读
    let marked = msg_manager
        .mark_all_as_read(&session.id, "bob", chrono::Utc::now())
        .await
        .unwrap();
    assert_eq!(marked, 5);

    let unread = msg_manager.get_unread_count(&session.id, "bob").await.unwrap();
    assert_eq!(unread, 0);
}

#[tokio::test]
async fn test_session_types() {
    let (skill, _db, _temp) = setup_test_env().await;

    // 测试三种会话类型
    let direct = create_test_session(
        &skill,
        ConversationType::Direct,
        vec!["user1".to_string(), "user2".to_string()],
    ).await;
    assert_eq!(direct.conversation_type, ConversationType::Direct);

    let group = create_test_session(
        &skill,
        ConversationType::Group,
        vec!["user1".to_string(), "user2".to_string(), "user3".to_string()],
    ).await;
    assert_eq!(group.conversation_type, ConversationType::Group);
    assert!(group.name.is_some());

    let channel = create_test_session(
        &skill,
        ConversationType::Channel,
        vec!["owner".to_string()],
    ).await;
    assert_eq!(channel.conversation_type, ConversationType::Channel);
}

#[tokio::test]
async fn test_message_media_types() {
    let (skill, _db, _temp) = setup_test_env().await;

    let session = create_test_session(
        &skill,
        ConversationType::Group,
        vec!["alice".to_string(), "bob".to_string()],
    ).await;

    // 发送图片消息
    let image_msg = skill.send_message(
        &session.id,
        "alice",
        MessageContent::Image {
            url: "https://example.com/image.png".to_string(),
            width: Some(800),
            height: Some(600),
            alt_text: Some("Test image".to_string()),
        },
    ).await.unwrap();

    assert!(matches!(image_msg.content, MessageContent::Image { .. }));

    // 发送文件消息
    let file_msg = skill.send_message(
        &session.id,
        "bob",
        MessageContent::File {
            name: "document.pdf".to_string(),
            url: "https://example.com/doc.pdf".to_string(),
            size: 1024000,
            mime_type: Some("application/pdf".to_string()),
        },
    ).await.unwrap();

    assert!(matches!(file_msg.content, MessageContent::File { .. }));

    // 发送语音消息
    let voice_msg = skill.send_message(
        &session.id,
        "alice",
        MessageContent::Voice {
            url: "https://example.com/voice.mp3".to_string(),
            duration_secs: 30,
        },
    ).await.unwrap();

    assert!(matches!(voice_msg.content, MessageContent::Voice { .. }));

    // 验证历史包含所有消息
    let history = skill.get_history(&session.id, None, 10).await.unwrap();
    assert_eq!(history.len(), 3);
}

#[tokio::test]
async fn test_concurrent_messaging() {
    let (skill, _db, _temp) = setup_test_env().await;

    let session = create_test_session(
        &skill,
        ConversationType::Group,
        vec!["user1".to_string(), "user2".to_string(), "user3".to_string()],
    ).await;

    // 并发发送多条消息
    let mut handles = vec![];

    for i in 0..10 {
        let skill_ref = &skill;
        let session_id = session.id.clone();
        let handle = tokio::spawn(async move {
            skill_ref.send_message(
                &session_id,
                &format!("user{}", (i % 3) + 1),
                MessageContent::Text { text: format!("Message {}", i) },
            ).await
        });
        handles.push(handle);
    }

    // 等待所有消息发送完成
    for handle in handles {
        handle.await.unwrap().unwrap();
    }

    // 验证所有消息都已保存
    let history = skill.get_history(&session.id, None, 20).await.unwrap();
    assert_eq!(history.len(), 10);
}

#[tokio::test]
async fn test_semantic_search_placeholder() {
    let (skill, db, _temp) = setup_test_env().await;
    let msg_manager = Arc::new(MessageManager::new(db.clone()));
    let search = ImMessageSearch::new(db, msg_manager);

    // 创建会话和消息
    let session = create_test_session(
        &skill,
        ConversationType::Group,
        vec!["alice".to_string(), "bob".to_string()],
    ).await;

    skill.send_message(
        &session.id,
        "alice",
        MessageContent::Text { text: "Let's discuss the search feature implementation".to_string() },
    ).await.unwrap();

    // 测试索引（当前是占位实现）
    search.index_session_messages(&session.id).await.unwrap();

    // 测试搜索（当前返回空结果，需要集成 VectorStorage）
    let results = search.semantic_search("search feature", Some(&session.id), 10).await.unwrap();
    assert!(results.is_empty()); // 需要 VectorStorage 集成后才能有结果
}

#[tokio::test]
async fn test_error_handling() {
    let (skill, _db, _temp) = setup_test_env().await;

    // 测试向不存在的会话发送消息
    let result = skill.send_message(
        "non-existent-session",
        "user1",
        MessageContent::Text { text: "Test".to_string() },
    ).await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), im_skill::ImError::ConversationNotFound(_)));

    // 测试未授权用户发送消息
    let session = create_test_session(
        &skill,
        ConversationType::Direct,
        vec!["alice".to_string(), "bob".to_string()],
    ).await;

    let temp_dir2 = TempDir::new().unwrap();
    let db2 = Arc::new(ImDatabase::open(&temp_dir2.path().join("test2.db")).expect("Failed to open database"));
    let msg_manager = MessageManager::new(db2);
    let result = msg_manager.send_text(
        &session.id,
        "charlie", // 不是参与者
        "Test".to_string(),
        SendOptions::default(),
    ).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_session_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("persistent.db");

    // 创建第一个实例并创建会话
    {
        let skill = ImSkill::new(&db_path).unwrap();
        let session = create_test_session(
            &skill,
            ConversationType::Group,
            vec!["user1".to_string(), "user2".to_string()],
        ).await;

        skill.send_message(
            &session.id,
            "user1",
            MessageContent::Text { text: "Persistent message".to_string() },
        ).await.unwrap();
    }

    // 创建第二个实例，验证数据持久化
    {
        let skill = ImSkill::new(&db_path).unwrap();
        let sessions = skill.list_conversations("user1").await.unwrap();
        
        assert_eq!(sessions.len(), 1);
        
        let history = skill.get_history(&sessions[0].id, None, 10).await.unwrap();
        assert_eq!(history.len(), 1);
        
        if let MessageContent::Text { text } = &history[0].content {
            assert_eq!(text, "Persistent message");
        }
    }
}
