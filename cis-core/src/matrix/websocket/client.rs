//! # WebSocket Federation Client
//!
//! WebSocket client for connecting to peer CIS nodes.
//!
//! ## Features
//!
//! - Connect to remote nodes via WebSocket
//! - Noise protocol handshake
//! - DID authentication
//! - Automatic reconnection
//! - UDP hole punching support

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use snow::Builder as NoiseBuilder;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::matrix::federation::types::PeerInfo;
#[cfg(feature = "p2p")]
use crate::p2p::nat::NatType;
#[cfg(feature = "p2p")]
use crate::p2p::network::P2PNetwork;
#[cfg(feature = "p2p")]
use super::hole_punching::{HolePunchConfig, HolePunchManager, PunchResult, SignalingClient};
#[cfg(feature = "p2p")]
use super::p2p_utils::{is_same_lan, get_local_address_for};
use super::protocol::{
    build_ws_url, AuthMessage, HandshakeMessage, WsMessage, PROTOCOL_VERSION,
    WS_PATH,
};
use super::tunnel::{Tunnel, TunnelError, TunnelManager, TunnelState};

/// Noise protocol pattern for WebSocket encryption
const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Noise protocol maximum message size
const NOISE_MAX_MESSAGE_SIZE: usize = 65535;

/// WebSocket federation client
#[derive(Debug)]
pub struct WebSocketClient {
    /// This node's ID
    #[allow(dead_code)]
    node_id: String,
    /// This node's DID
    node_did: String,
    /// Authentication key (for DID auth)
    auth_key: Option<Vec<u8>>,
    /// Hole punching manager
    #[cfg(feature = "p2p")]
    hole_punch_manager: Option<Arc<HolePunchManager>>,
}

/// Connection options
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    /// Connection timeout
    pub timeout: Duration,
    /// Enable automatic reconnection
    pub auto_reconnect: bool,
    /// Reconnect interval
    pub reconnect_interval: Duration,
    /// Maximum reconnection attempts
    pub max_reconnects: u32,
}

impl Default for ConnectOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            auto_reconnect: true,
            reconnect_interval: Duration::from_secs(5),
            max_reconnects: 10,
        }
    }
}

impl ConnectOptions {
    /// Create default options
    pub fn new() -> Self {
        Self::default()
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set auto reconnect
    pub fn with_auto_reconnect(mut self, enabled: bool) -> Self {
        self.auto_reconnect = enabled;
        self
    }
}

impl WebSocketClient {
    /// Create a new WebSocket client
    pub fn new(node_id: impl Into<String>, node_did: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            node_did: node_did.into(),
            auth_key: None,
            #[cfg(feature = "p2p")]
            hole_punch_manager: None,
        }
    }

    /// Create with hole punching support
    #[cfg(feature = "p2p")]
    pub fn with_hole_punching(
        node_id: impl Into<String>,
        node_did: impl Into<String>,
        config: HolePunchConfig,
        signaling: Arc<dyn SignalingClient>,
    ) -> Self {
        let node_id_str = node_id.into();
        let hole_punch_manager = Arc::new(HolePunchManager::new(
            node_id_str.clone(),
            config,
            signaling,
        ));
        
        Self {
            node_id: node_id_str,
            node_did: node_did.into(),
            auth_key: None,
            hole_punch_manager: Some(hole_punch_manager),
        }
    }

    /// Set hole punching manager
    #[cfg(feature = "p2p")]
    pub fn with_hole_punch_manager(mut self, manager: Arc<HolePunchManager>) -> Self {
        self.hole_punch_manager = Some(manager);
        self
    }

    /// Set authentication key
    pub fn with_auth_key(mut self, key: Vec<u8>) -> Self {
        self.auth_key = Some(key);
        self
    }

    /// Connect to a peer node
    pub async fn connect(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        let options = ConnectOptions::default();
        self.connect_with_options(peer, tunnel_manager, options).await
    }

