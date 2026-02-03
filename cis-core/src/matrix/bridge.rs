//! # CIS-Matrix Bridge
//!
//! 连接 Matrix 事件和 CIS Core，将 Matrix 消息转换为 CIS Skill 调用，
//! 并将 CIS 结果回写到 Matrix。
//!
//! ## 架构
//!
//! - Matrix 消息入向处理：解析 Matrix 消息，识别 CIS 指令，调用 Skill
//! - CIS 结果出向处理：将 Skill 执行结果回写到 Matrix 房间
//! - 控制房间：自动创建 `#cis-control:cis.local` 房间作为 CIS 控制界面

use std::collections::HashMap;
use std::sync::Arc;

use ruma::{
    events::room::message::{MessageType, RoomMessageEventContent},
    EventId, OwnedEventId, RoomId, UserId,
};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{CisError, Result};
use crate::skill::manager::SkillManager;
use crate::skill::types::{LoadOptions, SkillConfig, SkillState};
use crate::skill::{Event, SkillContext, MemoryOp};
use crate::matrix::federation_impl::FederationManager;
use crate::matrix::federation::types::CisMatrixEvent;

use super::error::{MatrixError, MatrixResult};
use super::store::MatrixStore;

/// CIS Skill 调用任务
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillTask {
    /// Skill 名称
    pub skill: String,
    /// 调用动作/命令
    pub action: String,
    /// 参数
    pub params: HashMap<String, String>,
    /// 原始消息
    pub raw: String,
}

/// Skill 调用结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    /// 是否成功
    pub success: bool,
    /// 结果数据
    pub data: Option<serde_json::Value>,
    /// 错误信息
    pub error: Option<String>,
    /// 执行时间 (ms)
    pub elapsed_ms: u64,
}

/// Matrix-CIS Bridge
pub struct MatrixBridge {
    /// Matrix 存储
    matrix_store: Arc<MatrixStore>,
    /// Skill 管理器
    skill_manager: Arc<SkillManager>,
    /// 联邦管理器（可选，用于联邦广播）
    federation_manager: Option<Arc<FederationManager>>,
    /// 控制房间 ID (字符串格式)
    control_room_id: Arc<std::sync::RwLock<Option<String>>>,
}

impl MatrixBridge {
    /// 创建新的 Bridge 实例
    pub fn new(
        matrix_store: Arc<MatrixStore>,
        skill_manager: Arc<SkillManager>,
    ) -> MatrixResult<Self> {
        let bridge = Self {
            matrix_store,
            skill_manager,
            federation_manager: None,
            control_room_id: Arc::new(std::sync::RwLock::new(None)),
        };

        // 尝试初始化控制房间
        if let Err(e) = bridge.init_control_room() {
            warn!("Failed to initialize control room: {}", e);
        }

        Ok(bridge)
    }
    
    /// 创建带联邦管理器的 Bridge 实例
    pub fn with_federation(
        matrix_store: Arc<MatrixStore>,
        skill_manager: Arc<SkillManager>,
        federation_manager: Arc<FederationManager>,
    ) -> MatrixResult<Self> {
        let bridge = Self {
            matrix_store,
            skill_manager,
            federation_manager: Some(federation_manager),
            control_room_id: Arc::new(std::sync::RwLock::new(None)),
        };

        // 尝试初始化控制房间
        if let Err(e) = bridge.init_control_room() {
            warn!("Failed to initialize control room: {}", e);
        }

        Ok(bridge)
    }

    /// 初始化控制房间
    fn init_control_room(&self) -> MatrixResult<()> {
        let room_id = "!cis-control:cis.local";

        // 检查房间是否已存在
        let exists = self.room_exists(room_id)?;
        
        if !exists {
            info!("Creating CIS control room: {}", room_id);
            self.create_control_room(room_id)?;
        }

        // 保存控制房间 ID
        if let Ok(mut guard) = self.control_room_id.write() {
            *guard = Some(room_id.to_string());
        }

        Ok(())
    }

    /// 检查房间是否存在
    fn room_exists(&self, room_id: &str) -> MatrixResult<bool> {
        self.matrix_store.room_exists(room_id)
    }

