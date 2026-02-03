//! QUIC 传输层

use crate::error::{CisError, Result};
use quinn::{Endpoint, Connection as QuinnConnection, ServerConfig, ClientConfig};
use std::net::SocketAddr;
use std::sync::Arc;

/// QUIC 传输层
pub struct QuicTransport {
    endpoint: Endpoint,
    listen_addr: SocketAddr,
}

/// QUIC 连接包装器
pub struct Connection {
    inner: QuinnConnection,
}

impl QuicTransport {
    /// 创建新的 QUIC 传输层
    pub async fn new(listen_addr: &str) -> Result<Self> {
        let addr: SocketAddr = listen_addr.parse()
            .map_err(|e| CisError::p2p(format!("Invalid address: {}", e)))?;
        
        // 配置 QUIC
        let server_config = Self::configure_server()?;
        let endpoint = Endpoint::server(server_config, addr)
            .map_err(|e| CisError::p2p(format!("Failed to create endpoint: {}", e)))?;
        
        Ok(Self {
            endpoint,
            listen_addr: addr,
        })
    }
    
    /// 开始监听连接
    pub async fn start_listening(&self) -> Result<()> {
        tracing::info!("QUIC listening on {}", self.listen_addr);
        
        while let Some(conn) = self.endpoint.accept().await {
            tokio::spawn(async move {
                match conn.await {
                    Ok(connection) => {
                        Self::handle_connection(connection).await;
                    }
                    Err(e) => {
                        tracing::error!("Connection failed: {}", e);
                    }
                }
            });
        }
        
        Ok(())
    }
    
    /// 连接到远程节点
    pub async fn connect(&self, addr: &str) -> Result<Connection> {
        let remote: SocketAddr = addr.parse()
            .map_err(|e| CisError::p2p(format!("Invalid address: {}", e)))?;
        
        let client_config = Self::configure_client()?;
        
        let connection = self.endpoint.connect_with(client_config, remote, "cis")
            .map_err(|e| CisError::p2p(format!("Failed to connect: {}", e)))?
            .await
            .map_err(|e| CisError::p2p(format!("Connection failed: {}", e)))?;
        
        Ok(Connection { inner: connection })
    }
    
    /// 处理新连接
    async fn handle_connection(conn: QuinnConnection) {
        tracing::debug!("New connection from {:?}", conn.remote_address());
        
        // 处理双向流
        loop {
            match conn.accept_bi().await {
                Ok((send, recv)) => {
                    tokio::spawn(async move {
                        // 处理流
                        tracing::debug!("New bidirectional stream opened");
                        // 实际实现中这里会处理具体的协议消息
                        drop(send);
                        drop(recv);
                    });
                }
                Err(e) => {
                    tracing::debug!("Connection closed: {}", e);
                    break;
                }
            }
        }
    }
    
    /// 配置服务器
    fn configure_server() -> Result<ServerConfig> {
        // 配置证书
        let cert = rcgen::generate_simple_self_signed(vec!["cis".into()])
            .map_err(|e| CisError::p2p(format!("Failed to generate certificate: {}", e)))?;
        let cert_der = cert.serialize_der()
            .map_err(|e| CisError::p2p(format!("Failed to serialize certificate: {}", e)))?;
        let key_der = cert.serialize_private_key_der();
        
        let cert_chain = vec![rustls::Certificate(cert_der)];
        let key = rustls::PrivateKey(key_der);
        
        let mut config = ServerConfig::with_single_cert(cert_chain, key)
            .map_err(|e| CisError::p2p(format!("Failed to create server config: {}", e)))?;
        
        // 配置 ALPN (注意: quinn 新 API 中 crypto 是 Arc<dyn ServerConfig>)
        // 暂时跳过 ALPN 配置，如有需要可以使用 with_crypto API
        // config.alpn_protocols = vec![b"cis/1.0".to_vec()];
        
        Ok(config)
    }
    
    /// 配置客户端
    fn configure_client() -> Result<ClientConfig> {
        let mut roots = rustls::RootCertStore::empty();
        
        // 添加系统根证书
        if let Ok(cert) = rustls_native_certs::load_native_certs() {
            for c in cert {
                let _ = roots.add(&rustls::Certificate(c.0));
            }
        }
        
        // 添加自定义根证书（用于验证节点身份）
        // ...
        
        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(roots)
            .with_no_client_auth();
        
        let mut config = config;
        config.alpn_protocols = vec![b"cis/1.0".to_vec()];
        
        Ok(ClientConfig::new(Arc::new(config)))
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
    
    /// 检查连接是否活跃
    pub fn is_alive(&self) -> bool {
        self.inner.close_reason().is_none()
    }
}
