# CIS 为主、兼容 ZeroClaw - 全面重构任务列表

> **核心原则**: CIS 是主项目，ZeroClaw 作为可选兼容层，减少重复造轮子

## 🎯 设计原则

### 1.1 CIS 为主项目

```
CIS Core (主项目)
├── Memory 系统（核心差异化）
│   ├── sqlite-vec 向量索引（O(log N)）
│   ├── 私域/公域分离
│   ├── 54周归档
│   └── 混合搜索（向量 + FTS5）
├── Network 系统（核心差异化）
│   ├── P2P/QUIC 节点通信
│   ├── DID 身份 + 硬件绑定
│   └── Matrix Room 联邦
├── Security 系统（核心差异化）
│   ├── DID 身份系统
│   ├── ChaCha20-Poly1305 + Argon2id
│   └── ACL 白名单
└── Sync 系统（核心差异化）
    ├── 公域记忆 P2P 同步
    ├── CRDT 冲突解决
    └── Merkle DAG 版本控制
```

### 1.2 ZeroClaw 兼容层（可选）

```
zeroclaw-compat/ (可选 crate)
├── provider/        # Provider 适配器（22+ 提供商）
├── channel/         # Channel 适配器（13+ 通道）
├── skill/           # Skill 适配器（3000+ Skill）
└── tool/            # Tool 适配器（20+ 工具）
```

### 1.3 能力边界

| 模块 | CIS 负责 | ZeroClaw 负责（复用） |
|------|---------|---------------------|
| **Memory** | ✅ 向量索引、域分离、归档 | ❌ 不复刻 |
| **Network** | ✅ P2P/QUIC、DID、Matrix | ❌ 不复刻，兼容其 Channel |
| **Security** | ✅ DID、加密、ACL | ❌ 不复刻 |
| **Sync** | ✅ P2P 同步、CRDT | ❌ 不复刻 |
| **Agent** | ❌ 复刻 | ✅ 直接使用 |
| **Provider** | ❌ 复刻 | ✅ 直接使用（22+） |
| **Skill** | ❌ 复刻 | ✅ 直接使用（3000+） |
| **Tool** | ❌ 复刻 | ✅ 直接使用（20+） |

---

## Phase 0: 准备工作（Week 0）🔧

### Task 0.1: 获取 ZeroClaw 源码

- [ ] Clone zeroclaw 仓库
  ```bash
  cd /Users/jiangxiaolong/work/project
  git clone https://github.com/zeroclaw-labs/zeroclaw.git
  ```
- [ ] 分析 zeroclaw 项目结构
  ```bash
  cd zeroclaw
  find . -name "*.rs" | head -20
  ls -la src/
  cat Cargo.toml
  ```
- [ ] 理解 zeroclaw trait 定义
  - 查看 `src/traits/` 或 `src/*/traits.rs`
  - 分析 Memory, Channel, Provider, Skill trait
- [ ] 提取可复用模式
  - 配置系统如何设计？
  - Factory 模式如何使用？
  - 错误处理如何统一？

**输出**: `docs/plan/v1.2.0/task/zeroclaw_analysis.md`

---

## Phase 1: CIS 核心 Trait 抽象（Week 1-2）🔥 **P0**

### 1.1 Memory Trait 层

#### Task 1.1.1: 创建 traits 目录结构

```bash
mkdir -p cis-core/src/traits/
mkdir -p cis-core/src/memory/backends/
mkdir -p cis-core/src/memory/ops/
```

#### Task 1.1.2: 定义 Memory Trait

**文件**: `cis-core/src/traits/memory.rs`

