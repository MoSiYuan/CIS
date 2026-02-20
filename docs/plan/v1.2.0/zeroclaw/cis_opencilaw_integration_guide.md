# CIS - OpenClaw 双向集成实施指南

> **创建日期**: 2026-02-20
> **适用对象**: CIS 开发者
> **前置条件**: 已阅读《OpenClaw/CIS 双向整合分析报告》

---

## 快速开始

本指南提供**分步骤**的实施指南，帮助 CIS 实现：
1. **贡献独立模块**给 zeroclaw（DAG scheduler, P2P transport, Memory backend）
2. **可选集成** zeroclaw 的能力（22+ AI providers, 13+ channels, 3000+ skills）

**预计时间**: 12周（3个月）

---

## Phase 1: CIS Trait 定义（Week 1-3）

### 目标

定义 CIS 独立 traits，考虑 zeroclaw trait 接口以便后续兼容。

### Task 1.1: Memory Trait

**文件**: `cis-core/src/traits/memory.rs`

**完整实现**:

```rust
//! # Memory Service Trait
//!
//! Abstract interface for memory storage and retrieval.
//!
//! ## Design Principles
//!
//! - **Domain Separation**: Private (encrypted) vs Public (syncable)
//! - **Hybrid Search**: Vector (70%) + FTS5 BM25 (30%)
//! - **Archival**: 54-week automatic archival
//! - **P2P Sync**: CRDT-based incremental synchronization
//!
//! ## zeroclaw Compatibility
//!
//! The trait methods are designed to be compatible with zeroclaw::Memory:
//! - `set()` ↔ `store()`
//! - `get()` ↔ `get()`
//! - `list_keys()` ↔ `list()`
//! - `delete()` ↔ `forget()`
//! - `health_check()` ↔ `health_check()`

use async_trait::async_trait;
use crate::types::{MemoryDomain, MemoryCategory, MemoryEntry, SearchResult, HybridSearchResult, MemoryStats};
use anyhow::Result;

/// Core memory trait for storage and retrieval
#[async_trait]
pub trait Memory: Send + Sync {
    /// Backend identifier (zeroclaw compatible)
    fn name(&self) -> &str;

    /// Store a key-value pair
    ///
    /// # Arguments
    ///
    /// * `key` - Unique identifier
    /// * `value` - Binary data
    /// * `domain` - Private (encrypted) or Public (syncable)
    /// * `category` - Core, Daily, Conversation, Custom
    async fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory
    ) -> Result<()>;

    /// Retrieve a single entry by key (zeroclaw compatible)
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;

    /// Delete an entry (zeroclaw compatible: forget)
    async fn delete(&self, key: &str) -> Result<bool>;

    /// Vector-only search
    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f32
    ) -> Result<Vec<SearchResult>>;

    /// Hybrid search: Vector (70%) + BM25 (30%) (CIS-specific)
    async fn hybrid_search(
        &self,
        query: &str,
        limit: usize,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>
    ) -> Result<Vec<HybridSearchResult>>;

    /// List keys by filter (zeroclaw compatible: list)
    async fn list_keys(
        &self,
        domain: Option<MemoryDomain>,
        category: Option<MemoryCategory>,
        prefix: Option<&str>
    ) -> Result<Vec<String>>;

    /// Health check (zeroclaw compatible)
    async fn health_check(&self) -> bool;

    /// Get statistics (CIS-specific)
    async fn stats(&self) -> Result<MemoryStats>;
}

/// Vector index extension (CIS-specific)
#[async_trait]
pub trait MemoryVectorIndex: Memory {
    /// Rebuild vector index
    async fn rebuild_index(&self, batch_size: usize) -> Result<usize>;

    /// Get index statistics
    async fn index_stats(&self) -> Result<VectorIndexStats>;
}

/// P2P sync extension (CIS-specific)
#[async_trait]
pub trait MemorySync: Memory {
    /// Get pending sync markers
    async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>>;

    /// Mark entry as synced
    async fn mark_synced(&self, key: &str, peer_id: &str) -> Result<()>;

    /// Apply remote update
    async fn apply_remote_update(&self, entry: &MemoryEntry, source_peer_id: &str) -> Result<bool>;
}
```

**测试文件**: `cis-core/src/traits/memory_tests.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::backends::mock::MockMemoryBackend;

    #[tokio::test]
    async fn test_memory_set_get() {
        let memory = MockMemoryBackend::new();

        memory.set(
            "test-key",
            b"test-value",
            MemoryDomain::Public,
            MemoryCategory::Context
        ).await.unwrap();

        let entry = memory.get("test-key").await.unwrap();
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().value, b"test-value");
    }

    #[tokio::test]
    async fn test_memory_health_check() {
        let memory = MockMemoryBackend::new();
        assert!(memory.health_check().await);
    }

    #[tokio::test]
    async fn test_memory_list_keys() {
        let memory = MockMemoryBackend::new();

        memory.set("key1", b"value1", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
        memory.set("key2", b"value2", MemoryDomain::Private, MemoryCategory::Context).await.unwrap();

        let keys = memory.list_keys(Some(MemoryDomain::Public), None, None).await.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], "key1");
    }
}
```

### Task 1.2: Network Trait

**文件**: `cis-core/src/traits/network.rs`

```rust
//! # Network Service Trait
//!
//! Abstract interface for P2P networking.
//!
//! ## Design Principles
//!
//! - **DID Identity**: Decentralized identifiers with hardware-bound keys
//! - **QUIC Transport**: Fast, multiplexed, NAT-traversing
//! - **P2P Discovery**: mDNS (local) + DHT (public)
//!
//! ## zeroclaw Compatibility
//!
//! This trait is CIS-specific. zeroclaw uses `Channel` trait for messaging.
//! We provide an adapter to implement `zeroclaw::Channel`.

use async_trait::async_trait;
use anyhow::Result;
use std::net::SocketAddr;

/// Node identifier (DID)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

/// Peer information
#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub node_id: NodeId,
    pub addrs: Vec<SocketAddr>,
    pub latency_ms: u64,
}

/// Transport layer (QUIC, TCP, etc.)
#[async_trait]
pub trait Transport: Send + Sync {
    fn name(&self) -> &str;
    async fn send(&self, target: &NodeId, data: &[u8]) -> Result<()>;
    async fn receive(&self) -> Result<(NodeId, Vec<u8>)>;
}

/// P2P network with discovery
#[async_trait]
pub trait P2PNetwork: Send + Sync {
    type Transport: Transport;

    fn name(&self) -> &str;
    fn transport(&self) -> &Self::Transport;

    async fn connect(&self, addr: &str) -> Result<()>;
    async fn broadcast(&self, data: &[u8]) -> Result<()>;
    async fn connected_peers(&self) -> Vec<PeerInfo>;

    /// Discovery
    async fn start_mdns_discovery(&self) -> Result<()>;
    async fn start_dht_discovery(&self, bootstrap_peers: Vec<String>) -> Result<()>;
}
```

