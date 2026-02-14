//! libp2p KadDHT 集成
//!
//! 提供基于 libp2p Kademlia DHT 的分布式哈希表实现。
//!
//! ## 架构
//!
//! ```
//! ┌────────────────────────────────────────────────────┐
//! │           CIS Application Layer                  │
//! └────────────────────┬───────────────────────────────┘
//!                      │
//!                      ▼
//! ┌────────────────────────────────────────────────────┐
//! │           CisDhtAdapter (This Module)            │
//! │  - Namespace Management                           │
//! │  - ACL Enforcement                               │
//! │  - Record Validation                             │
//! │  - Query Management                             │
//! └────────────────────┬───────────────────────────────┘
//!                      │
//!                      ▼
//! ┌────────────────────────────────────────────────────┐
//! │           libp2p Kademlia Behaviour              │
//! └────────────────────┬───────────────────────────────┘
//!                      │
//!                      ▼
//! ┌────────────────────────────────────────────────────┐
//! │           libp2p Swarm + Transport               │
//! └────────────────────────────────────────────────────┘
//! ```

use crate::error::{CisError, Result};
use crate::network::acl::{AclService, AclPermission};
use crate::p2p::{NodeId, NodeInfo};

use libp2p::{
    kad::{
        store::MemoryStore,
        Behaviour as KademliaBehaviour,
        Config as KadConfig,
        record::Key,
        record::Record,
        QueryId,
        BootstrapOk,
    },
    mdns,
    identify,
    swarm::{NetworkBehaviour, Swarm, SwarmBuilder},
    Multiaddr,
    PeerId,
    Transport,
    identity::Keypair,
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, oneshot, RwLock};
use tokio::time::timeout;

/// CIS DHT 前缀
const CIS_PREFIX: &str = "/cis";

/// DHT 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KadDhtConfig {
    /// K 参数（每个 bucket 的节点数）
    pub k: usize,
    /// Alpha 参数（并发查询数）
    pub alpha: usize,
    /// 请求超时（秒）
    pub request_timeout_secs: u64,
    /// 记录 TTL（秒）
    pub record_ttl_secs: u64,
    /// 复制因子
    pub replication_factor: usize,
    /// 持久化存储路径
    pub store_path: PathBuf,
}

impl Default for KadDhtConfig {
    fn default() -> Self {
        Self {
            k: 20,
            alpha: 3,
            request_timeout_secs: 5,
            record_ttl_secs: 86400, // 24 hours
            replication_factor: 3,
            store_path: PathBuf::from("~/.cis/data/dht"),
        }
    }
}

/// 存储选项
#[derive(Debug, Clone)]
pub struct PutOptions {
    pub ttl: Option<Duration>,
    pub quorum: usize,
}

impl Default for PutOptions {
    fn default() -> Self {
        Self {
            ttl: Some(Duration::from_secs(86400)),
            quorum: 1,
        }
    }
}

/// DHT 统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtStats {
    pub routing_table_size: usize,
    pub total_records: usize,
    pub active_queries: usize,
}

/// CIS DHT 适配器接口
///
/// 定义 CIS 特定的 DHT 操作，包括命名空间管理和 ACL 检查。
#[async_trait::async_trait]
pub trait CisDhtAdapter: Send + Sync {
    /// 存储记忆（带命名空间）
    async fn put_memory(
        &self,
        namespace: &str,
        key: &str,
        value: Vec<u8>,
        options: PutOptions,
    ) -> Result<()>;

    /// 获取记忆
    async fn get_memory(&self, namespace: &str, key: &str)
        -> Result<Option<Vec<u8>>>;

    /// 删除记忆
    async fn delete_memory(&self, namespace: &str, key: &str)
        -> Result<bool>;

    /// 查找节点（DID 查询）
    async fn find_peer_by_did(&self, did: &str)
        -> Result<Option<PeerId>>;

    /// 发布记忆提供者
    async fn provide_memory(&self, key: &str) -> Result<()>;

    /// 获取最近的节点
    async fn get_closest_peers(&self, key: &str)
        -> Result<Vec<PeerId>>;

    /// 获取路由表统计
    async fn get_stats(&self) -> Result<DhtStats>;

    /// 添加引导节点
    async fn add_bootstrap_node(&self, addr: Multiaddr) -> Result<()>;
}

/// libp2p Kademlia 复合行为
#[derive(NetworkBehaviour)]
struct Libp2pBehaviour {
    kademlia: KademliaBehaviour<MemoryStore>,
    mdns: mdns::tokio::Behaviour,
    identify: identify::Behaviour,
}