```rust
use async_trait::async_trait;

/// 记忆后端 trait（核心抽象）
#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;

    // CRUD
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> Result<bool>;

    // 搜索
    async fn search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>>;
    async fn hybrid_search(&self, query: &str, limit: usize) -> Result<Vec<HybridSearchResult>>;

    // 批量
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
- [ ] 定义扩展 trait `MemoryVectorIndex`, `MemorySync`
- [ ] 定义类型：`MemoryEntry`, `SearchResult`, `HybridSearchResult`
- [ ] 添加文档和示例

#### Task 1.1.3: 实现 CIS Memory Backend

**文件**: `cis-core/src/memory/backends/cis.rs`

- [ ] 创建 `CisMemoryBackend` 结构体（包装现有 `MemoryService`）
- [ ] 实现 `Memory` trait
- [ ] 实现 `MemoryVectorIndex` trait（包装 `VectorStorage`）
- [ ] 实现 `MemorySync` trait（包装同步逻辑）

```rust
pub struct CisMemoryBackend {
    service: Arc<MemoryService>,
    vector: Arc<VectorStorage>,
    sync: Arc<SyncEngine>,
}
```

#### Task 1.1.4: 实现 Mock Memory

**文件**: `cis-core/src/memory/backends/mock.rs`

- [ ] 创建 `MockMemoryBackend`（基于 `HashMap` + `Arc<RwLock<>>`）
- [ ] 实现所有 memory traits
- [ ] 添加测试辅助方法

---

### 1.2 Network Trait 层

#### Task 1.2.1: 定义 Network Trait

**文件**: `cis-core/src/traits/network.rs`

```rust
/// 传输层抽象
#[async_trait]
pub trait Transport: Send + Sync {
    fn name(&self) -> &str;
    fn local_addr(&self) -> String;

    async fn send(&self, target: &NodeId, data: &[u8]) -> Result<()>;
    async fn receive(&self) -> Result<(NodeId, Vec<u8>)>;
    async fn broadcast(&self, data: &[u8]) -> Result<usize>;

    async fn start(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}

/// 节点发现抽象
#[async_trait]
pub trait Discovery: Send + Sync {
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;
    async fn announce(&self, info: &PeerInfo) -> Result<()>;
}

/// P2P 网络抽象（组合）
#[async_trait]
pub trait P2PNetwork: Send + Sync {
    type Transport: Transport;
    type Discovery: Discovery;

