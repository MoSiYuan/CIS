//! P2P 网络模块
//!
//! 提供节点发现、连接管理和数据同步功能。

pub mod crdt;
pub mod discovery;
pub mod gossip;
pub mod peer;
pub mod sync;
pub mod transport;
pub mod dht;
pub mod nat;

pub use crdt::{LWWRegister, GCounter, PNCounter, ORSet, VectorClock};
pub use discovery::{DiscoveryService, PeerDiscoveryInfo};
pub use gossip::GossipProtocol;
pub use peer::{PeerManager, PeerInfo};
pub use sync::{MemorySyncManager, SyncMemoryEntry, SyncRequest, SyncResponse};
pub use transport::{QuicTransport, Connection};
pub use dht::{DhtService, DhtConfig};
pub use nat::{NatTraversal, NatType};

use crate::error::{CisError, Result};
use std::sync::Arc;
use std::time::Duration;
use std::net::SocketAddr;

use tokio::sync::RwLock;

/// P2P 网络管理器
pub struct P2PNetwork {
    /// 本节点信息
    pub local_node: NodeInfo,
    /// 节点发现服务
    discovery: Arc<DiscoveryService>,
    /// DHT 服务
    dht: Arc<DhtService>,
    /// NAT 穿透
    nat: Arc<RwLock<NatTraversal>>,
    /// 传输层
    transport: Arc<QuicTransport>,
    /// Gossip 协议
    gossip: Arc<GossipProtocol>,
    /// 对等节点管理
    peer_manager: Arc<PeerManager>,
    /// 配置
    config: P2PConfig,
}

/// P2P 配置
#[derive(Debug, Clone)]
pub struct P2PConfig {
    /// 启用 DHT
    pub enable_dht: bool,
    /// Bootstrap 节点
    pub bootstrap_nodes: Vec<String>,
    /// 启用 NAT 穿透
    pub enable_nat_traversal: bool,
    /// 外部地址（手动指定）
    pub external_address: Option<String>,
}

/// 节点信息
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub did: String,
    pub addresses: Vec<String>,
    pub capabilities: Vec<String>,
    pub public_key: Vec<u8>,
}

impl P2PNetwork {
    /// 创建 P2P 网络
    pub async fn new(
        node_id: String,
        did: String,
        listen_addr: &str,
        config: P2PConfig,
    ) -> Result<Self> {
        let peer_manager = Arc::new(PeerManager::new());
        let discovery = Arc::new(DiscoveryService::new(node_id.clone()));
        let transport = Arc::new(QuicTransport::new(listen_addr).await?);
        let gossip = Arc::new(GossipProtocol::new(
            Arc::clone(&peer_manager),
            Arc::clone(&transport),
        ));
        
        // DHT 服务
        let dht = Arc::new(DhtService::new(
            node_id.clone(),
            config.bootstrap_nodes.clone(),
        ));
        
        // NAT 穿透
        let port = listen_addr.parse::<SocketAddr>()
            .map(|a| a.port())
            .unwrap_or(7677);
        let nat = Arc::new(RwLock::new(NatTraversal::new(port)));
        
        let local_node = NodeInfo {
            node_id: node_id.clone(),
            did,
            addresses: vec![listen_addr.to_string()],
            capabilities: vec!["memory_sync".to_string(), "skill_invoke".to_string()],
            public_key: vec![], // 从 DID 获取
        };
        
        Ok(Self {
            local_node,
            discovery,
            dht,
            nat,
            transport,
            gossip,
            peer_manager,
            config,
        })
    }
    
    /// 启动 P2P 网络
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting P2P network for node {}", self.local_node.node_id);
        
        // NAT 穿透
        if self.config.enable_nat_traversal {
            match self.nat.write().await.try_traversal().await {
                Ok(Some(external_addr)) => {
                    tracing::info!("External address discovered: {}", external_addr);
                    // 更新本地节点地址
                    // self.local_node.addresses.push(external_addr.to_string());
                }
                Ok(None) => {
                    tracing::warn!("NAT traversal failed, may require manual port forwarding");
                }
                Err(e) => {
                    tracing::warn!("NAT traversal error: {}", e);
                }
            }
        }
        
