//! IM Skill Matrix 适配器
//!
//! 将 IM Skill 与 Matrix Room 集成

use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::{ImSkill, ImConfig, types::*};

/// Matrix 消息结构
#[derive(Debug, Clone)]
pub struct MatrixMessage {
    pub event_id: String,
    pub sender: String,
    pub room_id: String,
    pub body: String,
    pub msgtype: String,
    pub timestamp: i64,
}

impl MatrixMessage {
    /// 从 MatrixEvent 解析
    pub fn from_cis_core_event(event: &cis_core::matrix::nucleus::MatrixEvent) -> Option<Self> {
        if event.event_type != "m.room.message" {
            return None;
        }
        
        let content = event.content.get("content")?;
        let msgtype = content.get("msgtype")?.as_str()?.to_string();
        let body = content.get("body")?.as_str()?.to_string();
        
        Some(Self {
            event_id: event.event_id.to_string(),
            sender: event.sender.to_string(),
            room_id: event.room_id.to_string(),
            body,
            msgtype,
            timestamp: event.timestamp,
        })
    }
}

/// IM Skill Matrix 适配器
/// 
/// 包装 ImSkill 并实现 CIS Core Skill trait
pub struct ImMatrixAdapter {
    inner: Arc<ImSkill>,
}

impl ImMatrixAdapter {
    /// 创建新的适配器
    pub fn new(db_path: &Path) -> anyhow::Result<Self> {
        let inner = ImSkill::new(db_path)?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
    
    /// 使用自定义配置创建
    pub fn with_config(db_path: &Path, config: ImConfig) -> anyhow::Result<Self> {
        let inner = ImSkill::new(db_path)?.with_config(config);
        Ok(Self {
            inner: Arc::new(inner),
        })
    }
    
    /// 获取内部 IM Skill
    pub fn inner(&self) -> &Arc<ImSkill> {
        &self.inner
    }
    
    /// 处理 Matrix 消息
    async fn handle_matrix_message(&self, msg: MatrixMessage) -> anyhow::Result<()> {
        info!("Processing Matrix message from {} in room {}", msg.sender, msg.room_id);
        
        // 确保会话存在（如果不存在则创建）
        let conversation = match self.inner.get_conversation(&msg.room_id).await? {
            Some(conv) => conv,
            None => {
                // 创建新的会话，映射 Matrix Room 到 Conversation
                self.inner.create_conversation(
                    ConversationType::Group, // Matrix rooms 默认作为群组
                    Some(format!("Room {}", msg.room_id)),
                    vec![msg.sender.clone()],
                ).await?
            }
        };
        
        // 根据消息类型处理
        let content = match msg.msgtype.as_str() {
            "m.text" => {
                MessageContent::Text { text: msg.body }
            }
            "m.image" => {
                // 简化处理，实际应该解析 mxc URL
                MessageContent::Image { 
                    url: format!("mxc://{}/image", msg.room_id),
                    width: None,
                    height: None,
                    alt_text: Some(msg.body),
                }
            }
            "m.file" => {
                MessageContent::File {
                    name: msg.body,
                    url: format!("mxc://{}/file", msg.room_id),
                    size: 0,
                    mime_type: None,
                }
            }
            "m.audio" | "m.voice" => {
                MessageContent::Voice {
                    url: format!("mxc://{}/voice", msg.room_id),
                    duration_secs: 0,
                }
            }
            _ => {
                // 不支持的消息类型，转为文本
                MessageContent::Text { 
                    text: format!("[Unsupported message type: {}] {}", msg.msgtype, msg.body) 
                }
            }
        };
        
        // 发送消息
        self.inner.send_message(&conversation.id, &msg.sender, content).await?;
        
        debug!("Message saved successfully");
        Ok(())
    }
}

impl Default for ImMatrixAdapter {
    fn default() -> Self {
        Self {
            inner: Arc::new(ImSkill::default()),
        }
    }
}

// ==================== WASM 模式实现 ====================

#[cfg(feature = "wasm")]
impl cis_skill_sdk::Skill for ImMatrixAdapter {
    fn name(&self) -> &str {
        crate::SKILL_NAME
    }

    fn version(&self) -> &str {
        crate::SKILL_VERSION
    }

    fn handle_event(
        &self,
        _ctx: &dyn cis_skill_sdk::SkillContext,
        event: cis_skill_sdk::types::Event,
    ) -> cis_skill_sdk::error::Result<()> {
        match event {
            cis_skill_sdk::types::Event::Init { config } => {
                info!("IM Skill (WASM) initialized with config: {:?}", config);
            }
            cis_skill_sdk::types::Event::Custom { name, data } => {
                if name == "matrix_message" {
                    info!("Received Matrix message: {:?}", data);
                }
            }
            _ => {}
        }
        Ok(())
    }
}

// ==================== Native 模式实现 ====================

#[cfg(feature = "native")]
#[async_trait]
impl cis_skill_sdk::skill::NativeSkill for ImMatrixAdapter {
    fn name(&self) -> &str {
        crate::SKILL_NAME
    }

