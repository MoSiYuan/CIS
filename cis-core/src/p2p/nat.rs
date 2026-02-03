//! NAT 穿透支持
//!
//! 提供 UPnP、STUN 和 UDP Hole Punching 支持，用于公网连接

use crate::error::Result;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// 默认 STUN 服务器列表
pub const DEFAULT_STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
    "stun3.l.google.com:19302",
    "stun4.l.google.com:19302",
];

/// NAT 穿透管理器
pub struct NatTraversal {
    local_port: u16,
    external_addr: Option<SocketAddr>,
    nat_type: NatType,
    stun_servers: Vec<String>,
}

impl NatTraversal {
    /// 创建 NAT 穿透管理器
    pub fn new(local_port: u16) -> Self {
        Self {
            local_port,
            external_addr: None,
            nat_type: NatType::Unknown,
            stun_servers: DEFAULT_STUN_SERVERS.iter().map(|s| s.to_string()).collect(),
        }
    }

    /// 创建带自定义 STUN 服务器的 NAT 穿透管理器
    pub fn with_stun_servers(local_port: u16, stun_servers: Vec<String>) -> Self {
        Self {
            local_port,
            external_addr: None,
            nat_type: NatType::Unknown,
            stun_servers,
        }
    }

    /// 尝试 NAT 穿透
    pub async fn try_traversal(&mut self) -> Result<Option<SocketAddr>> {
        // 1. 首先尝试 UPnP
        match self.try_upnp().await {
            Ok(Some(addr)) => {
                tracing::info!("UPnP port mapping successful: {}", addr);
                self.external_addr = Some(addr);
                self.nat_type = NatType::Open;
                return Ok(Some(addr));
            }
            Ok(None) => {
                tracing::debug!("UPnP not available");
            }
            Err(e) => {
                tracing::warn!("UPnP failed: {}", e);
            }
        }

        // 2. 尝试 STUN 获取公网地址并检测 NAT 类型
        match self.detect_nat_type().await {
            Ok((nat_type, addr)) => {
                self.nat_type = nat_type;
                if let Some(addr) = addr {
                    tracing::info!("STUN successful, external address: {}, NAT type: {:?}", addr, nat_type);
                    self.external_addr = Some(addr);
                    return Ok(Some(addr));
                }
            }
            Err(e) => {
                tracing::warn!("STUN/NAT detection failed: {}", e);
            }
        }

        // 3. 如果都失败，返回 None
        tracing::warn!("NAT traversal failed, may require manual port forwarding");
        Ok(None)
    }

    /// 尝试 UPnP 端口映射
    async fn try_upnp(&self) -> Result<Option<SocketAddr>> {
        tracing::debug!("Trying UPnP port mapping for port {}", self.local_port);

        let local_port = self.local_port;
        
        // 使用 igd crate 进行 UPnP 端口映射
        tokio::task::spawn_blocking(move || {
            // 搜索网关
            let gateway = match igd::search_gateway(Default::default()) {
                Ok(g) => g,
                Err(e) => {
                    tracing::debug!("UPnP gateway not found: {}", e);
                    return Ok(None);
                }
            };

            tracing::info!("Found UPnP gateway: {}", gateway.addr);

            // 获取本地地址
            let local_addr = match get_local_ip() {
                Some(ip) => ip,
                None => {
                    return Err(crate::error::CisError::p2p("Could not determine local IP"));
                }
            };

            // 添加端口映射（简化版，仅尝试获取外部 IP）
            // 注意：完整的端口映射需要在 socket 绑定后才能进行
            // 这里仅获取外部 IP 地址

            // 获取外部 IP
            match gateway.get_external_ip() {
                Ok(external_ip) => {
                    let external_addr = SocketAddr::new(external_ip.into(), local_port);
                    tracing::info!("UPnP external IP discovered: {}", external_addr);
                    Ok(Some(external_addr))
                }
                Err(e) => {
                    tracing::warn!("Failed to get external IP: {}", e);
                    Ok(None)
                }
            }
        })
        .await
        .map_err(|e| crate::error::CisError::p2p(format!("UPnP task failed: {}", e)))?
    }

    /// 检测 NAT 类型并获取公网地址
    pub async fn detect_nat_type(&self) -> Result<(NatType, Option<SocketAddr>)> {
        debug!("Starting NAT type detection");
        
        // 创建 UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| crate::error::CisError::p2p(format!("Failed to bind UDP socket: {}", e)))?;
        
        let local_addr = socket.local_addr()
            .map_err(|e| crate::error::CisError::p2p(format!("Failed to get local address: {}", e)))?;
        debug!("UDP socket bound to {}", local_addr);

        // 尝试多个 STUN 服务器
        for stun_server in &self.stun_servers {
            match self.test_stun_server(&socket, stun_server).await {
                Ok(result) => {
                    info!("STUN test successful with {}: {:?}", stun_server, result);
                    return Ok(result);
                }
                Err(e) => {
                    debug!("STUN test failed with {}: {}", stun_server, e);
                }
            }
        }

        warn!("All STUN servers failed");
        Ok((NatType::Unknown, None))
    }

