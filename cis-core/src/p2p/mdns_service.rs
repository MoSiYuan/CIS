//! mDNS 服务封装
//!
//! 提供简洁的 mDNS 局域网服务发现和广播接口。
//! 基于 mdns-sd crate 实现。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use anyhow::{anyhow, Context, Result};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

const SERVICE_TYPE: &str = "_cis._tcp.local.";
const DEFAULT_DISCOVERY_TIMEOUT_MS: u64 = 10000;

/// 发现的节点信息
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredNode {
    pub node_id: String,
    pub address: SocketAddr,
    pub did: String,
    pub metadata: HashMap<String, String>,
}

impl DiscoveredNode {
    /// 从 mDNS 服务信息解析
    fn from_service_info(info: &mdns_sd::ServiceInfo, node_id_filter: &str) -> Option<Self> {
        let properties = info.get_properties();
        
        // 获取 node_id
        let node_id = properties.get("node_id")?.to_string();
        
        // 跳过自己
        if node_id == node_id_filter {
            return None;
        }
        
        // 获取 DID
        let did = properties.get("did")?.to_string();
        
        // 获取地址
        let addresses: Vec<std::net::IpAddr> = info.get_addresses().iter().cloned().collect();
        if addresses.is_empty() {
            warn!("No addresses found for node {}", node_id);
            return None;
        }
        
        let port = info.get_port();
        let address = SocketAddr::new(addresses[0], port);
        
        // 收集所有元数据
        let mut metadata = HashMap::new();
        for prop in properties.iter() {
            let key = prop.key();
            let val = prop.val();
            if key != "node_id" && key != "did" {
                let val_str = val.map(|v| String::from_utf8_lossy(v).to_string())
                    .unwrap_or_default();
                metadata.insert(key.to_string(), val_str);
            }
        }
        
        Some(DiscoveredNode {
            node_id,
            address,
            did,
            metadata,
        })
    }
}

/// mDNS 服务实例
pub struct MdnsService {
    daemon: mdns_sd::ServiceDaemon,
    node_id: String,
    _service_name: String,
}

impl MdnsService {
    /// 创建并启动 mDNS 广播服务
    ///
    /// # Arguments
    /// * `node_id` - 本节点唯一标识
    /// * `port` - 服务端口
    /// * `did` - 去中心化身份标识
    /// * `metadata` - 额外的元数据
    pub fn new(
        node_id: &str,
        port: u16,
        did: &str,
        metadata: HashMap<String, String>,
    ) -> Result<Self> {
        let node_id = node_id.to_string();
        let service_name = format!("{}", node_id);
        
        // 创建 mDNS 守护进程
        let daemon = mdns_sd::ServiceDaemon::new()
            .map_err(|e| anyhow!("Failed to create mDNS daemon: {}", e))?;
        
        // 构建主机名和服务类型
        let host_name = format!("{}.local", node_id);
        let service_type = SERVICE_TYPE.to_string();
        
        // 构建 TXT 记录
        let mut properties = HashMap::new();
        properties.insert("node_id".to_string(), node_id.clone());
        properties.insert("did".to_string(), did.to_string());
        properties.insert("version".to_string(), "1.1.5".to_string());
        
        // 添加额外元数据
        for (key, value) in metadata {
            properties.insert(key, value);
        }
        
        // 创建服务信息
        let service_info = mdns_sd::ServiceInfo::new(
            &service_type,
            &service_name,
            &host_name,
            "",  // 让系统自动选择接口
            port,
            properties,
        )
        .map_err(|e| anyhow!("Failed to create service info: {}", e))?;
        
        // 注册服务
        daemon
            .register(service_info)
            .map_err(|e| anyhow!("Failed to register mDNS service: {}", e))?;
        
        info!(
            "mDNS service registered: {} on port {}",
            service_name, port
        );
        
        Ok(Self {
            daemon,
            node_id,
            _service_name: service_name,
        })
    }
    