    async fn connect(&self, addr: &str) -> Result<()>;
    async fn disconnect(&self, peer: &NodeId) -> Result<()>;
    async fn peers(&self) -> Result<Vec<PeerInfo>>;
}
```

- [ ] 定义 `Transport`, `Discovery`, `P2PNetwork` traits
- [ ] 定义类型：`NodeId`, `PeerInfo`

#### Task 1.2.2: 实现 CIS Network Backend

**文件**: `cis-core/src/network/backends/cis.rs`

- [ ] 创建 `QuicTransport`（包装现有 QUIC 实现）
- [ ] 创建 `WsTransport`（包装 WebSocket）
- [ ] 创建 `MdnsDiscovery`（包装 mDNS）
- [ ] 创建 `CisP2PNetwork`（组合实现）

#### Task 1.2.3: 实现 Mock Network

**文件**: `cis-core/src/network/backends/mock.rs`

- [ ] 创建 `MockTransport`（基于 `mpsc::channel`）
- [ ] 创建 `MockDiscovery`
- [ ] 添加网络延迟/丢包模拟

---

### 1.3 Security Trait 层

#### Task 1.3.1: 定义 Security Trait

**文件**: `cis-core/src/traits/security.rs`

```rust
/// 加密抽象
#[async_trait]
pub trait Encryption: Send + Sync {
    fn algorithm(&self) -> &str;
    async fn encrypt(&self, plaintext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;
    async fn decrypt(&self, ciphertext: &[u8], key: &EncryptionKey) -> Result<Vec<u8>>;
    fn generate_key(&self) -> Result<EncryptionKey>;
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
    async fn sign(&self, data: &[u8]) -> Result<Signature>;
    async fn verify_proof(&self, proof: &Proof, did: &Did) -> Result<bool>;
}
```

- [ ] 定义 `Encryption`, `Signature`, `Identity` traits
- [ ] 定义类型：`Did`, `Signature`, `Proof`

#### Task 1.3.2: 实现 CIS Security Backend

- [ ] 创建 `ChaCha20Encryption`
- [ ] 创建 `Ed25519Signature`
- [ ] 创建 `CisIdentity`（包装现有 DID 实现）

---

### 1.4 Sync Trait 层

#### Task 1.4.1: 定义 Sync Trait

**文件**: `cis-core/src/traits/sync.rs`

```rust
/// 同步引擎抽象
#[async_trait]
pub trait SyncEngine: Send + Sync {
    async fn sync(&self) -> Result<SyncResult>;
    async fn get_pending(&self, limit: usize) -> Result<Vec<SyncItem>>;
    async fn mark_synced(&self, item: &SyncItem) -> Result<()>;
}
```

- [ ] 定义 `SyncEngine` trait
- [ ] 实现 `CrdtSyncEngine`
- [ ] 实现 `MockSyncEngine`

---

## Phase 2: 重构现有代码使用 Trait（Week 3-4）🔧

### 2.1 重构 MemoryService

#### Task 2.1.1: 重构 MemoryService 使用 trait

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
    vector: Box<dyn MemoryVectorIndex>,
    sync: Box<dyn MemorySync>,
}

impl MemoryService {
    pub fn new(memory: Box<dyn Memory>, vector: Box<dyn MemoryVectorIndex>) -> Result<Self> {
        Ok(Self { memory, vector, sync: ... })
    }

    // 便捷构造函数（向后兼容）
    pub fn open_default(node_id: &str) -> Result<Self> {
        let memory = Box::new(CisMemoryBackend::new(node_id)?);
        let vector = Box::new(CisVectorIndex::new(node_id)?);
        Self::new(memory, vector)
    }
}
```

- [ ] 重构 `MemoryService` 使用 trait
- [ ] 保持向后兼容（保留旧构造函数）
- [ ] 更新所有调用方
- [ ] 更新测试

---

### 2.2 重构 NetworkManager

#### Task 2.2.1: 重构 NetworkManager 使用 trait

- [ ] 重构 `NetworkManager` 使用 `Transport`, `Discovery`, `P2PNetwork` traits
- [ ] 支持运行时切换传输层
- [ ] 更新所有调用方

---

### 2.3 统一配置系统

#### Task 2.3.1: 创建统一配置

**文件**: `cis-core/src/config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CisConfig {
    pub node_id: String,
    pub data_dir: PathBuf,
    pub memory: MemoryConfig,
    pub network: NetworkConfig,
    pub security: SecurityConfig,
    pub sync: SyncConfig,
    pub zeroclaw: Option<ZeroclawCompatConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroclawCompatConfig {
    pub enabled: bool,
    pub providers: Vec<String>,
    pub channels: Vec<String>,
}
```

- [ ] 定义 `CisConfig`
- [ ] 支持 TOML 序列化/反序列化
- [ ] 添加配置验证

**配置示例**: `~/.cis/config.toml`

```toml
node_id = "my-workstation"
data_dir = "~/.cis"

[memory]
backend = "sqlite"
vector_dimensions = 384

[network]
transport = "quic"

[zeroclaw]
enabled = true
providers = ["openai", "anthropic"]
channels = ["telegram", "discord"]
```

---

## Phase 3: ZeroClaw 兼容层（Week 5-6）🌟 **可选**

### 3.1 创建 zeroclaw-compat crate

#### Task 3.1.1: 项目结构

```bash
mkdir -p cis-zeroclaw-compat/
cd cis-zeroclaw-compat/
cargo init --lib
```

**项目结构**:
```
cis-zeroclaw-compat/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── provider/        # Provider 适配器
│   │   ├── mod.rs
│   │   ├── adapter.rs   # 将 CIS Memory 作为 Provider 后端
│   │   └── factory.rs
│   ├── channel/         # Channel 适配器
│   │   ├── mod.rs
│   │   ├── adapter.rs   # CIS P2P 作为 Channel
│   │   └── factory.rs
│   ├── skill/           # Skill 适配器
│   │   ├── mod.rs
│   │   └── adapter.rs
│   └── tool/            # Tool 适配器
│       ├── mod.rs
│       └── adapter.rs
└── examples/
    └── basic.rs
```

#### Task 3.1.2: Provider 适配器

**文件**: `cis-zeroclaw-compat/src/provider/adapter.rs`

```rust
use async_trait::async_trait;
use zeroclaw::providers::Provider;
use cis_core::memory::MemoryService;

/// CIS Memory 作为 ZeroClaw Provider 的记忆后端
pub struct CisMemoryProvider {
    memory: Arc<MemoryService>,
    provider: Box<dyn Provider>,
}

#[async_trait]
impl Provider for CisMemoryProvider {
    async fn chat(&self, messages: Vec<Message>) -> Result<String> {
        // 1. 从 CIS Memory 检索上下文
        let context = self.memory.search("recent context", 10, 0.6).await?;

        // 2. 调用底层的 AI Provider
        self.provider.chat(messages).await
    }
}
```

- [ ] 实现 `CisMemoryProvider`（CIS Memory + ZeroClaw Provider）
- [ ] 实现 Provider Factory

#### Task 3.1.3: Channel 适配器

**文件**: `cis-zeroclaw-compat/src/channel/adapter.rs`

```rust
use zeroclaw::channels::Channel;
use cis_core::network::P2PNetwork;

/// CIS P2P 网络作为 ZeroClaw Channel
pub struct CisP2PChannel {
    network: Box<dyn P2PNetwork>,
}

#[async_trait]
impl Channel for CisP2PChannel {
    fn name(&self) -> &str {
        "cis-p2p"
    }

    async fn send(&self, message: SendMessage) -> anyhow::Result<()> {
        let payload = serde_json::to_vec(&message)?;
        self.network.broadcast(&payload).await
            .map_err(|e| anyhow::anyhow!("Send failed: {}", e))
    }

    async fn listen(&self, tx: mpsc::Sender<ChannelMessage>) -> anyhow::Result<()> {
        // 监听 P2P 消息并转换为 ChannelMessage
        todo!()
    }
}
```

- [ ] 实现 `CisP2PChannel`
- [ ] 实现 Channel Factory

---

## Phase 4: 测试和文档（Week 7-8）📝

### 4.1 Trait 单元测试

#### Task 4.1.1: Memory Trait 测试

**文件**: `cis-core/src/traits/tests/memory_tests.rs`

```rust
#[tokio::test]
async fn test_memory_trait_mock() {
    let mock = Box::new(MockMemory::new());

    mock.set("key", b"value", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    let entry = mock.get("key").await.unwrap().unwrap();

    assert_eq!(entry.key, "key");
}

#[tokio::test]
async fn test_memory_trait_polymorphism() {
    async fn test_with_memory(memory: Box<dyn Memory>) -> Result<()> {
        memory.set("test", b"data", MemoryDomain::Public, MemoryCategory::Context).await?;
        Ok(())
    }

    // 使用真实实现
    let real = Box::new(CisMemoryBackend::new("test")?);
    test_with_memory(real).await?;

    // 使用 mock 实现
    let mock = Box::new(MockMemory::new());
    test_with_memory(mock).await?;
}
```

- [ ] 测试所有 trait 基本功能
- [ ] 测试多态性
- [ ] 测试错误处理

---

### 4.2 集成测试

#### Task 4.2.1: 端到端集成测试

**文件**: `cis-core/tests/integration_traits.rs`

```rust
#[tokio::test]
async fn test_full_stack_with_traits() {
    let memory = Box::new(CisMemoryBackend::new(config)?);
    let transport = Box::new(QuicTransport::new(config)?);
    let security = Box::new(CisIdentity::new(config)?);

    // 执行完整的 workflow
    let agent = Agent::builder()
        .memory(memory)
        .transport(transport)
        .security(security)
        .build();

    agent.run().await?;
}
```

---

### 4.3 文档更新

#### Task 4.3.1: Trait 使用指南

**文件**: `docs/traits-guide.md`

- [ ] 如何使用 trait 抽象
- [ ] 如何实现自定义后端
- [ ] 代码示例和最佳实践

#### Task 4.3.2: ZeroClaw 集成文档

**文件**: `docs/zeroclaw-integration.md`

- [ ] 如何启用 ZeroClaw 兼容模式
- [ ] 配置示例
- [ ] Provider/Channel/Skill 适配器使用

---

## Phase 5: 性能优化（Week 9-10）⚡

### 5.1 性能基准测试

#### Task 5.1.1: 基准测试

**文件**: `cis-core/benches/trait_overhead.rs`

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_memory_trait(c: &mut Criterion) {
    let real = Box::new(CisMemoryBackend::new(config).unwrap());
    let mock = Box::new(MockMemory::new());

    c.bench_function("real_memory_set", |b| b.iter(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            black_box(real.set("key", b"value", ...).await.unwrap())
        })
    }));
}
```

- [ ] 对比重构前后性能
- [ ] 测量 trait object 分发开销
- [ ] 优化热点路径

---

## 📊 进度追踪

### Phase 0: 准备
- [ ] 0.1 Clone zeroclaw (0/4)

### Phase 1: Trait 抽象
- [ ] 1.1 Memory Trait (0/4)
- [ ] 1.2 Network Trait (0/4)
- [ ] 1.3 Security Trait (0/4)
- [ ] 1.4 Sync Trait (0/3)

**Phase 1 完成度**: 0% (0/15 tasks)

### Phase 2: 重构现有代码
- [ ] 2.1 重构 MemoryService (0/1)
- [ ] 2.2 重构 NetworkManager (0/1)
- [ ] 2.3 统一配置系统 (0/1)

**Phase 2 完成度**: 0% (0/3 tasks)

### Phase 3: ZeroClaw 兼容
- [ ] 3.1 Provider 适配器 (0/1)
- [ ] 3.2 Channel 适配器 (0/1)

**Phase 3 完成度**: 0% (0/2 tasks)

### Phase 4: 测试和文档
- [ ] 4.1 单元测试 (0/1)
- [ ] 4.2 集成测试 (0/1)
- [ ] 4.3 文档更新 (0/2)

**Phase 4 完成度**: 0% (0/4 tasks)

### Phase 5: 性能优化
- [ ] 5.1 基准测试 (0/1)

**Phase 5 完成度**: 0% (0/1 tasks)

---

## 🎯 验收标准

### Phase 1 验收
- [ ] 所有核心 trait 定义完成
- [ ] CIS backend 实现完成
- [ ] Mock 实现完成
- [ ] 单元测试覆盖率 > 70%

### Phase 2 验收
- [ ] 所有核心模块使用 trait 重构
- [ ] 向后兼容
- [ ] 所有测试通过

### Phase 3 验收
- [ ] ZeroClaw Provider 可以使用 CIS Memory
- [ ] ZeroClaw Channel 可以使用 CIS P2P
- [ ] 配置文件支持启用/禁用

### Phase 4 验收
- [ ] 文档完整
- [ ] 示例可运行

### Phase 5 验收
- [ ] 性能开销 < 5%
- [ ] 基准测试通过

---

## 🚀 快速开始

### Week 1 目标 (MVP)

**Day 1**: 准备工作
```bash
# Clone zeroclaw
cd /Users/jiangxiaolong/work/project
git clone https://github.com/zeroclaw-labs/zeroclaw.git