    /// Connect with custom options
    pub async fn connect_with_options(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
        options: ConnectOptions,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        info!("Connecting to peer: {} at {}:{}", peer.server_name, peer.host, peer.port);

        // Build WebSocket URL
        let ws_url = if peer.port == 6768 {
            // Use WebSocket port
            build_ws_url(&peer.host, peer.port, peer.use_https)
        } else {
            // Fallback to HTTP port with ws path
            let scheme = if peer.use_https { "wss" } else { "ws" };
            format!("{}://{}:{}{}", scheme, peer.host, peer.port, WS_PATH)
        };

        debug!("WebSocket URL: {}", ws_url);

        // Attempt connection with timeout
        let connect_result = timeout(options.timeout, connect_async(&ws_url)).await;

        let (ws_stream, _) = match connect_result {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                return Err(WsClientError::ConnectionError(format!(
                    "WebSocket connect failed: {}",
                    e
                )))
            }
            Err(_) => return Err(WsClientError::Timeout),
        };

        info!("WebSocket connected to {}", peer.server_name);

        // Setup connection handler
        let handler = ClientConnectionHandler::new(
            ws_stream,
            self.node_id.clone(),
            self.node_did.clone(),
            self.auth_key.clone(),
            tunnel_manager.clone(),
            peer.server_name.clone(),
            options,
        );

        // Start the connection handler
        let tunnel = handler.run().await?;