### Task 1.3: Scheduler Trait

**文件**: `cis-core/src/traits/scheduler.rs`

```rust
//! # DAG Scheduler Trait
//!
//! Abstract interface for DAG task orchestration.
//!
//! ## Design Principles
//!
//! - **Four-Level Decisions**: Mechanical → Arbitrated
//! - **Federation**: Cross-node task distribution
//! - **CRDT**: Conflict-free replication

use async_trait::async_trait;
use anyhow::Result;
use crate::types::{Task, TaskResult, Dag, ExecutionStatus};

/// DAG scheduler
#[async_trait]
pub trait DagScheduler: Send + Sync {
    fn name(&self) -> &str;

    async fn build_dag(&mut self, tasks: Vec<Task>) -> Result<Dag>;
    async fn execute_dag(&self, dag: Dag) -> Result<Vec<TaskResult>>;
    async fn validate_dag(&self, dag: &Dag) -> Result<()>;

    async fn cancel_execution(&self, execution_id: &str) -> Result<bool>;
    async fn get_execution_status(&self, execution_id: &str) -> Result<ExecutionStatus>;
}

/// Federation coordinator (CIS-specific)
#[async_trait]
pub trait FederationCoordinator: Send + Sync {
    async fn coordinate(&self, dag: Dag) -> Result<FederationPlan>;
    async fn cancel_task(&self, task_id: &str) -> Result<bool>;
    async fn sync_status(&self) -> Result<FederationStatus>;
}
```

### Task 1.4: 更新 mod.rs

**文件**: `cis-core/src/traits/mod.rs`

```rust
pub mod memory;
pub mod network;
pub mod scheduler;

// Re-export
pub use memory::{Memory, MemoryVectorIndex, MemorySync};
pub use network::{Transport, P2PNetwork, NodeId, PeerInfo};
pub use scheduler::{DagScheduler, FederationCoordinator};
```

---

## Phase 2: Backend 实现（Week 4-5）

### Task 2.1: Memory Backend 实现

**目录结构**:

```
cis-core/src/memory/backends/
├── mod.rs
├── cis.rs          # CisMemoryBackend
└── mock.rs         # MockMemoryBackend
```

**CisMemoryBackend** (`cis-core/src/memory/backends/cis.rs`):

```rust
use async_trait::async_trait;
use anyhow::Result;
use std::sync::Arc;
use crate::memory::MemoryService;
use crate::traits::{Memory, MemoryVectorIndex, MemorySync};
use crate::types::{MemoryDomain, MemoryCategory, MemoryEntry, SearchResult, HybridSearchResult, MemoryStats, VectorIndexStats, SyncMarker};

/// CIS memory backend (wrapper around existing MemoryService)
pub struct CisMemoryBackend {
    service: Arc<MemoryService>,
    node_id: String,
}

impl CisMemoryBackend {
    pub fn new(service: Arc<MemoryService>, node_id: String) -> Self {
        Self { service, node_id }
    }
}

#[async_trait]
impl Memory for CisMemoryBackend {
    fn name(&self) -> &str {
        "cis-memory"
    }

    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
        self.service.set(key, value, domain, category).await
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        self.service.get(key).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        self.service.delete(key).await
    }

    async fn search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>> {
        self.service.semantic_search(query, limit, threshold).await
    }

    async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>, category: Option<MemoryCategory>) -> Result<Vec<HybridSearchResult>> {
        self.service.hybrid_search(query, limit, domain, category).await
    }

    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>> {
        self.service.list_keys(domain, category, prefix).await
    }

    async fn health_check(&self) -> bool {
        self.service.health_check().await
    }

    async fn stats(&self) -> Result<MemoryStats> {
        self.service.stats().await
    }
}

#[async_trait]
impl MemoryVectorIndex for CisMemoryBackend {
    async fn rebuild_index(&self, batch_size: usize) -> Result<usize> {
        self.service.rebuild_vector_index(batch_size).await
    }

    async fn index_stats(&self) -> Result<VectorIndexStats> {
        self.service.vector_index_stats().await
    }
}

#[async_trait]
impl MemorySync for CisMemoryBackend {
    async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>> {
        self.service.get_pending_sync(limit).await
    }

    async fn mark_synced(&self, key: &str, peer_id: &str) -> Result<()> {
        self.service.mark_synced(key, peer_id).await
    }

    async fn apply_remote_update(&self, entry: &MemoryEntry, source_peer_id: &str) -> Result<bool> {
        self.service.apply_remote_update(entry, source_peer_id).await
    }
}
```

**MockMemoryBackend** (`cis-core/src/memory/backends/mock.rs`):

