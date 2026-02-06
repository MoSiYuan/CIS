//! IM Skill 事件处理器
//!
//! 处理来自 SDK 的各种事件。

use serde_json::Value;

use crate::{ImSkill, types::*};
use crate::message::SendOptions;

/// 发送消息请求
#[derive(Debug, serde::Deserialize)]
pub struct SendMessageRequest {
    pub session_id: String,
    pub content: MessageContent,
    #[serde(default)]
    pub reply_to: Option<String>,
}

/// 创建会话请求
#[derive(Debug, serde::Deserialize)]
pub struct CreateSessionRequest {
    #[serde(rename = "type")]
    pub session_type: String,
    pub title: Option<String>,
    pub participants: Vec<String>,
}

/// 获取消息请求
#[derive(Debug, serde::Deserialize)]
pub struct GetMessagesRequest {
    pub session_id: String,
    #[serde(default)]
    pub before: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    50
}

/// 搜索消息请求
#[derive(Debug, serde::Deserialize)]
pub struct SearchMessagesRequest {
    pub query: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    20
}

/// 标记已读请求
#[derive(Debug, serde::Deserialize)]
pub struct MarkReadRequest {
    pub session_id: String,
    pub message_id: Option<String>,
    #[serde(default)]
    pub mark_all_before: Option<chrono::DateTime<chrono::Utc>>,
}

/// 列出会话请求
#[derive(Debug, serde::Deserialize)]
pub struct ListSessionsRequest {
    pub user_id: String,
    #[serde(default = "default_list_limit")]
    pub limit: usize,
}

fn default_list_limit() -> usize {
    20
}

/// 处理发送消息事件
pub async fn handle_send_message(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let req: SendMessageRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::ImError::Serialization(e.to_string()))?;

    let _options = SendOptions {
        reply_to: req.reply_to.clone(),
        ..Default::default()
    };

    let message = skill.send_message(&req.session_id, "current_user", req.content).await?;

    Ok(serde_json::json!({
        "success": true,
        "message_id": message.id,
        "timestamp": message.created_at,
    }))
}

/// 处理创建会话事件
pub async fn handle_create_session(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let req: CreateSessionRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::ImError::Serialization(e.to_string()))?;

    let conversation = match req.session_type.as_str() {
        "direct" => {
            if req.participants.len() != 2 {
                return Err(crate::error::ImError::InvalidMessage(
                    "Direct session requires exactly 2 participants".to_string()
                ));
            }
            skill.create_conversation(
                ConversationType::Direct,
                req.title,
                req.participants,
            ).await?
        }
        "group" => {
            if req.participants.len() < 2 {
                return Err(crate::error::ImError::InvalidMessage(
                    "Group session requires at least 2 participants".to_string()
                ));
            }
            skill.create_conversation(
                ConversationType::Group,
                req.title,
                req.participants,
            ).await?
        }
        "channel" => {
            skill.create_conversation(
                ConversationType::Channel,
                req.title,
                req.participants,
            ).await?
        }
        _ => {
            return Err(crate::error::ImError::InvalidMessage(
                format!("Unknown session type: {}", req.session_type)
            ));
        }
    };

    Ok(serde_json::json!({
        "success": true,
        "session_id": conversation.id,
        "type": conversation.conversation_type,
        "created_at": conversation.created_at,
    }))
}

/// 处理获取消息事件
pub async fn handle_get_messages(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let req: GetMessagesRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::ImError::Serialization(e.to_string()))?;

    let messages = skill.get_history(&req.session_id, req.before, req.limit).await?;

    let messages_json: Vec<Value> = messages.iter().map(|msg| {
        serde_json::json!({
            "id": msg.id,
            "sender_id": msg.sender_id,
            "content": msg.content,
            "created_at": msg.created_at,
            "read_by": msg.read_by,
        })
    }).collect();

    Ok(serde_json::json!({
        "success": true,
        "session_id": req.session_id,
        "messages": messages_json,
        "count": messages.len(),
    }))
}

