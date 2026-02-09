//! # 6位数字组网码快速配对
//!
//! 零配置节点组网，通过6位数字验证码配对。

use crate::error::{CisError, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const PAIRING_PORT: u16 = 6768;
const CODE_LENGTH: usize = 6;
const CODE_TIMEOUT: Duration = Duration::from_secs(300); // 5分钟
const MAX_ATTEMPTS: u32 = 5;

/// 组网状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingState {
    Waiting,
    Verifying,
    Exchanging,
    Completed,
    Expired,
    Failed,
}

/// 组网节点信息（简化版）
#[derive(Debug, Clone)]
pub struct PairingNodeInfo {
    pub node_id: String,
    pub did: String,
    pub hostname: String,
}

impl Default for PairingNodeInfo {
    fn default() -> Self {
        Self {
            node_id: "unknown".to_string(),
            did: "unknown".to_string(),
            hostname: "unknown".to_string(),
        }
    }
}

/// 组网会话
#[derive(Debug, Clone)]
pub struct PairingSession {
    pub code: String,
    pub target_node: PairingNodeInfo,
    pub state: PairingState,
    pub created_at: Instant,
    pub attempts: u32,
    pub requester_info: Option<PairingNodeInfo>,
}

impl PairingSession {
    pub fn is_expired(&self) -> bool {
        Instant::now().duration_since(self.created_at) > CODE_TIMEOUT
    }
}

/// 组网码管理器
pub struct PairingManager {
    active_sessions: Arc<Mutex<HashMap<String, PairingSession>>>,
    max_attempts: u32,
}

impl PairingManager {
    /// 创建新的组网管理器
    pub fn new() -> Self {
        tracing::info!("[PAIRING] Creating new PairingManager");
        Self {
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
            max_attempts: MAX_ATTEMPTS,
        }
    }
    
    /// 生成6位组网码
    pub fn generate_code(&self, target_node: PairingNodeInfo) -> Result<String> {
        use rand::Rng;
        
        tracing::info!("[PAIRING] Generating pairing code for node: {}", target_node.node_id);
        
        let mut rng = rand::thread_rng();
        let mut sessions = self.active_sessions.lock()
            .map_err(|e| CisError::other(format!("Failed to lock sessions: {}", e)))?;
        
        // 清理过期会话
        let before_count = sessions.len();
        Self::cleanup_expired(&mut sessions);
        let after_count = sessions.len();
        if before_count != after_count {
            tracing::debug!("[PAIRING] Cleaned up {} expired sessions", before_count - after_count);
        }
        
        // 生成不重复的6位码
        let code = loop {
            let code: String = (0..CODE_LENGTH)
                .map(|_| rng.gen_range(0..10).to_string().chars().next().unwrap())
                .collect();
            
            if !sessions.contains_key(&code) {
                break code;
            }
        };
        
        tracing::info!("[PAIRING] Generated code: {}", code);
        
        // 创建会话
        let session = PairingSession {
            code: code.clone(),
            target_node,
            state: PairingState::Waiting,
            created_at: Instant::now(),
            attempts: 0,
            requester_info: None,
        };
        
        sessions.insert(code.clone(), session);
        tracing::info!("[PAIRING] Session created. Active sessions: {}", sessions.len());
        
        Ok(code)
    }
    
    /// 验证组网码
    pub fn verify_code(&self, code: &str, requester: PairingNodeInfo) -> Result<PairingSession> {
        tracing::info!("[PAIRING] Verifying code: {} from requester: {}", code, requester.node_id);
        
        let mut sessions = self.active_sessions.lock()
            .map_err(|e| CisError::other(format!("Failed to lock sessions: {}", e)))?;
        
        let session = sessions.get_mut(code)
            .ok_or_else(|| {
                tracing::warn!("[PAIRING] Code not found: {}", code);
                CisError::not_found("Invalid pairing code")
            })?;
        
        // 检查过期
        if session.is_expired() {
            tracing::warn!("[PAIRING] Code expired: {}", code);
            session.state = PairingState::Expired;
            return Err(CisError::invalid_state("Pairing code expired"));
        }
        
        // 检查状态
        if session.state != PairingState::Waiting {
            tracing::warn!("[PAIRING] Code already used: {} (state: {:?})", code, session.state);
            return Err(CisError::invalid_state("Pairing code already used"));
        }
        
        // 检查尝试次数
        if session.attempts >= self.max_attempts {
            tracing::warn!("[PAIRING] Too many attempts for code: {}", code);
            session.state = PairingState::Failed;
            return Err(CisError::invalid_input("Too many attempts"));
        }
        
        session.attempts += 1;
        session.state = PairingState::Verifying;
        session.requester_info = Some(requester);
        
        tracing::info!("[PAIRING] Code verified: {} (attempt: {})", code, session.attempts);
        
        Ok(session.clone())
    }
    