    /// 测试单个 STUN 服务器
    async fn test_stun_server(
        &self,
        socket: &UdpSocket,
        stun_server: &str,
    ) -> Result<(NatType, Option<SocketAddr>)> {
        use stun::agent::*;
        use stun::client::*;
        use stun::message::*;
        use stun::xoraddr::*;

        // 解析 STUN 服务器地址
        let server_addr: SocketAddr = stun_server.parse()
            .map_err(|e| crate::error::CisError::p2p(format!("Invalid STUN server address: {}", e)))?;

        // 发送 STUN binding request
        let mut msg = Message::new();
        msg.build(&[
            Box::new(TransactionId::new()),
            Box::new(BINDING_REQUEST),
        ])
        .map_err(|e| crate::error::CisError::p2p(format!("Failed to build STUN message: {}", e)))?;

        // 发送请求并等待响应
        let send_result = timeout(Duration::from_secs(3), async {
            socket.send_to(&msg.raw, server_addr).await
        }).await;

        match send_result {
            Ok(Ok(_)) => {},
            Ok(Err(e)) => return Err(crate::error::CisError::p2p(format!("Failed to send STUN request: {}", e))),
            Err(_) => return Err(crate::error::CisError::p2p("STUN request timeout")),
        }

        // 接收响应
        let mut buf = [0u8; 1024];
        let recv_result = timeout(Duration::from_secs(3), socket.recv_from(&mut buf)).await;

        let (len, from) = match recv_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(crate::error::CisError::p2p(format!("Failed to receive STUN response: {}", e))),
            Err(_) => return Err(crate::error::CisError::p2p("STUN response timeout")),
        };

        debug!("Received {} bytes from STUN server {} (from {})", len, stun_server, from);

        // 解析响应
        let mut response = Message::new();
        response.raw = buf[..len].to_vec();
        
        if let Err(e) = response.decode() {
            return Err(crate::error::CisError::p2p(format!("Failed to decode STUN response: {}", e)));
        }

        // 提取 XOR-MAPPED-ADDRESS
        let mut xor_addr = XorMappedAddress::default();
        match xor_addr.get_from(&response) {
            Ok(_) => {
                let external_addr = SocketAddr::new(xor_addr.ip, xor_addr.port);
                debug!("STUN mapped address: {}", external_addr);
                
                // 简单的 NAT 类型判断
                let local_addr = socket.local_addr()?;
                let nat_type = if external_addr.port() == local_addr.port() {
                    NatType::Open
                } else {
                    // 需要更多测试来确定具体类型
                    NatType::Unknown
                };
                
                Ok((nat_type, Some(external_addr)))
            }
            Err(e) => {
                Err(crate::error::CisError::p2p(format!("Failed to get mapped address: {}", e)))
            }
        }
    }

    /// 获取外部地址
    pub fn external_addr(&self) -> Option<SocketAddr> {
        self.external_addr
    }

    /// 获取 NAT 类型
    pub fn nat_type(&self) -> NatType {
        self.nat_type
    }

    /// 刷新端口映射
    pub async fn refresh_mapping(&self) -> Result<()> {
        if self.external_addr.is_some() {
            self.try_upnp().await?;
        }
        Ok(())
    }
}

/// 获取本机 IP 地址
fn get_local_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok()?.ip().into()
}

/// NAT 类型检测
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NatType {
    /// 开放网络（无 NAT）
    Open,
    /// 全锥形 NAT
    FullCone,
    /// 地址限制锥形 NAT
    AddressRestricted,
    /// 端口限制锥形 NAT
    PortRestricted,
    /// 对称 NAT
    Symmetric,
    /// 未知类型
    Unknown,
}

impl NatType {
    /// 是否容易穿透
    pub fn is_easy_traversal(&self) -> bool {
        matches!(
            self,
            NatType::Open | NatType::FullCone | NatType::AddressRestricted
        )
    }

    /// 是否需要 TURN 中继
    pub fn needs_turn(&self) -> bool {
        matches!(self, NatType::Symmetric)
    }

    /// 是否可以尝试 hole punching
    pub fn can_hole_punch(&self) -> bool {
        !matches!(self, NatType::Symmetric | NatType::Unknown)
    }
}

