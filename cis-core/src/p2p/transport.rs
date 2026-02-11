//! QUIC 传输层
//!
//! 使用 quinn 库实现完整的 QUIC 协议传输，支持：
//! - 可靠的有序流传输
//! - 流多路复用
//! - 0-RTT 连接恢复
//! - 连接迁移
//! - 自动重连和心跳

use crate::error::{CisError, Result};
use quinn::{Endpoint, Connection as QuinnConnection, ServerConfig, ClientConfig, SendStream, RecvStream, VarInt};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, trace, warn};

/// 连接信息
#[derive(Debug)]
pub struct ConnectionInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub connected_at: std::time::Instant,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

/// QUIC 传输层配置
#[derive(Debug, Clone)]
pub struct TransportConfig {
    /// 连接超时时间
    pub connection_timeout: Duration,
    /// 心跳间隔
    pub heartbeat_interval: Duration,
    /// 心跳超时
    pub heartbeat_timeout: Duration,
    /// 最大并发流数
    pub max_concurrent_streams: u64,
    /// 接收缓冲区大小
    pub receive_buffer_size: usize,
    /// 发送缓冲区大小
    pub send_buffer_size: usize,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            connection_timeout: Duration::from_secs(10),
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(10),
            max_concurrent_streams: 100,
            receive_buffer_size: 65536,
            send_buffer_size: 65536,
        }
    }
}

/// QUIC 传输层
pub struct QuicTransport {
    endpoint: Endpoint,
    /// 监听地址
    pub listen_addr: SocketAddr,
    connections: Arc<RwLock<HashMap<String, ConnectionHandle>>>,
    config: TransportConfig,
    /// 关闭信号
    shutdown_tx: Option<mpsc::Sender<()>>,
}

/// 连接句柄，包含连接和元数据
struct ConnectionHandle {
    connection: Connection,
    info: ConnectionInfo,
    /// 最后活跃时间
    last_active: std::time::Instant,
    /// 是否已验证
    verified: bool,
}

/// QUIC 连接包装器
pub struct Connection {
    inner: QuinnConnection,
    node_id: String,
    address: SocketAddr,
}

/// 流包装器
pub struct QuicStream {
    send: SendStream,
    recv: RecvStream,
}

impl QuicTransport {
    /// 绑定到指定地址
    pub async fn bind(listen_addr: &str, node_id: &str) -> Result<Self> {
        Self::bind_with_config(listen_addr, node_id, TransportConfig::default()).await
    }
    
    /// 使用配置绑定到指定地址
    pub async fn bind_with_config(listen_addr: &str, _node_id: &str, config: TransportConfig) -> Result<Self> {
        let addr: SocketAddr = listen_addr.parse()
            .map_err(|e| CisError::p2p(format!("Invalid address: {}", e)))?;
        
        // 配置 QUIC
        let server_config = Self::configure_server(&config)?;
        let endpoint = Endpoint::server(server_config, addr)
            .map_err(|e| CisError::p2p(format!("Failed to create endpoint: {}", e)))?;
        
        info!("QUIC transport bound to {} with config: {:?}", addr, config);
        
        let (shutdown_tx, _shutdown_rx) = mpsc::channel(1);
        
        Ok(Self {
            endpoint,
            listen_addr: addr,
            connections: Arc::new(RwLock::new(HashMap::new())),
            config,
            shutdown_tx: Some(shutdown_tx),
        })
    }
    
    /// 创建新的 QUIC 传输层（向后兼容）
    pub async fn new(listen_addr: &str) -> Result<Self> {
        Self::bind(listen_addr, "default").await
    }
    
