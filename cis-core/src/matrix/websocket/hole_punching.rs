//! # UDP Hole Punching 模块
//!
//! 实现 NAT 穿透的 UDP hole punching 机制。
//!
//! ## 流程
//!
//! 1. **NAT 类型检测**: 使用 STUN 服务器检测本机 NAT 类型
//! 2. **地址交换**: 通过信令服务器交换双方的公网地址
//! 3. **同时打洞**: 双方同时向对方公网地址发送 UDP 包
//! 4. **连接建立**: 打洞成功后建立直连
//!
//! ## NAT 类型兼容性
//!
//! | NAT Type | Full Cone | Restricted | Port Restricted | Symmetric |
//! |----------|-----------|------------|-----------------|-----------|
//! | Full Cone | ✓ | ✓ | ✓ | ✗ |
//! | Restricted | ✓ | ✓ | ✓ | ✗ |
//! | Port Restricted | ✓ | ✓ | ✓ | ✗ |
//! | Symmetric | ✗ | ✗ | ✗ | ✗ |

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::p2p::nat::{HolePunchCoordinator, HolePunchResult, NatTraversal, NatType};

/// 默认 STUN 服务器列表
pub const DEFAULT_STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
    "stun3.l.google.com:19302",
    "stun4.l.google.com:19302",
];

/// Hole punching 配置
#[derive(Debug, Clone)]
pub struct HolePunchConfig {
    /// STUN 服务器列表
    pub stun_servers: Vec<String>,
    /// 打洞超时时间
    pub punch_timeout: Duration,
    /// 打洞包数量
    pub punch_packet_count: u32,
    /// 打洞包间隔（毫秒）
    pub punch_interval_ms: u64,
    /// 是否启用 TURN 回退
    pub enable_turn_fallback: bool,
    /// TURN 服务器地址（可选）
    pub turn_server: Option<String>,
}

impl Default for HolePunchConfig {
    fn default() -> Self {
        Self {
            stun_servers: DEFAULT_STUN_SERVERS.iter().map(|s| s.to_string()).collect(),
            punch_timeout: Duration::from_secs(10),
            punch_packet_count: 10,
            punch_interval_ms: 50,
            enable_turn_fallback: true,
            turn_server: None,
        }
    }
}

impl HolePunchConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 STUN 服务器
    pub fn with_stun_servers(mut self, servers: Vec<String>) -> Self {
        self.stun_servers = servers;
        self
    }

    /// 设置打洞超时
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.punch_timeout = timeout;
        self
    }

    /// 设置打洞包数量
    pub fn with_packet_count(mut self, count: u32) -> Self {
        self.punch_packet_count = count;
        self
    }

    /// 禁用 TURN 回退
    pub fn without_turn_fallback(mut self) -> Self {
        self.enable_turn_fallback = false;
        self
    }
}

/// 信令客户端 trait（用于交换地址）
#[async_trait::async_trait]
pub trait SignalingClient: Send + Sync + std::fmt::Debug {
    /// 注册本机地址
    async fn register_endpoint(&self, node_id: &str, endpoint: SocketAddr) -> Result<()>;
    /// 获取对端地址
    async fn get_peer_endpoint(&self, peer_node_id: &str) -> Result<Option<SocketAddr>>;
    /// 等待打洞信号
    async fn wait_for_punch_signal(&self, peer_node_id: &str) -> Result<()>;
    /// 发送打洞信号
    async fn send_punch_signal(&self, peer_node_id: &str) -> Result<()>;
}

/// Hole punching 管理器
#[derive(Debug)]
pub struct HolePunchManager {
    config: HolePunchConfig,
    coordinator: Mutex<HolePunchCoordinator>,
    signaling: Arc<dyn SignalingClient>,
    node_id: String,
    state: Mutex<HolePunchState>,
}

/// Hole punching 状态
#[derive(Debug, Clone, PartialEq)]
pub enum HolePunchState {
    /// 空闲
    Idle,
    /// 正在检测 NAT
    DetectingNat,
    /// 正在交换地址
    ExchangingAddresses,
    /// 正在打洞
    Punching,
    /// 已连接
    Connected(SocketAddr),
    /// 失败
    Failed(String),
}

/// 打洞结果
#[derive(Debug, Clone)]
pub struct PunchResult {
    /// 是否成功
    pub success: bool,
    /// 对端地址
    pub peer_addr: Option<SocketAddr>,
    /// NAT 类型
    pub nat_type: NatType,
    /// 是否使用中继
    pub using_relay: bool,
    /// 错误信息
    pub error: Option<String>,
}

