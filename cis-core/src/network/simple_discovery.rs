//! # 简化的节点发现服务
//!
//! 提供零配置的局域网节点发现，基于 UDP 广播。
//!
//! ## 特点
//! - 无需配置，自动发现同一网络节点
//! - 使用设备 hostname 作为节点名
//! - 自动显示发现节点列表
//! - 一键添加邻居节点

use crate::error::{CisError, Result};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::time::interval;

const DISCOVERY_PORT: u16 = 6767;
const BROADCAST_ADDR: &str = "255.255.255.255";
const DISCOVERY_INTERVAL: Duration = Duration::from_secs(5);
const NODE_TIMEOUT: Duration = Duration::from_secs(30);

/// 发现的节点信息
#[derive(Debug, Clone)]
pub struct DiscoveredNode {
    pub node_id: String,
    pub hostname: String,
    pub did: String,
    pub addresses: Vec<IpAddr>,
    pub port: u16,
    pub last_seen: Instant,
}

/// 简单发现服务
pub struct SimpleDiscovery {
    node_id: String,
    hostname: String,
    did: String,
    port: u16,
    discovered: Arc<Mutex<HashMap<String, DiscoveredNode>>>,
}

impl SimpleDiscovery {
    /// 创建发现服务
    pub fn new(node_id: impl Into<String>, did: impl Into<String>) -> Result<Self> {
        let hostname = gethostname::gethostname()
            .to_string_lossy()
            .to_string();
        
        Ok(Self {
            node_id: node_id.into(),
            hostname,
            did: did.into(),
            port: DISCOVERY_PORT,
            discovered: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    /// 启动发现服务
    pub async fn start(&self) -> Result<()> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", self.port))
            .map_err(|e| CisError::network(format!("Failed to bind discovery socket: {}", e)))?;
        
        socket.set_broadcast(true)
            .map_err(|e| CisError::network(format!("Failed to enable broadcast: {}", e)))?;
        
        socket.set_nonblocking(true)
            .map_err(|e| CisError::network(format!("Failed to set non-blocking: {}", e)))?;
        
        let socket = Arc::new(socket);
        let discovered = self.discovered.clone();
        
        // 启动广播任务
        let broadcast_socket = socket.clone();
        let node_id = self.node_id.clone();
        let hostname = self.hostname.clone();
        let did = self.did.clone();
        let port = self.port;
        
        tokio::spawn(async move {
            let mut interval_timer = interval(DISCOVERY_INTERVAL);
            
            loop {
                interval_timer.tick().await;
                
                let announcement = format!("CIS_DISCOVER|{}|{}|{}|{}", 
                    node_id, hostname, did, port);
                
                if let Err(e) = broadcast_socket.send_to(
                    announcement.as_bytes(),
                    format!("{}:{}", BROADCAST_ADDR, port)
                ) {
                    tracing::debug!("Broadcast failed: {}", e);
                }
            }
        });
        
        // 启动监听任务
        let listen_socket = socket.clone();
        let local_node_id = self.node_id.clone();
        
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            
            loop {
                match listen_socket.recv_from(&mut buf) {
                    Ok((len, addr)) => {
                        if let Ok(msg) = String::from_utf8(buf[..len].to_vec()) {
                            Self::handle_discovery_message(
                                &msg, 
                                addr, 
                                &local_node_id,
                                &discovered
                            );
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    Err(e) => {
                        tracing::debug!("Receive error: {}", e);
                    }
                }
            }
        });
        
        // 启动清理任务
        let discovered_clean = self.discovered.clone();
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(10));
            
            loop {
                interval_timer.tick().await;
                
                let mut nodes = discovered_clean.lock().unwrap();
                let now = Instant::now();
                nodes.retain(|_, node| {
                    now.duration_since(node.last_seen) < NODE_TIMEOUT
                });
            }
        });
        
        Ok(())
    }
    
    /// 处理发现消息
    fn handle_discovery_message(
        msg: &str, 
        addr: SocketAddr,
        local_node_id: &str,
        discovered: &Arc<Mutex<HashMap<String, DiscoveredNode>>>
    ) {
        let parts: Vec<&str> = msg.split('|').collect();
        if parts.len() != 5 || parts[0] != "CIS_DISCOVER" {
            return;
        }
        
        let node_id = parts[1].to_string();
        let hostname = parts[2].to_string();
        let did = parts[3].to_string();
        let port: u16 = parts[4].parse().unwrap_or(DISCOVERY_PORT);
        
        // 忽略自己的广播
        if node_id == local_node_id {
            return;
        }
        
        let mut nodes = discovered.lock().unwrap();
        nodes.insert(node_id.clone(), DiscoveredNode {
            node_id,
            hostname,
            did,
            addresses: vec![addr.ip()],
            port,
            last_seen: Instant::now(),
        });
    }
    
    /// 获取发现的节点列表
    pub fn get_discovered_nodes(&self) -> Vec<DiscoveredNode> {
        let nodes = self.discovered.lock().unwrap();
        nodes.values().cloned().collect()
    }
    
    /// 获取特定节点
    pub fn get_node(&self, node_id: &str) -> Option<DiscoveredNode> {
        let nodes = self.discovered.lock().unwrap();
        nodes.get(node_id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_discovery_message_parsing() {
        let msg = "CIS_DISCOVER|node-1|myhost|did:cis:abc|6767";
        let parts: Vec<&str> = msg.split('|').collect();
        
        assert_eq!(parts[0], "CIS_DISCOVER");
        assert_eq!(parts[1], "node-1");
        assert_eq!(parts[2], "myhost");
        assert_eq!(parts[3], "did:cis:abc");
        assert_eq!(parts[4], "6767");
    }
}