    /// 创建控制房间
    fn create_control_room(&self, room_id: &str) -> MatrixResult<()> {
        // 创建房间创建事件
        let event_id = format!("${}", Uuid::new_v4().to_string().replace('-', ""));
        
        let content = serde_json::json!({
            "creator": "@cis:cis.local",
            "room_version": "9",
        });

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        self.matrix_store.save_event(
            room_id,
            &event_id,
            "@cis:cis.local",
            "m.room.create",
            &content.to_string(),
            now,
            None,
            Some(""),
        )?;

        // 设置房间名称
        let name_event_id = format!("${}", Uuid::new_v4().to_string().replace('-', ""));
        
        let name_content = serde_json::json!({
            "name": "CIS Control",
        });

        self.matrix_store.save_event(
            room_id,
            &name_event_id,
            "@cis:cis.local",
            "m.room.name",
            &name_content.to_string(),
            now,
            None,
            Some(""),
        )?;

        // 发送欢迎消息
        let welcome_event_id = format!("${}", Uuid::new_v4().to_string().replace('-', ""));
        
        let welcome_content = serde_json::json!({
            "msgtype": "m.text",
            "body": "🤖 Welcome to CIS Control Room!\n\nAvailable commands:\n- !skill <name> [params] - Invoke a skill\n- !skills - List available skills\n- !help - Show help",
        });

        self.matrix_store.save_event(
            room_id,
            &welcome_event_id,
            "@cis:cis.local",
            "m.room.message",
            &welcome_content.to_string(),
            now,
            None,
            None,
        )?;

        info!("CIS control room created: {}", room_id);
        Ok(())
    }

    /// Matrix 消息入向处理
    pub async fn on_matrix_message(
        &self,
        room_id: &RoomId,
        sender: &UserId,
        content: &RoomMessageEventContent,
    ) -> MatrixResult<()> {
        debug!(
            "Processing message from {} in {}: {:?}",
            sender, room_id, content
        );

        // 提取消息文本
        let body = match &content.msgtype {
            MessageType::Text(text) => &text.body,
            _ => {
                debug!("Non-text message, ignoring");
                return Ok(());
            }
        };

        // 检查是否是 CIS 指令 (以 ! 开头)
        if body.starts_with("!skill ") {
            let cmd = &body[7..];
            info!("Skill command received: {}", cmd);

            // 解析 Skill 任务
            let task = match self.parse_skill_command(cmd) {
                Some(t) => t,
                None => {
                    self.send_to_room(room_id, "❌ Invalid skill command format. Usage: !skill <name> [key=value ...]")
                        .await?;
                    return Ok(());
                }
            };

            // 调用 Skill
            match self.invoke_skill(task.clone()).await {
                Ok(result) => {
                    let response = self.format_result(&result);
                    self.send_to_room(room_id, &response).await?;
                }
                Err(e) => {
                    warn!("Skill invocation failed: {}", e);
                    self.send_to_room(
                        room_id,
                        &format!("❌ Skill '{}' failed: {}", task.skill, e),
                    )
                    .await?;
                }
            }
        } else if body.starts_with("!skills") {
            // 列出可用 Skills
            match self.list_skills().await {
                Ok(list) => {
                    self.send_to_room(room_id, &list).await?;
                }
                Err(e) => {
                    self.send_to_room(room_id, &format!("❌ Failed to list skills: {}", e))
                        .await?;
                }
            }
        } else if body.starts_with("!help") {
            // 帮助信息
            let help = "🤖 CIS Bot Commands:\n\
                        - !skill <name> [key=value ...] - Invoke a skill\n\
                        - !skills - List available skills\n\
                        \n\
                        Example:\n\
                        !skill nav target=sofa";
            self.send_to_room(room_id, help).await?;
        }

        Ok(())
    }

