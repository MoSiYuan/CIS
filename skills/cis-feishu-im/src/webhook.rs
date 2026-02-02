//! Webhook 服务器模块
//!
//! 使用 axum 实现 HTTP Webhook 接收

use axum::extract::State;
use super::config::FeishuImConfig;
use super::context::ConversationContext;
use super::feishu;
use cis_core::ai::AiProvider;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Webhook 服务器
pub struct WebhookServer {
    /// 配置
    config: FeishuImConfig,

    /// 对话上下文
    context: Arc<ConversationContext>,

    /// AI Provider
    ai_provider: Arc<dyn AiProvider>,

    /// 运行状态
    running: Arc<RwLock<bool>>,

    /// axum Server（需要 opaque 类型）
    #[allow(dead_code)]
    server_handle: Option<tokio::task::JoinHandle<()>>,
}

impl WebhookServer {
    /// 创建新的 Webhook 服务器
    pub fn new(
        config: FeishuImConfig,
        context: Arc<ConversationContext>,
        ai_provider: Arc<dyn AiProvider>,
    ) -> Self {
        Self {
            config,
            context,
            ai_provider,
            running: Arc::new(RwLock::new(false)),
            server_handle: None,
        }
    }

    /// 启动 Webhook 服务器
    pub async fn start(&mut self) -> Result<(), String> {
        if *self.running.read().await {
            return Err("服务器已在运行".to_string());
        }

        let bind_address = self.config.webhook.bind_address.clone();
        let port = self.config.webhook.port;
        let path = self.config.webhook.path.clone();

        info!("启动 Webhook 服务器: {}:{}", bind_address, port);

        // 构建 axum 应用
        let app = axum::Router::new()
            .route(&path, axum::routing::post(handle_webhook))
            .layer(tower_http::trace::TraceLayer::new_for_http())
            .layer(tower_http::cors::CorsLayer::permissive())
            .with_state(WebhookState {
                config: self.config.clone(),
                context: self.context.clone(),
                ai_provider: self.ai_provider.clone(),
            });

        // 启动服务器
        let addr = format!("{}:{}", bind_address, port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| format!("绑定地址失败: {}", e))?;

        info!("Webhook 服务器监听: {}", addr);

        *self.running.write().await = true;

        // 在后台任务中运行服务器
        let running = self.running.clone();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal(running))
                .await
                .unwrap();
        });

        self.server_handle = Some(handle);

        Ok(())
    }

    /// 停止 Webhook 服务器
    pub async fn stop(&mut self) -> Result<(), String> {
        *self.running.write().await = false;

        if let Some(handle) = self.server_handle.take() {
            // 等待服务器关闭
            let _ = handle.await;
        }

        info!("Webhook 服务器已停止");
        Ok(())
    }
}

/// Webhook 处理器状态
#[derive(Clone)]
struct WebhookState {
    config: FeishuImConfig,
    context: Arc<ConversationContext>,
    ai_provider: Arc<dyn AiProvider>,
}

/// Webhook 请求处理器
async fn handle_webhook(
    State(state): State<WebhookState>,
    axum::extract::Json(payload): axum::extract::Json<Value>,
) -> axum::extract::Json<Value> {
    info!("收到 Webhook 请求: {:?}", payload);

    // 1. 验证签名（如果启用）
    if state.config.verify_signature {
        // TODO: 实现签名验证
        info!("签名验证已启用（暂未实现）");
    }

    // 2. 解析事件
    let event = match parse_feishu_event(&payload) {
        Ok(event) => event,
        Err(e) => {
            error!("解析事件失败: {}", e);
            return axum::extract::Json(json!({
                "code": -1,
                "msg": format!("解析事件失败: {}", e)
            }));
        }
    };

    // 3. 异步处理事件
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = process_event(state_clone, event).await {
            error!("处理事件失败: {}", e);
        }
    });

    // 4. 立即返回成功
    axum::extract::Json(json!({
        "code": 0,
        "msg": "success"
    }))
}

/// 解析飞书事件
fn parse_feishu_event(payload: &Value) -> Result<FeishuEventInternal, String> {
    // 飞书 Webhook 事件格式
    // {
    //   "schema": "2.0",
    //   "header": { "event_type": "im.message.receive_v1", ... },
    //   "event": { ... }
    // }

    let event_type = payload["header"]["event_type"]
        .as_str()
        .ok_or("缺少 event_type")?;

    match event_type {
        "im.message.receive_v1" => {
            // 消息接收事件
            let event_data = &payload["event"];
            let message = parse_feishu_message(event_data)?;
            Ok(FeishuEventInternal::MessageReceived(message))
        }
        "im.chat.member.added_v1" => {
            // 用户加入群组
            let event_data = &payload["event"];
            Ok(FeishuEventInternal::UserJoined)
        }
        "im.chat.member.removed_v1" => {
            // 用户离开群组
            Ok(FeishuEventInternal::UserLeft)
        }
        _ => {
            // 未知事件类型
            error!("未知事件类型: {}", event_type);
            Ok(FeishuEventInternal::Unknown)
        }
    }
}