    /// 接受配对
    pub fn accept_pairing(&self, code: &str) -> Result<PairingSession> {
        tracing::info!("[PAIRING] Accepting pairing for code: {}", code);
        
        let mut sessions = self.active_sessions.lock()
            .map_err(|e| CisError::other(format!("Failed to lock sessions: {}", e)))?;
        
        let session = sessions.get_mut(code)
            .ok_or_else(|| {
                tracing::warn!("[PAIRING] Cannot accept - code not found: {}", code);
                CisError::not_found("Pairing code not found")
            })?;
        
        session.state = PairingState::Exchanging;
        tracing::info!("[PAIRING] Pairing accepted: {}", code);
        
        Ok(session.clone())
    }
    
    /// 完成配对
    pub fn complete_pairing(&self, code: &str) -> Result<()> {
        tracing::info!("[PAIRING] Completing pairing for code: {}", code);
        
        let mut sessions = self.active_sessions.lock()
            .map_err(|e| CisError::other(format!("Failed to lock sessions: {}", e)))?;
        
        if let Some(session) = sessions.get_mut(code) {
            session.state = PairingState::Completed;
            tracing::info!("[PAIRING] Pairing completed: {}", code);
        }
        
        // 配对成功后移除会话
        sessions.remove(code);
        tracing::info!("[PAIRING] Session removed. Active sessions: {}", sessions.len());
        
        Ok(())
    }
    
    /// 拒绝配对
    pub fn reject_pairing(&self, code: &str) -> Result<()> {
        tracing::info!("[PAIRING] Rejecting pairing for code: {}", code);
        
        let mut sessions = self.active_sessions.lock()
            .map_err(|e| CisError::other(format!("Failed to lock sessions: {}", e)))?;
        
        if let Some(session) = sessions.get_mut(code) {
            session.state = PairingState::Failed;
            tracing::info!("[PAIRING] Pairing rejected: {}", code);
        }
        
        Ok(())
    }
    
    /// 获取会话
    pub fn get_session(&self, code: &str) -> Result<PairingSession> {
        let sessions = self.active_sessions.lock()
            .map_err(|e| CisError::other(format!("Failed to lock sessions: {}", e)))?;
        
        sessions.get(code)
            .cloned()
            .ok_or_else(|| {
                tracing::warn!("[PAIRING] Session not found: {}", code);
                CisError::not_found("Pairing code not found")
            })
    }
    
    /// 清理过期会话
    fn cleanup_expired(sessions: &mut HashMap<String, PairingSession>) {
        let expired_codes: Vec<String> = sessions
            .iter()
            .filter(|(_, s)| s.is_expired() || s.state == PairingState::Completed)
            .map(|(k, _)| k.clone())
            .collect();
        
        for code in expired_codes {
            tracing::debug!("[PAIRING] Cleaning up expired session: {}", code);
            sessions.remove(&code);
        }
    }
    
    /// 启动清理任务
    pub fn start_cleanup_task(&self) {
        let sessions = self.active_sessions.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                if let Ok(mut guard) = sessions.lock() {
                    let before = guard.len();
                    Self::cleanup_expired(&mut guard);
                    let after = guard.len();
                    if before != after {
                        tracing::info!("[PAIRING] Cleanup task removed {} expired sessions", before - after);
                    }
                }
            }
        });
    }
}

impl Default for PairingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// 组网服务（UDP广播）
pub struct PairingService {
    manager: Arc<PairingManager>,
    port: u16,
}