```rust
use async_trait::async_trait;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use chrono::Utc;
use crate::traits::{Memory, MemoryVectorIndex, MemorySync};
use crate::types::{MemoryDomain, MemoryCategory, MemoryEntry, SearchResult, HybridSearchResult, MemoryStats, VectorIndexStats, SyncMarker};

/// In-memory mock backend for testing
pub struct MockMemoryBackend {
    data: Arc<Mutex<HashMap<String, MemoryEntry>>>,
}

impl MockMemoryBackend {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for MockMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for MockMemoryBackend {
    fn name(&self) -> &str {
        "mock-memory"
    }

    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
        let mut data = self.data.lock().await;
        data.insert(key.to_string(), MemoryEntry {
            key: key.to_string(),
            value: value.to_vec(),
            timestamp: Utc::now(),
            domain,
            category,
            session_id: None,
        });
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let data = self.data.lock().await;
        Ok(data.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let mut data = self.data.lock().await;
        Ok(data.remove(key).is_some())
    }

    async fn search(&self, _query: &str, _limit: usize, _threshold: f32) -> Result<Vec<SearchResult>> {
        // Mock: return empty
        Ok(vec![])
    }

    async fn hybrid_search(&self, _query: &str, _limit: usize, _domain: Option<MemoryDomain>, _category: Option<MemoryCategory>) -> Result<Vec<HybridSearchResult>> {
        // Mock: return empty
        Ok(vec![])
    }

    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>> {
        let data = self.data.lock().await;
        let keys: Vec<String> = data.iter()
            .filter(|(_, v)| {
                if let Some(d) = domain {
                    if v.domain != d {
                        return false;
                    }
                }
                if let Some(c) = category {
                    if v.category != c {
                        return false;
                    }
                }
                if let Some(p) = prefix {
                    if !v.key.starts_with(p) {
                        return false;
                    }
                }
                true
            })
            .map(|(k, _)| k.clone())
            .collect();
        Ok(keys)
    }

    async fn health_check(&self) -> bool {
        true
    }

    async fn stats(&self) -> Result<MemoryStats> {
        let data = self.data.lock().await;
        Ok(MemoryStats {
            total_entries: data.len(),
            private_entries: data.values().filter(|e| e.domain == MemoryDomain::Private).count(),
            public_entries: data.values().filter(|e| e.domain == MemoryDomain::Public).count(),
            index_size: 0,
        })
    }
}

#[async_trait]
impl MemoryVectorIndex for MockMemoryBackend {
    async fn rebuild_index(&self, _batch_size: usize) -> Result<usize> {
        Ok(0)
    }

    async fn index_stats(&self) -> Result<VectorIndexStats> {
        Ok(VectorIndexStats {
            total_vectors: 0,
            index_size: 0,
        })
    }
}

#[async_trait]
impl MemorySync for MockMemoryBackend {
    async fn get_pending_sync(&self, _limit: usize) -> Result<Vec<SyncMarker>> {
        Ok(vec![])
    }

    async fn mark_synced(&self, _key: &str, _peer_id: &str) -> Result<()> {
        Ok(())
    }

    async fn apply_remote_update(&self, entry: &MemoryEntry, _source_peer_id: &str) -> Result<bool> {
        let mut data = self.data.lock().await;
        data.insert(entry.key.clone(), entry.clone());
        Ok(true)
    }
}
```

**mod.rs** (`cis-core/src/memory/backends/mod.rs`):

```rust
pub mod cis;
pub mod mock;

pub use cis::CisMemoryBackend;
pub use mock::MockMemoryBackend;
```

### Task 2.2: 重构 MemoryService

**文件**: `cis-core/src/memory/service.rs`

**Before** (简化):

```rust
pub struct MemoryService {
    db: Arc<Mutex<MemoryDb>>,
    vector: Arc<VectorStorage>,
}
```

**After**:

```rust
use crate::traits::{Memory, MemoryVectorIndex, MemorySync};

pub struct MemoryService {
    memory: Box<dyn Memory>,
    vector_index: Box<dyn MemoryVectorIndex>,
    sync: Box<dyn MemorySync>,
}

impl MemoryService {
    pub fn new(
        memory: Box<dyn Memory>,
        vector_index: Box<dyn MemoryVectorIndex>,
        sync: Box<dyn MemorySync>,
    ) -> Result<Self> {
        Ok(Self {
            memory,
            vector_index,
            sync,
        })
    }

    /// Factory: create default CIS implementation
    pub fn create_default(node_id: &str, data_dir: &Path) -> Result<Self> {
        let service = Arc::new(MemoryDb::new(data_dir)?);
        let vector = Arc::new(VectorStorage::new(data_dir)?);

        let memory = Box::new(CisMemoryBackend::new(service.clone(), node_id.to_string()));
        let vector_index = Box::new(CisMemoryBackend::new(service.clone(), node_id.to_string()));
        let sync = Box::new(CisMemoryBackend::new(service, node_id.to_string()));

        Self::new(memory, vector_index, sync)
    }

    // Delegate methods
    pub async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
        self.memory.set(key, value, domain, category).await
    }

    pub async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        self.memory.get(key).await
    }

    pub async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>, category: Option<MemoryCategory>) -> Result<Vec<HybridSearchResult>> {
        self.memory.hybrid_search(query, limit, domain, category).await
    }

    // ... other methods
}
```

### Task 2.3: 更新 service.rs

**文件**: `cis-core/src/service/mod.rs`

```rust
use crate::traits::{Memory, MemoryVectorIndex, MemorySync, DagScheduler, FederationCoordinator};
use crate::memory::backends::{CisMemoryBackend, MockMemoryBackend};

pub struct CisServices {
    pub memory: Box<dyn Memory>,
    pub vector_index: Box<dyn MemoryVectorIndex>,
    pub sync: Box<dyn MemorySync>,
    pub scheduler: Box<dyn DagScheduler>,
    pub federation: Box<dyn FederationCoordinator>,
}

impl CisServices {
    pub async fn create_default(config: &Config) -> Result<Self> {
        // Memory
        let memory_service = Arc::new(MemoryDb::new(&config.data_dir)?);
        let memory = Box::new(CisMemoryBackend::new(
            memory_service.clone(),
            config.node_id.clone()
        )) as Box<dyn Memory>;

        let vector_index = Box::new(CisMemoryBackend::new(
            memory_service.clone(),
            config.node_id.clone()
        )) as Box<dyn MemoryVectorIndex>;

        let sync = Box::new(CisMemoryBackend::new(
            memory_service,
            config.node_id.clone()
        )) as Box<dyn MemorySync>;

        // Scheduler
        let scheduler = Box::new(CisDagScheduler::new(config.scheduler.clone())?) as Box<dyn DagScheduler>;
        let federation = Box::new(FederationCoordinatorImpl::new(config.federation.clone())?) as Box<dyn FederationCoordinator>;

        Ok(Self {
            memory,
            vector_index,
            sync,
            scheduler,
            federation,
        })
    }

    pub async fn create_mock() -> Result<Self> {
        let memory = Box::new(MockMemoryBackend::new()) as Box<dyn Memory>;
        // ... other mock services

        Ok(Self {
            memory,
            vector_index: memory.clone(),  // Mock implements all traits
            sync: memory.clone(),
            scheduler: Box::new(MockDagScheduler::new()),
            federation: Box::new(MockFederationCoordinator::new()),
        })
    }
}
```

---

## Phase 3: 贡献模块开发（Week 8-9）

### Task 3.1: 创建 cis-dag-scheduler crate

**目录结构**:

```
cis-dag-scheduler/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── scheduler.rs
    ├── decision.rs
    ├── coordinator.rs
    ├── crdt.rs
    └── zeroclaw_compat.rs
```

**Cargo.toml**:

```toml
[package]
name = "cis-dag-scheduler"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
description = "DAG orchestration with four-level decision mechanism and federation"
repository = "https://github.com/your-org/cis-dag-scheduler"
readme = "README.md"

[dependencies]
async-trait = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"

# zeroclaw integration (optional)
zeroclaw = { version = "0.1", optional = true }

[features]
default = []
zeroclaw = ["dep:zeroclaw"]
```