impl PunchResult {
    /// 创建成功结果
    pub fn success(peer_addr: SocketAddr, nat_type: NatType) -> Self {
        Self {
            success: true,
            peer_addr: Some(peer_addr),
            nat_type,
            using_relay: false,
            error: None,
        }
    }

    /// 创建中继结果
    pub fn relay(nat_type: NatType) -> Self {
        Self {
            success: true,
            peer_addr: None,
            nat_type,
            using_relay: true,
            error: None,
        }
    }

    /// 创建失败结果
    pub fn failed(error: impl Into<String>, nat_type: NatType) -> Self {
        Self {
            success: false,
            peer_addr: None,
            nat_type,
            using_relay: false,
            error: Some(error.into()),
        }
    }
}

impl HolePunchManager {
    /// 创建新的 hole punching 管理器
    pub fn new(
        node_id: impl Into<String>,
        config: HolePunchConfig,
        signaling: Arc<dyn SignalingClient>,
    ) -> Self {
        let coordinator = HolePunchCoordinator::with_stun_servers(config.stun_servers.clone());
        
        Self {
            config,
            coordinator: Mutex::new(coordinator),
            signaling,
            node_id: node_id.into(),
            state: Mutex::new(HolePunchState::Idle),
        }
    }

    /// 获取当前状态
    pub async fn state(&self) -> HolePunchState {
        self.state.lock().await.clone()
    }

    /// 检测 NAT 类型
    pub async fn detect_nat_type(&self) -> Result<NatType> {
        let mut state = self.state.lock().await;
        *state = HolePunchState::DetectingNat;
        drop(state);

        let mut coordinator = self.coordinator.lock().await;
        let (nat_type, external_addr) = coordinator.init().await?;
        
        info!(
            "NAT type detected: {}, external address: {:?}",
            nat_type, external_addr
        );

        let mut state = self.state.lock().await;
        *state = HolePunchState::Idle;
        
        Ok(nat_type)
    }

    /// 获取本机外部地址
    pub async fn external_addr(&self) -> Option<SocketAddr> {
        let coordinator = self.coordinator.lock().await;
        coordinator.external_addr()
    }

    /// 获取 NAT 类型
    pub async fn nat_type(&self) -> NatType {
        let coordinator = self.coordinator.lock().await;
        coordinator.nat_type()
    }

    /// 执行 hole punching
    pub async fn punch_hole(&self, target_node: &str) -> Result<PunchResult> {
        info!("Starting hole punching to node: {}", target_node);
        
        // 1. 确保已初始化
        let mut coordinator = self.coordinator.lock().await;
        if coordinator.local_addr().is_none() {
            let (nat_type, _) = coordinator.init().await?;
            info!("Initialized coordinator, NAT type: {}", nat_type);
        }
        drop(coordinator);

        // 2. 检查 NAT 类型
        let nat_type = self.nat_type().await;
        if nat_type.needs_turn() && !self.config.enable_turn_fallback {
            warn!("NAT type {} requires TURN but fallback is disabled", nat_type);
            let mut state = self.state.lock().await;
            *state = HolePunchState::Failed("Symmetric NAT requires TURN".to_string());
            return Ok(PunchResult::failed("Symmetric NAT requires TURN", nat_type));
        }

        // 3. 注册本机地址到信令服务器
        let mut state = self.state.lock().await;
        *state = HolePunchState::ExchangingAddresses;
        drop(state);

        if let Some(external_addr) = self.external_addr().await {
            if let Err(e) = self.signaling.register_endpoint(&self.node_id, external_addr).await {
                warn!("Failed to register endpoint: {}", e);
            }
        }

        // 4. 获取对端地址
        let peer_addr = match self.signaling.get_peer_endpoint(target_node).await? {
            Some(addr) => addr,
            None => {
                warn!("Peer {} endpoint not available", target_node);
                
                // 等待打洞信号
                info!("Waiting for punch signal from {}", target_node);
                if let Err(e) = self.signaling.wait_for_punch_signal(target_node).await {
                    warn!("Failed to wait for punch signal: {}", e);
                }
                
                // 重试获取对端地址
                match self.signaling.get_peer_endpoint(target_node).await? {
                    Some(addr) => addr,
                    None => {
                        let mut state = self.state.lock().await;
                        *state = HolePunchState::Failed("Peer endpoint not available".to_string());
                        return Ok(PunchResult::failed("Peer endpoint not available", nat_type));
                    }
                }
            }
        };

        info!("Peer {} public endpoint: {}", target_node, peer_addr);

        // 5. 发送打洞信号
        if let Err(e) = self.signaling.send_punch_signal(target_node).await {
            debug!("Failed to send punch signal: {}", e);
        }

        // 6. 执行打洞
        let mut state = self.state.lock().await;
        *state = HolePunchState::Punching;
        drop(state);

        let result = self.perform_punch(peer_addr).await?;
        
        let mut final_state = self.state.lock().await;
        match &result {
            HolePunchResult::Success { peer_addr, .. } => {
                *final_state = HolePunchState::Connected(*peer_addr);
                Ok(PunchResult::success(*peer_addr, nat_type))
            }
            HolePunchResult::RelayRequired { reason } => {
                if self.config.enable_turn_fallback {
                    *final_state = HolePunchState::Idle;
                    Ok(PunchResult::relay(nat_type))
                } else {
                    *final_state = HolePunchState::Failed(reason.clone());
                    Ok(PunchResult::failed(reason.clone(), nat_type))
                }
            }
            HolePunchResult::Failed { error } => {
                *final_state = HolePunchState::Failed(error.clone());
                Ok(PunchResult::failed(error.clone(), nat_type))
            }
        }
    }

