# CIS v1.2.0 - 全面 Trait 模块拆分任务列表

> **核心目标**: 将 CIS 所有核心模块拆分为 Trait 抽象，降低系统耦合度，提升可测试性和可维护性

## 🎯 设计原则

1. **依赖倒置**: 高层模块不依赖低层模块，都依赖抽象
2. **开闭原则**: 对扩展开放，对修改关闭
3. **接口隔离**: 使用者不应该依赖它不需要的接口
4. **单一职责**: 每个 trait 只关注一个抽象

---

## 📊 模块耦合度分析

| 模块 | 当前耦合度 | 耦合来源 | Trait 化优先级 |
|------|-----------|---------|---------------|
| **Memory** | 🔴 高 | 直接依赖 SQLite, VectorStorage | P0 |
| **Network** | 🔴 高 | 直接依赖 QUIC, WebSocket | P0 |
| **Skill** | 🔴 高 | 直接依赖 WASM, FileSystem | P0 |
| **Scheduler** | 🔴 高 | 直接依赖 DAG, TaskExecutor | P0 |
| **Vector** | 🟡 中 | 直接依赖 sqlite-vec | P1 |
| **P2P** | 🔴 高 | 直接依赖 libp2p, QUIC | P1 |
| **Security** | 🟡 中 | 直接依赖 DID, 加密算法 | P1 |
| **Storage** | 🟡 中 | 直接依赖 SQLite | P1 |
| **Identity** | 🟢 低 | 相对独立 | P2 |

---

## Phase 1: 核心 Trait 抽象（Week 1-3）🔥 **P0**

### 1. Memory Trait 层

#### Task 1.1: 定义 Memory Trait
**文件**: `cis-core/src/traits/memory.rs`

```rust
/// 核心 Memory 抽象
#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;

    // CRUD 操作
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> Result<bool>;

    // 搜索操作
    async fn search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>>;
    async fn hybrid_search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<HybridSearchResult>>;

    // 批量操作
    async fn list_keys(&self, prefix: Option<&str>, domain: Option<MemoryDomain>) -> Result<Vec<String>>;

    // 健康检查
    async fn health_check(&self) -> Result<HealthStatus>;
}

/// Memory 扩展 trait - 向量索引
#[async_trait]
pub trait MemoryVectorIndex: Memory {
    async fn index(&self, key: &str, content: &[u8], category: &str) -> Result<()>;
    async fn search_vector(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorResult>>;
}

/// Memory 扩展 trait - 同步
#[async_trait]
pub trait MemorySync: Memory {
    async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>>;
    async fn mark_synced(&self, key: &str) -> Result<()>;
}
```

- [ ] 定义核心 `Memory` trait
- [ ] 定义 `MemoryEntry`, `SearchResult`, `HybridSearchResult`
- [ ] 定义扩展 trait: `MemoryVectorIndex`, `MemorySync`
- [ ] 添加文档和示例

#### Task 1.2: 实现 CIS Memory Backend
**文件**: `cis-core/src/traits/implementations/cis_memory.rs`

- [ ] 创建 `CisMemoryBackend` (包装 `MemoryService`)
- [ ] 实现 `Memory` trait
- [ ] 实现 `MemoryVectorIndex` trait
- [ ] 实现 `MemorySync` trait
- [ ] 添加构造函数和类型转换

#### Task 1.3: 实现 Mock Memory
**文件**: `cis-core/src/traits/mock/mock_memory.rs`

- [ ] 创建 `MockMemory` (基于 `HashMap`)
- [ ] 实现所有 memory traits
- [ ] 添加测试辅助方法（如 `assert_called`）

---

### 1.2 Network Trait 层

#### Task 1.4: 定义 Network Trait
**文件**: `cis-core/src/traits/network.rs`

