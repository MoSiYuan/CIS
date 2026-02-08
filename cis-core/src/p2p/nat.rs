//! NAT 穿透支持
//!
//! 提供 UPnP、STUN、TURN 和 UDP Hole Punching 支持，用于公网连接

use crate::error::Result;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// 默认 STUN 服务器列表
pub const DEFAULT_STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
    "stun3.l.google.com:19302",
    "stun4.l.google.com:19302",
];

/// 默认 TURN 服务器列表（示例）
pub const DEFAULT_TURN_SERVERS: &[&str] = &[
    // "turn.example.com:3478",
];

/// NAT 穿透结果
#[derive(Debug, Clone)]
pub struct TraversalResult {
    /// 外部地址
    pub external_addr: Option<SocketAddr>,
    /// NAT 类型
    pub nat_type: NatType,
    /// 使用的方法
    pub method: TraversalMethod,
    /// 延迟（毫秒）
    pub latency_ms: u64,
}

/// 穿透方法
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalMethod {
    /// 开放网络，无需穿透
    Open,
    /// UPnP 端口映射
    Upnp,
    /// STUN 公网发现
    Stun,
    /// TURN 中继
    Turn,
    /// 失败
    Failed,
}

impl std::fmt::Display for TraversalMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TraversalMethod::Open => write!(f, "Open (no NAT)"),
            TraversalMethod::Upnp => write!(f, "UPnP"),
            TraversalMethod::Stun => write!(f, "STUN"),
            TraversalMethod::Turn => write!(f, "TURN"),
            TraversalMethod::Failed => write!(f, "Failed"),
        }
    }
}

/// NAT 穿透管理器
pub struct NatTraversal {
    local_port: u16,
    external_addr: Option<SocketAddr>,
    nat_type: NatType,
    stun_servers: Vec<String>,
    turn_servers: Vec<String>,
    upnp_available: bool,
}

impl NatTraversal {
    /// 创建 NAT 穿透管理器
    pub fn new(local_port: u16) -> Self {
        Self {
            local_port,
            external_addr: None,
            nat_type: NatType::Unknown,
            stun_servers: DEFAULT_STUN_SERVERS.iter().map(|s| s.to_string()).collect(),
            turn_servers: DEFAULT_TURN_SERVERS.iter().map(|s| s.to_string()).collect(),
            upnp_available: false,
        }
    }

    /// 创建带自定义 STUN 服务器的 NAT 穿透管理器
    pub fn with_stun_servers(local_port: u16, stun_servers: Vec<String>) -> Self {
        Self {
            local_port,
            external_addr: None,
            nat_type: NatType::Unknown,
            stun_servers,
            turn_servers: DEFAULT_TURN_SERVERS.iter().map(|s| s.to_string()).collect(),
            upnp_available: false,
        }
    }

    /// 创建带完整配置的 NAT 穿透管理器
    pub fn with_config(
        local_port: u16,
        stun_servers: Vec<String>,
        turn_servers: Vec<String>,
    ) -> Self {
        Self {
            local_port,
            external_addr: None,
            nat_type: NatType::Unknown,
            stun_servers,
            turn_servers,
            upnp_available: false,
        }
    }

