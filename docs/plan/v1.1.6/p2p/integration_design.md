# libp2p KadDHT 集成设计文档

> **版本**: 1.0
> **日期**: 2026-02-12
> **作者**: CIS Team I - 网络团队
> **任务**: P1-3.2 DHT 重构 - 集成架构设计
> **状态**: 设计阶段

---

## 目录

1. [概述](#1-概述)
2. [架构设计](#2-架构设计)
3. [核心组件](#3-核心组件)
4. [接口设计](#4-接口设计)
5. [数据流](#5-数据流)
6. [实现细节](#6-实现细节)
7. [迁移策略](#7-迁移策略)
8. [测试方案](#8-测试方案)
9. [性能优化](#9-性能优化)
10. [风险管理](#10-风险管理)

---

## 1. 概述

### 1.1 目标

将 libp2p KadDHT 集成到 CIS 现有 P2P 网络层，实现：

1. **无缝集成** - 不破坏现有 API 接口
2. **向后兼容** - 支持现有节点平滑迁移
3. **性能提升** - 查找延迟 < 100ms
4. **功能增强** - 真正的分布式 DHT 网络
5. **可维护性** - 清晰的模块边界

### 1.2 设计原则

- **渐进式迁移** - 双写模式 → 切换 → 清理
- **适配器模式** - 隔离 libp2p API 变更影响
- **保持兼容** - 现有 API 继续工作
- **测试先行** - 完整的测试覆盖

### 1.3 范围

**包含**：
- libp2p KadDHT 集成
- DHT Adapter 层实现
- 持久化存储层设计
- 数据迁移方案
- 测试框架

**不包含**：
- libp2p 传输层替换（现有 QUIC 继续使用）
- P2P 协议变更（保持现有协议兼容）
- Matrix 模块修改

---

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────────┐
│                         CIS Application                       │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      P2P Network Module                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │   mDNS       │  │   Bootstrap  │  │    NAT       │      │
│  │  Discovery   │  │    Service   │  │  Traversal   │      │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘      │
│         │                 │                  │                 │
│         └─────────────────┴──────────────────┘               │
│                           │                                  │
│                           ▼                                  │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │           libp2p Swarm + Behaviours                    │  │
│  │  ┌────────────┐  ┌────────────┐  ┌────────────┐    │  │
│  │  │  Kademlia  │  │   mDNS     │  │   Identify │    │  │
│  │  │  DHT       │  │  Behaviour │  │  Behaviour │    │  │
│  │  └─────┬──────┘  └────────────┘  └────────────┘    │  │
│  │        │                                              │  │
│  │  ┌─────▼───────────────────────────────────────────┐  │  │
│  │  │          libp2p Transport (QUIC)              │  │  │
│  │  └─────┬───────────────────────────────────────────┘  │  │
│  └──────────┼──────────────────────────────────────────────┘  │
└─────────────┼───────────────────────────────────────────────────┘
              │
              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   DHT Adapter Layer                           │
│  ┌─────────────────────────────────────────────────────────┐  │
│  │           CisDhtAdapter (New)                          │  │
│  │  - CIS Key Mapping                                      │  │
│  │  - Namespace Management                                 │  │
│  │  - ACL Enforcement                                      │  │
│  │  - Record Validation                                    │  │
│  └─────────────────────────────────────────────────────────┘  │
└─────────────┬─────────────────────────────────────────────────────┘
              │
      ┌───────┴────────┐
      │                │
      ▼                ▼
┌─────────────┐  ┌─────────────┐
│  Memory     │  │   Node      │
│  Service    │  │  Service    │
└─────────────┘  └─────────────┘
```

### 2.2 模块职责

| 模块 | 职责 | 接口 |
|------|------|------|
| **libp2p Swarm** | 网络连接管理、事件分发 | libp2p::Swarm |
| **Kademlia Behaviour** | DHT 协议实现 | libp2p::kad::Behaviour |
| **DHT Adapter** | CIS 特定逻辑适配 | CisDhtAdapter trait |
| **Persistent Store** | 节点和记录持久化 | PersistentRecordStore |
| **Legacy DHT** | 旧 DHT 实现（迁移期） | DhtService |

### 2.3 层次结构

```
Layer 4: Application Services
         └─> MemoryService, NodeService
         └─> 使用 CisDhtAdapter trait

Layer 3: DHT Adapter Layer
         └─> CisDhtAdapterImpl
         └─> 命名空间管理、ACL、验证
         └─> 实现 CisDhtAdapter trait

Layer 2: libp2p Behaviours
         └─> KademliaBehaviour
         └─> mDNS Behaviour
         └─> Identify Behaviour

Layer 1: libp2p Transport
         └─> QUIC (with Noise encryption)
```

---

## 3. 核心组件

### 3.1 libp2p Swarm 集成

**文件**: `cis-core/src/p2p/libp2p_swarm.rs`

**职责**：
- 创建和管理 libp2p Swarm
- 配置 Transport 和 Behaviours
- 处理网络事件

**核心结构**：

```rust
use libp2p::{
    kad::{Behaviour, Config, store::MemoryStore},
    mdns,
    noise,
    quic,
    swarm::{Swarm, SwarmBuilder},
    identity::Keypair,
    PeerId, Transport,
};

pub struct Libp2pSwarm {
    /// libp2p Swarm 实例
    swarm: Swarm<Libp2pBehaviour>,
    /// 本地 PeerId
    local_peer_id: PeerId,
    /// DHT 行为引用（用于外部操作）
    dht: Arc<RwLock<KademliaBehaviour>>,
}

/// 复合 Behaviour
#[derive(NetworkBehaviour)]
struct Libp2pBehaviour {
    kademlia: KademliaBehaviour,
    mdns: mdns::tokio::Behaviour,
    identify: libp2p::identify::Behaviour,
}

impl Libp2pSwarm {
    /// 创建新的 libp2p Swarm
    pub async fn new(keypair: Keypair) -> Result<Self> {
        let local_peer_id = PeerId::from(keypair.public());

        // 创建 QUIC Transport with Noise
        let transport = quic::tokio::Transport::new(quic::Config::new())
            .map(|(id, muxer)| (id, muxer))
            .map_err(|e| CisError::p2p(format!("Transport error: {}", e)))
            .boxed();

        // 配置 Kademlia DHT
        let kademlia_config = Config::new(local_peer_id);
        let kademlia = Behaviour::new(
            local_peer_id,
            MemoryStore::new(local_peer_id),
        );

        // 配置 mDNS
        let mdns = mdns::tokio::Behaviour::new(
            mdns::Config::default(),
            local_peer_id,
        ).await?;

        // 配置 Identify
        let identify = libp2p::identify::Behaviour::new(
            libp2p::identify::Config::new(
                "/cis/1.1.6".to_string(),
                keypair.public(),
            ),
        );

        // 创建复合 Behaviour
        let behaviour = Libp2pBehaviour {
            kademlia,
            mdns,
            identify,
        };

        // 创建 Swarm
        let swarm = SwarmBuilder::with_tokio_executor(
            transport,
            behaviour,
            local_peer_id,
        ).build();

        Ok(Self {
            swarm,
            local_peer_id,
            dht: Arc::new(RwLock::new(behaviour.kademlia)),
        })
    }

    /// 启动 Swarm 监听
    pub async fn start_listening(&mut self, addr: Multiaddr) -> Result<()> {
        self.swarm.listen_on(addr)?;
        Ok(())
    }

    /// 运行 Swarm 事件循环
    pub async fn run(&mut self) -> Result<()> {
        loop {
            match self.swarm.select_next_some().await {
                Libp2pBehaviourEvent::Kademlia(event) => {
                    self.handle_kademlia_event(event).await?;
                }
                Libp2pBehaviourEvent::Mdns(event) => {
                    self.handle_mdns_event(event).await?;
                }
                Libp2pBehaviourEvent::Identify(event) => {
                    self.handle_identify_event(event).await?;
                }
            }
        }
    }
}
```

### 3.2 DHT Adapter 层

**文件**: `cis-core/src/p2p/kad_dht.rs`

**职责**：
- 实现 CIS 特定的 DHT 操作接口
- 管理命名空间映射
- 执行 ACL 检查
- 验证记录签名

**核心接口**：

```rust
/// CIS DHT 适配器接口
#[async_trait]
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
    async fn get_memory(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>>;

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
}

/// 存储选项
#[derive(Debug, Clone)]
pub struct PutOptions {
    pub ttl: Option<Duration>,
    pub quorum: usize,
    pub sign: bool,
}

impl Default for PutOptions {
    fn default() -> Self {
        Self {
            ttl: Some(Duration::from_secs(86400)), // 24h
            quorum: 1,
            sign: true,
        }
    }
}

/// libp2p KadDHT 实现
pub struct Libp2pKadDht {
    /// libp2p DHT 行为引用
    dht: Arc<RwLock<Behaviour<MemoryStore>>>,
    /// 本地 PeerId
    local_peer_id: PeerId,
    /// ACL 服务
    acl: Arc<dyn AclService>,
    /// 命名空间配置
    namespace_config: NamespaceConfig,
}

#[async_trait]
impl CisDhtAdapter for Libp2pKadDht {
    async fn put_memory(
        &self,
        namespace: &str,
        key: &str,
        value: Vec<u8>,
        options: PutOptions,
    ) -> Result<()> {
        // 1. 构造 CIS DHT key
        let record_key = self.format_key(namespace, key);

        // 2. ACL 检查
        if !self.acl.check_write_permission(namespace).await? {
            return Err(CisError::Forbidden(
                "No write permission for namespace".to_string()
            ));
        }

        // 3. 创建 Record
        let mut record = Record {
            key: record_key.clone().into(),
            value: value.clone(),
            publisher: Some(*self.local_peer_id.as_ref()),
            expires: options.ttl.map(|t| {
                std::time::Instant::now() + t
            }),
        };

        // 4. 签名（如果需要）
        if options.sign {
            // TODO: 实现 DID 签名
            // record = self.sign_record(record).await?;
        }

        // 5. 存储到 DHT
        let mut dht = self.dht.write().await;
        let query_id = dht.put_record(record, options.quorum);

        // 等待完成
        tokio::time::timeout(
            Duration::from_secs(5),
            self.wait_for_query(query_id)
        ).await
            .map_err(|_| CisError::Timeout("DHT put timeout".to_string()))?
            .map_err(|e| CisError::p2p(format!("DHT put failed: {}", e)))?;

        tracing::debug!(
            "Stored memory: namespace={}, key={}, size={}",
            namespace, key, value.len()
        );

        Ok(())
    }

    async fn get_memory(
        &self,
        namespace: &str,
        key: &str,
    ) -> Result<Option<Vec<u8>>> {
        // 1. 构造 DHT key
        let record_key = self.format_key(namespace, key);

        // 2. ACL 检查
        if !self.acl.check_read_permission(namespace).await? {
            return Err(CisError::Forbidden(
                "No read permission for namespace".to_string()
            ));
        }

        // 3. 从 DHT 获取
        let mut dht = self.dht.write().await;
        let query_id = dht.get_record(record_key.into());

        // 等待结果
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            self.wait_for_query(query_id)
        ).await
            .map_err(|_| CisError::Timeout("DHT get timeout".to_string()))?
            .map_err(|e| CisError::p2p(format!("DHT get failed: {}", e)))?;

        match result {
            Some(record) => {
                tracing::debug!(
                    "Found memory: namespace={}, key={}, size={}",
                    namespace, key, record.value.len()
                );
                Ok(Some(record.value))
            }
            None => {
                tracing::debug!("Memory not found: namespace={}, key={}", namespace, key);
                Ok(None)
            }
        }
    }

    async fn find_peer_by_did(&self, did: &str)
        -> Result<Option<PeerId>>
    {
        // 将 DID 转换为 PeerId
        let key = Self::did_to_key(did);

        let mut dht = self.dht.write().await;
        let query_id = dht.get_closest_peers(key);

        // 等待结果
        let peers = tokio::time::timeout(
            Duration::from_secs(5),
            self.wait_for_query(query_id)
        ).await?
            .map_err(|e| CisError::p2p(format!("Find peer failed: {}", e)))?;

        Ok(peers.into_iter().next())
    }

    async fn provide_memory(&self, key: &str) -> Result<()> {
        let record_key = Key::new(&self.format_key("", key));

        let mut dht = self.dht.write().await;
        let query_id = dht.start_providing(record_key)?;

        tokio::time::timeout(
            Duration::from_secs(5),
            self.wait_for_query(query_id)
        ).await?
            .map_err(|e| CisError::p2p(format!("Provide failed: {}", e)))?;

        Ok(())
    }

    async fn get_closest_peers(&self, key: &str)
        -> Result<Vec<PeerId>>
    {
        let key = Key::new(&self.format_key("", key));
        let mut dht = self.dht.write().await;
        let query_id = dht.get_closest_peers(key);

        let peers = tokio::time::timeout(
            Duration::from_secs(5),
            self.wait_for_query(query_id)
        ).await?
            .map_err(|e| CisError::p2p(format!("Get closest failed: {}", e)))?;

        Ok(peers)
    }

    async fn get_stats(&self) -> Result<DhtStats> {
        let dht = self.dht.read().await;
        Ok(DhtStats {
            routing_table_size: dht.kbuckets().count(),
            total_records: dht.iter().count(),
        })
    }
}

impl Libp2pKadDht {
    /// 格式化 DHT key（带命名空间）
    fn format_key(&self, namespace: &str, key: &str) -> String {
        format!("/cis/{}/{}", namespace, key)
    }

    /// 将 DID 转换为 DHT key
    fn did_to_key(did: &str) -> Key {
        // 从 DID 提取唯一标识
        let id = did.split(':').last().unwrap_or(did);
        Key::new(&format!("/cis/did/{}", id))
    }

    /// 等待查询完成（需要实现查询管理器）
    async fn wait_for_query(&self, query_id: QueryId)
        -> Result<QueryResult>
    {
        // TODO: 实现查询结果等待机制
        // 可以使用 tokio::sync::oneshot channel
        Ok(QueryResult::Empty)
    }
}
```

### 3.3 持久化存储层

**文件**: `cis-core/src/p2p/node_store.rs`

**职责**：
- 持久化 DHT 记录
- 节点信息存储
- 快速恢复

**核心实现**：

```rust
use libp2p::kad::{
    record::Key,
    record::Record,
    store::{RecordStore, Error as StoreError},
};
use rocksdb::{DB, Options};
use std::path::PathBuf;
use std::sync::Arc;

/// 持久化 Record Store
pub struct PersistentRecordStore {
    db: Arc<DB>,
    local_peer_id: PeerId,
}

impl PersistentRecordStore {
    /// 创建新的持久化 Store
    pub fn new(local_peer_id: PeerId, db_path: PathBuf) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, db_path)
            .map_err(|e| CisError::storage(format!("Failed to open DB: {}", e)))?;

        Ok(Self {
            db: Arc::new(db),
            local_peer_id,
        })
    }
}

impl RecordStore for PersistentRecordStore {
    type RecordsIter<'a> = std::vec::IntoIter<Record>;

    fn get(&self, k: &Key) -> std::result::Result<Option<Record>, StoreError> {
        let key = k.as_ref().to_vec();
        match self.db.get(&key) {
            Ok(Some(value)) => {
                let record = bincode::deserialize(&value)
                    .map_err(|e| StoreError::Unavailable)?;
                Ok(Some(record))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(StoreError::Unavailable),
        }
    }

    fn put(&self, record: Record) -> std::result::Result<(), StoreError> {
        let key = record.key.as_ref().to_vec();
        let value = bincode::serialize(&record)
            .map_err(|_| StoreError::Unavailable)?;

        self.db.put(&key, &value)
            .map_err(|_| StoreError::Unavailable)?;
        Ok(())
    }

    fn remove(&self, k: &Key) -> std::result::Result<(), StoreError> {
        let key = k.as_ref().to_vec();
        self.db.delete(&key)
            .map_err(|_| StoreError::Unavailable)?;
        Ok(())
    }
}

/// 节点信息存储
pub struct NodeInfoStore {
    db: Arc<DB>,
}

impl NodeInfoStore {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);

        let db = DB::open(&opts, db_path)
            .map_err(|e| CisError::storage(format!("Failed to open DB: {}", e)))?;

        Ok(Self {
            db: Arc::new(db),
        })
    }

    pub fn save_node(&self, node: NodeInfo) -> Result<()> {
        let key = format!("node:{}", node.summary.id);
        let value = bincode::serialize(&node)
            .map_err(|e| CisError::storage(format!("Serialize error: {}", e)))?;

        self.db.put(key.as_bytes(), &value)
            .map_err(|e| CisError::storage(format!("DB error: {}", e)))?;
        Ok(())
    }

    pub fn get_node(&self, node_id: &str) -> Result<Option<NodeInfo>> {
        let key = format!("node:{}", node_id);
        match self.db.get(key.as_bytes()) {
            Ok(Some(value)) => {
                let node = bincode::deserialize(&value)
                    .map_err(|e| CisError::storage(format!("Deserialize error: {}", e)))?;
                Ok(Some(node))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(CisError::storage(format!("DB error: {}", e))),
        }
    }

    pub fn list_all_nodes(&self) -> Result<Vec<NodeInfo>> {
        let iter = self.db.prefix_iterator(b"node:");
        let mut nodes = Vec::new();

        for item in iter {
            let (_, value) = item
                .map_err(|e| CisError::storage(format!("DB error: {}", e)))?;
            let node = bincode::deserialize(&value)
                .map_err(|e| CisError::storage(format!("Deserialize error: {}", e)))?;
            nodes.push(node);
        }

        Ok(nodes)
    }
}
```

---

## 4. 接口设计

### 4.1 命名空间映射

CIS 使用分层命名空间，映射到 libp2p KadDHT 的 key：

```
CIS 命名空间           ->  libp2p DHT Key
----------------------------------------------------
memory/public/{key}    ->  /cis/memory/public/{key}
memory/private/{key}   ->  /cis/memory/private/{key}
node/{node_id}         ->  /cis/node/{node_id}
did/{did}              ->  /cis/did/{did}
sync/{sync_id}         ->  /cis/sync/{sync_id}
```

**实现**：

```rust
pub struct NamespaceConfig {
    pub prefix: String,
    pub allowed_operations: Vec<Operation>,
}

pub enum Operation {
    Read,
    Write,
    Delete,
    Provide,
}

impl Libp2pKadDht {
    const CIS_PREFIX: &'static str = "/cis";

    fn format_key(&self, namespace: &str, key: &str) -> String {
        format!(
            "/{}/{}/{}",
            Self::CIS_PREFIX,
            namespace.trim_start_matches('/'),
            key.trim_start_matches('/')
        )
    }

    fn parse_namespace(&self, key: &str) -> Option<String> {
        key.strip_prefix(Self::CIS_PREFIX)
            .and_then(|s| s.split('/').nth(1))
            .map(|s| s.to_string())
    }
}
```

### 4.2 ACL 集成

在 DHT Adapter 层集成 ACL 检查：

```rust
#[async_trait]
pub trait AclService: Send + Sync {
    async fn check_read_permission(&self, namespace: &str) -> Result<bool>;
    async fn check_write_permission(&self, namespace: &str) -> Result<bool>;
    async fn check_delete_permission(&self, namespace: &str) -> Result<bool>;
}

/// 在 Libp2pKadDht 中使用 ACL
impl Libp2pKadDht {
    async fn enforce_read_acl(&self, namespace: &str) -> Result<()> {
        if !self.acl.check_read_permission(namespace).await? {
            Err(CisError::Forbidden(
                format!("No read permission for namespace: {}", namespace)
            ))
        } else {
            Ok(())
        }
    }

    async fn enforce_write_acl(&self, namespace: &str) -> Result<()> {
        if !self.acl.check_write_permission(namespace).await? {
            Err(CisError::Forbidden(
                format!("No write permission for namespace: {}", namespace)
            ))
        } else {
            Ok(())
        }
    }
}
```

### 4.3 查询管理器

管理异步 DHT 查询：

```rust
use tokio::sync::{oneshot, Mutex};
use std::collections::HashMap;

pub struct QueryManager {
    pending: Arc<Mutex<HashMap<QueryId, oneshot::Sender<QueryResult>>>>,
}

impl QueryManager {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, query_id: QueryId)
        -> oneshot::Receiver<QueryResult>
    {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(query_id, tx);
        rx
    }

    pub async fn complete(&self, query_id: QueryId, result: QueryResult) {
        if let Some(tx) = self.pending.lock().await.remove(&query_id) {
            let _ = tx.send(result);
        }
    }
}
```

---

## 5. 数据流

### 5.1 存储记忆流程

```
MemoryService.put_memory()
    │
    ▼
CisDhtAdapter.put_memory(namespace, key, value)
    │
    ├─→ 1. 格式化 key: /cis/{namespace}/{key}
    │
    ├─→ 2. ACL 检查: acl.check_write_permission()
    │       │
    │       ├─→ 允许: 继续
    │       └─→ 拒绝: 返回 Forbidden 错误
    │
    ├─→ 3. 可选: DID 签名
    │
    ├─→ 4. 调用 libp2p KademliaBehaviour.put_record()
    │       │
    │       ├─→ 本地存储: MemoryStore.put()
    │       └─→ 网络复制: 发送到最近的 K 个节点
    │
    └─→ 5. 等待 quorum 确认
            │
            ├─→ 成功: 返回 Ok(())
            └─→ 超时: 返回 Timeout 错误
```

### 5.2 获取记忆流程

```
MemoryService.get_memory()
    │
    ▼
CisDhtAdapter.get_memory(namespace, key)
    │
    ├─→ 1. 格式化 key: /cis/{namespace}/{key}
    │
    ├─→ 2. ACL 检查: acl.check_read_permission()
    │
    ├─→ 3. 本地查找: MemoryStore.get()
    │       │
    │       ├─→ 找到: 立即返回
    │       └─→ 未找到: 继续
    │
    ├─→ 4. DHT 查找: KademliaBehaviour.get_record()
    │       │
    │       └─→ 迭代查找最近的节点
    │           ├─→ 并行查询 α 个节点
    │           ├─→ 收集返回的记录
    │           └─→ 验证记录签名
    │
    └─→ 5. 返回结果
            │
            ├─→ 找到: Some(value)
            └─→ 未找到: None
```

### 5.3 节点发现流程

```
NodeService.find_node_by_did(did)
    │
    ▼
CisDhtAdapter.find_peer_by_did(did)
    │
    ├─→ 1. DID 转换为 Key: /cis/did/{did}
    │
    ├─→ 2. DHT 查找: KademliaBehaviour.get_closest_peers()
    │       │
    │       └─→ 迭代查找
    │           ├─→ 从路由表获取初始节点
    │           ├─→ 并行查询 α 个节点
    │           └─→ 收集返回的节点
    │
    └─→ 3. 返回最近的 PeerId
            │
            └─→ 连接到 PeerId
```

---

## 6. 实现细节

### 6.1 文件结构

```
cis-core/src/p2p/
├── mod.rs                      # 模块导出
├── libp2p_swarm.rs           # libp2p Swarm 管理 (NEW)
├── kad_dht.rs                # DHT Adapter 层 (NEW)
├── node_store.rs             # 持久化存储 (NEW)
├── query_manager.rs          # 查询管理器 (NEW)
├── namespace.rs             # 命名空间管理 (NEW)
├── dht.rs                  # 旧 DHT 实现 (保留，迁移后移除)
└── ...                     # 其他现有文件
```

### 6.2 编译配置

**Cargo.toml**:

```toml
[dependencies]
# libp2p 核心依赖
libp2p = { version = "0.56", features = [
    "kad",              # Kademlia DHT
    "mdns",            # mDNS 发现
    "quic",            # QUIC 传输
    "noise",           # Noise 加密
    "yamux",           # 多路复用
    "identify",        # 节点识别
] }

# 持久化存储
rocksdb = { version = "0.21", optional = true }

# 序列化
bincode = "1.3"

[features]
default = ["persistent-storage"]
persistent-storage = ["rocksdb"]
```

### 6.3 配置结构

```toml
# ~/.cis/config.toml

[p2p]
# 启用 libp2p KadDHT
enable_libp2p_dht = true

# DHT 配置
[p2p.dht]
# K 参数（bucket 大小）
k = 20

# 并发查询数
alpha = 3

# 请求超时（秒）
timeout = 5

# 记录 TTL（秒）
record_ttl = 86400

# 复制因子
replication_factor = 3

# 持久化存储路径
store_path = "~/.cis/data/dht/"

# Bootstrap 节点
bootstrap_nodes = [
    "/ip4/1.2.3.4/tcp/7677/p2p/12D3KooW...",
    "/ip4/5.6.7.8/tcp/7677/p2p/12D3KooW...",
]
```

---

## 7. 迁移策略

### 7.1 阶段化迁移

**阶段 1: 双写模式** (Week 1-2)

```
                    ┌─────────────┐
MemoryService.put() │  Dual Write │
                    └──────┬──────┘
                           │
              ┌────────────┴────────────┐
              ▼                         ▼
      ┌──────────────┐          ┌──────────────┐
      │ Legacy DHT   │          │ libp2p KadDHT│
      │ (Primary)    │          │ (Secondary)   │
      └──────┬───────┘          └──────┬───────┘
             │                         │
             └────────────┬────────────┘
                          ▼
                    ┌─────────────┐
                    │   Compare   │
                    └─────────────┘
```

- 写入: 同时写入旧 DHT 和 libp2p KadDHT
- 读取: 优先从 libp2p KadDHT 读取，失败则 fallback 到旧 DHT
- 监控: 对比性能和正确性

**阶段 2: 切换模式** (Week 3)

```
                    ┌─────────────┐
MemoryService.put() │ libp2p Only │
                    └──────┬──────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ libp2p KadDHT│
                    │  (Primary)   │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ Legacy DHT   │
                    │  (Backup)    │
                    └──────────────┘
```

- 所有操作切换到 libp2p KadDHT
- 旧 DHT 仅作为备份
- 继续监控和验证

**阶段 3: 清理模式** (Week 4)

```
                    ┌─────────────┐
MemoryService.put() │ libp2p Only │
                    └──────┬──────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ libp2p KadDHT│
                    └──────────────┘

        ┌───────────────────┐
        │ Remove Legacy DHT │
        └───────────────────┘
```

- 迁移所有历史数据
- 移除旧 DHT 代码
- 更新文档和测试

### 7.2 数据迁移脚本

**文件**: `tools/p2p/migrate_dht.rs`

```rust
use cis_core::p2p::{LegacyDhtService, Libp2pKadDht};

#[tokio::main]
async fn main() -> Result<()> {
    let args = MigrationArgs::parse();

    // 1. 打开旧 DHT
    let legacy_dht = LegacyDhtService::load(&args.legacy_db_path)?;

    // 2. 创建新 DHT
    let new_dht = Libp2pKadDht::new(...).await?;

    // 3. 迁移所有记录
    let total = legacy_dht.count_records()?;
    for (i, (key, value)) in legacy_dht.iter_records()?.enumerate() {
        new_dht.put_raw(&key, &value).await?;
        println!("Migrated {}/{}", i + 1, total);
    }

    // 4. 迁移所有节点
    let nodes = legacy_dht.get_all_nodes().await?;
    for node in nodes {
        new_dht.add_node(node).await?;
    }

    println!("Migration completed successfully!");
    Ok(())
}
```

---

## 8. 测试方案

### 8.1 单元测试

**文件**: `cis-core/src/p2p/kad_dht_tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_put_and_get_memory() {
        let adapter = create_test_adapter().await;

        // 存储记忆
        adapter.put_memory(
            "test",
            "key1",
            b"value1".to_vec(),
            PutOptions::default(),
        ).await.unwrap();

        // 获取记忆
        let value = adapter.get_memory("test", "key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_namespace_isolation() {
        let adapter = create_test_adapter().await;

        // 不同命名空间的相同 key
        adapter.put_memory(
            "ns1", "key", b"value1".to_vec(),
            PutOptions::default(),
        ).await.unwrap();

        adapter.put_memory(
            "ns2", "key", b"value2".to_vec(),
            PutOptions::default(),
        ).await.unwrap();

        // 验证隔离
        let v1 = adapter.get_memory("ns1", "key").await.unwrap();
        let v2 = adapter.get_memory("ns2", "key").await.unwrap();

        assert_eq!(v1, Some(b"value1".to_vec()));
        assert_eq!(v2, Some(b"value2".to_vec()));
    }

    #[tokio::test]
    async fn test_acl_enforcement() {
        let adapter = create_test_adapter_with_acl().await;

        // 无权限写入
        let result = adapter.put_memory(
            "forbidden",
            "key",
            b"value".to_vec(),
            PutOptions::default(),
        ).await;

        assert!(matches!(result, Err(CisError::Forbidden(_)));
    }

    #[tokio::test]
    async fn test_find_peer_by_did() {
        let adapter = create_test_adapter().await;

        let did = "did:cis:abc123";
        let peer_id = adapter.find_peer_by_did(did).await.unwrap();

        // 验证找到的 peer
        assert!(peer_id.is_some());
    }
}
```

### 8.2 集成测试

**文件**: `cis-core/src/p2p/tests/dht_integration.rs`

```rust
#[tokio::test]
async fn test_dht_network() {
    // 创建 3 个节点
    let node1 = create_test_node("node1").await;
    let node2 = create_test_node("node2").await;
    let node3 = create_test_node("node3").await;

    // 连接节点
    connect_nodes(&node1, &node2).await;
    connect_nodes(&node2, &node3).await;

    // 在 node1 存储数据
    node1.put_memory("test", "key", b"value".to_vec(),
        PutOptions::default()).await.unwrap();

    // 等待传播
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 从 node3 获取数据
    let value = node3.get_memory("test", "key").await.unwrap();
    assert_eq!(value, Some(b"value".to_vec()));
}
```

### 8.3 性能测试

```rust
#[tokio::test]
async fn bench_dht_lookup_latency() {
    let adapter = create_test_adapter().await;

    // 预填充数据
    for i in 0..1000 {
        adapter.put_memory("bench", &format!("key{}", i),
            b"value".to_vec(), PutOptions::default()).await.unwrap();
    }

    // 测试查找延迟
    let start = Instant::now();
    for i in 0..100 {
        adapter.get_memory("bench", &format!("key{}", i)).await.unwrap();
    }
    let duration = start.elapsed();

    let avg_latency = duration / 100;
    assert!(avg_latency < Duration::from_millis(100),
        "Average latency should be < 100ms, got {:?}", avg_latency);
}
```

---

## 9. 性能优化

### 9.1 查询优化

```rust
// 使用批量查询减少网络往返
pub async fn batch_get(&self, keys: Vec<String>)
    -> Result<Vec<Option<Vec<u8>>>>
{
    let futures: Vec<_> = keys.into_iter()
        .map(|k| self.get_memory("ns", &k))
        .collect();

    let results = futures::future::join_all(futures).await;
    results.into_iter().collect()
}
```

### 9.2 缓存策略

```rust
use lru::LruCache;

pub struct CachedDhtAdapter {
    inner: Box<dyn CisDhtAdapter>,
    cache: Arc<Mutex<LruCache<String, Vec<u8>>>>,
}

#[async_trait]
impl CisDhtAdapter for CachedDhtAdapter {
    async fn get_memory(&self, namespace: &str, key: &str)
        -> Result<Option<Vec<u8>>>
    {
        let cache_key = format!("{}:{}", namespace, key);

        // 检查缓存
        {
            let mut cache = self.cache.lock().await;
            if let Some(value) = cache.get(&cache_key) {
                return Ok(Some(value.clone()));
            }
        }

        // 缓存未命中，查询 DHT
        let value = self.inner.get_memory(namespace, key).await?;

        // 更新缓存
        if let Some(ref v) = value {
            let mut cache = self.cache.lock().await;
            cache.put(cache_key, v.clone());
        }

        Ok(value)
    }
}
```

### 9.3 连接复用

```rust
// libp2p 自动复用连接
// 只需确保 Swarm 持续运行
impl Libp2pSwarm {
    pub async fn keep_alive(&mut self) -> Result<()> {
        loop {
            match self.swarm.select_next_some().await {
                event => {
                    // 处理事件，保持连接活跃
                    self.handle_event(event).await?;
                }
            }
        }
    }
}
```

---

## 10. 风险管理

### 10.1 技术风险缓解

| 风险 | 缓解措施 |
|------|---------|
| libp2p API 变更 | 锁定版本号，封装变化 |
| 性能不达预期 | 先进行性能基准测试 |
| NAT 穿透失败 | 保留现有 STUN/TURN |
| 数据迁移失败 | 完整备份 + 回滚方案 |

### 10.2 回滚方案

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DhtConfig {
    pub backend: DhtBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DhtBackend {
    Libp2pKad,
    Legacy,
    Both, // 双写模式
}

// 运行时切换
pub fn create_dht(config: DhtConfig) -> Result<Box<dyn CisDhtAdapter>> {
    match config.backend {
        DhtBackend::Libp2pKad => {
            Ok(Box::new(Libp2pKadDht::new(...)?))
        }
        DhtBackend::Legacy => {
            Ok(Box::new(LegacyDhtAdapter::new(...)?))
        }
        DhtBackend::Both => {
            Ok(Box::new(DualWriteAdapter::new(...)?))
        }
    }
}
```

### 10.3 监控指标

```rust
pub struct DhtMetrics {
    pub lookup_latency: Histogram,
    pub put_latency: Histogram,
    pub cache_hit_rate: Gauge,
    pub routing_table_size: Gauge,
    pub active_queries: Gauge,
    pub errors_total: Counter,
}
```

---

## 11. 总结

### 11.1 关键设计决策

1. **使用 libp2p KadDHT** - 生产验证，功能完整
2. **Adapter 模式** - 隔离 API 变更影响
3. **渐进式迁移** - 降低风险，平滑过渡
4. **命名空间映射** - 保持 CIS 语义兼容
5. **持久化存储** - 使用 RocksDB 保证数据安全

### 11.2 下一步

- [ ] 实现 Libp2pSwarm (P1-3.3)
- [ ] 实现 CisDhtAdapter (P1-3.3)
- [ ] 实现持久化存储 (P1-3.4)
- [ ] 实现迁移脚本 (P1-3.5)
- [ ] 编写完整测试 (P1-3.6)

### 11.3 预期成果

- ✅ 查找延迟 < 100ms
- ✅ 支持真正的分布式网络
- ✅ 向后兼容现有 API
- ✅ 测试覆盖率 > 80%
- ✅ 平滑迁移，无数据丢失

---

**文档版本**: 1.0
**作者**: CIS Team I - 网络团队
**审核状态**: 待审核
**最后更新**: 2026-02-12
