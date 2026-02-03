//! 消息轮询器
//!
//! 实现从飞书服务器主动拉取消息的轮询机制
//!
//! ## 设计原则
//!
//! - **随时关机友好**: 关机即离线，上线即恢复
//! - **无公网暴露**: 从本地主动连接飞书 API
//! - **冷冻模式**: 离线期间消息直接丢弃
//! - **自动重连**: 指数退避重试策略

use crate::{
    config::FeishuImConfig,
    context::ConversationContext,
    error::FeishuImError,
    feishu_api::{FeishuApiClient, FeishuMessage},
    session::{FeishuSessionManager, FeishuSessionType},
};
use cis_core::ai::{AiProvider, Message as AiMessage};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// 会话状态
#[derive(Debug, Clone)]
struct ConversationState {
    /// 会话 ID
    chat_id: String,
    /// 最后检查时间（Unix 毫秒时间戳）
    last_check_time: i64,
    /// 是否活跃（最近有消息）
    active: bool,
    /// 最后一条消息的创建时间
    last_message_time: Option<i64>,
}

/// 消息轮询器
pub struct MessagePoller {
    /// 配置
    config: FeishuImConfig,
    /// 飞书 API 客户端
    api_client: FeishuApiClient,
    /// 对话上下文
    context: Arc<ConversationContext>,
    /// AI Provider
    ai_provider: Arc<dyn AiProvider>,
    /// 会话管理器
    session_manager: Arc<FeishuSessionManager>,
    /// 会话状态追踪
    conversations: Arc<RwLock<HashMap<String, ConversationState>>>,
    /// 运行状态
    running: Arc<RwLock<bool>>,
    /// 节点名称（用于状态广播）
    node_name: String,
}

impl MessagePoller {
    /// 创建新的轮询器
    pub fn new(
        config: FeishuImConfig,
        context: Arc<ConversationContext>,
        ai_provider: Arc<dyn AiProvider>,
    ) -> Self {
        let node_name = std::env::var("CIS_NODE_NAME")
            .unwrap_or_else(|_| "CIS-Node".to_string());

        let api_client = FeishuApiClient::new(
            config.app_id.clone(),
            config.app_secret.clone(),
        );

        // 创建会话管理器
        let session_manager = Arc::new(FeishuSessionManager::new(
            config.im_db_path.clone(),
            context.clone(),
        ));

        Self {
            config,
            api_client,
            context,
            ai_provider,
            session_manager,
            conversations: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            node_name,
        }
    }

    /// 获取会话管理器
    pub fn session_manager(&self) -> Arc<FeishuSessionManager> {
        self.session_manager.clone()
    }

