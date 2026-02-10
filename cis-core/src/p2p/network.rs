//! P2P Network 状态管理
//!
//! 提供全局 P2P 网络单例，整合 mDNS 发现和 QUIC 传输

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::future::ready;

use crate::error::{CisError, Result};
use chrono::{DateTime, Utc};
use tokio::sync::{OnceCell, RwLock};
use tracing::{debug, error, info, warn};

use crate::p2p::{
    mdns_service::{DiscoveredNode, MdnsService},
    transport::{ConnectionInfo, QuicTransport},
};

/// 全局 P2P 网络实例
static P2P_INSTANCE: OnceCell<RwLock<Option<Arc<P2PNetwork>>>> = OnceCell::const_new();

/// P2P 网络配置
#[derive(Debug, Clone)]
pub struct P2PConfig {
    /// 节点 ID
    pub node_id: String,
    /// DID
    pub did: String,
    /// 监听地址
    pub listen_addr: String,
    /// 服务端口
    pub port: u16,
    /// 启用 mDNS
    pub enable_mdns: bool,
    /// 额外元数据
    pub metadata: HashMap<String, String>,
    /// 启用 DHT（向后兼容）
    pub enable_dht: bool,
    /// Bootstrap 节点（向后兼容）
    pub bootstrap_nodes: Vec<String>,
    /// 启用 NAT 穿透（向后兼容）
    pub enable_nat_traversal: bool,
    /// 外部地址（向后兼容）
    pub external_address: Option<String>,
}