```rust
/// 网络传输抽象
#[async_trait]
pub trait Transport: Send + Sync {
    fn name(&self) -> &str;
    fn local_addr(&self) -> String;

    // 点对点通信
    async fn send(&self, target: &NodeId, data: &[u8]) -> Result<()>;
    async fn receive(&self) -> Result<(NodeId, Vec<u8>)>;
    async fn broadcast(&self, data: &[u8]) -> Result<usize>;

    // 生命周期
    async fn start(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}

/// 节点发现抽象
#[async_trait]
pub trait Discovery: Send + Sync {
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;
    async fn announce(&self, info: &PeerInfo) -> Result<()>;
}

/// P2P 网络抽象（组合 Transport + Discovery）
#[async_trait]
pub trait P2PNetwork: Send + Sync {
    type Transport: Transport;
    type Discovery: Discovery;

    fn transport(&self) -> &Self::Transport;
    fn discovery(&self) -> &Self::Discovery;

    async fn connect(&self, addr: &str) -> Result<()>;
    async fn disconnect(&self, peer: &NodeId) -> Result<()>;
    async fn peers(&self) -> Result<Vec<PeerInfo>>;
}
```

- [ ] 定义 `Transport` trait
- [ ] 定义 `Discovery` trait
- [ ] 定义 `P2PNetwork` trait
- [ ] 定义 `NodeId`, `PeerInfo` 类型

#### Task 1.5: 实现 CIS Network Backend
**文件**: `cis-core/src/traits/implementations/cis_network.rs`

- [ ] 创建 `QuicTransport` (包装现有 QUIC 实现)
- [ ] 创建 `WsTransport` (包装 WebSocket)
- [ ] 创建 `MdnsDiscovery` (包装 mDNS 发现)
- [ ] 创建 `CisP2PNetwork` (组合实现)

#### Task 1.6: 实现 Mock Network
**文件**: `cis-core/src/traits/mock/mock_network.rs`

- [ ] 创建 `MockTransport` (基于 `mpsc::channel`)
- [ ] 创建 `MockDiscovery`
- [ ] 添加网络延迟/丢包模拟（用于测试）

---

### 1.3 Skill Trait 层

#### Task 1.7: 定义 Skill Trait
**文件**: `cis-core/src/traits/skill.rs`

```rust
/// Skill 执行抽象
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;

    // 生命周期
    async fn load(&self, skill_id: &str) -> Result<LoadResult>;
    async fn unload(&self, skill_id: &str) -> Result<()>;

    // 执行
    async fn execute(&self, skill_id: &str, input: &SkillInput) -> Result<SkillOutput>;
    async fn execute_stream(&self, skill_id: &str, input: &SkillInput) -> Pin<Box<dyn Stream<Item = Result<SkillOutput>> + Send>>;

    // 状态
    async fn list_skills(&self) -> Result<Vec<SkillInfo>>;
    async fn get_skill_status(&self, skill_id: &str) -> Result<SkillStatus>;
}

/// Skill 加载器抽象
#[async_trait]
pub trait SkillLoader: Send + Sync {
    async fn load_from_file(&self, path: &Path) -> Result<Box<dyn SkillExecutor>>;
    async fn load_from_bytes(&self, bytes: &[u8]) -> Result<Box<dyn SkillExecutor>>;
}

/// Skill 存储抽象
#[async_trait]
pub trait SkillRegistry: Send + Sync {
    async fn register(&self, skill: &SkillInfo) -> Result<()>;
    async fn unregister(&self, skill_id: &str) -> Result<()>;
    async fn get(&self, skill_id: &str) -> Result<Option<SkillInfo>>;
    async fn list(&self) -> Result<Vec<SkillInfo>>;
}
```

- [ ] 定义 `SkillExecutor` trait
- [ ] 定义 `SkillLoader` trait
- [ ] 定义 `SkillRegistry` trait
- [ ] 定义 `SkillInput`, `SkillOutput`, `SkillInfo` 类型

#### Task 1.8: 实现 CIS Skill Backend
**文件**: `cis-core/src/traits/implementations/cis_skill.rs`

- [ ] 创建 `WasmSkillExecutor` (包装现有 WASM 实现)
- [ ] 创建 `NativeSkillExecutor` (Native skills)
- [ ] 创建 `CisSkillRegistry` (包装现有注册表)
- [ ] 创建 `WasmSkillLoader`