        // 启动传输层监听
        let transport = Arc::clone(&self.transport);
        tokio::spawn(async move {
            if let Err(e) = transport.start_listening().await {
                tracing::error!("Transport error: {}", e);
            }
        });
        
        // 启动节点发现
        let discovery = Arc::clone(&self.discovery);
        let local_node = self.local_node.clone();
        tokio::spawn(async move {
            if let Err(e) = discovery.start(local_node).await {
                tracing::error!("Discovery error: {}", e);
            }
        });
        
        // 启动 DHT（如果启用）
        if self.config.enable_dht {
            let dht = Arc::clone(&self.dht);
            let local_node = self.local_node.clone();
            tokio::spawn(async move {
                if let Err(e) = dht.start(local_node).await {
                    tracing::error!("DHT error: {}", e);
                }
                // 定期 announce
                loop {
                    if let Err(e) = dht.announce().await {
                        tracing::error!("DHT announce error: {}", e);
                    }
                    tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
                }
            });
        }
        
        // 启动 Gossip 协议
        let gossip = Arc::clone(&self.gossip);
        tokio::spawn(async move {
            if let Err(e) = gossip.start().await {
                tracing::error!("Gossip error: {}", e);
            }
        });
        
        // 启动对等节点维护
        self.start_peer_maintenance().await?;
        
        tracing::info!("P2P network started");
        Ok(())
    }
    
    /// 连接到节点
    pub async fn connect(&self, addr: &str) -> Result<Connection> {
        self.transport.connect(addr).await
    }
    
    /// 广播消息
    pub async fn broadcast(&self, topic: &str, data: Vec<u8>) -> Result<()> {
        self.gossip.broadcast(topic, data).await
    }
    
    /// 订阅主题
    pub async fn subscribe<F>(&self, topic: &str, callback: F) -> Result<()>
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        self.gossip.subscribe(topic, callback).await
    }
    
    /// 同步公域记忆（使用 MemorySyncManager）
    /// 
    /// 注意：推荐使用 MemorySyncManager 进行更完善的同步管理
    pub async fn sync_public_memory(&self, peer_id: &str) -> Result<()> {
        let peer = self.peer_manager.get_peer(peer_id).await?
            .ok_or_else(|| CisError::p2p("Peer not found"))?;
        
        let conn = self.transport.connect(&peer.address).await?;
        
        // 使用新的同步协议
        let request = sync::SyncRequest {
            node_id: self.local_node.node_id.clone(),
            since: peer.last_sync_at,
            known_keys: vec![], // 简化的实现
        };
        
        let message = sync::SyncMessage::Request(request);
        conn.send(serde_json::to_vec(&message)?).await?;
        
        // 更新同步时间
        self.peer_manager.update_sync_time(peer_id).await?;
        
        Ok(())
    }
    
    /// 启动对等节点维护任务
    async fn start_peer_maintenance(&self) -> Result<()> {
        let peer_manager = Arc::clone(&self.peer_manager);
        let transport = Arc::clone(&self.transport);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            
            loop {
                interval.tick().await;
                
                // 检查对等节点健康状态
                let peers = peer_manager.get_all_peers().await;
                
                for peer in peers {
                    if peer.is_unhealthy() {
                        tracing::warn!("Peer {} is unhealthy, reconnecting", peer.node_id);
                        
                        match transport.connect(&peer.address).await {
                            Ok(_conn) => {
                                if let Err(e) = peer_manager.mark_healthy(&peer.node_id).await {
                                    tracing::error!("Failed to mark peer healthy: {}", e);
                                }
                                
                                // 重新同步
                                // ...
                            }
                            Err(e) => {
                                tracing::error!("Failed to reconnect to {}: {}", peer.node_id, e);
                                if let Err(e) = peer_manager.mark_unhealthy(&peer.node_id).await {
                                    tracing::error!("Failed to mark peer unhealthy: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// 获取已连接的对等节点
    pub async fn get_connected_peers(&self) -> Vec<PeerInfo> {
        self.peer_manager.get_connected_peers().await
    }
    
    /// 获取特定对等节点信息
    pub async fn get_peer(&self, node_id: &str) -> Result<Option<PeerInfo>> {
        self.peer_manager.get_peer(node_id).await
    }
}
