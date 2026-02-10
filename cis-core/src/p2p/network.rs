//! P2P Network 状态管理
//!
//! 提供全局 P2P 网络单例，整合 mDNS 发现和 QUIC 传输

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use tokio::sync::{OnceCell, RwLock};
use tracing::{debug, error, info, warn};

use crate::p2p::{
    mdns_service::{DiscoveredNode, MdnsService},
    quic_transport::{ConnectionInfo, QuicTransport},
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
}

/// P2P 网络管理器
pub struct P2PNetwork {
    /// 配置
    config: P2PConfig,
    /// mDNS 服务
    mdns: Option<MdnsService>,
    /// QUIC 传输层
    transport: QuicTransport,
    /// 已发现的节点
    discovered_peers: Arc<RwLock<HashMap<String, DiscoveredNode>>>,
    /// 启动时间
    started_at: std::time::Instant,
}

impl P2PNetwork {
    /// 获取全局实例
    ///
    /// # Returns
    /// - `Some(Arc<P2PNetwork>)` - 网络已启动
    /// - `None` - 网络未启动
    pub async fn global() -> Option<Arc<Self>> {
        let guard = P2P_INSTANCE.get_or_init(|| RwLock::new(None)).read().await;
        guard.as_ref().map(Arc::clone)
    }

    /// 初始化并启动 P2P 网络
    ///
    /// 如果网络已启动，返回现有实例
    pub async fn start(config: P2PConfig) -> Result<Arc<Self>> {
        // 检查是否已启动
        if let Some(existing) = Self::global().await {
            info!("P2P network already running, returning existing instance");
            return Ok(existing);
        }

        info!("Starting P2P network for node {}", config.node_id);

        // 获取写锁
        let mut guard = P2P_INSTANCE
            .get_or_init(|| RwLock::new(None))
            .write()
            .await;

        // 双重检查
        if let Some(existing) = guard.as_ref() {
            return Ok(Arc::clone(existing));
        }

        // 创建 QUIC 传输层
        let transport = QuicTransport::bind(&config.listen_addr, &config.node_id)
            .await
            .context("Failed to bind QUIC transport")?;

        // 创建 mDNS 服务（如果启用）
        let mdns = if config.enable_mdns {
            let mdns = MdnsService::new(
                &config.node_id,
                config.port,
                &config.did,
                config.metadata.clone(),
            )
            .context("Failed to create mDNS service")?;
            Some(mdns)
        } else {
            None
        };

        let network = Arc::new(P2PNetwork {
            config,
            mdns,
            transport,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
        });

        // 启动后台任务
        network.start_background_tasks().await?;

        // 存储实例
        *guard = Some(Arc::clone(&network));

        info!(
            "P2P network started successfully on {}",
            network.config.listen_addr
        );

        Ok(network)
    }

    /// 停止 P2P 网络
    pub async fn stop() -> Result<()> {
        info!("Stopping P2P network...");

        let mut guard = P2P_INSTANCE
            .get_or_init(|| RwLock::new(None))
            .write()
            .await;

        if let Some(network) = guard.take() {
            // 关闭传输层
            Arc::try_unwrap(network)
                .map_err(|_| anyhow!("Cannot stop: network has active references"))?
                .transport
                .shutdown()
                .await?;

            info!("P2P network stopped");
        } else {
            warn!("P2P network was not running");
        }

        Ok(())
    }

    /// 获取网络状态
    pub async fn status(&self) -> NetworkStatus {
        NetworkStatus {
            running: true,
            node_id: self.config.node_id.clone(),
            listen_addr: self.config.listen_addr.clone(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            connected_peers: self.transport.list_connections().await.len(),
            discovered_peers: self.discovered_peers.read().await.len(),
        }
    }

    /// 连接到指定节点
    pub async fn connect(&self, addr: &str) -> Result<()> {
        let socket_addr: SocketAddr = addr
            .parse()
            .map_err(|e| anyhow!("Invalid address '{}': {}", addr, e))?;

        // 从地址推断 node_id（实际应该从发现服务获取）
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
        let connected = self.transport.list_connections().await;
        let connected_ids: std::collections::HashSet<_> =
            connected.iter().map(|c| c.node_id.clone()).collect();

        discovered
            .values()
            .map(|node| PeerInfo {
                node_id: node.node_id.clone(),
                did: node.did.clone(),
                address: node.address.to_string(),
                connected: connected_ids.contains(&node.node_id),
                last_seen: std::time::SystemTime::now(), // TODO: 记录实际时间
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

    /// 启动后台任务
    async fn start_background_tasks(&self) -> Result<()> {
        // 启动 mDNS 发现任务
        if let Some(mdns) = &self.mdns {
            let mdns = mdns; // 解引用
            let discovered = Arc::clone(&self.discovered_peers);

            // 启动发现任务
            tokio::task::spawn_blocking({
                let mdns = unsafe {
                    // SAFETY: 我们知道 mDNS 服务在 P2PNetwork 生命周期内有效
                    std::mem::transmute::<&MdnsService, &'static MdnsService>(mdns)
                };
                move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    rt.block_on(async {
                        loop {
                            match mdns.discover(Duration::from_secs(30)) {
                                Ok(nodes) => {
                                    let mut peers = discovered.write().await;
                                    for node in nodes {
                                        info!(
                                            "Discovered peer: {} at {}",
                                            node.node_id, node.address
                                        );
                                        peers.insert(node.node_id.clone(), node);
                                    }
                                }
                                Err(e) => {
                                    warn!("Discovery error: {}", e);
                                }
                            }
                            tokio::time::sleep(Duration::from_secs(30)).await;
                        }
                    });
                }
            });
        }

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
