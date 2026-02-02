//! Push Client Skill
//!
//! 从 AgentFlow push 系统迁移
//! 注意: CIS 核心禁止云端同步，此 skill 用于:
//! 1. 向外部系统发送通知（webhook）
//! 2. 与其他 CIS 节点 P2P 通信（元数据同步）

use std::collections::HashMap;

/// 推送目标
#[derive(Debug, Clone)]
pub struct PushTarget {
    pub id: String,
    pub endpoint: String,
    pub target_type: TargetType,
}

#[derive(Debug, Clone, Copy)]
pub enum TargetType {
    Webhook,    // HTTP webhook
    P2PNode,    // CIS P2P 节点
    Log,        // 仅日志记录
}

/// 推送消息
#[derive(Debug, Clone)]
pub struct PushMessage {
    pub target_id: String,
    pub payload: Vec<u8>,
    pub headers: HashMap<String, String>,
}

/// 推送结果
#[derive(Debug, Clone)]
pub struct PushResult {
    pub success: bool,
    pub target_id: String,
    pub error: Option<String>,
}

pub struct PushClient {
    targets: Vec<PushTarget>,
}

impl PushClient {
    pub fn new() -> Self {
        Self { targets: vec![] }
    }
    
    pub fn add_target(&mut self, target: PushTarget) {
        self.targets.push(target);
    }
    
    /// 推送消息（通过 host 执行实际网络操作）
    pub fn push(&self, msg: &PushMessage) -> PushResult {
        let target = match self.targets.iter().find(|t| t.id == msg.target_id) {
            Some(t) => t,
            None => {
                return PushResult {
                    success: false,
                    target_id: msg.target_id.clone(),
                    error: Some("Target not found".to_string()),
                };
            }
        };
        
        // 调用 host 执行推送
        match target.target_type {
            TargetType::Webhook => self.push_webhook(target, msg),
            TargetType::P2PNode => self.push_p2p(target, msg),
            TargetType::Log => self.push_log(target, msg),
        }
    }
    
    fn push_webhook(&self, target: &PushTarget, msg: &PushMessage) -> PushResult {
        // 通过 host 调用 HTTP
        let _ = (target, msg);
        PushResult {
            success: true,
            target_id: target.id.clone(),
            error: None,
        }
    }
    
    fn push_p2p(&self, target: &PushTarget, msg: &PushMessage) -> PushResult {
        // 通过 host 调用 P2P
        let _ = (target, msg);
        PushResult {
            success: true,
            target_id: target.id.clone(),
            error: None,
        }
    }
    
    fn push_log(&self, target: &PushTarget, msg: &PushMessage) -> PushResult {
        // 仅记录日志
        let log_msg = format!(
            "[Push to {}] {} bytes",
            target.id,
            msg.payload.len()
        );
        self.host_log(&log_msg);
        
        PushResult {
            success: true,
            target_id: target.id.clone(),
            error: None,
        }
    }
    
    /// Host 日志接口
    fn host_log(&self, msg: &str) {
        eprintln!("{}", msg);
    }
}

impl Default for PushClient {
    fn default() -> Self { Self::new() }
}

// ==================== AgentFlow 迁移的代码 ====================

/// 从 AgentFlow push/client.rs 迁移的收据系统
pub mod receipt {
    use std::time::{SystemTime, UNIX_EPOCH};
    
    /// 推送回执
    #[derive(Debug, Clone)]
    pub struct PushReceipt {
        pub target: String,
        pub status: ReceiptStatus,
        pub timestamp: u64,
    }
    
    #[derive(Debug, Clone, Copy)]
    pub enum ReceiptStatus {
        Pending,
        Confirmed,
        Failed,
        Rejected,
    }
    
    impl PushReceipt {
        pub fn new(target: &str) -> Self {
            Self {
                target: target.to_string(),
                status: ReceiptStatus::Pending,
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            }
        }
    }
}

/// 从 AgentFlow push/mod.rs 迁移的类型定义
pub mod types {
    /// 记忆载荷（简化版）
    #[derive(Debug, Clone)]
    pub struct MemoryPayload {
        pub key: String,
        pub value: Vec<u8>,
        pub category: String,
    }
    
    /// 撤销请求
    #[derive(Debug, Clone)]
    pub struct RevokeRequest {
        pub key: String,
        pub target: String,
    }
}

// WASM 导出
#[no_mangle]
pub extern "C" fn skill_init() -> i32 {
    0
}

/// 添加推送目标
#[no_mangle]
pub extern "C" fn skill_add_target(json_ptr: *const u8, len: usize) -> i32 {
    0
}

/// 执行推送
#[no_mangle]
pub extern "C" fn skill_push(json_ptr: *const u8, len: usize) -> i32 {
    0
}