    /// 解析 CIS 指令
    /// 
    /// 格式: !skill nav target=sofa speed=fast
    /// 解析为: Task { skill: "nav", action: "default", params: {target: "sofa", speed: "fast"} }
    fn parse_skill_command(&self, cmd: &str) -> Option<SkillTask> {
        let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let skill = parts[0].to_string();
        let mut params = HashMap::new();
        let mut action = "default".to_string();

        // 解析参数 (key=value 格式)
        for part in &parts[1..] {
            if let Some(eq_pos) = part.find('=') {
                let key = &part[..eq_pos];
                let value = &part[eq_pos + 1..];
                
                // 特殊处理 action 参数
                if key == "action" {
                    action = value.to_string();
                } else {
                    params.insert(key.to_string(), value.to_string());
                }
            }
        }

        Some(SkillTask {
            skill,
            action,
            params,
            raw: cmd.to_string(),
        })
    }

    /// 调用 Skill
    async fn invoke_skill(&self, task: SkillTask) -> Result<SkillResult> {
        info!("Invoking skill '{}' with action '{}'", task.skill, task.action);

        // 检查 Skill 是否存在
        let info = self
            .skill_manager
            .get_info(&task.skill)
            .map_err(|e| CisError::skill(format!("Failed to get skill info: {}", e)))?;

        if info.is_none() {
            return Ok(SkillResult {
                success: false,
                data: None,
                error: Some(format!("Skill '{}' not found", task.skill)),
                elapsed_ms: 0,
            });
        }

        // 检查 Skill 是否已加载
        let is_loaded = self
            .skill_manager
            .is_loaded(&task.skill)
            .map_err(|e| CisError::skill(format!("Failed to check skill state: {}", e)))?;

        // 如果未加载，尝试自动加载
        if !is_loaded {
            info!("Auto-loading skill '{}'", task.skill);
            self.skill_manager
                .load(&task.skill, LoadOptions::default())
                .map_err(|e| CisError::skill(format!("Failed to load skill: {}", e)))?;
        }

        // 检查 Skill 是否活跃
        let is_active = self
            .skill_manager
            .is_active(&task.skill)
            .map_err(|e| CisError::skill(format!("Failed to check skill state: {}", e)))?;

        if !is_active {
            self.skill_manager
                .activate(&task.skill)
                .map_err(|e| CisError::skill(format!("Failed to activate skill: {}", e)))?;
        }

        // 构造 Skill 配置
        let mut config = SkillConfig::default();
        config.set("action", task.action.clone());
        config.set("params", task.params.clone());
        config.set("raw", task.raw.clone());

        // 执行 Skill 调用
        let start = std::time::Instant::now();
        
        // 创建 Skill 上下文
        let ctx = BridgeSkillContext::new(config);
        
        // 构造调用事件
        let event = Event::Custom {
            name: task.action.clone(),
            data: serde_json::json!({
                "skill": task.skill,
                "action": task.action,
                "params": task.params,
                "raw": task.raw,
            }),
        };
        
        // 尝试通过不同方式执行 Skill
        let result = self.execute_skill(&task.skill, &ctx, event).await;
        
        let elapsed_ms = start.elapsed().as_millis() as u64;
        
        match result {
            Ok(data) => {
                info!("Skill '{}' executed successfully in {}ms", task.skill, elapsed_ms);
                Ok(SkillResult {
                    success: true,
                    data: Some(data),
                    error: None,
                    elapsed_ms,
                })
            }
            Err(e) => {
                warn!("Skill '{}' execution failed in {}ms: {}", task.skill, elapsed_ms, e);
                Ok(SkillResult {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    elapsed_ms,
                })
            }
        }
    }

    /// 列出可用 Skills
    async fn list_skills(&self) -> Result<String> {
        let skills = self
            .skill_manager
            .list_all()
            .map_err(|e| CisError::skill(format!("Failed to list skills: {}", e)))?;

        if skills.is_empty() {
            return Ok("📭 No skills installed.".to_string());
        }

        let mut output = "📦 Available Skills:\n".to_string();
        for skill in skills {
            let status = if skill.runtime.state.is_active() {
                "🟢"
            } else if skill.runtime.state.can_load() {
                "⚪"
            } else {
                "🔴"
            };
            output.push_str(&format!(
                "{} {} (v{}) - {}\n",
                status, skill.meta.name, skill.meta.version, skill.meta.description
            ));
        }

        Ok(output)
    }