impl Default for P2PConfig {
    fn default() -> Self {
        Self {
            node_id: format!("node-{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap()),
            did: format!("did:cis:{}", uuid::Uuid::new_v4()),
            listen_addr: "0.0.0.0:7677".to_string(),
            port: 7677,
            enable_mdns: true,
            metadata: HashMap::new(),
            enable_dht: false,
            bootstrap_nodes: vec![],
            enable_nat_traversal: false,
            external_address: None,
        }
    }
}

/// 网络状态
#[derive(Debug, Clone)]
pub struct NetworkStatus {
    /// 是否运行中
    pub running: bool,
    /// 节点 ID
    pub node_id: String,
    /// 监听地址
    pub listen_addr: String,
    /// 运行时长（秒）
    pub uptime_secs: u64,
    /// 已连接节点数
    pub connected_peers: usize,
    /// 已发现节点数
    pub discovered_peers: usize,
}

/// 对等节点信息
#[derive(Debug, Clone)]
pub struct PeerInfo {
    /// 节点 ID
    pub node_id: String,
    /// DID
    pub did: String,
    /// 地址
    pub address: String,
    /// 是否已连接
    pub connected: bool,
    /// 最后可见时间
    pub last_seen: std::time::SystemTime,
    /// 最后同步时间
    pub last_sync_at: Option<DateTime<Utc>>,
}

/// P2P 网络管理器
pub struct P2PNetwork {
    /// 配置
    config: P2PConfig,
    /// mDNS 服务
    mdns: Option<MdnsService>,
    /// QUIC 传输层
    transport: Arc<QuicTransport>,
    /// 已发现的节点
    discovered_peers: Arc<RwLock<HashMap<String, DiscoveredNode>>>,
    /// 启动时间
    started_at: std::time::Instant,
}

impl P2PNetwork {
    /// 创建新的 P2P 网络（向后兼容）
    pub async fn new(
        node_id: String,
        did: String,
        listen_addr: &str,
        config: P2PConfig,
    ) -> Result<Self> {
        let transport = Arc::new(QuicTransport::bind(listen_addr, &node_id).await?);
        
        let mdns = if config.enable_mdns {
            match MdnsService::new(
                &node_id,
                config.port,
                &did,
                config.metadata.clone(),
            ) {
                Ok(mdns) => Some(mdns),
                Err(e) => {
                    warn!("Failed to create mDNS service: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self {
            config: P2PConfig {
                node_id,
                did,
                listen_addr: listen_addr.to_string(),
                port: config.port,
                enable_mdns: config.enable_mdns,
                metadata: config.metadata,
                enable_dht: config.enable_dht,
                bootstrap_nodes: config.bootstrap_nodes,
                enable_nat_traversal: config.enable_nat_traversal,
                external_address: config.external_address,
            },
            mdns,
            transport,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
        })
    }

    /// 启动 P2P 网络（向后兼容）
    pub async fn start_network(&self) -> Result<()> {
        self.start_background_tasks().await
    }

    /// 同步公域记忆（向后兼容）
    pub async fn sync_public_memory(&self, _peer_id: &str) -> Result<()> {
        // P2P 记忆同步尚未完全实现
        Err(CisError::p2p("P2P public memory sync not fully implemented".to_string()))
    }

    /// 获取全局实例
    ///
    /// # Returns
    /// - `Some(Arc<P2PNetwork>)` - 网络已启动
    /// - `None` - 网络未启动
    pub async fn global() -> Option<Arc<Self>> {
        let instance = P2P_INSTANCE.get()?;
        let guard = instance.read().await;
        guard.as_ref().map(Arc::clone)
    }

    /// 初始化并启动 P2P 网络
    ///
    /// 如果网络已启动，返回现有实例
    pub async fn start(config: P2PConfig) -> Result<Arc<Self>> {
        // 确保全局实例已初始化
        if P2P_INSTANCE.get().is_none() {
            let _ = P2P_INSTANCE.set(RwLock::new(None));
        }
        let instance = P2P_INSTANCE.get().unwrap();
        
        // 检查是否已启动
        {
            let guard = instance.read().await;
            if let Some(existing) = guard.as_ref() {
                info!("P2P network already running, returning existing instance");
                return Ok(Arc::clone(existing));
            }
        }

        info!("Starting P2P network for node {}", config.node_id);

        // 创建 QUIC 传输层
        let transport = QuicTransport::bind(&config.listen_addr, &config.node_id)
            .await
            .map_err(|e| CisError::p2p(format!("Failed to bind QUIC transport: {}", e)))?;

        // 创建 mDNS 服务（如果启用）
        let mdns = if config.enable_mdns {
            match MdnsService::new(
                &config.node_id,
                config.port,
                &config.did,
                config.metadata.clone(),
            ) {
                Ok(mdns) => Some(mdns),
                Err(e) => {
                    warn!("Failed to create mDNS service: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let network = Arc::new(P2PNetwork {
            config,
            mdns,
            transport: Arc::new(transport),
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
        });

        // 启动后台任务
        network.start_background_tasks().await?;

        // 存储实例
        {
            let mut guard = instance.write().await;
            *guard = Some(Arc::clone(&network));
        }

        info!(
            "P2P network started successfully on {}",
            network.config.listen_addr
        );

        Ok(network)
    }

    /// 停止 P2P 网络
    pub async fn stop() -> Result<()> {
        info!("Stopping P2P network...");

        let instance = match P2P_INSTANCE.get() {
            Some(i) => i,
            None => {
                warn!("P2P network was not initialized");
                return Ok(());
            }
        };
        
        let mut guard = instance.write().await;

        if let Some(network) = guard.take() {
            // 关闭传输层
            if let Ok(network) = Arc::try_unwrap(network) {
                if let Ok(transport) = Arc::try_unwrap(network.transport) {
                    transport.shutdown().await?;
                }
                info!("P2P network stopped");
            } else {
                warn!("Cannot stop: network has active references");
            }
        } else {
            warn!("P2P network was not running");
        }

        Ok(())
    }

    /// 获取网络状态
    pub async fn status(&self) -> NetworkStatus {
        let connections = self.transport.list_connections().await;
        NetworkStatus {
            running: true,
            node_id: self.config.node_id.clone(),
            listen_addr: self.config.listen_addr.clone(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            connected_peers: connections.len(),
            discovered_peers: self.discovered_peers.read().await.len(),
        }
    }

    /// 连接到指定节点
    pub async fn connect(&self, addr: &str) -> Result<()> {
        let socket_addr: SocketAddr = addr.parse()
            .map_err(|e| CisError::p2p(format!("Invalid address: {}", e)))?;

        // 从地址推断 node_id
        let node_id = format!("peer-{}", socket_addr.port());

        self.transport.connect(&node_id, socket_addr).await?;

        info!("Connected to {} at {}", node_id, addr);
        Ok(())
    }

    /// 断开与节点的连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()> {
        self.transport.disconnect(node_id).await?;
        info!("Disconnected from {}", node_id);
        Ok(())
    }

    /// 获取已发现的节点列表
    pub async fn discovered_peers(&self) -> Vec<PeerInfo> {
        let discovered = self.discovered_peers.read().await;
        let connections = self.transport.list_connections().await;
        let connected_ids: std::collections::HashSet<_> =
            connections.iter().map(|c| c.node_id.clone()).collect();

        discovered
            .values()
            .map(|node| PeerInfo {
                node_id: node.node_id.clone(),
                did: node.did.clone(),
                address: node.address.to_string(),
                connected: connected_ids.contains(&node.node_id),
                last_seen: std::time::SystemTime::now(),
                last_sync_at: None,
            })
            .collect()
    }

    /// 获取已连接的节点列表
    pub async fn connected_peers(&self) -> Vec<PeerInfo> {
        let connections = self.transport.list_connections().await;
        let discovered = self.discovered_peers.read().await;

        connections
            .into_iter()
            .map(|conn| {
                let discovered_info = discovered.get(&conn.node_id);
                PeerInfo {
                    node_id: conn.node_id.clone(),
                    did: discovered_info
                        .map(|d| d.did.clone())
                        .unwrap_or_default(),
                    address: conn.address.to_string(),
                    connected: true,
                    last_seen: std::time::SystemTime::now(),
                    last_sync_at: None,
                }
            })
            .collect()
    }

    /// 发送消息到指定节点
    pub async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()> {
        self.transport.send(node_id, data).await
    }

    /// 广播消息到所有连接节点
    pub async fn broadcast(&self, data: &[u8]) -> Result<usize> {
        let connections = self.transport.list_connections().await;
        let mut sent = 0;

        for conn in connections {
            if self.transport.send(&conn.node_id, data).await.is_ok() {
                sent += 1;
            }
        }

        Ok(sent)
    }

    /// 获取已连接节点列表（别名，用于兼容性）
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.connected_peers().await
    }

    /// 获取特定节点信息
    pub async fn get_peer(&self, node_id: &str) -> Option<PeerInfo> {
        let connections = self.transport.list_connections().await;
        connections.into_iter().find(|c| c.node_id == node_id).map(|conn| {
            let discovered = self.discovered_peers.blocking_read();
            let discovered_info = discovered.get(node_id);
            PeerInfo {
                node_id: conn.node_id,
                did: discovered_info.map(|d| d.did.clone()).unwrap_or_default(),
                address: conn.address.to_string(),
                connected: true,
                last_seen: std::time::SystemTime::now(),
                last_sync_at: None,
            }
        })
    }

    /// 订阅主题（简化实现）
    pub async fn subscribe<F>(&self, _topic: &str, _callback: F) -> Result<()>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        // 主题订阅尚未完全实现
        Err(CisError::p2p("Topic subscription not fully implemented".to_string()))
    }

    /// 启动后台任务
    async fn start_background_tasks(&self) -> Result<()> {
        // 启动 mDNS 发现任务
        if let Some(_mdns) = &self.mdns {
            // TODO: 启动 mDNS 发现任务
            debug!("mDNS service started");
        }

        // 启动传输层监听
        let transport: Arc<QuicTransport> = Arc::clone(&self.transport);
        tokio::spawn(async move {
            if let Err(e) = transport.start_listening().await {
                error!("Transport error: {}", e);
            }
        });

        Ok(())
    }

    /// 获取节点 ID
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// 获取 DID
    pub fn did(&self) -> &str {
        &self.config.did
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_p2p_config_default() {
        let config = P2PConfig::default();
        assert!(!config.node_id.is_empty());
        assert!(!config.did.is_empty());
        assert_eq!(config.port, 7677);
    }

    #[tokio::test]
    async fn test_network_status() {
        let status = NetworkStatus {
            running: true,
            node_id: "test".to_string(),
            listen_addr: "0.0.0.0:7677".to_string(),
            uptime_secs: 100,
            connected_peers: 5,
            discovered_peers: 10,
        };

        assert!(status.running);
        assert_eq!(status.connected_peers, 5);
    }
}