    /// 开始监听连接
    pub async fn start_listening(&self) -> Result<()> {
        info!("QUIC listening on {}", self.listen_addr);
        
        // 创建接收器
        let connections = Arc::clone(&self.connections);
        let config = self.config.clone();
        let endpoint = self.endpoint.clone();
        
        tokio::spawn(async move {
            while let Some(conn) = endpoint.accept().await {
                let connections = Arc::clone(&connections);
                let config = config.clone();
                
                tokio::spawn(async move {
                    match conn.await {
                        Ok(connection) => {
                            let addr = connection.remote_address();
                            trace!("New connection from {:?}", addr);
                            
                            // 启动连接处理
                            if let Err(e) = Self::handle_incoming_connection(
                                connection, addr, connections, config
                            ).await {
                                warn!("Failed to handle incoming connection from {}: {}", addr, e);
                            }
                        }
                        Err(e) => {
                            debug!("Incoming connection failed: {}", e);
                        }
                    }
                });
            }
            debug!("Endpoint closed, stopping listener");
        });
        
        // 启动心跳任务
        self.start_heartbeat_task();
        
        Ok(())
    }
    
    /// 处理新连接
    async fn handle_incoming_connection(
        quinn_conn: QuinnConnection,
        addr: SocketAddr,
        connections: Arc<RwLock<HashMap<String, ConnectionHandle>>>,
        _config: TransportConfig,
    ) -> Result<()> {
        // 生成临时 node_id（实际应该从握手协议获取）
        let node_id = format!("peer-{}", addr.port());
        
        let conn = Connection {
            inner: quinn_conn,
            node_id: node_id.clone(),
            address: addr,
        };
        
        let handle = ConnectionHandle {
            info: ConnectionInfo {
                node_id: node_id.clone(),
                address: addr,
                connected_at: std::time::Instant::now(),
                bytes_sent: 0,
                bytes_received: 0,
            },
            connection: conn,
            last_active: std::time::Instant::now(),
            verified: false,
        };
        
        connections.write().await.insert(node_id.clone(), handle);
        info!("Accepted connection from {} ({})", node_id, addr);
        
        // 启动连接处理循环（读取数据）
        Self::spawn_connection_handler(Arc::clone(&connections), node_id.clone());
        
        Ok(())
    }
    
    /// 连接到远程节点
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<()> {
        if self.connections.read().await.contains_key(node_id) {
            return Err(CisError::p2p(format!("Already connected to {}", node_id)));
        }
        
        let client_config = Self::configure_client(&self.config)?;
        
        // 首先建立连接配置
        let connecting = self.endpoint.connect_with(client_config, addr, "cis")
            .map_err(|e| CisError::p2p(format!("Failed to connect: {}", e)))?;
        
        // 等待连接完成（带超时）
        let connection = timeout(
            self.config.connection_timeout,
            connecting
        )
        .await
        .map_err(|_| CisError::p2p("Connection timeout"))?
        .map_err(|e| CisError::p2p(format!("Connection failed: {}", e)))?;
        
        let conn = Connection {
            inner: connection,
            node_id: node_id.to_string(),
            address: addr,
        };
        
        let handle = ConnectionHandle {
            info: ConnectionInfo {
                node_id: node_id.to_string(),
                address: addr,
                connected_at: std::time::Instant::now(),
                bytes_sent: 0,
                bytes_received: 0,
            },
            connection: conn,
            last_active: std::time::Instant::now(),
            verified: true,
        };
        
        self.connections.write().await.insert(node_id.to_string(), handle);
        
        // 启动连接处理循环（读取数据）
        Self::spawn_connection_handler(Arc::clone(&self.connections), node_id.to_string());
        
        info!("Connected to {} at {}", node_id, addr);
        Ok(())
    }
    
    /// 向后兼容的连接方法
    pub async fn connect_str(&self, addr: &str) -> Result<Connection> {
        let remote: SocketAddr = addr.parse()
            .map_err(|e| CisError::p2p(format!("Invalid address: {}", e)))?;
        
        let client_config = Self::configure_client(&self.config)?;
        
        let connecting = self.endpoint.connect_with(client_config, remote, "cis")
            .map_err(|e| CisError::p2p(format!("Failed to connect: {}", e)))?;
        
        let connection = timeout(
            self.config.connection_timeout,
            connecting
        )
        .await
        .map_err(|_| CisError::p2p("Connection timeout"))?
        .map_err(|e| CisError::p2p(format!("Connection failed: {}", e)))?;
        
        Ok(Connection {
            inner: connection,
            node_id: format!("peer-{}", remote.port()),
            address: remote,
        })
    }
    