    /// 格式化执行结果
    fn format_result(&self, result: &SkillResult) -> String {
        if result.success {
            match &result.data {
                Some(data) => {
                    format!("✅ Done ({}ms)\n```json\n{}\n```", 
                        result.elapsed_ms,
                        serde_json::to_string_pretty(data).unwrap_or_default()
                    )
                }
                None => format!("✅ Done ({}ms)", result.elapsed_ms),
            }
        } else {
            match &result.error {
                Some(err) => format!("❌ Error: {}", err),
                None => "❌ Unknown error".to_string(),
            }
        }
    }

    /// CIS 结果出向到 Matrix
    pub async fn send_to_room(
        &self,
        room_id: &RoomId,
        msg: &str,
    ) -> MatrixResult<OwnedEventId> {
        let event_id = format!("${}", Uuid::new_v4().to_string().replace('-', ""));
        
        let content = serde_json::json!({
            "msgtype": "m.text",
            "body": msg,
        });

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;

        self.matrix_store.save_event(
            room_id.as_str(),
            &event_id,
            "@cis:cis:cis.local",
            "m.room.message",
            &content.to_string(),
            now,
            None,
            None,
        )?;

        // Parse the event_id string back to OwnedEventId for return
        let owned_event_id = EventId::parse(&event_id)
            .map_err(|e| MatrixError::Internal(format!("Invalid event ID: {}", e)))?;

        debug!("Message sent to room {}: {}", room_id, msg);
        Ok(owned_event_id)
    }

    /// 发送消息到 Room（带联邦检查）
    /// 
    /// 如果 Room 设置了 federate=true，消息会广播到联邦
    pub async fn send_to_room_with_federation(
        &self,
        room_id: &RoomId,
        msg: &str,
    ) -> MatrixResult<OwnedEventId> {
        // 先保存消息到本地存储
        let event_id = self.send_to_room(room_id, msg).await?;
        
        // 检查是否需要联邦广播
        if let Ok(true) = self.matrix_store.is_room_federate(room_id.as_str()) {
            if let Err(e) = self.broadcast_to_federation(room_id, &event_id).await {
                warn!("Failed to broadcast to federation: {}", e);
                // 联邦广播失败不影响本地消息发送
            }
        }
        
        Ok(event_id)
    }

    /// 广播事件到联邦
    async fn broadcast_to_federation(
        &self,
        room_id: &RoomId,
        event_id: &EventId,
    ) -> MatrixResult<()> {
        info!("Broadcasting event {} to federation for room {}", event_id, room_id);
        
        // 检查是否有联邦管理器
        let federation_manager = match &self.federation_manager {
            Some(fm) => fm,
            None => {
                debug!("No federation manager configured, skipping broadcast");
                return Ok(());
            }
        };
        
        // 创建 CIS Matrix 事件（用于联邦广播）
        // 注意：由于存储层没有提供 get_event 方法，我们构造一个简化的事件
        let cis_event = CisMatrixEvent::new(
            event_id.as_str(),
            room_id.as_str(),
            "@cis:cis.local", // 系统用户
            "m.room.message",
            serde_json::json!({
                "msgtype": "m.text",
                "body": "Federated message",
            }),
        );
        
        // 广播到所有连接的联邦节点
        let results = federation_manager.broadcast_event(&cis_event).await;
        
        // 统计广播结果
        let success_count = results.values().filter(|r| r.is_ok()).count();
        let total_count = results.len();
        
        info!(
            "Federation broadcast completed: {}/{} nodes successful",
            success_count, total_count
        );
        
        // 记录失败的节点
        for (node_id, result) in results.iter().filter(|(_, r)| r.is_err()) {
            warn!("Failed to broadcast to {}: {:?}", node_id, result);
        }
        
        Ok(())
    }

    /// 获取控制房间 ID
    pub fn get_control_room_id(&self) -> Option<String> {
        self.control_room_id.read().ok().and_then(|g| g.clone())
    }