#### Task 1.9: 实现 Mock Skill
**文件**: `cis-core/src/traits/mock/mock_skill.rs`

- [ ] 创建 `MockSkillExecutor`
- [ ] 创建 `InMemorySkillRegistry`
- [ ] 添加执行历史记录（用于测试）

---

### 1.4 Scheduler Trait 层

#### Task 1.10: 定义 Scheduler Trait
**文件**: `cis-core/src/traits/scheduler.rs`

```rust
/// DAG 调度抽象
#[async_trait]
pub trait DagScheduler: Send + Sync {
    // DAG 管理
    async fn create_dag(&self, dag: &TaskDag) -> Result<DagId>;
    async fn get_dag(&self, id: &DagId) -> Result<Option<TaskDag>>;
    async fn delete_dag(&self, id: &DagId) -> Result<bool>;

    // 执行
    async fn execute(&self, dag_id: &DagId) -> Result<ExecutionId>;
    async fn get_execution(&self, id: &ExecutionId) -> Result<Option<Execution>>;

    // 控制
    async fn pause(&self, exec_id: &ExecutionId) -> Result<()>;
    async fn resume(&self, exec_id: &ExecutionId) -> Result<()>;
    async fn cancel(&self, exec_id: &ExecutionId) -> Result<()>;
}

/// 任务执行器抽象
#[async_trait]
pub trait TaskExecutor: Send + Sync {
    async fn execute_task(&self, task: &Task) -> Result<TaskResult>;
    async fn execute_skill(&self, skill_id: &str, input: &SkillInput) -> Result<SkillOutput>;
}

/// 任务持久化抽象
#[async_trait]
pub trait DagPersistence: Send + Sync {
    async fn save_dag(&self, dag: &TaskDag) -> Result<()>;
    async fn load_dag(&self, id: &DagId) -> Result<Option<TaskDag>>;
    async fn save_execution(&self, exec: &Execution) -> Result<()>;
    async fn load_execution(&self, id: &ExecutionId) -> Result<Option<Execution>>;
}
```

- [ ] 定义 `DagScheduler` trait
- [ ] 定义 `TaskExecutor` trait
- [ ] 定义 `DagPersistence` trait
- [ ] 定义相关类型: `TaskDag`, `Execution`, `TaskResult`

#### Task 1.11: 实现 CIS Scheduler Backend
**文件**: `cis-core/src/traits/implementations/cis_scheduler.rs`

- [ ] 创建 `CisDagScheduler` (包装现有 `DagScheduler`)
- [ ] 创建 `SkillTaskExecutor` (连接到 `SkillExecutor`)
- [ ] 创建 `SqliteDagPersistence` (包装现有持久化)

#### Task 1.12: 实现 Mock Scheduler
**文件**: `cis-core/src/traits/mock/mock_scheduler.rs`

- [ ] 创建 `MockDagScheduler`
- [ ] 创建 `InMemoryDagPersistence`

---

## Phase 2: 扩展 Trait 抽象（Week 4-5）🔥 **P1**

### 2.1 Vector Trait 层

#### Task 2.1: 定义 VectorIndex Trait
**文件**: `cis-core/src/traits/vector.rs`

```rust
/// 向量索引抽象
#[async_trait]
pub trait VectorIndex: Send + Sync {
    fn name(&self) -> &str;
    fn dimension(&self) -> usize;

    // 索引操作
    async fn insert(&self, id: &str, vector: &[f32], metadata: &Metadata) -> Result<()>;
    async fn delete(&self, id: &str) -> Result<bool>;
    async fn update(&self, id: &str, vector: &[f32]) -> Result<()>;

    // 搜索
    async fn search(&self, query: &[f32], limit: usize, threshold: f32) -> Result<Vec<VectorResult>>;

    // 批量
    async fn insert_batch(&self, items: &[(String, Vec<f32>, Metadata)]) -> Result<()>;
}
```

- [ ] 定义 `VectorIndex` trait
- [ ] 实现 `SqliteVecIndex` (包装 sqlite-vec)
- [ ] 实现 `MockVectorIndex`

---