    /// 执行实际的打洞操作
    async fn perform_punch(&self, peer_addr: SocketAddr) -> Result<HolePunchResult> {
        let coordinator = self.coordinator.lock().await;
        
        // 发送打洞包
        if let Err(e) = coordinator.send_punch_packets(peer_addr, self.config.punch_packet_count).await {
            return Ok(HolePunchResult::Failed {
                error: format!("Failed to send punch packets: {}", e),
            });
        }

        // 等待响应
        let timeout_secs = self.config.punch_timeout.as_secs();
        let result = coordinator.wait_for_punch(peer_addr, timeout_secs).await?;
        
        Ok(result)
    }

    /// 处理 incoming 打洞请求（用于服务器端）
    pub async fn handle_incoming_punch(&self, from_addr: SocketAddr) -> Result<()> {
        info!("Received hole punch request from {}", from_addr);
        
        let coordinator = self.coordinator.lock().await;
        
        // 发送响应
        if let Err(e) = coordinator.send_punch_packets(from_addr, 3).await {
            debug!("Failed to send punch response: {}", e);
        }
        
        Ok(())
    }
}

/// 简单的内存信令客户端（用于测试）
#[derive(Debug)]
pub struct InMemorySignalingClient {
    endpoints: Mutex<std::collections::HashMap<String, SocketAddr>>,
    punch_signals: Mutex<std::collections::HashMap<String, mpsc::Sender<()>>>,
}