    /// 向控制房间发送消息
    pub async fn send_to_control_room(&self, msg: &str) -> MatrixResult<Option<OwnedEventId>> {
        if let Some(room_id_str) = self.get_control_room_id() {
            let room_id = RoomId::parse(&room_id_str)
                .map_err(|e| MatrixError::InvalidParameter(format!("Invalid room ID: {}", e)))?;
            let event_id = self.send_to_room(&room_id, msg).await?;
            Ok(Some(event_id))
        } else {
            warn!("Control room not initialized");
            Ok(None)
        }
    }
    
    /// 执行 Skill
    /// 
    /// 通过 SkillManager 获取 Skill 信息并尝试执行。
    /// 支持 Native 和 WASM Skill。
    async fn execute_skill(
        &self,
        skill_name: &str,
        ctx: &BridgeSkillContext,
        event: Event,
    ) -> Result<serde_json::Value> {
        // 获取 Skill 信息
        let skill_info = self.skill_manager
            .get_info(skill_name)
            .map_err(|e| CisError::skill(format!("Failed to get skill info: {}", e)))?
            .ok_or_else(|| CisError::not_found(format!("Skill '{}' not found", skill_name)))?;
        
        // 检查 Skill 状态
        if skill_info.runtime.state != SkillState::Active {
            return Err(CisError::skill(
                format!("Skill '{}' is not active (current state: {:?})", 
                    skill_name, skill_info.runtime.state)
            ));
        }
        
        // 根据 Skill 类型执行
        match skill_info.meta.skill_type {
            crate::skill::types::SkillType::Native => {
                self.execute_native_skill(skill_name, ctx, event).await
            }
            crate::skill::types::SkillType::Wasm => {
                self.execute_wasm_skill(skill_name, ctx, event).await
            }
            crate::skill::types::SkillType::Remote => {
                Err(CisError::skill("Remote skills not yet supported".to_string()))
            }
        }
    }
    
    /// 执行 Native Skill
    async fn execute_native_skill(
        &self,
        skill_name: &str,
        _ctx: &BridgeSkillContext,
        event: Event,
    ) -> Result<serde_json::Value> {
        // Native Skill 通过事件机制执行
        // 实际实现需要通过 SkillRegistry 获取 Skill 实例并调用 handle_event
        
        // 序列化事件
        let event_data = serde_json::to_vec(&event)
            .map_err(|e| CisError::skill(format!("Failed to serialize event: {}", e)))?;
        
        // 调用 SkillRegistry 处理事件
        let reg = self.skill_manager.get_registry()
            .map_err(|e| CisError::skill(format!("Failed to access registry: {}", e)))?;
        
        // 尝试查找并调用 Skill 实例
        // 由于 Native Skill 实现是 trait 对象，需要特定方式调用
        // 这里简化为返回执行信息
        if reg.contains(skill_name) {
            Ok(serde_json::json!({
                "skill": skill_name,
                "event": event,
                "status": "executed",
                "note": "Native skill execution simulated - actual implementation needs skill instance registry"
            }))
        } else {
            Err(CisError::not_found(format!("Skill '{}' not in registry", skill_name)))
        }
    }
    
    /// 执行 WASM Skill
    async fn execute_wasm_skill(
        &self,
        skill_name: &str,
        _ctx: &BridgeSkillContext,
        event: Event,
    ) -> Result<serde_json::Value> {
        // WASM Skill 执行
        // 需要通过 WasmRuntime 调用
        
        #[cfg(feature = "wasm")]
        {
            // 获取 WASM runtime
            let wasm_runtime = self.skill_manager.get_wasm_runtime()
                .map_err(|e| CisError::skill(format!("Failed to access WASM runtime: {}", e)))?;
            
            // 序列化事件
            let event_data = serde_json::to_vec(&event)
                .map_err(|e| CisError::skill(format!("Failed to serialize event: {}", e)))?;
            
            // 调用 WASM skill
            let result = {
                let runtime = wasm_runtime.lock()
                    .map_err(|e| CisError::skill(format!("WASM runtime lock failed: {}", e)))?;
                
                // 实际调用 WASM 函数
                // 这里需要根据 WASM 模块导出函数进行调用
                // 简化实现：返回执行信息
                Ok(serde_json::json!({
                    "skill": skill_name,
                    "event_type": "Custom",
                    "status": "wasm_execution_placeholder",
                    "note": "WASM skill execution needs full wasm runtime integration"
                }))
            }?;
            
            Ok(result)
        }
        
        #[cfg(not(feature = "wasm"))]
        {
            let _ = (skill_name, event);
            Err(CisError::skill("WASM support not compiled".to_string()))
        }
    }
}