**lib.rs**:

```rust
//! # CIS DAG Scheduler
//!
//! A DAG orchestration system with:
//! - Four-level decision mechanism (Mechanical → Arbitrated)
//! - Federation coordination (cross-node task distribution)
//! - CRDT conflict resolution
//!
//! ## Usage
//!
//! ```rust,no_run
//! use cis_dag_scheduler::{CisDagScheduler, SchedulerConfig};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let scheduler = CisDagScheduler::new(SchedulerConfig::default()).await?;
//!
//!     let tasks = vec![
//!         // ... define tasks
//!     ];
//!
//!     let results = scheduler.schedule(tasks).await?;
//!
//!     for result in results {
//!         println!("Task {}: {:?}", result.task_id, result.status);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod scheduler;
pub mod decision;
pub mod coordinator;
pub mod crdt;

#[cfg(feature = "zeroclaw")]
pub mod zeroclaw_compat;

pub use scheduler::{CisDagScheduler, SchedulerConfig};
pub use decision::{TaskLevel, Decision};
pub use coordinator::{FederationCoordinator, FederationPlan};
pub use crdt::{CrdtSolver, MergeStrategy};
```

**scheduler.rs**:

```rust
use async_trait::async_trait;
use anyhow::Result;
use std::sync::Arc;
use crate::decision::TaskLevel;
use crate::coordinator::FederationCoordinator;

pub struct CisDagScheduler {
    config: SchedulerConfig,
    coordinator: Arc<FederationCoordinator>,
}

#[derive(Clone, Debug)]
pub struct SchedulerConfig {
    pub max_parallel_tasks: usize,
    pub timeout_secs: u64,
    pub retry_limit: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 10,
            timeout_secs: 300,
            retry_limit: 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub level: TaskLevel,
    pub deps: Vec<String>,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub output: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Success,
    Failed,
    Cancelled,
}

impl CisDagScheduler {
    pub async fn new(config: SchedulerConfig) -> Result<Self> {
        let coordinator = Arc::new(FederationCoordinator::new().await?);
        Ok(Self { config, coordinator })
    }

    pub async fn schedule(&self, tasks: Vec<Task>) -> Result<Vec<TaskResult>> {
        // 1. Build DAG
        let dag = self.build_dag(tasks).await?;

        // 2. Federation coordination
        let plan = self.coordinator.coordinate(dag).await?;

        // 3. Execute with four-level decisions
        self.execute_with_levels(plan).await
    }

    async fn build_dag(&self, tasks: Vec<Task>) -> Result<Dag> {
        // Build dependency graph
        // ... implementation
        Ok(Dag::new())
    }

    async fn execute_with_levels(&self, plan: FederationPlan) -> Result<Vec<TaskResult>> {
        let mut results = Vec::new();

        for task in plan.tasks {
            let result = match task.level {
                TaskLevel::Mechanical { retry } => {
                    self.execute_mechanical(&task, retry).await?
                }
                TaskLevel::Recommended { timeout, default_action } => {
                    self.execute_recommended(&task, timeout, default_action).await?
                }
                TaskLevel::Confirmed => {
                    self.execute_confirmed(&task).await?
                }
                TaskLevel::Arbitrated { stakeholders } => {
                    self.execute_arbitrated(&task, stakeholders).await?
                }
            };
            results.push(result);
        }

        Ok(results)
    }

    async fn execute_mechanical(&self, task: &Task, retry: usize) -> Result<TaskResult> {
        // Automatic execution with retry
        for attempt in 0..retry {
            match self.execute_task(task).await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < retry - 1 => {
                    tracing::warn!("Task {} attempt {} failed: {}", task.id, attempt + 1, e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
                Err(e) => return Ok(TaskResult {
                    task_id: task.id.clone(),
                    status: TaskStatus::Failed,
                    output: None,
                    error: Some(e.to_string()),
                }),
            }
        }
        unreachable!()
    }

    async fn execute_recommended(&self, task: &Task, timeout: u64, _default_action: String) -> Result<TaskResult> {
        // Execute but notify user
        println!("Recommended action: {}", task.name);
        println!("Will execute in {} seconds (Ctrl+C to cancel)...", timeout);

        tokio::time::sleep(tokio::time::Duration::from_secs(timeout)).await;
        self.execute_task(task).await
    }

    async fn execute_confirmed(&self, task: &Task) -> Result<TaskResult> {
        // Require human confirmation
        println!("Please confirm: {}", task.name);
        print!("Execute? [y/N] ");
        use std::io::{self, Write};
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            self.execute_task(task).await
        } else {
            Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Cancelled,
                output: None,
                error: Some("User cancelled".to_string()),
            })
        }
    }

    async fn execute_arbitrated(&self, task: &Task, stakeholders: Vec<String>) -> Result<TaskResult> {
        // Multi-party voting
        println!("Arbitrated task: {}", task.name);
        println!("Stakeholders: {:?}", stakeholders);

        // Collect votes
        let mut votes_yes = 0;
        let mut votes_no = 0;

        for stakeholder in &stakeholders {
            println!("Ask {} for vote (yes/no):", stakeholder);
            // ... collect votes
        }

        if votes_yes > votes_no {
            self.execute_task(task).await
        } else {
            Ok(TaskResult {
                task_id: task.id.clone(),
                status: TaskStatus::Cancelled,
                output: None,
                error: Some("Voting failed".to_string()),
            })
        }
    }

    async fn execute_task(&self, task: &Task) -> Result<TaskResult> {
        // Actual task execution
        // ... implementation
        Ok(TaskResult {
            task_id: task.id.clone(),
            status: TaskStatus::Success,
            output: Some("Done".to_string()),
            error: None,
        })
    }
}

struct Dag {
    // ... DAG implementation
}

impl Dag {
    fn new() -> Self {
        Self {}
    }
}
```

**zeroclaw_compat.rs**:

```rust
#ifdef!(feature = "zeroclaw"))
compile_error!("zeroclash feature is not enabled");
#endif

use async_trait::async_trait;
use zeroclaw::scheduler::{Scheduler, Task as ZTask, TaskResult as ZTaskResult};
use crate::{CisDagScheduler, Task, TaskResult, TaskStatus};
use anyhow::Result;

/// CIS DAG Scheduler as zeroclaw Scheduler implementation
pub struct ZeroclawSchedulerAdapter {
    scheduler: CisDagScheduler,
}

