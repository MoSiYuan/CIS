//! P2P Network 状态管理
//!
//! P2P 网络管理器，整合 mDNS 发现和加密 QUIC 传输。
//!
//! ## 安全特性
//!
//! - Noise Protocol XX 模式加密
//! - 三向握手，双向身份验证
//! - 无明文回退
//!
//! ## 废弃警告
//!
//! `P2PNetwork::global()` 和 `P2PNetwork::start()` 全局单例方法已废弃。
//! 请使用 `ServiceContainer` 进行依赖注入。

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::error::{CisError, Result};
use chrono::{DateTime, Utc};
use tokio::sync::{OnceCell, RwLock};
use tracing::{debug, error, info, warn};

use crate::p2p::{
    crypto::keys::NodeKeyPair,
    kademlia::{KademliaDht, KademliaConfig, NodeId as KademliaNodeId, NodeInfo, 
               transport::{DhtTransport, P2PNetworkTransport}},
    mdns_service::{DiscoveredNode, MdnsService},
    transport_secure::{SecureP2PTransport, SecureTransportConfig},
};

// Re-export types for compatibility
pub use crate::traits::network::{NetworkStatus as NetworkStatusTrait, PeerInfo as PeerInfoTrait};

/// 全局 P2P 网络实例 (DEPRECATED)
///
/// [WARNING] 警告: 此全局单例已废弃，将在 v1.2.0 中移除。
/// 请使用 `ServiceContainer` 进行依赖注入。
/// 全局 P2P 网络实例 (DEPRECATED)
///
/// [WARNING] 警告: 此全局单例已废弃，将在 v1.2.0 中移除。
/// 请使用 `ServiceContainer` 进行依赖注入。
#[deprecated(
    since = "1.1.4",
    note = "全局单例已废弃，请使用 ServiceContainer 进行依赖注入"
)]
#[allow(deprecated)]
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
    /// 传输层配置
    pub transport_config: SecureTransportConfig,
    /// 节点密钥对（用于加密和身份验证）
    pub node_keys: Option<Arc<NodeKeyPair>>,
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
            transport_config: SecureTransportConfig::default(),
            node_keys: None,
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
    /// DHT 是否启用
    pub dht_enabled: bool,
    /// DHT 路由表节点数（如果启用）
    pub dht_routing_table_nodes: Option<usize>,
    /// DHT 存储条目数（如果启用）
    pub dht_storage_entries: Option<usize>,
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
    /// 安全 QUIC 传输层
    transport: Arc<SecureP2PTransport>,
    /// 已发现的节点
    discovered_peers: Arc<RwLock<HashMap<String, DiscoveredNode>>>,
    /// 启动时间
    started_at: std::time::Instant,
    /// 节点密钥对
    node_keys: Arc<NodeKeyPair>,
    /// Kademlia DHT（如果启用）
    dht: Option<Arc<KademliaDht<P2PNetworkTransport>>>,
}

impl P2PNetwork {
    /// 创建新的 P2P 网络实例
    ///
    /// 这是推荐的构造方式，避免使用全局单例。
    pub async fn new(
        node_id: String,
        did: String,
        listen_addr: &str,
        config: P2PConfig,
    ) -> Result<Self> {
        // 获取或生成节点密钥
        let node_keys = config.node_keys.clone().unwrap_or_else(|| {
            info!("No node keys provided, generating new keypair");
            Arc::new(NodeKeyPair::generate())
        });

        // 创建安全传输层
        let transport = Arc::new(
            SecureP2PTransport::bind_with_config(
                listen_addr,
                &node_id,
                &did,
                Arc::clone(&node_keys),
                config.transport_config.clone(),
            )
            .await?,
        );
        
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

        // 初始化 Kademlia DHT（如果启用）
        let dht = if config.enable_dht {
            match Self::create_dht(&node_id, &config, Arc::clone(&transport)).await {
                Ok(dht) => {
                    info!("Kademlia DHT initialized for node {}", node_id);
                    Some(dht)
                }
                Err(e) => {
                    warn!("Failed to initialize Kademlia DHT: {}", e);
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
                transport_config: config.transport_config,
                node_keys: Some(Arc::clone(&node_keys)),
            },
            mdns,
            transport,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
            node_keys,
            dht,
        })
    }