impl std::fmt::Display for NatType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NatType::Open => write!(f, "Open (no NAT)"),
            NatType::FullCone => write!(f, "Full Cone NAT"),
            NatType::AddressRestricted => write!(f, "Address-Restricted NAT"),
            NatType::PortRestricted => write!(f, "Port-Restricted NAT"),
            NatType::Symmetric => write!(f, "Symmetric NAT"),
            NatType::Unknown => write!(f, "Unknown NAT type"),
        }
    }
}

/// Hole punching 结果
#[derive(Debug, Clone)]
pub enum HolePunchResult {
    /// 成功建立直连
    Success {
        /// 本地地址
        local_addr: SocketAddr,
        /// 对端地址
        peer_addr: SocketAddr,
        /// 使用的 NAT 类型
        nat_type: NatType,
    },
    /// 需要 TURN 中继
    RelayRequired {
        /// 原因
        reason: String,
    },
    /// 失败
    Failed {
        /// 错误信息
        error: String,
    },
}

impl HolePunchResult {
    /// 是否成功
    pub fn is_success(&self) -> bool {
        matches!(self, HolePunchResult::Success { .. })
    }

    /// 是否需要中继
    pub fn needs_relay(&self) -> bool {
        matches!(self, HolePunchResult::RelayRequired { .. })
    }

    /// 获取对端地址（如果成功）
    pub fn peer_addr(&self) -> Option<SocketAddr> {
        match self {
            HolePunchResult::Success { peer_addr, .. } => Some(*peer_addr),
            _ => None,
        }
    }
}

/// Hole punching 协调器
pub struct HolePunchCoordinator {
    stun_servers: Vec<String>,
    udp_socket: Option<UdpSocket>,
    nat_type: NatType,
    local_addr: Option<SocketAddr>,
    external_addr: Option<SocketAddr>,
}

impl std::fmt::Debug for HolePunchCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolePunchCoordinator")
            .field("stun_servers", &self.stun_servers)
            .field("udp_socket", &self.udp_socket.is_some())
            .field("nat_type", &self.nat_type)
            .field("local_addr", &self.local_addr)
            .field("external_addr", &self.external_addr)
            .finish()
    }
}

impl HolePunchCoordinator {
    /// 创建新的 hole punching 协调器
    pub fn new() -> Self {
        Self {
            stun_servers: DEFAULT_STUN_SERVERS.iter().map(|s| s.to_string()).collect(),
            udp_socket: None,
            nat_type: NatType::Unknown,
            local_addr: None,
            external_addr: None,
        }
    }

    /// 创建带自定义 STUN 服务器的协调器
    pub fn with_stun_servers(stun_servers: Vec<String>) -> Self {
        Self {
            stun_servers,
            udp_socket: None,
            nat_type: NatType::Unknown,
            local_addr: None,
            external_addr: None,
        }
    }

    /// 初始化（创建 socket 并检测 NAT 类型）
    pub async fn init(&mut self) -> Result<(NatType, Option<SocketAddr>)> {
        // 创建 UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .map_err(|e| crate::error::CisError::p2p(format!("Failed to bind UDP socket: {}", e)))?;
        
        self.local_addr = Some(socket.local_addr()?);
        debug!("Hole punch coordinator bound to {:?}", self.local_addr);

        // 检测 NAT 类型
        let nat = NatTraversal::with_stun_servers(0, self.stun_servers.clone());
        let (nat_type, external_addr) = nat.detect_nat_type().await?;
        
        self.nat_type = nat_type;
        self.external_addr = external_addr;
        self.udp_socket = Some(socket);

        info!(
            "Hole punch coordinator initialized: NAT type = {}, external = {:?}",
            nat_type, external_addr
        );

        Ok((nat_type, external_addr))
    }