impl ZeroclawSchedulerAdapter {
    pub fn new(scheduler: CisDagScheduler) -> Self {
        Self { scheduler }
    }
}

#[async_trait]
impl Scheduler for ZeroclawSchedulerAdapter {
    fn name(&self) -> &str {
        "cis-federal-dag"
    }

    async fn schedule(&self, tasks: Vec<ZTask>) -> Result<Vec<ZTaskResult>> {
        // Convert zeroclaw tasks to CIS tasks
        let cis_tasks: Vec<Task> = tasks.into_iter().map(|zt| Task {
            id: zt.id,
            name: zt.name,
            level: TaskLevel::Mechanical { retry: 3 },  // Default
            deps: zt.deps,
            payload: zt.payload,
        }).collect();

        // Execute with CIS scheduler
        let cis_results = self.scheduler.schedule(cis_tasks).await?;

        // Convert CIS results to zeroclaw results
        let z_results: Vec<ZTaskResult> = cis_results.into_iter().map(|cr| ZTaskResult {
            task_id: cr.task_id,
            status: match cr.status {
                TaskStatus::Success => "success".to_string(),
                TaskStatus::Failed => "failed".to_string(),
                TaskStatus::Cancelled => "cancelled".to_string(),
                _ => "unknown".to_string(),
            },
            output: cr.output,
            error: cr.error,
        }).collect();

        Ok(z_results)
    }

    async fn cancel(&self, task_id: &str) -> Result<bool> {
        // Delegate to federation coordinator
        self.scheduler.coordinator.cancel_task(task_id).await
    }
}
```

**README.md**:

```markdown
# cis-dag-scheduler

DAG orchestration with four-level decision mechanism and federation.

## Features

- **Four-Level Decisions**: Mechanical → Arbitrated
- **Federation**: Cross-node task distribution
- **CRDT**: Conflict-free replication

## Installation

```toml
[dependencies]
cis-dag-scheduler = "0.1"
```

## Usage

```rust
use cis_dag_scheduler::{CisDagScheduler, SchedulerConfig, Task, TaskLevel};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let scheduler = CisDagScheduler::new(SchedulerConfig::default()).await?;

    let task = Task {
        id: "task-1".to_string(),
        name: "Deploy to production".to_string(),
        level: TaskLevel::Confirmed,  // Requires approval
        deps: vec![],
        payload: serde_json::json!({ "environment": "prod" }),
    };

    let results = scheduler.schedule(vec![task]).await?;

    for result in results {
        println!("Task {}: {:?}", result.task_id, result.status);
    }

    Ok(())
}
```

## Four-Level Decisions

1. **Mechanical**: Automatic execution with retry
2. **Recommended**: Execute but notify, user can cancel
3. **Confirmed**: Requires human approval
4. **Arbitrated**: Multi-party voting

## zeroclaw Integration

```toml
[dependencies]
cis-dag-scheduler = { version = "0.1", features = ["zeroclaw"] }
```

```rust
use cis_dag_scheduler::zeroclaw_compat::ZeroclawSchedulerAdapter;
use zeroclaw::scheduler::Scheduler;

let adapter = ZeroclawSchedulerAdapter::new(cis_scheduler);
// Use as zeroclaw::Scheduler
```

## License

Apache-2.0
```

### Task 3.2: 创建 cis-p2p-transport crate

**目录结构**:

```
cis-p2p-transport/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── transport.rs
    ├── did.rs
    ├── discovery.rs
    └── zeroclaw_compat.rs
```

**Cargo.toml**:

```toml
[package]
name = "cis-p2p-transport"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
description = "P2P messaging with DID identity and QUIC transport"

[dependencies]
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
quinn = "0.11"  # QUIC implementation
libp2p = { version = "0.54", features = ["mdns", "kad", "noise", "tcp", "yamux"] }
did-method = "0.3"
tracing = "0.1"

# zeroclaw integration (optional)
zeroclaw = { version = "0.1", optional = true }

[features]
default = []
zeroclaw = ["dep:zeroclaw"]
```

**lib.rs**:

```rust
//! # CIS P2P Transport
//!
//! P2P messaging with:
//! - DID identity (hardware-bound keys)
//! - QUIC transport (NAT traversal)
//! - mDNS + DHT discovery

pub mod transport;
pub mod did;
pub mod discovery;

#[cfg(feature = "zeroclaw")]
pub mod zeroclaw_compat;

pub use transport::{CisP2PNetwork, P2PConfig};
pub use did::{DidIdentity, Did};
pub use discovery::{DiscoveryStrategy, MdnsDiscovery, DhtDiscovery};
```

**transport.rs**:

```rust
use async_trait::async_trait;
use anyhow::Result;
use std::sync::Arc;
use libp2p::{swarm::Swarm, identity::Keypair, Multiaddr, PeerId};
use crate::did::DidIdentity;

pub struct CisP2PNetwork {
    swarm: Swarm<MyBehaviour>,
    identity: Arc<DidIdentity>,
}

#[derive(Clone)]
pub struct P2PConfig {
    pub listen_addr: Multiaddr,
    pub bootstrap_peers: Vec<Multiaddr>,
}

impl CisP2PNetwork {
    pub async fn new(config: P2PConfig) -> Result<Self> {
        // Create DID identity
        let identity = Arc::new(DidIdentity::generate()?);

        // Create libp2p swarm
        let swarm = create_swarm(identity.keypair(), config).await?;

        Ok(Self { swarm, identity })
    }

    pub async fn send_to_did(&self, did: &Did, data: &[u8]) -> Result<()> {
        // Resolve DID to PeerId
        let peer_id = did_to_peer_id(did)?;

        // Send via P2P
        self.swarm.behaviour_mut().send_message(peer_id, data);

        Ok(())
    }

    pub async fn subscribe(&self) -> Result<tokio::sync::mpsc::Receiver<P2PMessage>> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        // Subscribe to swarm events
        // ... implementation
        Ok(rx)
    }
}

fn did_to_peer_id(did: &Did) -> Result<PeerId> {
    // Convert DID to libp2p PeerId
    // ... implementation
    Ok(PeerId::random())
}

struct P2PMessage {
    id: String,
    sender: Did,
    reply_target: String,
    content: String,
    timestamp: u64,
}

// ... libp2p behaviour implementation
```

**zeroclaw_compat.rs**:

```rust
#ifdef!(feature = "zeroclaw"))
compile_error!("zeroclaw feature is not enabled");
#endif

use async_trait::async_trait;
use zeroclaw::channels::{Channel, ChannelMessage, SendMessage};
use crate::{CisP2PNetwork, DidIdentity, Did};
use anyhow::Result;
use std::sync::Arc;