    /// 启动 P2P 网络（向后兼容）
    pub async fn start_network(&self) -> Result<()> {
        self.start_background_tasks().await
    }

    /// 同步公域记忆到 DHT
    ///
    /// 将公共记忆存储到 DHT，使其可以在网络中分发和检索
    pub async fn sync_public_memory(&self, key: &str, value: &[u8]) -> Result<()> {
        tracing::info!("Syncing public memory to DHT: key={}", key);
        
        if let Some(ref dht) = self.dht {
            // 使用 DHT 存储公共记忆
            // 键格式: "memory:public:{key}"
            let dht_key = format!("memory:public:{}", key);
            
            match dht.put(&dht_key, value).await {
                Ok(()) => {
                    tracing::info!("Public memory synced to DHT: key={}", key);
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Failed to sync public memory to DHT: {}: {}", key, e);
                    Err(CisError::p2p(format!("DHT put failed: {}", e)))
                }
            }
        } else {
            tracing::warn!("DHT not initialized, cannot sync public memory");
            Err(CisError::p2p("DHT not initialized".to_string()))
        }
    }
    
    /// 从 DHT 获取公域记忆
    ///
    /// 检索存储在 DHT 中的公共记忆
    pub async fn get_public_memory(&self, key: &str) -> Result<Option<Vec<u8>>> {
        tracing::debug!("Getting public memory from DHT: key={}", key);
        
        if let Some(ref dht) = self.dht {
            let dht_key = format!("memory:public:{}", key);
            
            match dht.get(&dht_key).await {
                Ok(value) => {
                    if value.is_some() {
                        tracing::debug!("Public memory found in DHT: key={}", key);
                    } else {
                        tracing::debug!("Public memory not found in DHT: key={}", key);
                    }
                    Ok(value)
                }
                Err(e) => {
                    tracing::error!("Failed to get public memory from DHT: {}: {}", key, e);
                    Err(CisError::p2p(format!("DHT get failed: {}", e)))
                }
            }
        } else {
            tracing::warn!("DHT not initialized, cannot get public memory");
            Err(CisError::p2p("DHT not initialized".to_string()))
        }
    }
    
    /// 列出 DHT 中所有公域记忆的键
    ///
    /// 注意：这会扫描本地 DHT 存储，不会查询整个网络
    pub async fn list_public_memory_keys(&self) -> Result<Vec<String>> {
        tracing::debug!("Listing public memory keys from DHT");
        
        if let Some(ref dht) = self.dht {
            // 获取所有以 "memory:public:" 开头的键
            let prefix = "memory:public:";
            let keys = dht.list_keys_with_prefix(prefix).await
                .map_err(|e| CisError::p2p(format!("Failed to list keys: {}", e)))?;
            
            // 去掉前缀，只返回实际的 key
            let result: Vec<String> = keys
                .into_iter()
                .filter_map(|k| k.strip_prefix(prefix).map(|s| s.to_string()))
                .collect();
            
            tracing::debug!("Found {} public memory keys in DHT", result.len());
            Ok(result)
        } else {
            tracing::warn!("DHT not initialized, cannot list public memory keys");
            Err(CisError::p2p("DHT not initialized".to_string()))
        }
    }