# 分析项目结构
cd zeroclaw
ls -la src/
cat Cargo.toml
```

**Day 2-3**: Memory Trait
```bash
# 创建 traits 目录
mkdir -p cis-core/src/traits/
mkdir -p cis-core/src/memory/backends/

# 实现 Memory trait
# - traits/memory.rs
# - memory/backends/cis.rs
# - memory/backends/mock.rs
```

**Day 4-5**: Network Trait
```bash
# - traits/network.rs
# - network/backends/cis.rs
# - network/backends/mock.rs
```

---

## 📚 参考资源

### ZeroClaw 源码
- **仓库**: https://github.com/zeroclaw-labs/zeroclaw
- **本地路径**: `/Users/jiangxiaolong/work/project/zeroclaw/`
- **关键文件**:
  - `src/traits/` - Trait 定义
  - `src/providers/` - Provider 实现
  - `src/channels/` - Channel 实现
  - `src/skills/` - Skill 实现

### 设计文档
- [cis_zeroclaw_implementation_guide.md](../kimi/cis_zeroclaw_implementation_guide.md)
- [cis_primary_zeroclaw_compatible_plan.md](../kimi/cis_primary_zeroclaw_compatible_plan.md)

---

**创建日期**: 2026-02-20
**最后更新**: 2026-02-20
**负责人**: Claude AI
**状态**: 📋 待审阅
**预计工期**: 9-10 周