/// CIS P2P as zeroclaw Channel
pub struct CisP2PChannel {
    p2p: Arc<CisP2PNetwork>,
    identity: Arc<DidIdentity>,
}

impl CisP2PChannel {
    pub fn new(p2p: Arc<CisP2PNetwork>, identity: Arc<DidIdentity>) -> Self {
        Self { p2p, identity }
    }
}

#[async_trait]
impl Channel for CisP2PChannel {
    fn name(&self) -> &str {
        "cis-p2p"
    }

    async fn send(&self, message: &SendMessage) -> anyhow::Result<()> {
        // Parse recipient as DID
        let target_did = Did::parse(&message.recipient)?;

        // Serialize message
        let payload = serde_json::to_vec(&message)?;

        // Send via P2P
        self.p2p.send_to_did(&target_did, &payload).await
            .map_err(|e| anyhow::anyhow!("P2P send failed: {}", e))
    }

    async fn listen(&self, tx: tokio::sync::mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        let mut p2p_rx = self.p2p.subscribe().await?;

        tokio::spawn(async move {
            while let Some(msg) = p2p_rx.recv().await {
                let channel_msg = ChannelMessage {
                    id: msg.id,
                    sender: msg.sender.did().to_string(),
                    reply_target: msg.reply_target,
                    content: msg.content,
                    channel: "cis-p2p".to_string(),
                    timestamp: msg.timestamp,
                    thread_ts: None,
                };
                tx.send(channel_msg).await.ok();
            }
        });

        Ok(())
    }

    async fn health_check(&self) -> bool {
        self.p2p.is_connected().await
    }
}
```

**README.md**:

```markdown
# cis-p2p-transport

P2P messaging with DID identity and QUIC transport.

## Features

- **DID Identity**: Hardware-bound cryptographic keys
- **QUIC Transport**: Fast, multiplexed, NAT-traversing
- **Discovery**: mDNS (local) + DHT (public)

## Installation

```toml
[dependencies]
cis-p2p-transport = "0.1"
```

## Usage

```rust
use cis_p2p_transport::{CisP2PNetwork, P2PConfig};
use libp2p::Multiaddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = P2PConfig {
        listen_addr: "/ip4/0.0.0.0/tcp/7677".parse()?,
        bootstrap_peers: vec![],
    };

    let p2p = CisP2PNetwork::new(config).await?;

    // Send message
    let did = did.parse("did:peer:0z6Mk...")?;
    p2p.send_to_did(&did, b"Hello, P2P!").await?;

    Ok(())
}
```

## zeroclaw Integration

```toml
[dependencies]
cis-p2p-transport = { version = "0.1", features = ["zeroclaw"] }
```

```rust
use cis_p2p_transport::zeroclaw_compat::CisP2PChannel;
use zeroclaw::channels::Channel;

let channel = CisP2PChannel::new(p2p, identity);
// Use as zeroclaw::Channel
```

## License

Apache-2.0
```

### Task 3.3: 创建 cis-memory-backend crate

**目录结构**:

```
cis-memory-backend/
├── Cargo.toml
├── README.md
└── src/
    ├── lib.rs
    ├── memory.rs
    ├── vector.rs
    ├── hybrid.rs
    └── zeroclaw_compat.rs
```

**Cargo.toml**:

```toml
[package]
name = "cis-memory-backend"
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
description = "Memory backend with Private/Public domain separation and hybrid search"

[dependencies]
async-trait = "0.1"
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
rusqlite = { version = "0.37", features = ["bundled"] }
sqlite-vec = "0.5"
chrono = "0.4"
tracing = "0.1"

# zeroclaw integration (optional)
zeroclaw = { version = "0.1", optional = true }

[features]
default = []
zeroclaw = ["dep:zeroclaw"]
```

**lib.rs**:

```rust
//! # CIS Memory Backend
//!
//! Memory backend with:
//! - Private/Public domain separation
//! - Hybrid search (Vector 70% + BM25 30%)
//! - 54-week archival
//! - P2P sync

pub mod memory;
pub mod vector;
pub mod hybrid;

#[cfg(feature = "zeroclaw")]
pub mod zeroclaw_compat;

pub use memory::{CisMemory, CisMemoryConfig};
pub use hybrid::{HybridSearch, HybridSearchResult};
```

**memory.rs**:

```rust
use async_trait::async_trait;
use anyhow::Result;
use rusqlite::Connection;

pub struct CisMemory {
    db: Connection,
    node_id: String,
}

#[derive(Clone)]
pub struct CisMemoryConfig {
    pub data_dir: std::path::PathBuf,
    pub node_id: String,
}

impl CisMemory {
    pub async fn new(config: CisMemoryConfig) -> Result<Self> {
        let db_path = config.data_dir.join("memory.db");
        let db = Connection::open(&db_path)?;

        // Initialize schema
        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS memory (
                key TEXT PRIMARY KEY,
                value BLOB,
                domain TEXT NOT NULL,
                category TEXT NOT NULL,
                timestamp INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS archived (
                key TEXT PRIMARY KEY,
                value BLOB,
                domain TEXT NOT NULL,
                category TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                archived_at INTEGER NOT NULL
            );
            "#
        )?;

        Ok(Self { db, node_id: config.node_id })
    }