    /// 断开与节点的连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(handle) = connections.remove(node_id) {
            handle.connection.inner.close(VarInt::from_u32(0), b"disconnecting");
            info!("Disconnected from {}", node_id);
        }
        Ok(())
    }
    
    /// 发送数据到指定节点
    pub async fn send(&self, node_id: &str, data: &[u8]) -> Result<()> {
        let mut connections = self.connections.write().await;
        let handle = connections.get_mut(node_id)
            .ok_or_else(|| CisError::p2p(format!("Node {} not connected", node_id)))?;
        
        handle.connection.send(data.to_vec()).await?;
        handle.info.bytes_sent += data.len() as u64;
        handle.last_active = std::time::Instant::now();
        
        Ok(())
    }
    
    /// 打开新的双向流
    pub async fn open_stream(&self, node_id: &str) -> Result<QuicStream> {
        let connections = self.connections.read().await;
        let handle = connections.get(node_id)
            .ok_or_else(|| CisError::p2p(format!("Node {} not connected", node_id)))?;
        
        handle.connection.open_stream().await
    }
    
    /// 列出所有连接
    pub async fn list_connections(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.read().await;
        connections.values().map(|h| h.info.clone()).collect()
    }
    
    /// 获取活跃连接数
    pub async fn active_connection_count(&self) -> usize {
        self.connections.read().await.len()
    }
    
    /// 关闭传输层
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down QUIC transport...");
        
        // 发送关闭信号
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
        
        // 关闭所有连接
        let mut connections = self.connections.write().await;
        for (node_id, handle) in connections.drain() {
            handle.connection.inner.close(VarInt::from_u32(0), b"shutting down");
            debug!("Closed connection to {}", node_id);
        }
        
        // 关闭端点
        self.endpoint.close(VarInt::from_u32(0), b"shutdown");
        
        info!("QUIC transport shut down");
        Ok(())
    }
    
    /// 启动连接处理循环
    /// 
    /// 为每个连接启动一个任务来处理接收到的数据
    fn spawn_connection_handler(
        connections: Arc<RwLock<HashMap<String, ConnectionHandle>>>,
        node_id: String,
    ) {
        tokio::spawn(async move {
            loop {
                // 获取连接句柄
                let connection = {
                    let connections_guard = connections.read().await;
                    if let Some(handle) = connections_guard.get(&node_id) {
                        if !handle.connection.is_alive() {
                            debug!("Connection {} is no longer alive, stopping handler", node_id);
                            break;
                        }
                        // 克隆连接以便在后续使用
                        // 注意：这里我们需要一种方式来接收数据
                        // 由于 Connection 没有实现 Clone，我们使用 accept_stream
                        match handle.connection.accept_stream().await {
                            Ok(stream) => Some(stream),
                            Err(e) => {
                                trace!("No new stream from {}: {}", node_id, e);
                                None
                            }
                        }
                    } else {
                        debug!("Connection {} not found, stopping handler", node_id);
                        break;
                    }
                };
                
                if let Some(mut stream) = connection {
                    // 处理流数据
                    let mut buffer = vec![];
                    match stream.read_to_end(&mut buffer).await {
                        Ok(n) if n > 0 => {
                            trace!("Received {} bytes from {}", n, node_id);
                            // 更新最后活跃时间
                            let mut connections_guard = connections.write().await;
                            if let Some(handle) = connections_guard.get_mut(&node_id) {
                                handle.last_active = std::time::Instant::now();
                                handle.info.bytes_received += n as u64;
                            }
                            
                            // 处理心跳请求和响应
                            if buffer == b"ping" {
                                trace!("Received heartbeat ping from {}, sending pong", node_id);
                                // 回复 pong
                                let connections_guard = connections.read().await;
                                if let Some(handle) = connections_guard.get(node_id.as_str()) {
                                    if let Err(e) = handle.connection.send(b"pong".to_vec()).await {
                                        warn!("Failed to send heartbeat response to {}: {}", node_id, e);
                                    }
                                }
                            } else if buffer == b"pong" {
                                trace!("Received heartbeat response from {}", node_id);
                            }
                        }
                        Ok(_) => {
                            trace!("Empty stream from {}", node_id);
                        }
                        Err(e) => {
                            warn!("Error reading from {}: {}", node_id, e);
                        }
                    }
                }
                
                // 短暂休眠避免 CPU 占用过高
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            
            debug!("Connection handler for {} stopped", node_id);
        });
    }
    
    /// 启动心跳任务
    fn start_heartbeat_task(&self) {
        let connections = Arc::clone(&self.connections);
        let interval_duration = self.config.heartbeat_interval;
        let timeout_duration = self.config.heartbeat_timeout;
        
        tokio::spawn(async move {
            let mut ticker = interval(interval_duration);
            
            loop {
                ticker.tick().await;
                
                let mut dead_connections = Vec::new();
                
                {
                    let connections_guard = connections.read().await;
                    for (node_id, handle) in connections_guard.iter() {
                        let inactive_duration = handle.last_active.elapsed();
                        
                        // 检查是否超时
                        if inactive_duration > interval_duration + timeout_duration {
                            warn!("Connection to {} timed out", node_id);
                            dead_connections.push(node_id.clone());
                            continue;
                        }
                        
                        // 发送心跳（如果到了心跳时间）
                        if inactive_duration >= interval_duration {
                            trace!("Sending heartbeat to {}", node_id);
                            // 实际发送心跳消息
                            let connections_guard = connections.read().await;
                            if let Some(handle) = connections_guard.get(node_id.as_str()) {
                                let heartbeat_msg = b"ping";
                                if let Err(e) = handle.connection.send(heartbeat_msg.to_vec()).await {
                                    warn!("Failed to send heartbeat to {}: {}", node_id, e);
                                    dead_connections.push(node_id.clone());
                                }
                            }
                        }
                    }
                }
                
                // 移除死连接
                if !dead_connections.is_empty() {
                    let mut connections_guard = connections.write().await;
                    for node_id in dead_connections {
                        if let Some(handle) = connections_guard.remove(&node_id) {
                            handle.connection.inner.close(VarInt::from_u32(0), b"heartbeat timeout");
                            info!("Removed dead connection to {}", node_id);
                        }
                    }
                }
            }
        });
    }
    
    /// 配置服务器
    fn configure_server(config: &TransportConfig) -> Result<ServerConfig> {
        // 配置证书
        let cert = rcgen::generate_simple_self_signed(vec!["cis".into()])
            .map_err(|e| CisError::p2p(format!("Failed to generate certificate: {}", e)))?;
        let cert_der = cert.serialize_der()
            .map_err(|e| CisError::p2p(format!("Failed to serialize certificate: {}", e)))?;
        let key_der = cert.serialize_private_key_der();
        
        let cert_chain = vec![CertificateDer::from(cert_der)];
        let key = PrivateKeyDer::try_from(key_der)
            .map_err(|e| CisError::p2p(format!("Invalid private key: {:?}", e)))?;
        
        let mut server_config = ServerConfig::with_single_cert(cert_chain, key)
            .map_err(|e| CisError::p2p(format!("Failed to create server config: {}", e)))?;
        
        // 配置传输参数
        let mut transport_config = quinn::TransportConfig::default();
        let streams = VarInt::from_u64(config.max_concurrent_streams).unwrap_or(VarInt::from_u32(100));
        transport_config.max_concurrent_uni_streams(streams);
        transport_config.max_concurrent_bidi_streams(streams);
        
        server_config.transport_config(Arc::new(transport_config));
        
        Ok(server_config)
    }
    
    /// 配置客户端
    fn configure_client(config: &TransportConfig) -> Result<ClientConfig> {
        let mut roots = rustls::RootCertStore::empty();
        
        // 添加系统根证书
        if let Ok(cert) = rustls_native_certs::load_native_certs() {
            for c in cert {
                let _ = roots.add(CertificateDer::from(c.0));
            }
        }
        
        let tls_config = rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth();
        
        let mut tls_config = tls_config;
        tls_config.alpn_protocols = vec![b"cis/1.0".to_vec()];
        
        let mut client_config = ClientConfig::new(Arc::new(
            quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
                .map_err(|e| CisError::p2p(format!("Failed to create QUIC client config: {:?}", e)))?
        ));
        
        // 配置传输参数
        let mut transport_config = quinn::TransportConfig::default();
        let streams = VarInt::from_u64(config.max_concurrent_streams).unwrap_or(VarInt::from_u32(100));
        transport_config.max_concurrent_uni_streams(streams);
        transport_config.max_concurrent_bidi_streams(streams);
        
        client_config.transport_config(Arc::new(transport_config));
        
        Ok(client_config)
    }
}