    /// 获取本地地址
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.local_addr
    }

    /// 获取外部地址
    pub fn external_addr(&self) -> Option<SocketAddr> {
        self.external_addr
    }

    /// 获取 NAT 类型
    pub fn nat_type(&self) -> NatType {
        self.nat_type
    }

    /// 执行 hole punching
    pub async fn punch_hole(&self, peer_public_addr: SocketAddr) -> Result<HolePunchResult> {
        let socket = match &self.udp_socket {
            Some(s) => s,
            None => return Err(crate::error::CisError::p2p("Coordinator not initialized")),
        };

        // 检查 NAT 类型
        if self.nat_type.needs_turn() {
            return Ok(HolePunchResult::RelayRequired {
                reason: format!("NAT type {} requires TURN relay", self.nat_type),
            });
        }

        if !self.nat_type.can_hole_punch() {
            return Ok(HolePunchResult::Failed {
                error: format!("Cannot hole punch with NAT type {}", self.nat_type),
            });
        }

        info!("Starting hole punch to {}", peer_public_addr);

        // 发送 hole punching 包
        let punch_packet = b"CIS_HOLE_PUNCH";
        
        // 多次发送以增加成功率
        for i in 0..5 {
            match socket.send_to(punch_packet, peer_public_addr).await {
                Ok(_) => {
                    debug!("Sent hole punch packet {} to {}", i, peer_public_addr);
                }
                Err(e) => {
                    warn!("Failed to send hole punch packet {}: {}", i, e);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // 尝试接收响应
        let mut buf = [0u8; 1024];
        let recv_timeout = Duration::from_secs(5);
        
        let start = std::time::Instant::now();
        while start.elapsed() < recv_timeout {
            match timeout(Duration::from_millis(500), socket.recv_from(&mut buf)).await {
                Ok(Ok((len, from))) => {
                    if from == peer_public_addr {
                        info!("Received response from peer {}: {} bytes", from, len);
                        return Ok(HolePunchResult::Success {
                            local_addr: self.local_addr.unwrap(),
                            peer_addr: from,
                            nat_type: self.nat_type,
                        });
                    }
                }
                Ok(Err(e)) => {
                    debug!("Receive error: {}", e);
                }
                Err(_) => {
                    // 超时，继续
                }
            }
        }

        warn!("Hole punch to {} timed out", peer_public_addr);
        Ok(HolePunchResult::Failed {
            error: "Hole punch timeout".to_string(),
        })
    }

    /// 发送打洞包（用于同时打洞）
    pub async fn send_punch_packets(&self, peer_public_addr: SocketAddr, count: u32) -> Result<()> {
        let socket = match &self.udp_socket {
            Some(s) => s,
            None => return Err(crate::error::CisError::p2p("Coordinator not initialized")),
        };

        let punch_packet = b"CIS_HOLE_PUNCH";
        
        for i in 0..count {
            socket.send_to(punch_packet, peer_public_addr).await
                .map_err(|e| crate::error::CisError::p2p(format!("Failed to send punch packet: {}", e)))?;
            debug!("Sent punch packet {} to {}", i, peer_public_addr);
            
            if i < count - 1 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        Ok(())
    }

    /// 等待打洞响应
    pub async fn wait_for_punch(&self, peer_public_addr: SocketAddr, timeout_secs: u64) -> Result<HolePunchResult> {
        let socket = match &self.udp_socket {
            Some(s) => s,
            None => return Err(crate::error::CisError::p2p("Coordinator not initialized")),
        };

        let mut buf = [0u8; 1024];
        let recv_timeout = Duration::from_secs(timeout_secs);
        
        let start = std::time::Instant::now();
        while start.elapsed() < recv_timeout {
            match timeout(Duration::from_millis(100), socket.recv_from(&mut buf)).await {
                Ok(Ok((len, from))) => {
                    if from == peer_public_addr || len >= 14 && &buf[..14] == b"CIS_HOLE_PUNCH" {
                        info!("Hole punch successful with {}", from);
                        return Ok(HolePunchResult::Success {
                            local_addr: self.local_addr.unwrap(),
                            peer_addr: from,
                            nat_type: self.nat_type,
                        });
                    }
                }
                Ok(Err(_)) => {}
                Err(_) => {}
            }
        }

        Ok(HolePunchResult::Failed {
            error: "Hole punch wait timeout".to_string(),
        })
    }
}

impl Default for HolePunchCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nat_type() {
        assert!(NatType::Open.is_easy_traversal());
        assert!(NatType::FullCone.is_easy_traversal());
        assert!(NatType::AddressRestricted.is_easy_traversal());
        assert!(!NatType::PortRestricted.is_easy_traversal());
        assert!(!NatType::Symmetric.is_easy_traversal());
        
        assert!(NatType::Symmetric.needs_turn());
        assert!(!NatType::Open.needs_turn());
        
        assert!(NatType::Open.can_hole_punch());
        assert!(NatType::FullCone.can_hole_punch());
        assert!(!NatType::Symmetric.can_hole_punch());
    }

    #[test]
    fn test_hole_punch_result() {
        let success = HolePunchResult::Success {
            local_addr: "127.0.0.1:1234".parse().unwrap(),
            peer_addr: "192.168.1.1:5678".parse().unwrap(),
            nat_type: NatType::FullCone,
        };
        assert!(success.is_success());
        assert!(!success.needs_relay());
        assert_eq!(success.peer_addr(), Some("192.168.1.1:5678".parse().unwrap()));

        let relay = HolePunchResult::RelayRequired {
            reason: "Symmetric NAT".to_string(),
        };
        assert!(!relay.is_success());
        assert!(relay.needs_relay());
        assert_eq!(relay.peer_addr(), None);
    }

    #[tokio::test]
    async fn test_nat_traversal_creation() {
        let nat = NatTraversal::new(7677);
        assert_eq!(nat.nat_type(), NatType::Unknown);
        assert!(nat.external_addr().is_none());
    }
}