/// 处理列出会话事件
pub async fn handle_list_sessions(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let req: ListSessionsRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::ImError::Serialization(e.to_string()))?;

    let conversations = skill.list_conversations(&req.user_id).await?;

    // 限制返回数量
    let conversations: Vec<_> = conversations.into_iter().take(req.limit).collect();

    let sessions_json: Vec<Value> = conversations.iter().map(|conv| {
        serde_json::json!({
            "id": conv.id,
            "type": conv.conversation_type,
            "name": conv.name,
            "participants": conv.participants,
            "last_message_at": conv.last_message_at,
            "updated_at": conv.updated_at,
        })
    }).collect();

    Ok(serde_json::json!({
        "success": true,
        "user_id": req.user_id,
        "sessions": sessions_json,
        "count": sessions_json.len(),
    }))
}

/// 处理搜索消息事件
pub async fn handle_search_messages(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let req: SearchMessagesRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::ImError::Serialization(e.to_string()))?;

    // 如果指定了会话 ID，使用本地搜索
    if let Some(session_id) = req.session_id {
        // 获取该会话的消息
        let messages = skill.get_history(&session_id, None, 1000).await?;
        
        // 简单的文本匹配搜索
        let query_lower = req.query.to_lowercase();
        let results: Vec<&Message> = messages.iter()
            .filter(|msg| {
                if let Some(text) = msg.content.text_content() {
                    text.to_lowercase().contains(&query_lower)
                } else {
                    false
                }
            })
            .take(req.limit)
            .collect();

        let messages_json: Vec<Value> = results.iter().map(|msg| {
            serde_json::json!({
                "id": msg.id,
                "session_id": msg.conversation_id,
                "sender_id": msg.sender_id,
                "content": msg.content,
                "created_at": msg.created_at,
            })
        }).collect();

        Ok(serde_json::json!({
            "success": true,
            "query": req.query,
            "results": messages_json,
            "count": messages_json.len(),
            "search_type": "keyword",
        }))
    } else {
        // 全局搜索需要在所有会话中搜索
        Ok(serde_json::json!({
            "success": false,
            "error": "Global search requires semantic search integration",
        }))
    }
}

/// 处理标记已读事件
pub async fn handle_mark_read(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let req: MarkReadRequest = serde_json::from_value(data)
        .map_err(|e| crate::error::ImError::Serialization(e.to_string()))?;

    let result = if let Some(message_id) = req.message_id {
        // 标记单条消息已读
        skill.mark_read(&message_id, "current_user").await?;
        serde_json::json!({
            "success": true,
            "marked_count": 1,
        })
    } else if let Some(before) = req.mark_all_before {
        // 批量标记已读
        let message_manager = crate::message::MessageManager::new(skill.db().clone());
        let count = message_manager.mark_all_as_read(&req.session_id, "current_user", before).await?;
        serde_json::json!({
            "success": true,
            "marked_count": count,
            "session_id": req.session_id,
        })
    } else {
        return Err(crate::error::ImError::InvalidMessage(
            "Either message_id or mark_all_before must be provided".to_string()
        ));
    };

    Ok(result)
}

/// 处理获取会话信息事件
pub async fn handle_get_session(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let session_id = data.get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::ImError::InvalidMessage(
            "session_id is required".to_string()
        ))?;

    let conversation = skill.get_conversation(session_id).await?;

    match conversation {
        Some(conv) => Ok(serde_json::json!({
            "success": true,
            "session": {
                "id": conv.id,
                "type": conv.conversation_type,
                "name": conv.name,
                "participants": conv.participants,
                "created_at": conv.created_at,
                "updated_at": conv.updated_at,
                "last_message_at": conv.last_message_at,
            }
        })),
        None => Ok(serde_json::json!({
            "success": false,
            "error": "Session not found",
        })),
    }
}