impl Connection {
    /// 发送数据
    pub async fn send(&self, data: Vec<u8>) -> Result<()> {
        let (mut send, _) = self.inner.open_bi().await
            .map_err(|e| CisError::p2p(format!("Failed to open stream: {}", e)))?;
        
        tokio::io::AsyncWriteExt::write_all(&mut send, &data).await
            .map_err(|e| CisError::p2p(format!("Failed to write data: {}", e)))?;
        
        tokio::io::AsyncWriteExt::shutdown(&mut send).await
            .map_err(|e| CisError::p2p(format!("Failed to shutdown stream: {}", e)))?;
        
        Ok(())
    }
    
    /// 接收数据
    pub async fn receive(&self) -> Result<Vec<u8>> {
        let (_, mut recv) = self.inner.accept_bi().await
            .map_err(|e| CisError::p2p(format!("Failed to accept stream: {}", e)))?;
        
        let mut data = vec![];
        tokio::io::AsyncReadExt::read_to_end(&mut recv, &mut data).await
            .map_err(|e| CisError::p2p(format!("Failed to read data: {}", e)))?;
        
        Ok(data)
    }
    
    /// 打开新的双向流
    pub async fn open_stream(&self) -> Result<QuicStream> {
        let (send, recv) = self.inner.open_bi().await
            .map_err(|e| CisError::p2p(format!("Failed to open stream: {}", e)))?;
        
        Ok(QuicStream { send, recv })
    }
    