    /// 启动轮询
    pub async fn start(&mut self) -> Result<(), FeishuImError> {
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(FeishuImError::Polling("轮询器已在运行".to_string()));
            }
            *running = true;
        }

        info!("🚀 消息轮询器启动: {}", self.node_name);
        info!("   模式: 冷冻模式（离线消息丢弃）");
        info!("   策略: 主动拉取 + 自动重连");

        // 发送上线广播
        self.broadcast_online().await;

        // 启动轮询任务
        let conversations = self.conversations.clone();
        let running = self.running.clone();
        let api_client = self.api_client.clone(); // 需要实现 Clone
        let config = self.config.clone();
        let context = self.context.clone();
        let ai_provider = self.ai_provider.clone();
        let session_manager = self.session_manager.clone();
        let node_name = self.node_name.clone();

        tokio::spawn(async move {
            let mut error_count = 0u32;
            let mut last_error_time = SystemTime::now();

            while *running.read().await {
                // 指数退避重试
                if error_count > 0 {
                    let backoff = Duration::from_secs(2u64.pow(error_count.min(6)) as u64);
                    debug!("错误退避: 等待 {:?} (连续错误: {})", backoff, error_count);
                    tokio::time::sleep(backoff).await;
                }

                // 执行轮询
                match Self::poll_once(
                    &api_client,
                    &config,
                    &context,
                    &ai_provider,
                    &session_manager,
                    &node_name,
                    conversations.clone(),
                ).await {
                    Ok(_) => {
                        // 成功，重置错误计数
                        error_count = 0;
                    }
                    Err(e) => {
                        error_count += 1;
                        let now = SystemTime::now();

                        // 只在首次错误和每分钟记录一次
                        if error_count == 1 || now.duration_since(last_error_time).unwrap_or(Duration::ZERO) > Duration::from_secs(60) {
                            warn!("轮询错误 (连续 {} 次): {}", error_count, e);
                            last_error_time = now;
                        }

                        // 检查是否需要刷新 token
                        if e.to_string().contains("Token") || e.to_string().contains("Auth") {
                            if let Err(token_err) = api_client.refresh_token().await {
                                error!("刷新 Token 失败: {}", token_err);
                            }
                        }
                    }
                }

                // 等待下次轮询
                let interval = Duration::from_secs(config.polling.http_interval);
                tokio::time::sleep(interval).await;
            }

            info!("消息轮询器已停止");
        });

        Ok(())
    }

    /// 停止轮询
    pub async fn stop(&mut self) -> Result<(), FeishuImError> {
        // 发送离线广播
        self.broadcast_offline().await;

        let mut running = self.running.write().await;
        *running = false;

        info!("消息轮询器已停止: {}", self.node_name);
        Ok(())
    }

    /// 单次轮询
    async fn poll_once(
        api_client: &FeishuApiClient,
        config: &FeishuImConfig,
        context: &Arc<ConversationContext>,
        ai_provider: &Arc<dyn AiProvider>,
        session_manager: &Arc<FeishuSessionManager>,
        node_name: &str,
        conversations: Arc<RwLock<HashMap<String, ConversationState>>>,
    ) -> Result<(), FeishuImError> {
        // 1. 获取会话列表（定期刷新）
        let should_refresh = {
            let convs = conversations.read().await;
            convs.is_empty() || {
                // 每隔一段时间刷新会话列表
                true // 简化版：每次都检查
            }
        };

        if should_refresh {
            Self::refresh_conversations(api_client, conversations.clone()).await?;
        }

        // 2. 轮询每个会话的新消息
        let convs = conversations.read().await;
        let chat_ids: Vec<String> = convs.keys().cloned().collect();
        drop(convs);

        for chat_id in chat_ids {
            if let Err(e) = Self::poll_conversation(
                api_client,
                config,
                context,
                ai_provider,
                session_manager,
                node_name,
                conversations.clone(),
                &chat_id,
            ).await {
                warn!("轮询会话 {} 失败: {}", chat_id, e);
            }
        }

        Ok(())
    }

    /// 刷新会话列表
    async fn refresh_conversations(
        api_client: &FeishuApiClient,
        conversations: Arc<RwLock<HashMap<String, ConversationState>>>,
    ) -> Result<(), FeishuImError> {
        debug!("刷新会话列表...");

        let api_conversations = api_client.list_conversations().await
            .map_err(|e| FeishuImError::FeishuApi(e.to_string()))?;

        let mut convs = conversations.write().await;

        for api_conv in api_conversations {
            // 只保留群聊和私聊
            if api_conv.chat_type != "p2p" && api_conv.chat_type != "group" {
                continue;
            }

            // 如果会话不存在，创建新状态
            if !convs.contains_key(&api_conv.chat_id) {
                info!("发现新会话: {} ({})", api_conv.name, api_conv.chat_id);

                convs.insert(api_conv.chat_id.clone(), ConversationState {
                    chat_id: api_conv.chat_id.clone(),
                    last_check_time: 0, // 0 表示从头开始或丢弃历史
                    active: false,
                    last_message_time: None,
                });
            }
        }

        debug!("会话列表刷新完成: {} 个会话", convs.len());
        Ok(())
    }

    /// 轮询单个会话
    async fn poll_conversation(
        api_client: &FeishuApiClient,
        config: &FeishuImConfig,
        context: &Arc<ConversationContext>,
        ai_provider: &Arc<dyn AiProvider>,
        session_manager: &Arc<FeishuSessionManager>,
        _node_name: &str,
        conversations: Arc<RwLock<HashMap<String, ConversationState>>>,
        chat_id: &str,
    ) -> Result<(), FeishuImError> {
        // 获取会话状态
        let (last_check_time, _should_process) = {
            let convs = conversations.read().await;
            let state = convs.get(chat_id);

            match state {
                Some(state) => (state.last_check_time, state.active),
                None => return Ok(()), // 会话不存在，跳过
            }
        };

        // 拉取新消息
        let start_time = if last_check_time == 0 {
            // 冷冻模式：从头开始，但跳过历史消息
            // 使用当前时间作为起点，只处理新消息
            Some(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64
            )
        } else {
            Some(last_check_time)
        };

        let messages = api_client
            .list_messages(chat_id, start_time, config.polling.batch_size)
            .await
            .map_err(|e| FeishuImError::FeishuApi(e.to_string()))?;

        if messages.is_empty() {
            return Ok(());
        }

        // 获取或创建会话
        let session_type = if chat_id.starts_with("oc_") {
            FeishuSessionType::Group
        } else {
            FeishuSessionType::Private
        };

        let session = session_manager.get_or_create_session(
            chat_id,
            &format!("会话 {}", chat_id),
            session_type,
        ).await;

        // 更新活跃时间和消息计数
        session_manager.update_activity(chat_id).await;
        session_manager.increment_message_count(chat_id).await;

        info!("会话 {} ({}) 收到 {} 条新消息", chat_id, session.name, messages.len());

        // 处理消息
        for msg in messages {
            // 跳过自己发送的消息
            if msg.sender.sender_type == "app" {
                continue;
            }

            // 检查触发模式
            if !Self::should_trigger(config, &msg, chat_id).await {
                continue;
            }

            // 提取消息内容
            let user_message = Self::extract_message_text(&msg);

            // 更新上下文
            context.add_message(chat_id, AiMessage::user(&user_message)).await;

            // 生成 AI 回复
            let history = context.get_history(chat_id).await;
            let response = ai_provider
                .chat_with_context("你是一个有用的AI助手。", &history)
                .await
                .map_err(|e| FeishuImError::Ai(e.to_string()))?;

            let reply = response.trim();

            // 发送回复
            let receive_id_type = if chat_id.starts_with("oc_") {
                "chat"
            } else {
                "open_id"
            };

            api_client
                .send_text_message(chat_id, receive_id_type, reply)
                .await
                .map_err(|e| FeishuImError::FeishuApi(e.to_string()))?;

            // 更新上下文
            context.add_message(chat_id, AiMessage::assistant(reply)).await;

            info!("✅ 已回复 {}: {}", chat_id, reply.chars().take(50).collect::<String>());
        }

        // 更新会话状态
        {
            let mut convs = conversations.write().await;
            if let Some(state) = convs.get_mut(chat_id) {
                // 更新为当前时间
                state.last_check_time = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as i64;
                state.active = true;
            }
        }

        Ok(())
    }

    /// 检查是否应该触发
    async fn should_trigger(
        config: &FeishuImConfig,
        _msg: &FeishuMessage,
        _chat_id: &str,
    ) -> bool {
        match config.trigger_mode {
            crate::config::TriggerMode::AtMentionOnly => {
                // 仅 @ 机器人时响应（TODO: 需要解析消息内容）
                false
            }
            crate::config::TriggerMode::PrivateAndAtMention => {
                // 私聊自动响应 + @机器人
                // 判断是否为私聊（chat_id 以 oc_ 开头是群聊，ou_ 开头可能是私聊）
                true // 简化版：全部响应
            }
            crate::config::TriggerMode::All => {
                // 所有消息都响应
                true
            }
        }
    }

    /// 提取消息文本
    fn extract_message_text(msg: &FeishuMessage) -> String {
        match msg.msg_type.as_str() {
            "text" => {
                // 解析 JSON 格式的文本内容
                if let Ok(content) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    content["text"]
                        .as_str()
                        .unwrap_or("")
                        .to_string()
                } else {
                    msg.content.clone()
                }
            }
            "post" => {
                // 富文本内容
                if let Ok(content) = serde_json::from_str::<serde_json::Value>(&msg.content) {
                    content["post"]
                        .as_str()
                        .unwrap_or("[富文本消息]")
                        .to_string()
                } else {
                    "[富文本消息]".to_string()
                }
            }
            _ => format!("[{} 类型的消息]", msg.msg_type),
        }
    }

    /// 发送上线广播
    async fn broadcast_online(&self) {
        info!("📢 节点上线广播: {}", self.node_name);

        // TODO: 向"节点监控群"发送上线消息
        // 需要配置一个专门的监控群 chat_id
    }

    /// 发送离线广播
    async fn broadcast_offline(&self) {
        info!("📢 节点离线广播: {}", self.node_name);

        // TODO: 向"节点监控群"发送离线消息
    }

    /// 检查运行状态
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// 轮询配置
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PollingConfig {
    /// HTTP 轮询间隔（秒）
    pub http_interval: u64,

    /// 批量拉取消息数量
    pub batch_size: u32,

    /// 是否处理历史消息
    pub process_history: bool,

    /// 会话检查间隔（秒）
    pub conversation_check_interval: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            http_interval: 10,    // 10秒轮询一次
            batch_size: 20,       // 每次拉取20条
            process_history: false, // 不处理历史消息（冷冻模式）
            conversation_check_interval: 60, // 60秒检查一次新会话
        }
    }
}