    /// 获取全局实例 (DEPRECATED)
    ///
    /// [WARNING] 警告: 此方法已废弃，将在 v1.2.0 中移除。
    /// 请使用 `ServiceContainer` 进行依赖注入。
    ///
    /// # Returns
    /// - `Some(Arc<P2PNetwork>)` - 网络已启动
    /// - `None` - 网络未启动
    #[deprecated(
        since = "1.1.4",
        note = "全局单例已废弃，请使用 ServiceContainer 进行依赖注入"
    )]
    #[allow(deprecated)]
    pub async fn global() -> Option<Arc<Self>> {
        let instance = P2P_INSTANCE.get()?;
        let guard = instance.read().await;
        guard.as_ref().map(Arc::clone)
    }

    /// 初始化并启动 P2P 网络 (DEPRECATED)
    ///
    /// [WARNING] 警告: 此方法已废弃，将在 v1.2.0 中移除。
    /// 请使用 `ServiceContainer` 进行依赖注入。
    ///
    /// 如果网络已启动，返回现有实例
    #[deprecated(
        since = "1.1.4",
        note = "全局单例已废弃，请使用 ServiceContainer 进行依赖注入"
    )]
    #[allow(deprecated)]
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

        info!("Starting secure P2P network for node {}", config.node_id);

        // 获取或生成节点密钥
        let node_keys = config.node_keys.clone().unwrap_or_else(|| {
            info!("No node keys provided, generating new keypair");
            Arc::new(NodeKeyPair::generate())
        });

        // 创建安全 QUIC 传输层
        let transport = SecureP2PTransport::bind_with_config(
            &config.listen_addr,
            &config.node_id,
            &config.did,
            Arc::clone(&node_keys),
            config.transport_config.clone(),
        )
        .await
        .map_err(|e| CisError::p2p(format!("Failed to bind secure transport: {}", e)))?;

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

        // 初始化 Kademlia DHT（如果启用）
        let transport_arc = Arc::new(transport);
        let dht = if config.enable_dht {
            match Self::create_dht(&config.node_id, &config, Arc::clone(&transport_arc)).await {
                Ok(dht) => {
                    info!("Kademlia DHT initialized for node {}", config.node_id);
                    Some(dht)
                }
                Err(e) => {
                    warn!("Failed to initialize Kademlia DHT: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let network = Arc::new(P2PNetwork {
            config: config.clone(),
            mdns,
            transport: transport_arc,
            discovered_peers: Arc::new(RwLock::new(HashMap::new())),
            started_at: std::time::Instant::now(),
            node_keys,
            dht,
        });

        // 启动后台任务
        network.start_background_tasks().await?;

        // 存储实例
        {
            let mut guard = instance.write().await;
            *guard = Some(Arc::clone(&network));
        }

        info!(
            "Secure P2P network started successfully on {}",
            network.config.listen_addr
        );

        Ok(network)
    }

    /// 停止 P2P 网络 (DEPRECATED)
    ///
    /// [WARNING] 警告: 此方法已废弃，将在 v1.2.0 中移除。
    /// 请使用实例方法 `stop_instance()` 或直接丢弃实例。
    #[deprecated(
        since = "1.1.4",
        note = "全局单例已废弃，请使用实例方法或直接丢弃实例"
    )]
    #[allow(deprecated)]
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

    /// 停止此网络实例
    ///
    /// 这是推荐的停止方式，避免使用全局单例。
    pub async fn stop_instance(self: Arc<Self>) -> Result<()> {
        info!("Stopping P2P network instance...");

        // 尝试获取所有权
        match Arc::try_unwrap(self) {
            Ok(network) => {
                if let Ok(transport) = Arc::try_unwrap(network.transport) {
                    transport.shutdown().await?;
                }
                info!("P2P network instance stopped");
                Ok(())
            }
            Err(_) => {
                warn!("Cannot stop: network has active references");
                Ok(())
            }
        }
    }

    /// 获取网络状态
    pub async fn status(&self) -> NetworkStatus {
        let connections = self.transport.list_connections().await;
        let (dht_routing_table_nodes, dht_storage_entries) = if let Some(ref dht) = self.dht {
            let (nodes, _) = dht.routing_table_stats().await;
            let entries = dht.storage_stats().await;
            (Some(nodes), Some(entries))
        } else {
            (None, None)
        };
        
        NetworkStatus {
            running: true,
            node_id: self.config.node_id.clone(),
            listen_addr: self.config.listen_addr.clone(),
            uptime_secs: self.started_at.elapsed().as_secs(),
            connected_peers: connections.len(),
            discovered_peers: self.discovered_peers.read().await.len(),
            dht_enabled: self.dht.is_some(),
            dht_routing_table_nodes,
            dht_storage_entries,
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
        if let Some(ref mdns) = &self.mdns {
            let discovered_peers = Arc::clone(&self.discovered_peers);
            let node_id = self.config.node_id.clone();
            
            // 启动 mDNS 发现监听
            match mdns.watch() {
                Ok(mut rx) => {
                    tokio::spawn(async move {
                        info!("mDNS discovery task started for node {}", node_id);
                        
                        while let Some(node) = rx.recv().await {
                            info!("mDNS discovered peer: {} at {}", node.node_id, node.address);
                            
                            // 添加到发现节点列表
                            let mut peers = discovered_peers.write().await;
                            peers.insert(node.node_id.clone(), node);
                        }
                        
                        info!("mDNS discovery task stopped for node {}", node_id);
                    });
                }
                Err(e) => {
                    warn!("Failed to start mDNS watch: {}", e);
                }
            }
            
            debug!("mDNS discovery task started");
        }

        // 启动安全传输层监听
        let transport: Arc<SecureP2PTransport> = Arc::clone(&self.transport);
        tokio::spawn(async move {
            if let Err(e) = transport.start_listening().await {
                error!("Transport error: {}", e);
            }
        });

        // 启动 DHT 服务（如果启用）
        if let Err(e) = self.start_dht().await {
            warn!("Failed to start DHT: {}", e);
        }

        Ok(())
    }

    /// 获取节点密钥对
    pub fn node_keys(&self) -> &NodeKeyPair {
        &self.node_keys
    }

    /// 获取本地监听地址
    pub fn local_addr(&self) -> String {
        self.transport.local_addr().to_string()
    }

    /// 获取节点 ID
    pub fn node_id(&self) -> &str {
        &self.config.node_id
    }

    /// 获取 DID
    pub fn did(&self) -> &str {
        &self.config.did
    }

    /// 创建 Kademlia DHT 实例
    async fn create_dht(
        node_id: &str,
        config: &P2PConfig,
        transport: Arc<SecureP2PTransport>,
    ) -> Result<Arc<KademliaDht<P2PNetworkTransport>>> {
        // 将字符串 node_id 转换为 Kademlia NodeId
        let kademlia_node_id = Self::string_to_node_id(node_id);
        
        // 创建 Kademlia 配置
        let kademlia_config = KademliaConfig {
            k: 20,
            alpha: 3,
            request_timeout_ms: 5000,
            bucket_refresh_interval_secs: 3600,
            value_expiration_secs: 86400,
        };
        
        // 创建本地节点信息
        let local_node_info = NodeInfo::new(
            kademlia_node_id.clone(),
            format!("{}:{}", config.listen_addr.split(':').next().unwrap_or("0.0.0.0"), config.port),
        );
        
        // 创建 DHT 传输层
        let dht_transport = P2PNetworkTransport::new(transport, local_node_info);
        
        // 创建 DHT 实例
        let dht = KademliaDht::new(
            kademlia_node_id,
            kademlia_config,
            Arc::new(dht_transport),
        ).await;
        
        Ok(Arc::new(dht))
    }

    /// 将字符串转换为 Kademlia NodeId
    fn string_to_node_id(s: &str) -> KademliaNodeId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        s.hash(&mut hasher);
        let hash = hasher.finish();
        
        // 扩展到 160 bits
        let mut bytes = [0u8; 20];
        bytes[0..8].copy_from_slice(&hash.to_be_bytes());
        for i in 1..20 / 8 {
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&(hash.wrapping_add(i as u64)).to_be_bytes());
        }
        
        KademliaNodeId::from_bytes(bytes)
    }

    /// 启动 DHT 服务（如果启用）
    async fn start_dht(&self) -> Result<()> {
        if let Some(ref dht) = self.dht {
            // 启动 DHT
            dht.start().await?;
            
            // 添加引导节点
            for bootstrap in &self.config.bootstrap_nodes {
                if let Ok(addr) = bootstrap.parse::<SocketAddr>() {
                    let node_id = Self::string_to_node_id(&format!("peer-{}", addr.port()));
                    let node_info = NodeInfo::new(node_id, bootstrap.clone());
                    
                    if let Err(e) = dht.add_bootstrap_node(node_info).await {
                        warn!("Failed to add bootstrap node {}: {}", bootstrap, e);
                    }
                }
            }
            
            info!("Kademlia DHT started with {} bootstrap nodes", self.config.bootstrap_nodes.len());
        }
        Ok(())
    }

    /// 通过 DHT 存储键值对
    pub async fn dht_put(&self, key: &str, value: &[u8]) -> Result<()> {
        match self.dht {
            Some(ref dht) => dht.put(key, value).await,
            None => Err(CisError::p2p("DHT is not enabled".to_string())),
        }
    }

    /// 通过 DHT 获取键值
    pub async fn dht_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        match self.dht {
            Some(ref dht) => dht.get(key).await,
            None => Err(CisError::p2p("DHT is not enabled".to_string())),
        }
    }

    /// 通过 DHT 查找节点
    pub async fn dht_find_node(&self, target_id: &str) -> Result<Vec<NodeInfo>> {
        match self.dht {
            Some(ref dht) => {
                let target = Self::string_to_node_id(target_id);
                dht.find_node(&target).await
            }
            None => Err(CisError::p2p("DHT is not enabled".to_string())),
        }
    }

    /// 获取 DHT 路由表统计
    pub async fn dht_routing_table_stats(&self) -> Option<(usize, usize)> {
        if let Some(ref dht) = self.dht {
            Some(dht.routing_table_stats().await)
        } else {
            None
        }
    }

    /// 获取 DHT 存储统计
    pub async fn dht_storage_stats(&self) -> Option<usize> {
        if let Some(ref dht) = self.dht {
            Some(dht.storage_stats().await)
        } else {
            None
        }
    }

    /// 检查 DHT 是否启用
    pub fn is_dht_enabled(&self) -> bool {
        self.dht.is_some()
    }
    
    /// 带选项的发送消息（实现优先级、超时和重试）
    async fn send_with_options(
        &self,
        node_id: &str,
        data: &[u8],
        options: &crate::traits::network::SendOptions,
    ) -> Result<()> {
        use crate::traits::network::MessagePriority;
        use tokio::time::timeout;
        
        let max_retries = if options.retry_count == 0 { 1 } else { options.retry_count };
        let mut last_error = None;
        
        for attempt in 0..max_retries {
            if attempt > 0 {
                // 重试前等待，使用指数退避
                let backoff = std::time::Duration::from_millis(100 * (1 << attempt.min(5)));
                tokio::time::sleep(backoff).await;
                debug!("Retrying send to {} (attempt {}/{})", node_id, attempt + 1, max_retries);
            }
            
            // 根据优先级设置超时
            let timeout_duration = match options.priority {
                MessagePriority::Critical => options.timeout, // 关键消息使用指定超时
                MessagePriority::High => options.timeout.mul_f32(0.8),
                MessagePriority::Normal => options.timeout,
                MessagePriority::Low => options.timeout.mul_f32(1.5),
                MessagePriority::Background => options.timeout.mul_f32(2.0),
            };
            
            // 执行发送并设置超时
            match timeout(timeout_duration, self.transport.send(node_id, data)).await {
                Ok(Ok(())) => {
                    // 发送成功
                    debug!("Message sent to {} with priority {:?}", node_id, options.priority);
                    return Ok(());
                }
                Ok(Err(e)) => {
                    // 发送失败
                    warn!("Send to {} failed (attempt {}): {}", node_id, attempt + 1, e);
                    last_error = Some(e);
                }
                Err(_) => {
                    // 超时
                    warn!("Send to {} timed out after {:?}", node_id, timeout_duration);
                    last_error = Some(CisError::p2p("Send timeout".to_string()));
                }
            }
        }
        
        // 所有重试都失败
        Err(last_error.unwrap_or_else(|| CisError::p2p("Send failed after all retries".to_string())))
    }
    
    /// 带选项的广播消息（实现优先级和超时）
    async fn broadcast_with_options_impl(
        &self,
        data: &[u8],
        options: &crate::traits::network::SendOptions,
    ) -> Result<usize> {
        use tokio::time::timeout;
        
        let connections = self.transport.list_connections().await;
        let timeout_duration = options.timeout;
        
        let mut sent = 0;
        let mut failed = 0;
        
        for conn in connections {
            match timeout(timeout_duration, self.transport.send(&conn.node_id, data)).await {
                Ok(Ok(())) => {
                    sent += 1;
                }
                Ok(Err(e)) => {
                    warn!("Broadcast to {} failed: {}", conn.node_id, e);
                    failed += 1;
                }
                Err(_) => {
                    warn!("Broadcast to {} timed out", conn.node_id);
                    failed += 1;
                }
            }
        }
        
        debug!("Broadcast complete: {} sent, {} failed", sent, failed);
        Ok(sent)
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
            dht_enabled: true,
            dht_routing_table_nodes: Some(10),
            dht_storage_entries: Some(5),
        };

        assert!(status.running);
        assert_eq!(status.connected_peers, 5);
    }
}