impl PairingService {
    pub fn new(manager: Arc<PairingManager>) -> Self {
        tracing::info!("[PAIRING] Creating PairingService on port {}", PAIRING_PORT);
        Self {
            manager,
            port: PAIRING_PORT,
        }
    }
    
    /// 启动监听（阻塞）
    pub async fn listen(&self, code: String) -> Result<PairingResult> {
        use tokio::net::UdpSocket;
        
        tracing::info!("[PAIRING] Starting listen for code: {}", code);
        
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", self.port)).await
            .map_err(|e| {
                tracing::error!("[PAIRING] Failed to bind socket on port {}: {}", self.port, e);
                CisError::network(format!("Failed to bind socket: {}", e))
            })?;
        
        tracing::info!("[PAIRING] Socket bound to port {}", self.port);
        
        let mut buf = [0u8; 1024];
        let timeout = tokio::time::Duration::from_secs(300);
        
        println!("🔄 等待组网请求，组网码: {}...", code);
        
        loop {
            match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, addr))) => {
                    tracing::debug!("[PAIRING] Received {} bytes from {}", len, addr);
                    if let Ok(msg) = String::from_utf8(buf[..len].to_vec()) {
                        tracing::debug!("[PAIRING] Message: {}", msg);
                        if let Some(result) = self.handle_message(&msg, &code, addr, &socket).await? {
                            tracing::info!("[PAIRING] Listen completed with result: {:?}", result);
                            return Ok(result);
                        }
                    } else {
                        tracing::warn!("[PAIRING] Invalid UTF-8 message from {}", addr);
                    }
                }
                Ok(Err(e)) => {
                    tracing::error!("[PAIRING] Socket error: {}", e);
                }
                Err(_) => {
                    tracing::error!("[PAIRING] Listen timeout");
                    return Err(CisError::invalid_state("Pairing timeout"));
                }
            }
        }
    }
    
    /// 发送组网请求
    pub async fn request_pairing(&self, code: &str, target_addr: SocketAddr, local_node: PairingNodeInfo) -> Result<PairingResult> {
        use tokio::net::UdpSocket;
        
        tracing::info!("[PAIRING] Requesting pairing with code {} to {}", code, target_addr);
        
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| {
                tracing::error!("[PAIRING] Failed to bind client socket: {}", e);
                CisError::network(format!("Failed to bind socket: {}", e))
            })?;
        
        // 发送组网请求
        let request = format!("PAIR_REQ|{}|{}", code, local_node.node_id);
        tracing::info!("[PAIRING] Sending request: {}", request);
        
        socket.send_to(request.as_bytes(), target_addr).await
            .map_err(|e| {
                tracing::error!("[PAIRING] Failed to send request: {}", e);
                CisError::network(format!("Failed to send request: {}", e))
            })?;
        
        tracing::info!("[PAIRING] Request sent, waiting for response...");
        
        // 等待响应
        let mut buf = [0u8; 1024];
        let timeout = tokio::time::Duration::from_secs(30);
        
        match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, addr))) => {
                if let Ok(msg) = String::from_utf8(buf[..len].to_vec()) {
                    tracing::info!("[PAIRING] Received response from {}: {}", addr, msg);
                    self.parse_response(&msg)
                } else {
                    tracing::error!("[PAIRING] Invalid UTF-8 response");
                    Err(CisError::invalid_input("Invalid response"))
                }
            }
            Ok(Err(e)) => {
                tracing::error!("[PAIRING] Socket error while waiting for response: {}", e);
                Err(CisError::network(format!("Socket error: {}", e)))
            }
            Err(_) => {
                tracing::error!("[PAIRING] Timeout waiting for response from {}", target_addr);
                Err(CisError::p2p("No response from target"))
            }
        }
    }
    
    /// 处理收到的消息
    async fn handle_message(&self, msg: &str, expected_code: &str, addr: SocketAddr, socket: &tokio::net::UdpSocket) -> Result<Option<PairingResult>> {
        let parts: Vec<&str> = msg.split('|').collect();
        
        tracing::debug!("[PAIRING] Handling message from {}: {:?}", addr, parts);
        
        if parts.len() < 2 {
            tracing::warn!("[PAIRING] Invalid message format from {}: {}", addr, msg);
            return Ok(None);
        }
        
        match parts[0] {
            "PAIR_REQ" => {
                // 收到组网请求
                let code = parts.get(1).unwrap_or(&"");
                let requester_id = parts.get(2).unwrap_or(&"unknown");
                
                tracing::info!("[PAIRING] PAIR_REQ received from {}: code={}, requester={}", addr, code, requester_id);
                
                if *code != expected_code {
                    tracing::warn!("[PAIRING] Code mismatch: expected {}, got {}", expected_code, code);
                    return Ok(None);
                }
                
                println!("📥 收到组网请求来自: {} ({})", requester_id, addr);
                
                // 发送响应 - 修复：实际发送 UDP 响应！
                let response = format!("PAIR_ACK|SUCCESS|node1|{}", addr);
                tracing::info!("[PAIRING] Sending response to {}: {}", addr, response);
                
                match socket.send_to(response.as_bytes(), addr).await {
                    Ok(n) => tracing::info!("[PAIRING] Response sent: {} bytes", n),
                    Err(e) => tracing::error!("[PAIRING] Failed to send response: {}", e),
                }
                
                println!("✅ 接受组网请求，请确认添加此节点");
                
                Ok(Some(PairingResult {
                    success: true,
                    node_id: requester_id.to_string(),
                    endpoint: addr.to_string(),
                    did: None,
                }))
            }
            "PAIR_ACK" => {
                tracing::info!("[PAIRING] PAIR_ACK received from {}: {:?}", addr, parts);
                Ok(None)
            }
            _ => {
                tracing::debug!("[PAIRING] Unknown message type from {}: {}", addr, parts[0]);
                Ok(None)
            }
        }
    }
    
    /// 解析响应
    fn parse_response(&self, msg: &str) -> Result<PairingResult> {
        let parts: Vec<&str> = msg.split('|').collect();
        
        tracing::debug!("[PAIRING] Parsing response: {:?}", parts);
        
        if parts.len() < 2 {
            tracing::error!("[PAIRING] Invalid response format: {}", msg);
            return Err(CisError::invalid_input("Invalid response format"));
        }
        
        match parts[0] {
            "PAIR_ACK" => {
                tracing::info!("[PAIRING] Pairing acknowledged: {:?}", parts);
                Ok(PairingResult {
                    success: true,
                    node_id: parts.get(1).unwrap_or(&"unknown").to_string(),
                    endpoint: parts.get(2).unwrap_or(&"").to_string(),
                    did: parts.get(3).map(|s| s.to_string()),
                })
            }
            "PAIR_REJECT" => {
                tracing::warn!("[PAIRING] Pairing rejected");
                Err(CisError::invalid_input("Pairing rejected"))
            }
            _ => {
                tracing::error!("[PAIRING] Unknown response type: {}", parts[0]);
                Err(CisError::invalid_input("Unknown response type"))
            }
        }
    }
}

/// 组网结果
#[derive(Debug, Clone)]
pub struct PairingResult {
    pub success: bool,
    pub node_id: String,
    pub endpoint: String,
    pub did: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_generate_code() {
        let manager = PairingManager::new();
        let node = PairingNodeInfo::default();
        
        let code = manager.generate_code(node).unwrap();
        
        assert_eq!(code.len(), CODE_LENGTH);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }
    
    #[test]
    fn test_code_expiration() {
        let session = PairingSession {
            code: "123456".to_string(),
            target_node: PairingNodeInfo::default(),
            state: PairingState::Waiting,
            created_at: Instant::now() - Duration::from_secs(400), // 已过期
            attempts: 0,
            requester_info: None,
        };
        
        assert!(session.is_expired());
    }
    
    #[test]
    fn test_max_attempts() {
        let manager = PairingManager::new();
        let node = PairingNodeInfo::default();
        
        let code = manager.generate_code(node.clone()).unwrap();
        
        // 模拟多次失败尝试
        for _ in 0..MAX_ATTEMPTS {
            let _ = manager.verify_code(&code, node.clone());
        }
        
        // 第6次应该失败
        let result = manager.verify_code(&code, node);
        assert!(result.is_err());
    }
}