/// 解析飞书消息
fn parse_feishu_message(data: &Value) -> Result<feishu::FeishuMessage, String> {
    // 飞书消息格式
    // {
    //   "message_id": "...",
    //   "chat_type": "p2p|group",
    //   "chat_id": "...",
    //   "sender": { "sender_id": { ... } },
    //   "message_type": "text",
    //   "content": "{\"text\":\"...\"}"
    // }

    let message_id = data["message"]["message_id"]
        .as_str()
        .ok_or("缺少 message_id")?
        .to_string();

    let chat_type = data["message"]["chat_type"]
        .as_str()
        .ok_or("缺少 chat_type")?
        .to_string();

    let chat_id = data["message"]["chat_id"]
        .as_str()
        .ok_or("缺少 chat_id")?
        .to_string();

    let sender_id = data["sender"]["sender_id"]["open_id"]
        .as_str()
        .ok_or("缺少 sender_id")?
        .to_string();

    let sender_name = data["sender"]["sender_id"]["name"]
        .as_str()
        .unwrap_or("Unknown")
        .to_string();

    let msg_type = data["message"]["message_type"]
        .as_str()
        .ok_or("缺少 message_type")?
        .to_string();

    // 解析内容（通常是 JSON 字符串）
    let content_str = data["message"]["content"]
        .as_str()
        .unwrap_or("{}");

    let content_obj: serde_json::Value = serde_json::from_str(content_str)
        .unwrap_or_else(|_| serde_json::json!({}));

    let text = content_obj["text"].as_str().map(|s| s.to_string());

    // 构建 FeishuMessage
    Ok(feishu::FeishuMessage {
        message_id,
        msg_type,
        sender: feishu::FeishuSender {
            user_id: sender_id,
            name: sender_name,
            avatar_url: None,
        },
        content: feishu::FeishuContent {
            text,
            post: None,
            card: None,
            extra: Default::default(),
        },
        chat_type: Some(chat_type),
        chat_id: Some(chat_id),
        is_at: false, // TODO: 检测 @ 提及
        timestamp: Some(chrono::Utc::now().timestamp() as u64),
        extra: Default::default(),
    })
}

/// 处理事件
async fn process_event(
    state: WebhookState,
    event: FeishuEventInternal,
) -> Result<(), Box<dyn std::error::Error>> {
    match event {
        FeishuEventInternal::MessageReceived(msg) => {
            info!("收到消息: {} - {}", msg.sender.name, msg.extract_text());

            // TODO: 调用 AI 生成回复
            // 1. 检查是否应该响应
            // 2. 获取对话历史
            // 3. 调用 AI
            // 4. 保存对话历史
            // 5. 发送回复
        }
        FeishuEventInternal::UserJoined => {
            info!("用户加入群组");
        }
        FeishuEventInternal::UserLeft => {
            info!("用户离开群组");
        }
        FeishuEventInternal::Unknown => {
            info!("未知事件类型");
        }
    }

    Ok(())
}

/// 内部事件类型
#[derive(Debug, Clone)]
enum FeishuEventInternal {
    MessageReceived(feishu::FeishuMessage),
    UserJoined,
    UserLeft,
    Unknown,
}

/// 等待关闭信号
async fn shutdown_signal(running: Arc<RwLock<bool>>) {
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("收到 Ctrl+C，准备关闭...");
        }
        _ = async {
            // 轮询运行状态
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                if !*running.read().await {
                    break;
                }
            }
        } => {
            info!("收到关闭信号...");
        }
    }

    *running.write().await = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_feishu_event() {
        let payload = json!({
            "schema": "2.0",
            "header": {
                "event_type": "im.message.receive_v1",
                "event_id": "test_event_id"
            },
            "event": {
                "message": {
                    "message_id": "om_testmsgid",
                    "chat_type": "p2p",
                    "chat_id": "oc_testchatid",
                    "message_type": "text",
                    "content": "{\"text\":\"Hello, World!\"}"
                },
                "sender": {
                    "sender_id": {
                        "open_id": "ou_testuid",
                        "name": "Test User"
                    }
                }
            }
        });

        let event = parse_feishu_event(&payload).unwrap();
        assert!(matches!(event, FeishuEventInternal::MessageReceived(_)));
    }

    #[test]
    fn test_parse_feishu_message() {
        let data = json!({
            "message": {
                "message_id": "om_testmsgid",
                "chat_type": "p2p",
                "chat_id": "oc_testchatid",
                "message_type": "text",
                "content": "{\"text\":\"Hello!\"}"
            },
            "sender": {
                "sender_id": {
                    "open_id": "ou_testuid",
                    "name": "Test User"
                }
            }
        });

        let msg = parse_feishu_message(&data).unwrap();
        assert_eq!(msg.message_id, "om_testmsgid");
        assert_eq!(msg.sender.name, "Test User");
        assert_eq!(msg.extract_text(), "Hello!");
    }
}