    /// 发现同网段的 CIS 节点
    ///
    /// # Arguments
    /// * `timeout` - 发现超时时间
    ///
    /// # Returns
    /// 发现的节点列表（不包含本节点）
    pub fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredNode>> {
        let service_type = SERVICE_TYPE.to_string();
        
        // 启动浏览
        let receiver = self
            .daemon
            .browse(&service_type)
            .map_err(|e| anyhow!("Failed to browse mDNS: {}", e))?;
        
        let mut discovered = HashMap::new();
        let start = std::time::Instant::now();
        
        // 收集发现的节点
        while start.elapsed() < timeout {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    match event {
                        mdns_sd::ServiceEvent::ServiceResolved(info) => {
                            debug!("Discovered service: {}", info.get_fullname());
                            
                            if let Some(node) = DiscoveredNode::from_service_info(&info, &self.node_id) {
                                info!(
                                    "Discovered peer: {} at {} (DID: {})",
                                    node.node_id, node.address, node.did
                                );
                                discovered.insert(node.node_id.clone(), node);
                            }
                        }
                        mdns_sd::ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                            debug!("Service removed: {}", fullname);
                            // 从发现列表中移除
                            discovered.retain(|_, node| !fullname.contains(&node.node_id));
                        }
                        _ => {}
                    }
                }
                Err(_) => {
                    // 超时，继续循环检查总超时
                    continue;
                }
                Err(e) => {
                    warn!("mDNS receive error: {}", e);
                    break;
                }
            }
        }
        
        let nodes: Vec<DiscoveredNode> = discovered.into_values().collect();
        info!("Discovery complete: found {} node(s)", nodes.len());
        
        Ok(nodes)
    }
    
    /// 持续监听新节点加入
    ///
    /// # Returns
    /// 接收新发现节点的 channel
    pub fn watch(&self) -> Result<mpsc::Receiver<DiscoveredNode>> {
        let (tx, rx) = mpsc::channel(100);
        let service_type = SERVICE_TYPE.to_string();
        let node_id = self.node_id.clone();
        
        // 启动浏览
        let receiver = self
            .daemon
            .browse(&service_type)
            .map_err(|e| anyhow!("Failed to browse mDNS: {}", e))?;
        
        // 在后台任务中持续监听
        std::thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    mdns_sd::ServiceEvent::ServiceResolved(info) => {
                        if let Some(node) = DiscoveredNode::from_service_info(&info, &node_id) {
                            info!(
                                "Watch discovered: {} at {}",
                                node.node_id, node.address
                            );
                            if tx.blocking_send(node).is_err() {
                                // 接收端已关闭
                                break;
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
        
        Ok(rx)
    }
    
    /// 停止 mDNS 服务
    pub fn shutdown(self) -> Result<()> {
        info!("Shutting down mDNS service for node {}", self.node_id);
        
        self.daemon
            .shutdown()
            .map_err(|e| anyhow!("Failed to shutdown mDNS daemon: {}", e))?;
        
        info!("mDNS service shutdown complete");
        Ok(())
    }
    
    /// 获取本节点 ID
    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    /// 使用指定服务类型发现节点
    ///
    /// # Arguments
    /// * `service_type` - 服务类型（如 "_matrix._tcp.local"）
    /// * `timeout` - 发现超时时间
    ///
    /// # Returns
    /// 发现的节点列表
    pub fn discover_with_type(
        &self,
        service_type: &str,
        timeout: Duration,
    ) -> Result<Vec<DiscoveredNode>> {
        // 启动浏览
        let receiver = self
            .daemon
            .browse(service_type)
            .map_err(|e| anyhow!("Failed to browse mDNS: {}", e))?;

        let mut discovered = HashMap::new();
        let start = std::time::Instant::now();

        // 收集发现的节点
        while start.elapsed() < timeout {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    match event {
                        mdns_sd::ServiceEvent::ServiceResolved(info) => {
                            debug!("Discovered service: {}", info.get_fullname());

                            if let Some(node) =
                                DiscoveredNode::from_service_info(&info, &self.node_id)
                            {
                                info!(
                                    "Discovered peer: {} at {} (DID: {})",
                                    node.node_id, node.address, node.did
                                );
                                discovered.insert(node.node_id.clone(), node);
                            }
                        }
                        mdns_sd::ServiceEvent::ServiceRemoved(_service_type, fullname) => {
                            debug!("Service removed: {}", fullname);
                            discovered.retain(|_, node| !fullname.contains(&node.node_id));
                        }
                        _ => {}
                    }
                }
                Err(_) => {
                    continue;
                }
            }
        }

        let nodes: Vec<DiscoveredNode> = discovered.into_values().collect();
        info!(
            "Discovery complete for {}: found {} node(s)",
            service_type,
            nodes.len()
        );

        Ok(nodes)
    }
}

impl Drop for MdnsService {
    fn drop(&mut self) {
        // 尝试优雅关闭
        // 注意：这里不能直接调用 shutdown，因为 self 是 &mut
        // 实际的清理在显式调用 shutdown 时完成
        debug!("MdnsService dropped for node {}", self.node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[test]
    fn test_mdns_service_creation() {
        let metadata = HashMap::new();
        let service = MdnsService::new(
            "test-node-1",
            7676,
            "did:cis:test1",
            metadata,
        );
        
        assert!(service.is_ok());
        
        let service = service.unwrap();
        assert_eq!(service.node_id(), "test-node-1");
        
        // 清理
        service.shutdown().unwrap();
    }
    
    #[test]
    fn test_mdns_discover_timeout() {
        let metadata = HashMap::new();
        let service = MdnsService::new(
            "test-node-2",
            7677,
            "did:cis:test2",
            metadata,
        )
        .unwrap();
        
        // 设置短超时
        let nodes = service.discover(Duration::from_millis(500)).unwrap();
        
        // 应该返回空列表（没有其他节点）
        assert!(nodes.is_empty());
        
        service.shutdown().unwrap();
    }
    
    #[test]
    fn test_discovered_node_equality() {
        let node1 = DiscoveredNode {
            node_id: "node1".to_string(),
            address: "127.0.0.1:7676".parse().unwrap(),
            did: "did:cis:node1".to_string(),
            metadata: HashMap::new(),
        };
        
        let node2 = DiscoveredNode {
            node_id: "node1".to_string(),
            address: "127.0.0.1:7676".parse().unwrap(),
            did: "did:cis:node1".to_string(),
            metadata: HashMap::new(),
        };
        
        assert_eq!(node1, node2);
    }
    
    #[test]
    fn test_mdns_service_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("capability".to_string(), "memory_sync".to_string());
        metadata.insert("version".to_string(), "1.0.0".to_string());
        
        let service = MdnsService::new(
            "test-node-3",
            7678,
            "did:cis:test3",
            metadata,
        );
        
        assert!(service.is_ok());
        service.unwrap().shutdown().unwrap();
    }
}