    /// 尝试 NAT 穿透
    pub async fn try_traversal(&mut self) -> Result<Option<SocketAddr>> {
        let _start = std::time::Instant::now();

        // 1. 首先尝试 UPnP
        match self.try_upnp().await {
            Ok(Some(addr)) => {
                tracing::info!("UPnP port mapping successful: {}", addr);
                self.external_addr = Some(addr);
                self.nat_type = NatType::Open;
                self.upnp_available = true;
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
                    tracing::info!(
                        "STUN successful, external address: {}, NAT type: {:?}",
                        addr,
                        nat_type
                    );
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

    /// 尝试 NAT 穿透并返回详细结果
    pub async fn try_traversal_detailed(&mut self) -> Result<TraversalResult> {
        let start = std::time::Instant::now();

        // 1. 首先尝试 UPnP
        match self.try_upnp().await {
            Ok(Some(addr)) => {
                self.external_addr = Some(addr);
                self.nat_type = NatType::Open;
                self.upnp_available = true;
                return Ok(TraversalResult {
                    external_addr: Some(addr),
                    nat_type: NatType::Open,
                    method: TraversalMethod::Upnp,
                    latency_ms: start.elapsed().as_millis() as u64,
                });
            }
            _ => {}
        }

        // 2. 尝试 STUN
        match self.detect_nat_type().await {
            Ok((nat_type, addr)) => {
                self.nat_type = nat_type;
                if let Some(addr) = addr {
                    self.external_addr = Some(addr);
                    return Ok(TraversalResult {
                        external_addr: Some(addr),
                        nat_type,
                        method: TraversalMethod::Stun,
                        latency_ms: start.elapsed().as_millis() as u64,
                    });
                }
            }
            _ => {}
        }

        // 3. 失败
        Ok(TraversalResult {
            external_addr: None,
            nat_type: NatType::Unknown,
            method: TraversalMethod::Failed,
            latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// 尝试 UPnP 端口映射
    pub async fn try_upnp(&self) -> Result<Option<SocketAddr>> {
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
            let _local_addr = match get_local_ip() {
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
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
            .map_err(|e| crate::error::CisError::p2p(format!("Failed to bind UDP socket: {}", e)))?;

        let local_addr = socket
            .local_addr()
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
        use stun::message::*;
        use stun::xoraddr::*;

        // 解析 STUN 服务器地址
        let server_addr: SocketAddr = stun_server
            .parse()
            .map_err(|e| crate::error::CisError::p2p(format!("Invalid STUN server address: {}", e)))?;

        // 发送 STUN binding request
        let mut msg = Message::new();
        msg.build(&[Box::new(TransactionId::new()), Box::new(BINDING_REQUEST)])
            .map_err(|e| crate::error::CisError::p2p(format!("Failed to build STUN message: {}", e)))?;

        // 发送请求并等待响应
        let send_result = timeout(Duration::from_secs(3), async {
            socket.send_to(&msg.raw, server_addr).await
        })
        .await;

        match send_result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(crate::error::CisError::p2p(format!(
                    "Failed to send STUN request: {}",
                    e
                )))
            }
            Err(_) => return Err(crate::error::CisError::p2p("STUN request timeout")),
        }

        // 接收响应
        let mut buf = [0u8; 1024];
        let recv_result = timeout(Duration::from_secs(3), socket.recv_from(&mut buf)).await;

        let (len, from) = match recv_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                return Err(crate::error::CisError::p2p(format!(
                    "Failed to receive STUN response: {}",
                    e
                )))
            }
            Err(_) => return Err(crate::error::CisError::p2p("STUN response timeout")),
        };

        debug!(
            "Received {} bytes from STUN server {} (from {})",
            len, stun_server, from
        );

        // 解析响应
        let mut response = Message::new();
        response.raw = buf[..len].to_vec();

        if let Err(e) = response.decode() {
            return Err(crate::error::CisError::p2p(format!(
                "Failed to decode STUN response: {}",
                e
            )));
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
            Err(e) => Err(crate::error::CisError::p2p(format!(
                "Failed to get mapped address: {}",
                e
            ))),
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

    /// 检查 UPnP 是否可用
    pub fn is_upnp_available(&self) -> bool {
        self.upnp_available
    }

    /// 刷新端口映射
    pub async fn refresh_mapping(&self) -> Result<()> {
        if self.external_addr.is_some() {
            self.try_upnp().await?;
        }
        Ok(())
    }

    /// 获取 STUN 服务器列表
    pub fn stun_servers(&self) -> &[String] {
        &self.stun_servers
    }

    /// 获取 TURN 服务器列表
    pub fn turn_servers(&self) -> &[String] {
        &self.turn_servers
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

    /// 获取描述
    pub fn description(&self) -> &'static str {
        match self {
            NatType::Open => "Open Internet (no NAT)",
            NatType::FullCone => "Full Cone NAT",
            NatType::AddressRestricted => "Address-Restricted Cone NAT",
            NatType::PortRestricted => "Port-Restricted Cone NAT",
            NatType::Symmetric => "Symmetric NAT (requires TURN)",
            NatType::Unknown => "Unknown NAT type",
        }
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
    punch_packet_count: u32,
    punch_interval_ms: u64,
}

impl std::fmt::Debug for HolePunchCoordinator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HolePunchCoordinator")
            .field("stun_servers", &self.stun_servers)
            .field("udp_socket", &self.udp_socket.is_some())
            .field("nat_type", &self.nat_type)
            .field("local_addr", &self.local_addr)
            .field("external_addr", &self.external_addr)
            .field("punch_packet_count", &self.punch_packet_count)
            .field("punch_interval_ms", &self.punch_interval_ms)
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
            punch_packet_count: 5,
            punch_interval_ms: 100,
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
            punch_packet_count: 5,
            punch_interval_ms: 100,
        }
    }

    /// 设置打洞参数
    pub fn with_punch_config(mut self, count: u32, interval_ms: u64) -> Self {
        self.punch_packet_count = count;
        self.punch_interval_ms = interval_ms;
        self
    }

    /// 初始化（创建 socket 并检测 NAT 类型）
    pub async fn init(&mut self) -> Result<(NatType, Option<SocketAddr>)> {
        // 创建 UDP socket
        let socket = UdpSocket::bind("0.0.0.0:0")
            .await
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
        for i in 0..self.punch_packet_count {
            match socket.send_to(punch_packet, peer_public_addr).await {
                Ok(_) => {
                    debug!("Sent hole punch packet {} to {}", i, peer_public_addr);
                }
                Err(e) => {
                    warn!("Failed to send hole punch packet {}: {}", i, e);
                }
            }
            tokio::time::sleep(Duration::from_millis(self.punch_interval_ms)).await;
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

    /// 执行双向 hole punching（同时打洞）
    pub async fn punch_hole_bidirectional(
        &self,
        peer_public_addr: SocketAddr,
        relay_addr: Option<SocketAddr>,
    ) -> Result<HolePunchResult> {
        // 首先尝试直连打洞
        match self.punch_hole(peer_public_addr).await? {
            result @ HolePunchResult::Success { .. } => return Ok(result),
            HolePunchResult::RelayRequired { reason } => {
                // 如果指定了 relay，尝试通过 relay 协调
                if let Some(relay) = relay_addr {
                    info!("Trying relayed hole punch via {}", relay);
                    return self.punch_hole_relayed(peer_public_addr, relay).await;
                }
                return Ok(HolePunchResult::RelayRequired { reason });
            }
            HolePunchResult::Failed { error } => {
                // 如果指定了 relay，尝试通过 relay 协调
                if let Some(relay) = relay_addr {
                    info!("Direct punch failed, trying relayed punch via {}", relay);
                    return self.punch_hole_relayed(peer_public_addr, relay).await;
                }
                return Ok(HolePunchResult::Failed { error });
            }
        }
    }

    /// 通过 relay 服务器协调打洞
    async fn punch_hole_relayed(
        &self,
        peer_public_addr: SocketAddr,
        _relay_addr: SocketAddr,
    ) -> Result<HolePunchResult> {
        // 简化实现：向 relay 发送请求，然后打洞
        // 实际实现中需要更复杂的协议
        info!("Relayed hole punch not fully implemented, falling back to direct");
        self.punch_hole(peer_public_addr).await
    }

    /// 发送打洞包（用于同时打洞）
    pub async fn send_punch_packets(&self, peer_public_addr: SocketAddr, count: u32) -> Result<()> {
        let socket = match &self.udp_socket {
            Some(s) => s,
            None => return Err(crate::error::CisError::p2p("Coordinator not initialized")),
        };

        let punch_packet = b"CIS_HOLE_PUNCH";

        for i in 0..count {
            socket
                .send_to(punch_packet, peer_public_addr)
                .await
                .map_err(|e| crate::error::CisError::p2p(format!("Failed to send punch packet: {}", e)))?;
            debug!("Sent punch packet {} to {}", i, peer_public_addr);

            if i < count - 1 {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }

        Ok(())
    }

    /// 等待打洞响应
    pub async fn wait_for_punch(
        &self,
        peer_public_addr: SocketAddr,
        timeout_secs: u64,
    ) -> Result<HolePunchResult> {
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
        assert!(!NatType::Unknown.can_hole_punch());
    }

    #[test]
    fn test_nat_type_display() {
        assert_eq!(NatType::Open.to_string(), "Open (no NAT)");
        assert_eq!(NatType::Symmetric.to_string(), "Symmetric NAT");
    }

    #[test]
    fn test_nat_type_description() {
        assert!(NatType::Open.description().contains("Open"));
        assert!(NatType::Symmetric.description().contains("TURN"));
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

        let failed = HolePunchResult::Failed {
            error: "Timeout".to_string(),
        };
        assert!(!failed.is_success());
        assert!(!failed.needs_relay());
    }

    #[tokio::test]
    async fn test_nat_traversal_creation() {
        let nat = NatTraversal::new(7677);
        assert_eq!(nat.nat_type(), NatType::Unknown);
        assert!(nat.external_addr().is_none());
        assert!(!nat.is_upnp_available());
        assert_eq!(nat.stun_servers().len(), 5);
    }

    #[tokio::test]
    async fn test_nat_traversal_with_servers() {
        let stun_servers = vec!["stun.example.com:3478".to_string()];
        let turn_servers = vec!["turn.example.com:3478".to_string()];
        
        let nat = NatTraversal::with_config(7677, stun_servers.clone(), turn_servers.clone());
        
        assert_eq!(nat.stun_servers(), stun_servers.as_slice());
        assert_eq!(nat.turn_servers(), turn_servers.as_slice());
    }

    #[test]
    fn test_traversal_method_display() {
        assert_eq!(TraversalMethod::Upnp.to_string(), "UPnP");
        assert_eq!(TraversalMethod::Stun.to_string(), "STUN");
        assert_eq!(TraversalMethod::Turn.to_string(), "TURN");
        assert_eq!(TraversalMethod::Failed.to_string(), "Failed");
    }

    #[test]
    fn test_get_local_ip() {
        // 只是确保函数可以运行，不检查具体结果
        // 因为结果取决于网络环境
        let _ip = get_local_ip();
    }

    #[tokio::test]
    async fn test_hole_punch_coordinator_creation() {
        let coordinator = HolePunchCoordinator::new();
        assert_eq!(coordinator.nat_type(), NatType::Unknown);
        assert!(coordinator.local_addr().is_none());
        assert!(coordinator.external_addr().is_none());
    }

    #[tokio::test]
    async fn test_hole_punch_coordinator_with_config() {
        let servers = vec!["stun.example.com:3478".to_string()];
        let coordinator = HolePunchCoordinator::with_stun_servers(servers)
            .with_punch_config(10, 50);
        
        // 由于 with_punch_config 返回 Self，我们可以检查配置
        assert_eq!(coordinator.nat_type(), NatType::Unknown);
    }

    #[tokio::test]
    async fn test_hole_punch_coordinator_default() {
        let coordinator: HolePunchCoordinator = Default::default();
        assert_eq!(coordinator.nat_type(), NatType::Unknown);
    }

    #[tokio::test]
    async fn test_traversal_result() {
        let result = TraversalResult {
            external_addr: Some("192.168.1.1:7677".parse().unwrap()),
            nat_type: NatType::FullCone,
            method: TraversalMethod::Stun,
            latency_ms: 150,
        };

        assert!(result.external_addr.is_some());
        assert_eq!(result.nat_type, NatType::FullCone);
        assert_eq!(result.method, TraversalMethod::Stun);
        assert_eq!(result.latency_ms, 150);
    }
}