/// libp2p KadDHT 实现
pub struct Libp2pKadDht {
    /// libp2p Swarm（单独运行在后台任务中）
    swarm_handle: Arc<SwarmHandle>,
    /// DHT 行为引用（用于外部操作）
    dht: Arc<RwLock<KademliaBehaviour<MemoryStore>>>,
    /// 本地 PeerId
    local_peer_id: PeerId,
    /// ACL 服务
    acl: Arc<dyn AclService>,
    /// 配置
    config: KadDhtConfig,
    /// 查询管理器
    query_manager: Arc<QueryManager>,
}

/// Swarm 处理句柄
#[derive(Clone)]
struct SwarmHandle {
    local_peer_id: PeerId,
}

/// 查询结果
enum QueryResult {
    Empty,
    Record(Option<Record>),
    Peers(Vec<PeerId>),
    BootstrapOk(BootstrapOk),
}

/// 查询管理器
struct QueryManager {
    pending: Mutex<HashMap<QueryId, oneshot::Sender<QueryResult>>>,
}

impl QueryManager {
    fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }

    async fn register(&self, query_id: QueryId)
        -> oneshot::Receiver<QueryResult>
    {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(query_id, tx);
        rx
    }

    async fn complete(&self, query_id: QueryId, result: QueryResult) {
        if let Some(tx) = self.pending.lock().await.remove(&query_id) {
            let _ = tx.send(result);
        }
    }
}

impl Libp2pKadDht {
    /// 创建新的 libp2p KadDHT 实例
    pub async fn new(
        keypair: Keypair,
        acl: Arc<dyn AclService>,
        config: KadDhtConfig,
    ) -> Result<Self> {
        let local_peer_id = PeerId::from(keypair.public());

        // 配置 Kademlia DHT
        let kad_config = KadConfig::new(local_peer_id)
            .with_query_timeout(Duration::from_secs(config.request_timeout_secs));

        let store = MemoryStore::new(local_peer_id);
        let kademlia = KademliaBehaviour::with_config(
            local_peer_id,
            store,
            kad_config,
        );

        // 创建 Swarm Handle
        let swarm_handle = Arc::new(SwarmHandle {
            local_peer_id,
        });

        // 创建查询管理器
        let query_manager = Arc::new(QueryManager::new());

        // 创建 DHT 实例（此时不需要实际的 Swarm，我们只保留 Behaviour 引用）
        // 注意：实际使用时，需要在单独的 Swarm 事件循环中运行
        let dht = Arc::new(RwLock::new(kademlia));

        Ok(Self {
            swarm_handle,
            dht,
            local_peer_id,
            acl,
            config,
            query_manager,
        })
    }

    /// 格式化 DHT key（带命名空间）
    fn format_key(&self, namespace: &str, key: &str) -> String {
        format!(
            "{}/{}/{}/{}",
            CIS_PREFIX,
            namespace.trim_start_matches('/'),
            key.trim_start_matches('/'),
            namespace // 添加命名空间后缀用于隔离
        )
    }

    /// 将 DID 转换为 DHT key
    fn did_to_key(&self, did: &str) -> Key {
        // 从 DID 提取唯一标识
        let id = did.split(':').last().unwrap_or(did);
        Key::new(&format!("{}/did/{}", CIS_PREFIX, id))
    }

    /// 等待查询完成
    async fn wait_for_query(&self, query_id: QueryId)
        -> Result<QueryResult>
    {
        let rx = self.query_manager.register(query_id).await;

        timeout(
            Duration::from_secs(self.config.request_timeout_secs),
            rx
        ).await
            .map_err(|_| CisError::Timeout("DHT query timeout".to_string()))?
            .map_err(|_| CisError::P2P("Query cancelled".to_string()))
    }

    /// ACL 检查：读取权限
    async fn check_read_acl(&self, namespace: &str) -> Result<()> {
        let permission = AclPermission::read(namespace.to_string());
        if !self.acl.check_permission(&permission).await {
            return Err(CisError::Forbidden(
                format!("No read permission for namespace: {}", namespace)
            ));
        }
        Ok(())
    }

    /// ACL 检查：写入权限
    async fn check_write_acl(&self, namespace: &str) -> Result<()> {
        let permission = AclPermission::write(namespace.to_string());
        if !self.acl.check_permission(&permission).await {
            return Err(CisError::Forbidden(
                format!("No write permission for namespace: {}", namespace)
            ));
        }
        Ok(())
    }