/// 处理加入会话事件
pub async fn handle_join_session(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let session_id = data.get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::ImError::InvalidMessage(
            "session_id is required".to_string()
        ))?;
    
    let user_id = data.get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::ImError::InvalidMessage(
            "user_id is required".to_string()
        ))?;
    
    let session_manager = crate::session::SessionManager::new(skill.db().clone());
    session_manager.add_participant(session_id, user_id.to_string()).await?;
    
    Ok(serde_json::json!({
        "success": true,
        "message": "Joined session",
        "session_id": session_id,
        "user_id": user_id,
    }))
}

/// 处理离开会话事件
pub async fn handle_leave_session(
    skill: &ImSkill,
    data: Value,
) -> Result<Value, crate::error::ImError> {
    let session_id = data.get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::ImError::InvalidMessage(
            "session_id is required".to_string()
        ))?;
    
    let user_id = data.get("user_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| crate::error::ImError::InvalidMessage(
            "user_id is required".to_string()
        ))?;
    
    let session_manager = crate::session::SessionManager::new(skill.db().clone());
    session_manager.remove_participant(session_id, user_id).await?;
    
    Ok(serde_json::json!({
        "success": true,
        "message": "Left session",
        "session_id": session_id,
        "user_id": user_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::ImSkill;

    async fn setup_skill() -> (ImSkill, tempfile::TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let skill = ImSkill::new(&temp_dir.path().join("im.db")).unwrap();
        (skill, temp_dir)
    }

    #[tokio::test]
    async fn test_handle_create_session() {
        let (skill, _temp) = setup_skill().await;

        let data = serde_json::json!({
            "type": "group",
            "title": "Test Group",
            "participants": ["user1", "user2", "user3"],
        });

        let result = handle_create_session(&skill, data).await.unwrap();
        assert_eq!(result["success"], true);
        assert!(result["session_id"].is_string());
    }

    #[tokio::test]
    async fn test_handle_send_and_get_messages() {
        let (skill, _temp) = setup_skill().await;

        // 先创建会话
        let create_data = serde_json::json!({
            "type": "group",
            "title": "Test",
            "participants": ["user1", "user2"],
        });
        let create_result = handle_create_session(&skill, create_data).await.unwrap();
        let session_id = create_result["session_id"].as_str().unwrap();

        // 发送消息
        let send_data = serde_json::json!({
            "session_id": session_id,
            "content": {
                "type": "text",
                "content": {
                    "text": "Hello World"
                }
            },
        });
        let send_result = handle_send_message(&skill, send_data).await.unwrap();
        assert_eq!(send_result["success"], true);

        // 获取消息
        let get_data = serde_json::json!({
            "session_id": session_id,
            "limit": 10,
        });
        let get_result = handle_get_messages(&skill, get_data).await.unwrap();
        assert_eq!(get_result["success"], true);
        assert_eq!(get_result["count"], 1);
    }

    #[tokio::test]
    async fn test_handle_search_messages() {
        let (skill, _temp) = setup_skill().await;

        // 创建会话
        let create_data = serde_json::json!({
            "type": "group",
            "title": "Test",
            "participants": ["user1", "user2"],
        });
        let create_result = handle_create_session(&skill, create_data).await.unwrap();
        let session_id = create_result["session_id"].as_str().unwrap();

        // 发送消息
        let send_data = serde_json::json!({
            "session_id": session_id,
            "content": {
                "type": "text",
                "content": {
                    "text": "Hello World from Rust"
                }
            },
        });
        handle_send_message(&skill, send_data).await.unwrap();

        // 搜索消息
        let search_data = serde_json::json!({
            "query": "Hello",
            "session_id": session_id,
            "limit": 10,
        });
        let search_result = handle_search_messages(&skill, search_data).await.unwrap();
        assert_eq!(search_result["success"], true);
        assert_eq!(search_result["count"], 1);
    }
}