impl InMemorySignalingClient {
    /// 创建新的内存信令客户端
    pub fn new() -> Self {
        Self {
            endpoints: Mutex::new(std::collections::HashMap::new()),
            punch_signals: Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// 添加对端地址（用于测试）
    pub async fn add_peer_endpoint(&self, node_id: impl Into<String>, addr: SocketAddr) {
        let mut endpoints = self.endpoints.lock().await;
        endpoints.insert(node_id.into(), addr);
    }
}

impl Default for InMemorySignalingClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SignalingClient for InMemorySignalingClient {
    async fn register_endpoint(&self, node_id: &str, endpoint: SocketAddr) -> Result<()> {
        let mut endpoints = self.endpoints.lock().await;
        endpoints.insert(node_id.to_string(), endpoint);
        info!("Registered endpoint for {}: {}", node_id, endpoint);
        Ok(())
    }

    async fn get_peer_endpoint(&self, peer_node_id: &str) -> Result<Option<SocketAddr>> {
        let endpoints = self.endpoints.lock().await;
        Ok(endpoints.get(peer_node_id).copied())
    }

    async fn wait_for_punch_signal(&self, peer_node_id: &str) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(1);
        {
            let mut signals = self.punch_signals.lock().await;
            signals.insert(peer_node_id.to_string(), tx);
        }
        
        // 等待最多 10 秒
        let _ = timeout(Duration::from_secs(10), rx.recv()).await;
        Ok(())
    }

    async fn send_punch_signal(&self, peer_node_id: &str) -> Result<()> {
        let signals = self.punch_signals.lock().await;
        if let Some(tx) = signals.get(peer_node_id) {
            let _ = tx.send(()).await;
        }
        Ok(())
    }
}

/// 创建 UDP socket 用于打洞
pub async fn create_punch_socket(preferred_port: Option<u16>) -> Result<UdpSocket> {
    let bind_addr = match preferred_port {
        Some(port) => format!("0.0.0.0:{}", port),
        None => "0.0.0.0:0".to_string(),
    };
    
    let socket = UdpSocket::bind(&bind_addr).await
        .map_err(|e| crate::error::CisError::p2p(format!("Failed to bind UDP socket: {}", e)))?;
    
    info!("Created punch socket bound to {}", socket.local_addr()?);
    Ok(socket)
}

/// 快速 NAT 类型检测
pub async fn quick_nat_test(_stun_server: &str) -> Result<(NatType, Option<SocketAddr>)> {
    let nat = NatTraversal::new(0);
    nat.detect_nat_type().await
}

/// 执行同时打洞（双方都发送打洞包）
pub async fn simultaneous_punch(
    socket: std::net::UdpSocket,
    peer_addr: SocketAddr,
    packet_count: u32,
    interval_ms: u64,
) -> Result<bool> {
    // 克隆 socket 用于接收任务
    let recv_socket = socket.try_clone()
        .map_err(|e| crate::error::CisError::p2p(format!("Failed to clone socket: {}", e)))?;
    
    let punch_packet = b"CIS_PUNCH_SYN";
    let punch_packet_owned = *punch_packet;
    
    // 创建接收任务 (在 blocking 任务中执行)
    let recv_task = tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 1024];
        let start = std::time::Instant::now();
        
        recv_socket.set_nonblocking(false).ok();
        recv_socket.set_read_timeout(Some(std::time::Duration::from_millis(100))).ok();
        
        while start.elapsed() < Duration::from_secs(10) {
            match recv_socket.recv_from(&mut buf) {
                Ok((len, _)) => {
                    if len >= 13 && &buf[..13] == b"CIS_PUNCH_ACK" {
                        return true;
                    }
                }
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
        false
    });

    // 发送打洞包
    for i in 0..packet_count {
        if let Err(e) = socket.send_to(&punch_packet_owned, peer_addr) {
            warn!("Failed to send punch packet {}: {}", i, e);
        } else {
            debug!("Sent punch packet {} to {}", i, peer_addr);
        }
        
        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
        
        // 检查是否已收到响应
        if recv_task.is_finished() {
            break;
        }
    }

    // 等待接收任务完成
    match timeout(Duration::from_secs(5), recv_task).await {
        Ok(Ok(true)) => Ok(true),
        _ => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hole_punch_config() {
        let config = HolePunchConfig::new()
            .with_packet_count(20)
            .with_timeout(Duration::from_secs(15))
            .without_turn_fallback();
        
        assert_eq!(config.punch_packet_count, 20);
        assert_eq!(config.punch_timeout, Duration::from_secs(15));
        assert!(!config.enable_turn_fallback);
    }

    #[test]
    fn test_punch_result() {
        let success = PunchResult::success(
            "192.168.1.1:1234".parse().unwrap(),
            NatType::FullCone,
        );
        assert!(success.success);
        assert!(!success.using_relay);
        assert_eq!(success.nat_type, NatType::FullCone);

        let relay = PunchResult::relay(NatType::Symmetric);
        assert!(relay.success);
        assert!(relay.using_relay);
        
        let failed = PunchResult::failed("timeout", NatType::Unknown);
        assert!(!failed.success);
        assert!(failed.error.is_some());
    }

    #[tokio::test]
    async fn test_in_memory_signaling() {
        let signaling = Arc::new(InMemorySignalingClient::new());
        let addr: SocketAddr = "192.168.1.1:1234".parse().unwrap();
        
        // 注册端点
        signaling.register_endpoint("node1", addr).await.unwrap();
        
        // 获取端点
        let retrieved = signaling.get_peer_endpoint("node1").await.unwrap();
        assert_eq!(retrieved, Some(addr));
        
        // 获取不存在的端点
        let not_found = signaling.get_peer_endpoint("node2").await.unwrap();
        assert_eq!(not_found, None);
    }

    #[tokio::test]
    async fn test_create_punch_socket() {
        let socket = create_punch_socket(None).await.unwrap();
        let local_addr = socket.local_addr().unwrap();
        assert!(local_addr.port() > 0);
    }
}