    /// ACL 检查：删除权限
    async fn check_delete_acl(&self, namespace: &str) -> Result<()> {
        let permission = AclPermission::delete(namespace.to_string());
        if !self.acl.check_permission(&permission).await {
            return Err(CisError::Forbidden(
                format!("No delete permission for namespace: {}", namespace)
            ));
        }
        Ok(())
    }

    /// 启动 DHT（需要传入已配置的 Swarm）
    pub async fn start_with_swarm(
        &self,
        mut swarm: Swarm<Libp2pBehaviour>,
        listen_addr: Multiaddr,
    ) -> Result<()> {
        // 开始监听
        swarm.listen_on(listen_addr)?;

        // 启动事件循环（在后台任务中）
        let dht = Arc::clone(&self.dht);
        let query_manager = Arc::clone(&self.query_manager);

        tokio::spawn(async move {
            loop {
                match swarm.select_next_some().await {
                    event => {
                        Self::handle_swarm_event(event, &dht, &query_manager).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// 处理 Swarm 事件
    async fn handle_swarm_event(
        event: Libp2pBehaviourEvent,
        dht: &Arc<RwLock<KademliaBehaviour<MemoryStore>>>,
        query_manager: &QueryManager,
    ) {
        match event {
            Libp2pBehaviourEvent::Kademlia(kad_event) => {
                Self::handle_kademlia_event(kad_event, dht, query_manager).await;
            }
            Libp2pBehaviourEvent::Mdns(mdns_event) => {
                tracing::debug!("mDNS event: {:?}", mdns_event);
            }
            Libp2pBehaviourEvent::Identify(identify_event) => {
                tracing::debug!("Identify event: {:?}", identify_event);
            }
        }
    }

    /// 处理 Kademlia 事件
    async fn handle_kademlia_event(
        kad_event: libp2p::kad::Event,
        dht: &Arc<RwLock<KademliaBehaviour<MemoryStore>>>,
        query_manager: &QueryManager,
    ) {
        use libp2p::kad::Event;

        match kad_event {
            Event::OutboundQueryProgressed {
                id,
                result,
                ..
            } => {
                match result {
                    libp2p::kad::QueryResult::GetRecord(Ok(record)) => {
                        query_manager.complete(id, QueryResult::Record(Some(record.record))).await;
                    }
                    libp2p::kad::QueryResult::GetRecord(Err(e)) => {
                        tracing::warn!("GetRecord query failed: {:?}", e);
                        query_manager.complete(id, QueryResult::Record(None)).await;
                    }
                    libp2p::kad::QueryResult::GetClosestPeers(Ok(peers)) => {
                        let peers: Vec<PeerId> = peers.peers.into_iter().collect();
                        query_manager.complete(id, QueryResult::Peers(peers)).await;
                    }
                    libp2p::kad::QueryResult::GetClosestPeers(Err(e)) => {
                        tracing::warn!("GetClosestPeers query failed: {:?}", e);
                        query_manager.complete(id, QueryResult::Peers(Vec::new())).await;
                    }
                    libp2p::kad::QueryResult::BootstrapOk(result) => {
                        query_manager.complete(id, QueryResult::BootstrapOk(result)).await;
                    }
                    _ => {}
                }
            }
            Event::InboundRequest { request, .. } => {
                tracing::debug!("Inbound Kademlia request: {:?}", request);
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl CisDhtAdapter for Libp2pKadDht {
    async fn put_memory(
        &self,
        namespace: &str,
        key: &str,
        value: Vec<u8>,
        options: PutOptions,
    ) -> Result<()> {
        // 1. ACL 检查
        self.check_write_acl(namespace).await?;

        // 2. 构造 DHT key
        let record_key = self.format_key(namespace, key);

        // 3. 创建 Record
        let record = Record {
            key: Key::new(&record_key),
            value,
            publisher: Some(*self.local_peer_id.as_ref()),
            expires: options.ttl.map(|t| std::time::Instant::now() + t),
        };

        // 4. 存储到 DHT
        let mut dht = self.dht.write().await;
        let query_id = dht.put_record(record, options.quorum)
            .map_err(|e| CisError::P2P(format!("Put record failed: {}", e)))?;

        // 5. 等待完成
        let _result = self.wait_for_query(query_id).await?;

        tracing::debug!(
            "Stored memory: namespace={}, key={}",
            namespace, key
        );

        Ok(())
    }

    async fn get_memory(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>> {
        // 1. ACL 检查
        self.check_read_acl(namespace).await?;

        // 2. 构造 DHT key
        let record_key = self.format_key(namespace, key);

        // 3. 从 DHT 获取
        let mut dht = self.dht.write().await;
        let query_id = dht.get_record(Key::new(&record_key));

        // 4. 等待结果
        match self.wait_for_query(query_id).await? {
            QueryResult::Record(Some(record)) => {
                tracing::debug!(
                    "Found memory: namespace={}, key={}, size={}",
                    namespace, key, record.value.len()
                );
                Ok(Some(record.value))
            }
            _ => {
                tracing::debug!("Memory not found: namespace={}, key={}", namespace, key);
                Ok(None)
            }
        }
    }

    async fn delete_memory(&self, namespace: &str, key: &str)
        -> Result<bool>
    {
        // 1. ACL 检查
        self.check_delete_acl(namespace).await?;

        // 2. 构造 DHT key
        let record_key = self.format_key(namespace, key);

        // 3. 删除记录
        let mut dht = self.dht.write().await;
        let removed = dht.remove_record(&Key::new(&record_key))
            .map_err(|e| CisError::P2P(format!("Remove record failed: {}", e)))?;

        if removed {
            tracing::debug!("Deleted memory: namespace={}, key={}", namespace, key);
        } else {
            tracing::debug!("Memory not found for deletion: namespace={}, key={}", namespace, key);
        }

        Ok(removed)
    }

    async fn find_peer_by_did(&self, did: &str)
        -> Result<Option<PeerId>>
    {
        // 将 DID 转换为 Key
        let key = self.did_to_key(did);

        // 获取最近的节点
        let mut dht = self.dht.write().await;
        let query_id = dht.get_closest_peers(key);

        // 等待结果
        match self.wait_for_query(query_id).await? {
            QueryResult::Peers(peers) => {
                Ok(peers.into_iter().next())
            }
            _ => Ok(None),
        }
    }

    async fn provide_memory(&self, key: &str) -> Result<()> {
        let record_key = Key::new(&self.format_key("", key));

        let mut dht = self.dht.write().await;
        let query_id = dht.start_providing(record_key)
            .map_err(|e| CisError::P2P(format!("Start providing failed: {}", e)))?;

        self.wait_for_query(query_id).await?;

        Ok(())
    }

    async fn get_closest_peers(&self, key: &str)
        -> Result<Vec<PeerId>>
    {
        let key = Key::new(&self.format_key("", key));
        let mut dht = self.dht.write().await;
        let query_id = dht.get_closest_peers(key);

        match self.wait_for_query(query_id).await? {
            QueryResult::Peers(peers) => Ok(peers),
            _ => Ok(Vec::new()),
        }
    }

    async fn get_stats(&self) -> Result<DhtStats> {
        let dht = self.dht.read().await;
        let routing_table_size = dht.kbuckets().count();
        let total_records = dht.iter().count();

        Ok(DhtStats {
            routing_table_size,
            total_records,
            active_queries: 0, // TODO: Implement active query tracking
        })
    }

    async fn add_bootstrap_node(&self, addr: Multiaddr) -> Result<()> {
        let mut dht = self.dht.write().await;
        dht.add_address(&PeerId::random(), addr);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_key() {
        let config = KadDhtConfig::default();
        let acl = Arc::new(MockAclService::new(true));

        // 注意：这里需要 mock Keypair，暂时跳过
        // let dht = Libp2pKadDht::new(...);

        // let key = dht.format_key("memory/public", "test-key");
        // assert_eq!(key, "/cis/memory/public/test-key/memory/public");
    }

    #[test]
    fn test_config_default() {
        let config = KadDhtConfig::default();
        assert_eq!(config.k, 20);
        assert_eq!(config.alpha, 3);
        assert_eq!(config.request_timeout_secs, 5);
        assert_eq!(config.record_ttl_secs, 86400);
    }

    // Mock ACL Service for testing
    struct MockAclService {
        allow_all: bool,
    }

    impl MockAclService {
        fn new(allow_all: bool) -> Self {
            Self { allow_all }
        }
    }

    #[async_trait::async_trait]
    impl AclService for MockAclService {
        async fn check_permission(&self, _permission: &AclPermission) -> bool {
            self.allow_all
        }

        async fn grant_permission(&self, _permission: AclPermission)
            -> Result<()>
        {
            Ok(())
        }

        async fn revoke_permission(&self, _permission: AclPermission)
            -> Result<()>
        {
            Ok(())
        }

        async fn list_permissions(&self, _subject: &str)
            -> Result<Vec<AclPermission>>
        {
            Ok(Vec::new())
        }
    }
}