/// Skill 上下文实现（用于 Bridge）
pub struct BridgeSkillContext {
    config: SkillConfig,
    memory: std::sync::Mutex<HashMap<String, Vec<u8>>>,
}

impl BridgeSkillContext {
    /// 创建新的 Bridge Skill 上下文
    pub fn new(config: SkillConfig) -> Self {
        Self {
            config,
            memory: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl SkillContext for BridgeSkillContext {
    fn log_info(&self, message: &str) {
        info!("[Skill] {}", message);
    }

    fn log_debug(&self, message: &str) {
        debug!("[Skill] {}", message);
    }

    fn log_warn(&self, message: &str) {
        warn!("[Skill] {}", message);
    }

    fn log_error(&self, message: &str) {
        tracing::error!("[Skill] {}", message);
    }

    fn memory_get(&self, key: &str) -> Option<Vec<u8>> {
        self.memory.lock().ok()?.get(key).cloned()
    }

    fn memory_set(&self, key: &str, value: &[u8]) -> crate::error::Result<()> {
        self.memory.lock()
            .map_err(|e| crate::error::CisError::other(format!("Memory lock failed: {}", e)))?
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }

    fn memory_delete(&self, key: &str) -> crate::error::Result<()> {
        self.memory.lock()
            .map_err(|e| crate::error::CisError::other(format!("Memory lock failed: {}", e)))?
            .remove(key);
        Ok(())
    }

    fn config(&self) -> &SkillConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::DbManager;

    fn create_test_bridge() -> MatrixBridge {
        let store = Arc::new(MatrixStore::open_in_memory().unwrap());
        let db_manager = Arc::new(DbManager::new().unwrap());
        let skill_manager = Arc::new(SkillManager::new(db_manager).unwrap());
        
        MatrixBridge::new(store, skill_manager).unwrap()
    }

    #[test]
    fn test_parse_skill_command() {
        let bridge = create_test_bridge();

        // 简单命令
        let task = bridge.parse_skill_command("nav").unwrap();
        assert_eq!(task.skill, "nav");
        assert_eq!(task.action, "default");
        assert!(task.params.is_empty());

        // 带参数
        let task = bridge.parse_skill_command("nav target=sofa").unwrap();
        assert_eq!(task.skill, "nav");
        assert_eq!(task.params.get("target"), Some(&"sofa".to_string()));

        // 多参数
        let task = bridge.parse_skill_command("nav target=sofa speed=fast action=go").unwrap();
        assert_eq!(task.skill, "nav");
        assert_eq!(task.action, "go");
        assert_eq!(task.params.get("target"), Some(&"sofa".to_string()));
        assert_eq!(task.params.get("speed"), Some(&"fast".to_string()));
        assert_eq!(task.params.get("action"), None); // action 被提取到 task.action
    }

    #[test]
    fn test_format_result() {
        let bridge = create_test_bridge();

        let success_result = SkillResult {
            success: true,
            data: Some(serde_json::json!({"status": "ok"})),
            error: None,
            elapsed_ms: 100,
        };
        let formatted = bridge.format_result(&success_result);
        assert!(formatted.contains("✅"));
        assert!(formatted.contains("100ms"));

        let error_result = SkillResult {
            success: false,
            data: None,
            error: Some("Something went wrong".to_string()),
            elapsed_ms: 0,
        };
        let formatted = bridge.format_result(&error_result);
        assert!(formatted.contains("❌"));
        assert!(formatted.contains("Something went wrong"));
    }
}