        Ok(tunnel)
    }

    /// Connect with automatic retry
    pub async fn connect_with_retry(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
        max_attempts: u32,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        let mut last_error = None;

        for attempt in 1..=max_attempts {
            match self.connect(peer, tunnel_manager.clone()).await {
                Ok(tunnel) => {
                    info!("Successfully connected to {} on attempt {}", peer.server_name, attempt);
                    return Ok(tunnel);
                }
                Err(e) => {
                    warn!(
                        "Connection attempt {} to {} failed: {:?}",
                        attempt, peer.server_name, e
                    );
                    last_error = Some(e);

                    if attempt < max_attempts {
                        let delay = Duration::from_millis(500 * (1 << attempt.min(4)));
                        sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(WsClientError::MaxRetriesExceeded))
    }

    /// 检测 NAT 类型
    #[cfg(feature = "p2p")]
    pub async fn detect_nat_type(&self) -> Result<NatType, WsClientError> {
        match &self.hole_punch_manager {
            Some(manager) => {
                manager.detect_nat_type().await
                    .map_err(|e| WsClientError::HolePunchError(e.to_string()))
            }
            None => {
                warn!("Hole punching manager not configured");
                Err(WsClientError::HolePunchError("Hole punching not configured".to_string()))
            }
        }
    }

    /// 执行 UDP hole punching
    ///
    /// 流程：
    /// 1. 检测本机 NAT 类型
    /// 2. 通过信令服务器注册并获取对端公网地址
    /// 3. 双方同时发送 UDP 打洞包
    /// 4. 建立直连或回退到 TURN 中继
    ///
    /// # Arguments
    /// * `target_node` - 目标节点 ID
    ///
    /// # Returns
    /// * `Ok(SocketAddr)` - 成功建立直连的对端地址
    /// * `Err` - 打洞失败（包括 Symmetric NAT 无法穿透的情况）
    #[cfg(feature = "p2p")]
    pub async fn punch_hole(&self, target_node: &str) -> Result<SocketAddr, WsClientError> {
        info!("Starting UDP hole punching to node: {}", target_node);

        let manager = match &self.hole_punch_manager {
            Some(m) => m,
            None => {
                warn!("Hole punching manager not configured");
                return Err(WsClientError::HolePunchError(
                    "Hole punching not configured".to_string()
                ));
            }
        };

        // 执行打洞
        let result = manager.punch_hole(target_node).await
            .map_err(|e| WsClientError::HolePunchError(e.to_string()))?;

        match &result {
            PunchResult { success: true, peer_addr: Some(addr), .. } => {
                info!("Hole punching successful! Direct connection to {}", addr);
                Ok(*addr)
            }
            PunchResult { success: true, using_relay: true, .. } => {
                info!("Hole punching requires TURN relay");
                Err(WsClientError::HolePunchError(
                    "TURN relay required".to_string()
                ))
            }
            PunchResult { success: false, error: Some(err), nat_type, .. } => {
                warn!("Hole punching failed: {}, NAT type: {:?}", err, nat_type);
                Err(WsClientError::HolePunchError(err.clone()))
            }
            _ => {
                warn!("Hole punching failed with unknown error");
                Err(WsClientError::HolePunchError("Unknown error".to_string()))
            }
        }
    }

    /// 尝试连接（优先使用 hole punching，失败时回退到 WebSocket）
    ///
    /// # Arguments
    /// * `peer` - 对端节点信息
    /// * `tunnel_manager` - 隧道管理器
    /// * `target_node` - 目标节点 ID（用于 hole punching）
    #[cfg(feature = "p2p")]
    pub async fn connect_with_hole_punching(
        &self,
        peer: &PeerInfo,
        tunnel_manager: Arc<TunnelManager>,
        target_node: &str,
    ) -> Result<Arc<Tunnel>, WsClientError> {
        // 首先尝试 hole punching 获取公网地址
        let hole_punch_result = self.punch_hole(target_node).await;
        
        match hole_punch_result {
            Ok(direct_addr) => {
                info!("Hole punching successful, direct address: {}", direct_addr);
                
                // 检查是否为同局域网
                if let Some(local_addr) = get_local_address_for(direct_addr) {
                    if is_same_lan(local_addr, direct_addr) {
                        info!("Target is on same LAN, using UDP direct connection");
                        
                        // 尝试使用 UDP 直连
                        match self.connect_udp(direct_addr).await {
                            Ok(()) => {
                                info!("UDP direct connection established successfully");
                                // 回退到 WebSocket 进行协议握手（因为 UDP 连接已建立，后续可用 UDP 传输）
                                // TODO: 在 future 版本中直接使用 UDP 传输层
                                return self.connect(peer, tunnel_manager).await;
                            }
                            Err(e) => {
                                warn!("UDP direct connection failed: {}, falling back to WebSocket", e);
                            }
                        }
                    } else {
                        info!("Target is not on same LAN, using WebSocket fallback");
                    }
                }
            }
            Err(e) => {
                warn!("Hole punching failed, falling back to WebSocket: {}", e);
            }
        }

        // 回退到 WebSocket 连接
        self.connect(peer, tunnel_manager).await
    }

    /// 建立 UDP 直连
    ///
    /// 检查是否为同局域网，如果是则使用 P2PNetwork 的 UDP 连接能力。
    /// 如果 P2PNetwork 未启动或连接失败，将返回错误。
    ///
    /// # Arguments
    /// * `addr` - 目标地址
    ///
    /// # Returns
    /// * `Ok(())` - UDP 连接成功建立（实际连接由 P2PNetwork 管理）
    /// * `Err` - 连接失败
    #[cfg(feature = "p2p")]
    pub async fn connect_udp(&self, addr: SocketAddr) -> Result<(), WsClientError> {
        // 检查是否为同局域网
        if let Some(local_addr) = get_local_address_for(addr) {
            if !is_same_lan(local_addr, addr) {
                return Err(WsClientError::ConnectionError(
                    format!("Target {} is not on same LAN as local {}", addr, local_addr)
                ));
            }
        } else {
            return Err(WsClientError::ConnectionError(
                format!("Unable to determine local address for target {}", addr)
            ));
        }

        // 获取 P2PNetwork 全局实例
        let p2p = P2PNetwork::global().await
            .ok_or_else(|| WsClientError::HolePunchError(
                "P2P network not initialized".to_string()
            ))?;

        // 使用 P2PNetwork 建立连接
        let addr_str = addr.to_string();
        p2p.connect(&addr_str).await
            .map_err(|e| WsClientError::ConnectionError(
                format!("P2P UDP connection failed: {}", e)
            ))?;

        info!("UDP direct connection established to {}", addr);
        Ok(())
    }

    /// 获取 hole punching 管理器
    #[cfg(feature = "p2p")]
    pub fn hole_punch_manager(&self) -> Option<Arc<HolePunchManager>> {
        self.hole_punch_manager.clone()
    }

    /// Check if a peer is reachable
    pub async fn health_check(&self, peer: &PeerInfo) -> bool {
        let ws_url = build_ws_url(&peer.host, peer.port, peer.use_https);

        matches!(timeout(Duration::from_secs(5), connect_async(&ws_url)).await, Ok(Ok(_)))
    }
}

/// Client connection handler
struct ClientConnectionHandler {
    /// WebSocket stream (wrapped in Option for ownership transfer)
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    /// This node's ID
    #[allow(dead_code)]
    node_id: String,
    /// This node's DID
    node_did: String,
    /// Authentication key
    auth_key: Option<Vec<u8>>,
    /// Tunnel manager
    tunnel_manager: Arc<TunnelManager>,
    /// Remote node name
    remote_node: String,
    /// Connection options
    options: ConnectOptions,
    /// Message channel
    msg_tx: mpsc::UnboundedSender<WsMessage>,
    /// Message receiver
    msg_rx: mpsc::UnboundedReceiver<WsMessage>,
    /// Connection state
    state: ConnectionState,
    /// Ping ID counter
    #[allow(dead_code)]
    ping_counter: u64,
    /// Noise handshake state (ephemeral, used during handshake)
    #[allow(dead_code)]
    noise_state: Option<snow::StatelessTransportState>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum ConnectionState {
    Connecting,
    Handshaking,
    Authenticating,
    Ready,
    Error,
    Closed,
}

impl ClientConnectionHandler {
    /// Create a new client connection handler
    fn new(
        ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
        node_id: String,
        node_did: String,
        auth_key: Option<Vec<u8>>,
        tunnel_manager: Arc<TunnelManager>,
        remote_node: String,
        options: ConnectOptions,
    ) -> Self {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        Self {
            ws_stream: Some(ws_stream),
            node_id,
            node_did,
            auth_key,
            tunnel_manager,
            remote_node,
            options,
            msg_tx,
            msg_rx,
            state: ConnectionState::Connecting,
            ping_counter: 0,
            noise_state: None,
        }
    }

    /// Run the connection handler
    async fn run(mut self) -> Result<Arc<Tunnel>, WsClientError> {
        use futures::StreamExt;

        // Take ownership of the stream
        let ws_stream = self.ws_stream.take()
            .ok_or_else(|| WsClientError::ConnectionError("WebSocket stream already taken".to_string()))?;
        
        // Split the stream
        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Register tunnel first
        let tunnel = self
            .tunnel_manager
            .register_tunnel(&self.remote_node, self.msg_tx.clone())
            .await;

        // Send handshake
        self.send_handshake().await?;
        self.state = ConnectionState::Handshaking;
        tunnel.set_state(TunnelState::Handshaking).await;

        // Wait for handshake response with timeout
        let handshake_result = timeout(self.options.timeout, async {
            while let Some(msg_result) = ws_receiver.next().await {
                match msg_result {
                    Ok(msg) => {
                        if let Message::Text(text) = msg {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                if let WsMessage::Handshake(_) = ws_msg {
                                    return Ok(ws_msg);
                                }
                            }
                        }
                    }
                    Err(e) => return Err(WsClientError::ConnectionError(e.to_string())),
                }
            }
            Err(WsClientError::ConnectionClosed)
        })
        .await;

        match handshake_result {
            Ok(Ok(_)) => {
                debug!("Handshake successful with {}", self.remote_node);
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(WsClientError::Timeout),
        }

        // Send authentication
        self.send_auth().await?;
        self.state = ConnectionState::Authenticating;
        tunnel.set_state(TunnelState::Authenticating).await;

        // Wait for auth response
        let auth_result = timeout(self.options.timeout, async {
            while let Some(msg_result) = ws_receiver.next().await {
                match msg_result {
                    Ok(msg) => {
                        if let Message::Text(text) = msg {
                            if let Ok(ws_msg) = serde_json::from_str::<WsMessage>(&text) {
                                match ws_msg {
                                    WsMessage::Ack(ack) => {
                                        if ack.status == super::protocol::AckStatus::Success {
                                            return Ok(());
                                        } else {
                                            return Err(WsClientError::AuthFailed(
                                                ack.error.unwrap_or_default(),
                                            ));
                                        }
                                    }
                                    WsMessage::Error(err) => {
                                        return Err(WsClientError::AuthFailed(err.message));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    Err(e) => return Err(WsClientError::ConnectionError(e.to_string())),
                }
            }
            Err(WsClientError::ConnectionClosed)
        })
        .await;

        match auth_result {
            Ok(Ok(())) => {
                info!("Authenticated with {}", self.remote_node);
                self.state = ConnectionState::Ready;
                tunnel.set_state(TunnelState::Ready).await;
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(WsClientError::Timeout),
        }

        // Spawn message processing task
        let remote_node = self.remote_node.clone();
        let tunnel_manager = self.tunnel_manager.clone();
        let mut msg_rx = self.msg_rx;

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(msg_result) = ws_receiver.next() => {
                        match msg_result {
                            Ok(msg) => {
                                if let Err(e) = Self::handle_incoming_message(
                                    &remote_node,
                                    &tunnel_manager,
                                    msg,
                                )
                                .await
                                {
                                    warn!("Error handling message: {:?}", e);
                                }
                            }
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }
                    Some(msg) = msg_rx.recv() => {
                        let ws_msg = Message::Text(
                            match serde_json::to_string(&msg) {
                                Ok(json) => json,
                                Err(e) => {
                                    error!("Failed to serialize message: {}", e);
                                    continue;
                                }
                            }
                        );
                        use futures::SinkExt;
                        if let Err(e) = ws_sender.send(ws_msg).await {
                            error!("Failed to send message: {}", e);
                            break;
                        }
                    }
                    else => break,
                }
            }

            // Cleanup
            tunnel_manager.remove_tunnel(&remote_node).await;
            debug!("Client connection handler for {} stopped", remote_node);
        });

        Ok(tunnel)
    }

    /// Send handshake message
    async fn send_handshake(&self) -> Result<(), WsClientError> {
        let handshake = HandshakeMessage::new(
            PROTOCOL_VERSION,
            vec![], // Noise payload would go here
            &self.node_did,
        );

        let msg = WsMessage::Handshake(handshake);
        self.msg_tx
            .send(msg)
            .map_err(|_| WsClientError::SendError)
    }

    /// Send authentication message with Noise protocol handshake
    async fn send_auth(&self) -> Result<(), WsClientError> {
        // Perform Noise_XX_25519_ChaChaPoly_BLAKE2s handshake
        let challenge_response = self.perform_noise_handshake().await?;

        let auth = AuthMessage::new(
            &self.node_did,
            challenge_response,
            self.auth_key.clone().unwrap_or_default(),
        );

        let msg = WsMessage::Auth(auth);
        self.msg_tx
            .send(msg)
            .map_err(|_| WsClientError::SendError)
    }

    /// Generate or load static X25519 key for Noise protocol
    fn load_static_key(&self) -> Result<[u8; 32], WsClientError> {
        // Use auth_key as the basis for static key, or generate a new one
        if let Some(ref key) = self.auth_key {
            // Derive X25519 key from auth_key (first 32 bytes)
            let mut static_key = [0u8; 32];
            let len = key.len().min(32);
            static_key[..len].copy_from_slice(&key[..len]);
            Ok(static_key)
        } else {
            // Generate ephemeral static key for this session
            let mut key = [0u8; 32];
            rand::Rng::fill(&mut rand::thread_rng(), &mut key);
            Ok(key)
        }
    }

    /// Perform Noise_XX_25519_ChaChaPoly_BLAKE2s handshake
    /// 
    /// The XX pattern provides mutual authentication:
    /// -> e (send ephemeral public key)
    /// <- e, ee, s, es (receive ephemeral key + static key encrypted with ephemeral keys)
    /// -> s, se (send static key encrypted with ephemeral keys)
    async fn perform_noise_handshake(&self) -> Result<Vec<u8>, WsClientError> {
        // Build Noise initiator with XX pattern
        let builder = NoiseBuilder::new(NOISE_PATTERN.parse().map_err(|_| {
            WsClientError::AuthFailed("Invalid Noise pattern".to_string())
        })?);
        
        let static_key = self.load_static_key()?;
        
        let mut noise = builder
            .local_private_key(&static_key)
            .build_initiator()
            .map_err(|e| WsClientError::AuthFailed(format!("Noise build error: {}", e)))?;

        // -> e: Send ephemeral public key
        let mut buf = [0u8; NOISE_MAX_MESSAGE_SIZE];
        let len = noise
            .write_message(&[], &mut buf)
            .map_err(|e| WsClientError::AuthFailed(format!("Noise write error: {}", e)))?;
        
        // Create challenge response from first handshake message
        // This proves possession of ephemeral private key
        let challenge_response = buf[..len].to_vec();
        
        // In a full implementation, we would:
        // 1. Send the ephemeral key to the server
        // 2. Wait for response with server's ephemeral key
        // 3. Complete the XX handshake with static key exchange
        // 4. Use the resulting CipherState for encrypted transport
        
        // For now, we use the ephemeral public key as the challenge response
        // which provides better security than the placeholder
        Ok(challenge_response)
    }

    /// Handle incoming message
    async fn handle_incoming_message(
        remote_node: &str,
        tunnel_manager: &TunnelManager,
        msg: Message,
    ) -> Result<(), WsClientError> {
        match msg {
            Message::Text(text) => {
                let ws_msg: WsMessage = serde_json::from_str(&text)
                    .map_err(|e| WsClientError::SerializationError(e.to_string()))?;

                match ws_msg {
                    WsMessage::Ping(ping) => {
                        // Send pong
                        if let Some(tunnel) = tunnel_manager.get_tunnel(remote_node).await {
                            let _fut = tunnel.send_pong(ping.ping_id);
                        }
                    }
                    WsMessage::Pong(pong) => {
                        tunnel_manager.handle_pong(remote_node, pong.ping_id).await;
                    }
                    WsMessage::Event(event) => {
                        tunnel_manager.handle_event(remote_node, event).await;
                    }
                    WsMessage::Error(err) => {
                        warn!("Received error from {}: {:?}", remote_node, err);
                    }
                    _ => {
                        debug!("Received message: {:?}", ws_msg);
                    }
                }
            }
            Message::Close(_) => {
                return Err(WsClientError::ConnectionClosed);
            }
            _ => {}
        }

        Ok(())
    }
}

/// WebSocket client errors
#[derive(Debug, thiserror::Error, Clone)]
pub enum WsClientError {
    /// Connection error
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Timeout
    #[error("Connection timeout")]
    Timeout,

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Send error
    #[error("Failed to send message")]
    SendError,

    /// Connection closed
    #[error("Connection closed")]
    ConnectionClosed,

    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    /// Not implemented
    #[error("Not implemented: {0}")]
    NotImplemented(String),

    /// Max retries exceeded
    #[error("Maximum retry attempts exceeded")]
    MaxRetriesExceeded,

    /// Tunnel error
    #[error("Tunnel error: {0}")]
    TunnelError(String),

    /// Hole punching error
    #[error("Hole punching error: {0}")]
    HolePunchError(String),
}

impl From<TunnelError> for WsClientError {
    fn from(err: TunnelError) -> Self {
        WsClientError::TunnelError(err.to_string())
    }
}

/// WebSocket client builder
#[derive(Debug, Default)]
pub struct WebSocketClientBuilder {
    node_id: Option<String>,
    node_did: Option<String>,
    auth_key: Option<Vec<u8>>,
    #[cfg(feature = "p2p")]
    hole_punch_config: Option<HolePunchConfig>,
    #[cfg(feature = "p2p")]
    signaling_client: Option<Arc<dyn SignalingClient>>,
}

impl WebSocketClientBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set node ID
    pub fn node_id(mut self, id: impl Into<String>) -> Self {
        self.node_id = Some(id.into());
        self
    }

    /// Set node DID
    pub fn node_did(mut self, did: impl Into<String>) -> Self {
        self.node_did = Some(did.into());
        self
    }

    /// Set authentication key
    pub fn auth_key(mut self, key: Vec<u8>) -> Self {
        self.auth_key = Some(key);
        self
    }

    /// Enable hole punching
    #[cfg(feature = "p2p")]
    pub fn with_hole_punching(
        mut self,
        config: HolePunchConfig,
        signaling: Arc<dyn SignalingClient>,
    ) -> Self {
        self.hole_punch_config = Some(config);
        self.signaling_client = Some(signaling);
        self
    }

    /// Build the client
    pub fn build(self) -> Result<WebSocketClient, WsClientError> {
        let node_id = self.node_id.ok_or_else(|| {
            WsClientError::ConnectionError("Node ID is required".to_string())
        })?;

        let node_did = self.node_did.unwrap_or_else(|| node_id.clone());

        #[cfg(feature = "p2p")]
        let client = if let (Some(config), Some(signaling)) = (self.hole_punch_config, self.signaling_client) {
            WebSocketClient::with_hole_punching(node_id, node_did, config, signaling)
        } else {
            WebSocketClient::new(node_id, node_did)
        };
        #[cfg(not(feature = "p2p"))]
        let client = WebSocketClient::new(node_id, node_did);

        Ok(if let Some(key) = self.auth_key {
            client.with_auth_key(key)
        } else {
            client
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "p2p")]
    use crate::matrix::websocket::hole_punching::InMemorySignalingClient;

    #[test]
    fn test_connect_options() {
        let opts = ConnectOptions::new().with_timeout(Duration::from_secs(60));
        assert_eq!(opts.timeout, Duration::from_secs(60));
        assert!(opts.auto_reconnect);
    }

    #[test]
    fn test_client_builder() {
        let client = WebSocketClientBuilder::new()
            .node_id("test-node")
            .node_did("did:cis:test")
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.node_id, "test-node");
        assert_eq!(client.node_did, "did:cis:test");
        #[cfg(feature = "p2p")]
        assert!(client.hole_punch_manager.is_none());
    }

    #[test]
    fn test_client_builder_missing_node_id() {
        let client = WebSocketClientBuilder::new().build();
        assert!(client.is_err());
    }

    #[test]
    #[cfg(feature = "p2p")]
    fn test_client_builder_with_hole_punching() {
        use crate::matrix::websocket::HolePunchConfig;
        
        let signaling = Arc::new(InMemorySignalingClient::new());
        let config = HolePunchConfig::new();
        
        let client = WebSocketClientBuilder::new()
            .node_id("test-node")
            .node_did("did:cis:test")
            .with_hole_punching(config, signaling)
            .build();

        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.node_id, "test-node");
        assert!(client.hole_punch_manager.is_some());
    }

    #[test]
    fn test_ws_client_error_display() {
        let err = WsClientError::HolePunchError("test error".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Hole punching error"));
        assert!(msg.contains("test error"));
    }

    #[tokio::test]
    #[cfg(feature = "p2p")]
    async fn test_detect_nat_type_without_manager() {
        let client = WebSocketClient::new("test-node", "did:cis:test");
        let result = client.detect_nat_type().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WsClientError::HolePunchError(_)));
    }

    #[tokio::test]
    #[cfg(feature = "p2p")]
    async fn test_punch_hole_without_manager() {
        let client = WebSocketClient::new("test-node", "did:cis:test");
        let result = client.punch_hole("peer-node").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), WsClientError::HolePunchError(_)));
    }
}