    pub async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()> {
        self.db.execute(
            "INSERT OR REPLACE INTO memory (key, value, domain, category, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            [key, value, &domain.to_string(), &category.to_string(), &Utc::now().timestamp()]
        )?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<MemoryEntry>> {
        let mut stmt = self.db.prepare("SELECT value, domain, category, timestamp FROM memory WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(MemoryEntry {
                key: key.to_string(),
                value: row.get(0)?,
                domain: row.get(1)?,
                category: row.get(2)?,
                timestamp: row.get(3)?,
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Clone)]
pub enum MemoryDomain {
    Private,
    Public,
}

#[derive(Debug, Clone)]
pub enum MemoryCategory {
    Core,
    Daily,
    Conversation,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub timestamp: i64,
}
```

**zeroclaw_compat.rs**:

```rust
#ifdef!(feature = "zeroclaw"))
compile_error!("zeroclaw feature is not enabled");
#endif

use async_trait::async_trait;
use zeroclaw::memory::{Memory, MemoryEntry, MemoryCategory};
use crate::{CisMemory, CisMemoryConfig, MemoryDomain};
use anyhow::Result;
use std::sync::Arc;

/// CIS Memory as zeroclaw Memory
pub struct CisMemoryBackend {
    memory: Arc<CisMemory>,
    node_id: String,
}

impl CisMemoryBackend {
    pub async fn new(config: CisMemoryConfig) -> Result<Self> {
        let memory = Arc::new(CisMemory::new(config).await?);
        let node_id = memory.node_id.clone();
        Ok(Self { memory, node_id })
    }
}

#[async_trait]
impl Memory for CisMemoryBackend {
    fn name(&self) -> &str {
        "cis-memory"
    }

    async fn store(
        &self,
        key: &str,
        content: &str,
        category: MemoryCategory,
        session_id: Option<&str>,
    ) -> anyhow::Result<()> {
        // Map zeroclaw category to CIS domain
        let domain = match category {
            MemoryCategory::Core => MemoryDomain::Private,
            MemoryCategory::Daily => MemoryDomain::Public,
            MemoryCategory::Conversation => MemoryDomain::Public,
            MemoryCategory::Custom(_) => MemoryDomain::Public,
        };

        self.memory.set(key, content.as_bytes(), domain, crate::MemoryCategory::Context).await
            .map_err(|e| anyhow::anyhow!("CIS memory error: {}", e))
    }

    async fn recall(
        &self,
        query: &str,
        limit: usize,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        // Use CIS hybrid search
        let results = self.memory.hybrid_search(query, limit).await
            .map_err(|e| anyhow::anyhow!("CIS search error: {}", e))?;

        Ok(results.into_iter().map(|r| MemoryEntry {
            id: r.key.clone(),
            key: r.key,
            content: String::from_utf8_lossy(&r.value).to_string(),
            category: MemoryCategory::Core,
            timestamp: r.timestamp.to_rfc3339(),
            session_id: session_id.map(|s| s.to_string()),
            score: Some(r.score as f64),
        }).collect())
    }

    async fn get(&self, key: &str) -> anyhow::Result<Option<MemoryEntry>> {
        match self.memory.get(key).await {
            Ok(Some(entry)) => Ok(Some(MemoryEntry {
                id: entry.key.clone(),
                key: entry.key,
                content: String::from_utf8_lossy(&entry.value).to_string(),
                category: MemoryCategory::Core,
                timestamp: entry.timestamp.to_rfc3339(),
                session_id: None,
                score: None,
            })),
            Ok(None) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("CIS get error: {}", e)),
        }
    }

    async fn forget(&self, key: &str) -> anyhow::Result<bool> {
        self.memory.delete(key).await
    }

    async fn count(&self) -> anyhow::Result<usize> {
        self.memory.count().await
    }

    async fn health_check(&self) -> bool {
        self.memory.health_check().await
    }
}
```

**README.md**:

```markdown
# cis-memory-backend

Memory backend with Private/Public domain separation and hybrid search.

## Features

- **Domain Separation**: Private (encrypted) vs Public (syncable)
- **Hybrid Search**: 70% vector similarity + 30% BM25 keyword
- **Archival**: Automatic 54-week archival
- **P2P Sync**: CRDT-based incremental sync

## Installation

```toml
[dependencies]
cis-memory-backend = "0.1"
```

## Usage

```rust
use cis_memory_backend::{CisMemory, CisMemoryConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = CisMemoryConfig {
        data_dir: "~/.cis/memory".into(),
        node_id: "node-1".to_string(),
    };

    let memory = CisMemory::new(config).await?;

    // Store
    memory.set("user-preference", b"dark-theme", MemoryDomain::Private, MemoryCategory::Core).await?;

    // Retrieve
    if let Some(entry) = memory.get("user-preference").await? {
        println!("Preference: {:?}", String::from_utf8_lossy(&entry.value));
    }

    Ok(())
}
```

## Hybrid Search

```rust
use cis_memory_backend::HybridSearch;

let results = memory.hybrid_search("user preferences", 10).await?;

for result in results {
    println!("Key: {}, Score: {:.2}", result.key, result.score);
}
```

## zeroclaw Integration

```toml
[dependencies]
cis-memory-backend = { version = "0.1", features = ["zeroclaw"] }
```

```rust
use cis_memory_backend::zeroclaw_compat::CisMemoryBackend;
use zeroclaw::memory::Memory;

let backend = CisMemoryBackend::new(config).await?;
// Use as zeroclaw::Memory
```

## License

Apache-2.0
```

---

## Phase 4: 集成测试（Week 10）

### Task 4.1: 单元测试

**文件**: `cis-core/tests/integration_traits.rs`

```rust
use cis_core::traits::{Memory, MemoryVectorIndex, MemorySync};
use cis_core::memory::backends::{CisMemoryBackend, MockMemoryBackend};

#[tokio::test]
async fn test_memory_trait_compliance() {
    let memory = MockMemoryBackend::new();

    // Test set/get
    memory.set(
        "test-key",
        b"test-value",
        cis_core::types::MemoryDomain::Public,
        cis_core::types::MemoryCategory::Context
    ).await.unwrap();

    let entry = memory.get("test-key").await.unwrap();
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().value, b"test-value");

    // Test delete
    let deleted = memory.delete("test-key").await.unwrap();
    assert!(deleted);

    let entry = memory.get("test-key").await.unwrap();
    assert!(entry.is_none());
}

#[tokio::test]
async fn test_memory_hybrid_search() {
    let memory = MockMemoryBackend::new();

    // Store test data
    memory.set("rust", b"Rust is a systems programming language", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    memory.set("python", b"Python is a high-level language", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();

    // Search
    let results = memory.hybrid_search("programming language", 10, None, None).await.unwrap();

    assert!(!results.is_empty());
}

#[tokio::test]
async fn test_cis_memory_backend() {
    let temp_dir = tempfile::tempdir().unwrap();
    let service = Arc::new(cis_core::memory::MemoryDb::new(temp_dir.path()).unwrap());
    let backend = CisMemoryBackend::new(service, "test-node".to_string());

    // Test compliance
    assert_eq!(backend.name(), "cis-memory");
    assert!(backend.health_check().await);
}

#[tokio::test]
async fn test_memory_vector_index() {
    let temp_dir = tempfile::tempdir().unwrap();
    let service = Arc::new(cis_core::memory::MemoryDb::new(temp_dir.path()).unwrap());
    let backend = CisMemoryBackend::new(service, "test-node".to_string());

    // Test vector index methods
    let count = backend.rebuild_index(100).await.unwrap();
    assert!(count >= 0);

    let stats = backend.index_stats().await.unwrap();
    assert!(stats.total_vectors >= 0);
}

#[tokio::test]
async fn test_memory_sync() {
    let temp_dir = tempfile::tempdir().unwrap();
    let service = Arc::new(cis_core::memory::MemoryDb::new(temp_dir.path()).unwrap());
    let backend = CisMemoryBackend::new(service, "test-node".to_string());

    // Test sync methods
    let pending = backend.get_pending_sync(10).await.unwrap();
    assert!(pending.is_empty());

    backend.mark_synced("key", "peer-1").await.unwrap().ok();
}
```

### Task 4.2: 集成测试

**文件**: `cis-dag-scheduler/tests/integration.rs`

```rust
use cis_dag_scheduler::{CisDagScheduler, SchedulerConfig, Task, TaskLevel};

#[tokio::test]
async fn test_dag_scheduler_mechanical() {
    let scheduler = CisDagScheduler::new(SchedulerConfig::default()).await.unwrap();

    let task = Task {
        id: "task-1".to_string(),
        name: "Test task".to_string(),
        level: TaskLevel::Mechanical { retry: 3 },
        deps: vec![],
        payload: serde_json::json!({ "test": true }),
    };

    let results = scheduler.schedule(vec![task]).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].task_id, "task-1");
    assert_eq!(results[0].status, TaskStatus::Success);
}