### 2.2 Storage Trait 层

#### Task 2.2: 定义 Storage Trait
**文件**: `cis-core/src/traits/storage.rs`

```rust
/// 键值存储抽象
#[async_trait]
pub trait KeyValueStore: Send + Sync {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &[u8], value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &[u8]) -> Result<bool>;

    async fn scan(&self, prefix: &[u8], limit: usize) -> Result<Vec<(Vec<u8>, Vec<u8>)>>;
}

/// 数据库连接抽象
#[async_trait]
pub trait Database: Send + Sync {
    type Connection: DatabaseConnection;

    async fn connect(&self) -> Result<Self::Connection>;
    async fn close(&self) -> Result<()>;
}

/// 数据库连接抽象
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    async fn execute(&self, sql: &str, params: &[Value]) -> Result<ExecuteResult>;
    async fn query(&self, sql: &str, params: &[Value]) -> Result<Vec<Row>>;
}
```

- [ ] 定义 `KeyValueStore` trait
- [ ] 定义 `Database` trait
- [ ] 实现 `SqliteKVStore`, `SqliteDatabase`

---

### 2.3 Security Trait 层

#### Task 2.3: 定义 Security Trait
**文件**: `cis-core/src/traits/security.rs`

```rust
/// 加密抽象
#[async_trait]
pub trait Encryption: Send + Sync {
    fn algorithm(&self) -> &str;

    async fn encrypt(&self, plaintext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;
    async fn decrypt(&self, ciphertext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;

    fn generate_key(&self) -> Result<EncryptionKey>;
    fn derive_key(&self, password: &str, salt: &[u8]) -> Result<EncryptionKey>;
}

/// 签名抽象
#[async_trait]
pub trait Signature: Send + Sync {
    fn sign(&self, data: &[u8], key: &PrivateKey) -> Result<Signature>;
    fn verify(&self, data: &[u8], sig: &Signature, key: &PublicKey) -> Result<bool>;

    fn generate_keypair(&self) -> Result<(PrivateKey, PublicKey)>;
}

/// 身份抽象
#[async_trait]
pub trait Identity: Send + Sync {
    fn did(&self) -> &Did;
    fn public_key(&self) -> &PublicKey;

    async fn authenticate(&self, challenge: &Challenge) -> Result<Proof>;
    async fn verify_proof(&self, proof: &Proof, did: &Did) -> Result<bool>;
}
```

- [ ] 定义 `Encryption` trait
- [ ] 定义 `Signature` trait
- [ ] 定义 `Identity` trait
- [ ] 实现对应的后端

---

## Phase 3: ZeroClaw 兼容层（Week 6-7）🌟

### 3.1 ZeroClaw 适配器

#### Task 3.1: ZeroClaw Memory Adapter
**文件**: `cis-core/src/zeroclaw/memory_adapter.rs` (或独立 crate `zeroclaw-cis-memory`)

```rust
/// ZeroClaw Memory → CIS Memory 适配器
pub struct ZeroClawCisMemory {
    inner: Box<dyn Memory>,
}

#[async_trait]
impl zeroclaw::memory::Memory for ZeroClawCisMemory {
    async fn store(&self, key: &str, content: &str, category: MemoryCategory, session_id: Option<&str>) -> anyhow::Result<()> {
        // 映射 ZeroClaw 概念 → CIS 概念
        let domain = Self::map_category_to_domain(category);
        let cis_category = Self::map_category(category);

        self.inner.set(key, content.as_bytes(), domain, cis_category).await
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    async fn recall(&self, query: &str, limit: usize, session_id: Option<&str>) -> anyhow::Result<Vec<MemoryEntry>> {
        let results = self.inner.search(query, limit, 0.6).await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;

        Ok(results.into_iter().map(|r| Self::to_zeroclaw_entry(r)).collect())
    }
}
```

- [ ] 实现 ZeroClaw Memory → CIS Memory 适配器
- [ ] 映射概念：MemoryCategory, session_id → scope_id
- [ ] 处理错误转换
- [ ] 添加配置解析

