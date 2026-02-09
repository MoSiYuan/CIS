//! QUIC 传输层实现
//!
//! 提供基于 QUIC 的 P2P 传输能力

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// QUIC 传输层
pub struct QuicTransport {
    endpoint: quinn::Endpoint,
    connections: Arc<Mutex<HashMap<String, PeerConnection>>>,
    node_id: String,
}

/// 对等节点连接
#[derive(Debug, Clone)]
pub struct PeerConnection {
    pub node_id: String,
    pub address: SocketAddr,
    pub connection: quinn::Connection,
    pub established_at: std::time::Instant,
}

/// 连接信息
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub established_at: std::time::Instant,
    pub rtt_ms: Option<u64>,
}

impl QuicTransport {
    /// 创建 QUIC 端点
    pub async fn bind(bind_addr: &str, node_id: &str) -> Result<Self> {
        let addr: SocketAddr = bind_addr.parse()
            .map_err(|e| anyhow!("Invalid bind address: {}", e))?;
        
        // 生成自签名证书
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()])
            .map_err(|e| anyhow!("Failed to generate certificate: {}", e))?;
        let cert_der = cert.serialize_der()
            .map_err(|e| anyhow!("Failed to serialize certificate: {}", e))?;
        let key_der = cert.serialize_private_key_der();
        
        let cert_chain = vec![rustls::pki_types::CertificateDer::from(cert_der)];
        let key = rustls::pki_types::PrivateKeyDer::from(rustls::pki_types::PrivatePkcs8KeyDer::from(key_der));
        
        let mut server_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(cert_chain.clone(), key)
            .map_err(|e| anyhow!("Failed to create server config: {}", e))?;
        
        let mut client_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(SkipServerVerification))
            .with_no_client_auth();
        
        let mut endpoint = quinn::Endpoint::server(server_config.into(), addr)
            .map_err(|e| anyhow!("Failed to create endpoint: {}", e))?;
        
        endpoint.set_default_client_config(client_config.into());
        
        info!("QUIC endpoint bound to {}", endpoint.local_addr()?);
        
        Ok(Self {
            endpoint,
            connections: Arc::new(Mutex::new(HashMap::new())),
            node_id: node_id.to_string(),
        })
    }
    
    /// 连接到远程节点
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<bool> {
        let mut connections = self.connections.lock().await;
        
        if connections.contains_key(node_id) {
            debug!("Already connected to {}", node_id);
            return Ok(false);
        }
        
        info!("Connecting to {} at {}", node_id, addr);
        
        let connection = self.endpoint.connect(addr, "localhost")
            .map_err(|e| anyhow!("Failed to create connection: {}", e))?
            .await
            .map_err(|e| anyhow!("Failed to establish connection: {}", e))?;
        
        let peer = PeerConnection {
            node_id: node_id.to_string(),
            address: addr,
            connection,
            established_at: std::time::Instant::now(),
        };
        
        connections.insert(node_id.to_string(), peer);
        info!("Connected to {} at {}", node_id, addr);
        
        Ok(true)
    }
    
    /// 断开连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()> {
        let mut connections = self.connections.lock().await;
        
        if let Some(peer) = connections.remove(node_id) {
            peer.connection.close(0u32.into(), b"disconnect");
            info!("Disconnected from {}", node_id);
        }
        
        Ok(())
    }
    
    /// 列出连接
    pub async fn list_connections(&self) -> Vec<ConnectionInfo> {
        let connections = self.connections.lock().await;
        
        connections.values().map(|peer| {
            let rtt = peer.connection.rtt();
            ConnectionInfo {
                node_id: peer.node_id.clone(),
                address: peer.address,
                established_at: peer.established_at,
                rtt_ms: Some(rtt.as_millis() as u64),
            }
        }).collect()
    }
    
    /// 发送数据
    pub async fn send(&self, node_id: &str, data: &[u8]) -> Result<()> {
        let connections = self.connections.lock().await;
        
        let peer = connections.get(node_id)
            .ok_or_else(|| anyhow!("Not connected to {}", node_id))?;
        
        let (mut send, _recv) = peer.connection.open_bi()
            .await
            .map_err(|e| anyhow!("Failed to open stream: {}", e))?;
        
        send.write_all(data).await
            .map_err(|e| anyhow!("Failed to write data: {}", e))?;
        send.finish()
            .map_err(|e| anyhow!("Failed to finish stream: {}", e))?;
        
        Ok(())
    }
    
    /// 获取本地地址
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.endpoint.local_addr()
            .map_err(|e| anyhow!("Failed to get local address: {}", e))
    }
    
    /// 关闭传输层
    pub async fn shutdown(self) -> Result<()> {
        self.endpoint.close(0u32.into(), b"shutdown");
        Ok(())
    }
}

/// 跳过服务器证书验证（用于自签名证书）
#[derive(Debug)]
struct SkipServerVerification;

impl rustls::client::danger::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_quic_bind() {
        let transport = QuicTransport::bind("127.0.0.1:0", "test-node").await;
        assert!(transport.is_ok());
        
        let transport = transport.unwrap();
        assert!(transport.local_addr().unwrap().port() > 0);
    }
}