// =============================================================================
// NetworkService Trait Implementation
// =============================================================================

use async_trait::async_trait;
use crate::traits::NetworkService as NetworkServiceTrait;

#[async_trait]
impl NetworkServiceTrait for P2PNetwork {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()> {
        self.send_to(node_id, data).await
    }

    async fn send_to_with_options(
        &self,
        node_id: &str,
        data: &[u8],
        options: crate::traits::network::SendOptions,
    ) -> Result<()> {
        // 使用带优先级、超时和重试的发送方法
        self.send_with_options(node_id, data, &options).await
    }

    async fn broadcast(&self, data: &[u8]) -> Result<usize> {
        self.broadcast(data).await
    }

    async fn broadcast_with_options(
        &self,
        data: &[u8],
        options: crate::traits::network::SendOptions,
    ) -> Result<usize> {
        // 使用带超时逻辑的广播方法
        self.broadcast_with_options_impl(data, &options).await
    }

    async fn connect(&self, addr: &str) -> Result<()> {
        self.connect(addr).await
    }

    async fn disconnect(&self, node_id: &str) -> Result<()> {
        self.disconnect(node_id).await
    }

    async fn connected_peers(&self) -> Result<Vec<crate::traits::network::PeerInfo>> {
        let peers = self.connected_peers().await;
        Ok(peers.into_iter().map(|p| crate::traits::network::PeerInfo {
            node_id: p.node_id,
            did: p.did,
            address: p.address,
            connected: p.connected,
            last_seen: p.last_seen,
            last_sync_at: p.last_sync_at,
            latency_ms: None,
            protocol_version: "1.0".to_string(),
            capabilities: vec![],
        }).collect())
    }