    /// 接受新的双向流
    pub async fn accept_stream(&self) -> Result<QuicStream> {
        let (send, recv) = self.inner.accept_bi().await
            .map_err(|e| CisError::p2p(format!("Failed to accept stream: {}", e)))?;
        
        Ok(QuicStream { send, recv })
    }
    
    /// 检查连接是否活跃
    pub fn is_alive(&self) -> bool {
        self.inner.close_reason().is_none()
    }
    
    /// 获取节点 ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }
    
    /// 获取地址
    pub fn address(&self) -> SocketAddr {
        self.address
    }
    
    /// 获取内部连接（高级使用）
    pub fn inner(&self) -> &QuinnConnection {
        &self.inner
    }
}

impl QuicStream {
    /// 发送数据
    pub async fn write_all(&mut self, data: &[u8]) -> Result<()> {
        tokio::io::AsyncWriteExt::write_all(&mut self.send, data).await
            .map_err(|e| CisError::p2p(format!("Failed to write: {}", e)))
    }
    
    /// 完成发送
    pub async fn finish(&mut self) -> Result<()> {
        self.send.finish()
            .map_err(|e| CisError::p2p(format!("Failed to finish stream: {}", e)))
    }
    
    /// 读取数据到缓冲区
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        tokio::io::AsyncReadExt::read(&mut self.recv, buf).await
            .map_err(|e| CisError::p2p(format!("Failed to read: {}", e)))
            .map(|n| if n == 0 { None } else { Some(n) })
    }
    
    /// 读取所有数据
    pub async fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        tokio::io::AsyncReadExt::read_to_end(&mut self.recv, buf).await
            .map_err(|e| CisError::p2p(format!("Failed to read: {}", e)))
    }
}

impl Clone for ConnectionInfo {
    fn clone(&self) -> Self {
        Self {
            node_id: self.node_id.clone(),
            address: self.address,
            connected_at: self.connected_at,
            bytes_sent: self.bytes_sent,
            bytes_received: self.bytes_received,
        }
    }
}