    fn version(&self) -> &str {
        crate::SKILL_VERSION
    }

    async fn init(&mut self, config: cis_skill_sdk::types::SkillConfig) -> cis_skill_sdk::error::Result<()> {
        info!("IM Skill (Native) initialized");
        if let Some(cfg) = config.get::<serde_json::Value>("config") {
            debug!("Config: {:?}", cfg);
        }
        Ok(())
    }

    async fn handle_event(
        &self,
        ctx: &dyn cis_skill_sdk::SkillContext,
        event: cis_skill_sdk::types::Event,
    ) -> cis_skill_sdk::error::Result<()> {
        match event {
            cis_skill_sdk::types::Event::Init { config } => {
                ctx.log_info(&format!("IM Skill initialized: {:?}", config));
            }
            cis_skill_sdk::types::Event::Custom { name, data } => {
                match name.as_str() {
                    "send_message" => {
                        ctx.log_info(&format!("Sending IM message: {:?}", data));
                    }
                    "get_history" => {
                        ctx.log_info(&format!("Getting message history: {:?}", data));
                    }
                    "create_conversation" => {
                        ctx.log_info(&format!("Creating conversation: {:?}", data));
                    }
                    _ => {
                        ctx.log_debug(&format!("Unknown custom event: {}", name));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// CIS Core Skill trait 实现（用于 CIS Core 内部集成）
#[cfg(feature = "native")]
#[async_trait]
impl cis_core::skill::Skill for ImMatrixAdapter {
    fn name(&self) -> &str {
        crate::SKILL_NAME
    }

    fn version(&self) -> &str {
        crate::SKILL_VERSION
    }

    fn description(&self) -> &str {
        crate::SKILL_DESCRIPTION
    }

    /// Skill 对应的 Matrix Room ID
    fn room_id(&self) -> Option<String> {
        Some(crate::SKILL_ROOM_ID.to_string())
    }

    /// 是否联邦同步
    /// 
    /// IM 消息需要联邦同步以支持跨节点通信
    fn federate(&self) -> bool {
        crate::SKILL_FEDERATE
    }

    /// 初始化
    async fn init(&mut self, config: cis_core::skill::SkillConfig) -> cis_core::error::Result<()> {
        info!("IM Skill initializing with config: {:?}", config);
        Ok(())
    }

    /// 处理 Matrix Event
    /// 
    /// 将 Matrix 消息转换为 IM 消息处理
    async fn on_matrix_event(
        &self,
        event: cis_core::matrix::nucleus::MatrixEvent,
    ) -> cis_core::error::Result<()> {
        debug!("Received Matrix event: {} in room {}", event.event_type, event.room_id);
        
        if let Some(msg) = MatrixMessage::from_cis_core_event(&event) {
            if let Err(e) = self.handle_matrix_message(msg).await {
                error!("Failed to handle Matrix message: {}", e);
                return Err(cis_core::error::CisError::skill(format!("IM error: {}", e)));
            }
        }
        
        Ok(())
    }

    /// 处理 CIS 事件
    async fn handle_event(
        &self,
        ctx: &dyn cis_core::skill::SkillContext,
        event: cis_core::skill::Event,
    ) -> cis_core::error::Result<()> {
        match event {
            cis_core::skill::Event::Init { config } => {
                ctx.log_info(&format!("IM Skill initialized: {:?}", config));
            }
            cis_core::skill::Event::Custom { name, data } => {
                match name.as_str() {
                    "send_message" => {
                        ctx.log_info(&format!("Sending IM message: {:?}", data));
                    }
                    "mark_read" => {
                        ctx.log_info(&format!("Marking messages as read: {:?}", data));
                    }
                    _ => {
                        ctx.log_debug(&format!("Unknown event: {}", name));
                    }
                }
            }
            cis_core::skill::Event::Tick => {
                // 定时任务，可以清理过期消息等
                debug!("IM Skill tick");
            }
            _ => {}
        }
        Ok(())
    }

    /// 关闭
    async fn shutdown(&self) -> cis_core::error::Result<()> {
        info!("IM Skill shutting down");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[cfg(feature = "native")]
    use cis_core::skill::Skill;

    #[test]
    fn test_matrix_message_parsing() {
        // 注意：这里需要 cis_core::matrix::nucleus::MatrixEvent 来测试
        // 实际测试需要在集成环境中进行
    }

    #[tokio::test]
    #[cfg(feature = "native")]
    async fn test_adapter_creation() {
        let temp_dir = TempDir::new().unwrap();
        let adapter = ImMatrixAdapter::new(&temp_dir.path().join("im.db")).unwrap();
        
        // 验证基本属性
        assert_eq!(adapter.name(), "im");
        assert_eq!(adapter.version(), "0.1.0");
        assert_eq!(adapter.room_id(), Some("!im:cis.local".to_string()));
        assert!(adapter.federate());
    }
}