#### Task 3.2: ZeroClaw Skill Adapter
- [ ] 实现 ZeroClaw Skill → CIS Skill 适配器
- [ ] 映射执行模型

#### Task 3.3: ZeroClaw 配置支持
**配置示例**: `zeroclaw-config.toml`

```toml
[memory]
backend = "cis"  # 使用 CIS 作为 Memory 后端

[memory.cis]
node_id = "my-workstation"
data_dir = "~/.cis"
enable_p2p = true
enable_hybrid_search = true

[skill]
backend = "cis"  # 使用 CIS 作为 Skill 后端

[skill.cis]
wasm_enabled = true
native_enabled = true
```

- [ ] 添加配置解析
- [ ] 添加工厂模式 `create_backend()`
- [ ] 支持运行时切换

---

## Phase 4: 重构现有代码（Week 8-10）🔧

### 4.1 重构 MemoryService

#### Task 4.1: 使用 trait 重写 MemoryService
**文件**: `cis-core/src/memory/service.rs`

**之前**:
```rust
pub struct MemoryService {
    memory_db: Arc<Mutex<MemoryDb>>,
    vector_storage: Arc<VectorStorage>,
}
```

**之后**:
```rust
pub struct MemoryService {
    memory: Box<dyn Memory>,
    vector_index: Box<dyn MemoryVectorIndex>,
    sync: Box<dyn MemorySync>,
}

impl MemoryService {
    pub fn new(memory: Box<dyn Memory>, vector_index: Box<dyn MemoryVectorIndex>) -> Self {
        Self { memory, vector_index, sync: ... }
    }
}
```

- [ ] 重构 `MemoryService` 使用 trait
- [ ] 保持向后兼容（保留旧构造函数）
- [ ] 更新所有调用方
- [ ] 更新测试

### 4.2 重构 NetworkManager

#### Task 4.2: 使用 trait 重写 NetworkManager
- [ ] 重构 `NetworkManager` 使用 `Transport`, `Discovery`, `P2PNetwork` traits
- [ ] 支持运行时切换传输层
- [ ] 更新所有调用方

### 4.3 重构 SkillManager

#### Task 4.3: 使用 trait 重写 SkillManager
- [ ] 重构 `SkillManager` 使用 `SkillExecutor`, `SkillLoader`, `SkillRegistry` traits
- [ ] 支持多种 skill 类型
- [ ] 更新测试

### 4.4 重构 Scheduler

#### Task 4.4: 使用 trait 重写 Scheduler
- [ ] 重构 `DagScheduler` 使用 trait
- [ ] 解耦 DAG 执行和持久化
- [ ] 更新测试

---

## Phase 5: 测试和文档（Week 11-12）📝

### 5.1 Trait 单元测试

#### Task 5.1: Memory Trait 测试
**文件**: `cis-core/src/traits/tests/memory_tests.rs`

```rust
#[tokio::test]
async fn test_memory_trait_mock() {
    let mock = Box::new(MockMemory::new());

    mock.set("key", b"value", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    let entry = mock.get("key").await.unwrap().unwrap();

    assert_eq!(entry.key, "key");
    assert_eq!(entry.value, b"value");
}

#[tokio::test]
async fn test_memory_trait_polymorphism() {
    // 同一份代码，不同的实现
    async fn test_memory(memory: Box<dyn Memory>) -> Result<()> {
        memory.set("test", b"data", MemoryDomain::Public, MemoryCategory::Context).await?;
        Ok(())
    }

    // 使用真实实现
    let real = Box::new(CisMemoryBackend::new(config)?);
    test_memory(real).await?;

    // 使用 mock 实现
    let mock = Box::new(MockMemory::new());
    test_memory(mock).await?;
}
```

- [ ] 测试所有 trait 基本功能
- [ ] 测试多态性
- [ ] 测试错误处理

#### Task 5.2: Network Trait 测试
- [ ] 测试 Transport, Discovery, P2PNetwork traits
- [ ] 测试 mock 的延迟/丢包模拟

#### Task 5.3: Skill Trait 测试
- [ ] 测试 SkillExecutor, SkillLoader, SkillRegistry traits
- [ ] 测试执行流和错误恢复

