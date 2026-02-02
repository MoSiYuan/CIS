//! 对话上下文管理测试

use cis_feishu_im::{ConversationContext, ContextConfig};
use cis_core::ai::Message;

#[tokio::test]
async fn test_add_and_get_messages() {
    let ctx = ConversationContext::default();
    let session_id = "test_session";

    // 添加消息
    ctx.add_message(session_id, Message::user("Hello")).await;
    ctx.add_message(session_id, Message::assistant("Hi there!")).await;

    // 获取历史
    let history = ctx.get_history(session_id).await;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].content, "Hello");
    assert_eq!(history[1].content, "Hi there!");
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
async fn test_multiple_sessions() {
    let ctx = ConversationContext::default();

    // 创建多个会话
    ctx.add_message("session1", Message::user("User 1")).await;
    ctx.add_message("session2", Message::user("User 2")).await;
    ctx.add_message("session1", Message::assistant("Reply 1")).await;

    let history1 = ctx.get_history("session1").await;
    let history2 = ctx.get_history("session2").await;

    assert_eq!(history1.len(), 2);
    assert_eq!(history2.len(), 1);
}

#[tokio::test]
async fn test_max_turns_limit() {
    let mut config = ContextConfig::default();
    config.max_turns = 3;
    let ctx = ConversationContext::new(config);
    let session_id = "test_session";

    // 添加超过限制的消息
    for i in 0..5 {
        ctx.add_message(session_id, Message::user(&format!("Message {}", i))).await;
    }

    // 应该只保留最后 3 条
    let history = ctx.get_history(session_id).await;
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].content, "Message 2");
    assert_eq!(history[1].content, "Message 3");
    assert_eq!(history[2].content, "Message 4");
}

#[tokio::test]
async fn test_get_stats() {
    let ctx = ConversationContext::default();

    ctx.add_message("session1", Message::user("Msg 1")).await;
    ctx.add_message("session1", Message::user("Msg 2")).await;
    ctx.add_message("session2", Message::user("Msg 3")).await;

    let stats = ctx.get_stats().await;
    assert_eq!(stats.total_sessions, 2);
    assert_eq!(stats.total_messages, 3);
}