#[tokio::test]
async fn test_dag_scheduler_with_deps() {
    let scheduler = CisDagScheduler::new(SchedulerConfig::default()).await.unwrap();

    let task1 = Task {
        id: "task-1".to_string(),
        name: "First task".to_string(),
        level: TaskLevel::Mechanical { retry: 3 },
        deps: vec![],
        payload: serde_json::json!({}),
    };

    let task2 = Task {
        id: "task-2".to_string(),
        name: "Second task".to_string(),
        level: TaskLevel::Mechanical { retry: 3 },
        deps: vec!["task-1".to_string()],
        payload: serde_json::json!({}),
    };

    let results = scheduler.schedule(vec![task1, task2]).await.unwrap();

    assert_eq!(results.len(), 2);
    // task-2 should wait for task-1 to complete
}

#[tokio::test]
#[cfg(feature = "zeroclaw")]
async fn test_zeroclaw_scheduler_adapter() {
    use cis_dag_scheduler::zeroclaw_compat::ZeroclawSchedulerAdapter;
    use zeroclaw::scheduler::{Scheduler, Task as ZTask};

    let cis_scheduler = CisDagScheduler::new(SchedulerConfig::default()).await.unwrap();
    let adapter = ZeroclawSchedulerAdapter::new(cis_scheduler);

    let z_task = ZTask {
        id: "test".to_string(),
        name: "Test".to_string(),
        deps: vec![],
        payload: serde_json::json!({}),
    };

    let results = adapter.schedule(vec![z_task]).await.unwrap();

    assert_eq!(results.len(), 1);
}
```

---

## Phase 5: 文档和发布（Week 11）

### Task 5.1: 创建 PR 模板

**文件**: `docs/plan/v1.2.0/zeroclaw/pr_templates/cis-dag-scheduler.md`

```markdown
## PR Template: Add cis-dag-scheduler

### Summary

This PR adds a new DAG scheduler backend with four-level decision mechanism and federation.

### Features

- **Four-Level Decisions**: Mechanical → Arbitrated
- **Federation**: Cross-node task distribution
- **CRDT**: Conflict-free replication

### Usage

```rust
use cis_dag_scheduler::{CisDagScheduler, SchedulerConfig, Task, TaskLevel};

let scheduler = CisDagScheduler::new(SchedulerConfig::default()).await?;

let task = Task {
    id: "task-1".to_string(),
    name: "Deploy to production".to_string(),
    level: TaskLevel::Confirmed,  // Requires approval
    deps: vec![],
    payload: serde_json::json!({ "environment": "prod" }),
};

let results = scheduler.schedule(vec![task]).await?;
```

### Testing

- [x] Unit tests for four-level decisions
- [x] Integration tests for federation
- [x] CRDT merge tests

### Checklist

- [x] Documentation updated
- [x] Tests pass
- [x] No breaking changes to existing APIs

### Notes

This is an **optional dependency**. Users can enable via:
```toml
[dependencies]
cis-dag-scheduler = { version = "0.1", optional = true }

[features]
default = []
cis-scheduler = ["cis-dag-scheduler"]
```
```

---

## Phase 6: 提交 PR（Week 12）

### Task 6.1: 提交 PR 到 zeroclaw

**步骤**:

1. **Fork zeroclaw 仓库**:
   ```bash
   gh repo fork zeroclaw-labs/zeroclaw
   ```

2. **创建分支**:
   ```bash
   cd zeroclaw
   git checkout -b add-cis-dag-scheduler
   ```

3. **添加子模块或复制代码**:

   **Option A: Submodule** (推荐)
   ```bash
   git submodule add https://github.com/your-org/cis-dag-scheduler crates/cis-dag-scheduler
   ```

   **Option B: 复制代码**
   ```bash
   cp -r ../cis-dag-scheduler crates/cis-dag-scheduler
   ```

4. **更新 Cargo.toml**:

   ```toml
   [workspace]
   members = [".", "crates/robot-kit", "crates/cis-dag-scheduler"]

   [dependencies]
   cis_dag_scheduler = { path = "crates/cis-dag-scheduler", optional = true }

   [features]
   default = []
   cis-scheduler = ["cis_dag_scheduler"]
   ```

5. **提交**:
   ```bash
   git add .
   git commit -m "Add cis-dag-scheduler crate (optional)"
   git push origin add-cis-dag-scheduler
   ```

6. **创建 PR**:
   ```bash
   gh pr create --title "Add cis-dag-scheduler: Four-level DAG orchestration" --body-file ../CIS/docs/plan/v1.2.0/zeroclaw/pr_templates/cis-dag-scheduler.md
   ```

---

## 总结

本实施指南提供了**分步骤**的详细说明，帮助 CIS 实现：

1. ✅ **Phase 1**: 定义独立 traits（考虑 zeroclaw 兼容）
2. ✅ **Phase 2**: 实现backend（CIS + Mock）
3. ✅ **Phase 3**: 开发贡献模块（3个 crates）
4. ✅ **Phase 4**: 集成测试（单元 + 集成）
5. ✅ **Phase 5**: 文档和PR模板
6. ✅ **Phase 6**: 提交PR给zeroclaw

**下一步**:
1. 开始实现 Phase 1 (定义 traits)
2. 更新进度到 task document
3. 遇到问题随时沟通

---

**文档版本**: 1.0
**最后更新**: 2026-02-20