#### Task 5.4: Scheduler Trait 测试
- [ ] 测试 DagScheduler, TaskExecutor, DagPersistence traits
- [ ] 测试 DAG 执行和状态管理

---

### 5.2 集成测试

#### Task 5.5: 端到端集成测试
**文件**: `cis-core/tests/integration_traits.rs`

```rust
#[tokio::test]
async fn test_full_stack_with_traits() {
    // 使用 trait 组合完整的 CIS 系统
    let memory = Box::new(CisMemoryBackend::new(config));
    let transport = Box::new(QuicTransport::new(config));
    let skill_executor = Box::new(WasmSkillExecutor::new());
    let scheduler = Box::new(CisDagScheduler::new(...));

    // 执行完整的 workflow
    let agent = Agent::builder()
        .memory(memory)
        .transport(transport)
        .skill_executor(skill_executor)
        .scheduler(scheduler)
        .build();

    agent.run().await.unwrap();
}
```

---

### 5.3 文档更新

#### Task 5.6: Trait 使用指南
**文件**: `docs/traits-guide.md`

- [ ] 如何使用 trait 抽象
- [ ] 如何实现自定义后端
- [ ] 代码示例和最佳实践

#### Task 5.7: ZeroClaw 集成文档
**文件**: `docs/zeroclaw-integration.md`

- [ ] 如何将 CIS 作为 ZeroClaw 后端
- [ ] 配置示例
- [ ] 迁移指南

#### Task 5.8: API 文档
- [ ] 为所有 trait 添加 rustdoc 注释
- [ ] 添加示例代码

---

## Phase 6: 性能优化和清理（Week 13+）⚡

### 6.1 性能基准测试

#### Task 6.1: 基准测试
**文件**: `cis-core/benches/trait_overhead.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_memory_trait(c: &mut Criterion) {
    let real = Box::new(CisMemoryBackend::new(config));
    let mock = Box::new(MockMemory::new());

    c.bench_function("real_memory_set", |b| b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            black_box(real.set("key", b"value", ...).await.unwrap())
        })
    });

    c.bench_function("mock_memory_set", |b| b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            black_box(mock.set("key", b"value", ...).await.unwrap())
        })
    });
}
```

- [ ] 对比重构前后性能
- [ ] 测量 trait object 分发开销
- [ ] 优化热点路径

### 6.2 API 弃用

#### Task 6.2: 标记旧 API 为 deprecated
- [ ] 保留 `MemoryService` 等旧 API
- [ ] 添加 `#[deprecated]` 注解
- [ ] 提供迁移指南

---

## 📊 完整进度追踪

### Phase 1: 核心 Trait (P0)
- [ ] Memory Trait (0/3 tasks)
- [ ] Network Trait (0/3 tasks)
- [ ] Skill Trait (0/3 tasks)
- [ ] Scheduler Trait (0/3 tasks)

**Phase 1 完成度**: 0% (0/12)

### Phase 2: 扩展 Trait (P1)
- [ ] Vector Trait (0/1 tasks)
- [ ] Storage Trait (0/1 tasks)
- [ ] Security Trait (0/1 tasks)

**Phase 2 完成度**: 0% (0/3)

### Phase 3: ZeroClaw 兼容
- [ ] Memory Adapter (0/1 tasks)
- [ ] Skill Adapter (0/1 tasks)
- [ ] 配置支持 (0/1 tasks)

**Phase 3 完成度**: 0% (0/3)

### Phase 4: 重构现有代码
- [ ] MemoryService (0/1 tasks)
- [ ] NetworkManager (0/1 tasks)
- [ ] SkillManager (0/1 tasks)
- [ ] Scheduler (0/1 tasks)

**Phase 4 完成度**: 0% (0/4)

### Phase 5: 测试和文档
- [ ] 单元测试 (0/4 tasks)
- [ ] 集成测试 (0/1 tasks)
- [ ] 文档更新 (0/3 tasks)

**Phase 5 完成度**: 0% (0/8)

