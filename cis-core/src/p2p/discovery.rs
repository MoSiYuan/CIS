//! 节点发现服务
//!
//! 使用 mDNS 在局域网发现节点，DHT 在广域网发现节点。

use crate::error::Result;
use crate::p2p::NodeInfo;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

const SERVICE_NAME: &str = "_cis._tcp.local";
const DEFAULT_PORT: u16 = 7676;

/// 节点发现服务
pub struct DiscoveryService {
    node_id: String,
    discovered_peers: Arc<Mutex<HashSet<PeerDiscoveryInfo>>>,
}

/// 对等节点发现信息
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PeerDiscoveryInfo {
    pub node_id: String,
    pub addresses: Vec<String>,
    pub did: String,
}

impl DiscoveryService {
    /// 创建新的发现服务
    pub fn new(node_id: String) -> Self {
        Self {
            node_id,
            discovered_peers: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    
    /// 启动发现服务
    pub async fn start(&self, local_node: NodeInfo) -> Result<()> {
        // 启动 mDNS 广播
        self.start_mdns_broadcast(local_node).await?;
        
        // 启动 mDNS 监听
        self.start_mdns_listener().await?;
        
        Ok(())
    }
    
    /// 启动 mDNS 广播
    async fn start_mdns_broadcast(&self, local_node: NodeInfo) -> Result<()> {
        let node_id = self.node_id.clone();
        let service_name = SERVICE_NAME.to_string();
        let did = local_node.did.clone();
        
        tokio::spawn(async move {
            // 使用 mdns-sd 库进行 mDNS 广播
            let mdns = match mdns_sd::ServiceDaemon::new() {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to create mDNS daemon: {}", e);
                    return;
                }
            };
            
            // 创建服务信息
            let host_name = format!("{}.local", node_id);
            let service_type = service_name.to_string();
            
            // 构建 TXT 记录
            let mut properties = std::collections::HashMap::new();
            properties.insert("node_id".to_string(), node_id.clone());
            properties.insert("did".to_string(), did);
            
            // 获取本机地址
            let addresses: Vec<SocketAddr> = local_node.addresses
                .iter()
                .filter_map(|addr| addr.parse().ok())
                .collect();
            
            let ip_addrs: Vec<std::net::IpAddr> = addresses
                .iter()
                .map(|a| a.ip())
                .collect();
            
            let service_info = match mdns_sd::ServiceInfo::new(
                &service_type,
                &node_id,
                &host_name,
                &ip_addrs[..],
                DEFAULT_PORT,
                properties,
            ) {
                Ok(info) => info,
                Err(e) => {
                    tracing::error!("Failed to create service info: {}", e);
                    return;
                }
            };
            
            if let Err(e) = mdns.register(service_info) {
                tracing::error!("Failed to register mDNS service: {}", e);
                return;
            }
            
            tracing::info!("mDNS broadcast started: {}", service_name);
            
            // 保持运行
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            }
        });
        
        Ok(())
    }
    
    /// 启动 mDNS 监听
    async fn start_mdns_listener(&self) -> Result<()> {
        let discovered = Arc::clone(&self.discovered_peers);
        let node_id = self.node_id.clone();
        
        tokio::spawn(async move {
            let mdns = match mdns_sd::ServiceDaemon::new() {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("Failed to create mDNS daemon: {}", e);
                    return;
                }
            };
            
            let service_type = SERVICE_NAME.to_string();
            let receiver = match mdns.browse(&service_type) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("Failed to browse mDNS: {}", e);
                    return;
                }
            };
            
            while let Ok(event) = receiver.recv() {
                match event {
                    mdns_sd::ServiceEvent::ServiceResolved(info) => {
                        // 解析节点信息
                        if let Some(peer_info) = Self::parse_service_info(&info) {
                            // 跳过自己
                            if peer_info.node_id != node_id {
                                tracing::info!("Discovered peer: {} at {:?}", 
                                    peer_info.node_id, peer_info.addresses);
                                discovered.lock().unwrap().insert(peer_info);
                            }
                        }
                    }
                    mdns_sd::ServiceEvent::ServiceRemoved(_, fullname) => {
                        tracing::debug!("Service removed: {}", fullname);
                        // 从发现列表中移除
                        let mut peers = discovered.lock().unwrap();
                        peers.retain(|p| !fullname.contains(&p.node_id));
                    }
                    _ => {}
                }
            }
        });
        
        Ok(())
    }
    
    /// 解析服务信息
    fn parse_service_info(info: &mdns_sd::ServiceInfo) -> Option<PeerDiscoveryInfo> {
        let properties = info.get_properties();
        
        let node_id = properties.get("node_id")?.to_string();
        let did = properties.get("did")?.to_string();
        
        let addresses: Vec<String> = info.get_addresses()
            .iter()
            .map(|ip| format!("{}:{}", ip, info.get_port()))
            .collect();
        
        Some(PeerDiscoveryInfo {
            node_id,
            addresses,
            did,
        })
    }
    
    /// 获取发现的节点
    pub fn get_discovered_peers(&self) -> Vec<PeerDiscoveryInfo> {
        self.discovered_peers.lock().unwrap().iter().cloned().collect()
    }
}