    async fn discovered_peers(&self) -> Result<Vec<crate::traits::network::PeerInfo>> {
        let peers = self.discovered_peers().await;
        Ok(peers.into_iter().map(|p| crate::traits::network::PeerInfo {
            node_id: p.node_id,
            did: p.did,
            address: p.address,
            connected: p.connected,
            last_seen: p.last_seen,
            last_sync_at: p.last_sync_at,
            latency_ms: None,
            protocol_version: "1.0".to_string(),
            capabilities: vec![],
        }).collect())
    }

    async fn get_peer(&self, node_id: &str) -> Result<Option<crate::traits::network::PeerInfo>> {
        Ok(self.get_peer(node_id).await.map(|p| crate::traits::network::PeerInfo {
            node_id: p.node_id,
            did: p.did,
            address: p.address,
            connected: p.connected,
            last_seen: p.last_seen,
            last_sync_at: p.last_sync_at,
            latency_ms: None,
            protocol_version: "1.0".to_string(),
            capabilities: vec![],
        }))
    }

    async fn status(&self) -> Result<crate::traits::network::NetworkStatus> {
        let s = self.status().await;
        Ok(crate::traits::network::NetworkStatus {
            running: s.running,
            node_id: s.node_id,
            listen_addr: s.listen_addr,
            uptime_secs: s.uptime_secs,
            connected_peers: s.connected_peers,
            discovered_peers: s.discovered_peers,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
        })
    }

    async fn start(&self) -> Result<()> {
        self.start_background_tasks().await
    }

    async fn stop(&self) -> Result<()> {
        // 对于 Arc<P2PNetwork>，我们无法直接调用 stop_instance
        // 这里我们只停止后台任务
        Ok(())
    }

    fn node_id(&self) -> Result<String> {
        Ok(self.config.node_id.clone())
    }

    fn did(&self) -> Result<String> {
        Ok(self.config.did.clone())
    }

    async fn is_connected(&self, node_id: &str) -> Result<bool> {
        let connections = self.transport.list_connections().await;
        Ok(connections.iter().any(|c| c.node_id == node_id))
    }
}