### Phase 6: 优化和清理
- [ ] 性能测试 (0/1 tasks)
- [ ] API 弃用 (0/1 tasks)

**Phase 6 完成度**: 0% (0/2)

---

## 🎯 验收标准

### Phase 1 验收
- [ ] 所有核心 trait 定义完成
- [ ] CIS backend 实现完成
- [ ] Mock 实现完成
- [ ] 单元测试覆盖率 > 70%

### Phase 2 验收
- [ ] 扩展 trait 定义完成
- [ ] 所有 trait 有对应实现

### Phase 3 验收
- [ ] ZeroClaw 可以使用 CIS 作为后端
- [ ] 配置文件支持后端切换

### Phase 4 验收
- [ ] 所有核心模块使用 trait 重构
- [ ] 向后兼容
- [ ] 所有测试通过

### Phase 5 验收
- [ ] 文档完整
- [ ] 示例可运行

### Phase 6 验收
- [ ] 性能开销 < 5%
- [ ] 旧 API 标记为 deprecated

---

## 🚀 快速开始

### Week 1 目标 (MVP)

**Day 1-2**: Memory Trait
```bash
# 创建 trait 模块
mkdir -p cis-core/src/traits

# 实现核心 trait
# - traits/memory.rs
# - traits/implementations/cis_memory.rs
# - traits/mock/mock_memory.rs
```

**Day 3-4**: Network Trait
```bash
# - traits/network.rs
# - traits/implementations/cis_network.rs
# - traits/mock/mock_network.rs
```

**Day 5**: Skill Trait
```bash
# - traits/skill.rs
# - traits/implementations/cis_skill.rs
# - traits/mock/mock_skill.rs
```

**Day 6-7**: 测试和文档
```bash
# 单元测试
cargo test --package cis-core --lib traits

# 文档
cargo doc --open
```

---

## 📚 参考资源

### 设计模式
- [Trait Bound Pattern](https://doc.rust-lang.org/book/ch10-02-traits.html)
- [Trait Object vs Generics](https://rust-lang.github.io/rust-clippy/master/index.html#/trait_bound)
- [Factory Pattern](https://refactoring.guru/design-patterns/factory-method)

### Rust 异步 Trait
- [async-trait](https://docs.rs/async-trait/)
- [Rust async book](https://rust-lang.github.io/async-book/)

### ZeroClaw 集成
- [ZeroClaw plugin guide](https://github.com/example/zeroclaw/plugins)
- [CIS vs ZeroClaw analysis](../kimi/cis_zeroclaw_integration_report.md)

---

## ⚠️ 风险和缓解

| 风险 | 等级 | 缓解措施 |
|------|------|---------|
| Trait object 性能开销 | 🟡 中 | 使用 `Box<dyn>`，开销 < 5%；热点路径使用泛型 |
| 编译时间增加 | 🟡 中 | 使用 `impl Trait` 减少单态化 |
| API 破坏性变更 | 🔴 高 | 保持旧 API，标记为 deprecated；渐进式迁移 |
| 测试覆盖率下降 | 🟡 中 | 每个 trait 配一个 Mock，保持测试 |
| 学习曲线 | 🟢 低 | 详细文档，示例代码 |
| 维护负担 | 🟡 中 | trait 定义即文档；减少重复代码 |

---

## 💡 最佳实践

### 1. Trait 设计
- **小而专注**: 每个 trait 只关注一个抽象
- **按需扩展**: 使用 extension traits 添加可选功能
- **文档优先**: trait 定义即文档

### 2. 实现
- **先 Mock 后真实**: Mock 实现用于定义接口契约
- **组合优于继承**: 使用多个小 trait 组合大功能
- **错误处理**: 统一使用 `Result<T, Error>`

### 3. 测试
- **Trait 驱动**: 先定义 trait，再实现
- **Mock 隔离**: 使用 Mock 测试单个组件
- **集成验证**: 使用真实实现测试集成

---

**创建日期**: 2026-02-20
**最后更新**: 2026-02-20
**负责人**: Claude AI
**状态**: 📋 待审阅
**预计工期**: 12-13 周
