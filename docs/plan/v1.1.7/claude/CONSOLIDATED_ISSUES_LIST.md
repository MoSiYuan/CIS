# CIS 项目问题清单（综合版）

**项目名称**: CIS (Cluster of Independent Systems)
**数据来源**: GLM Agent + Kimi Agent 综合分析
**汇总日期**: 2026-02-17
**问题总数**: **40 个**（P0: 6, P1: 14, P2: 20）

---

## 执行摘要

本清单整合了 GLM Agent 和 Kimi Agent 发现的所有问题，按严重程度排序，并标注问题来源（双方共识 vs 独特发现）。

### 问题统计

| 严重程度 | GLM Agent | Kimi Agent | 共识 | 独特 | 合计 |
|---------|-----------|------------|------|------|------|
| **P0 (立即处理)** | 0 | 6 | 1 | 5 | **6** |
| **P1 (短期处理)** | 5 | 9 | 4 | 10 | **14** |
| **P2 (长期规划)** | 10 | 10 | 0 | 20 | **20** |
| **合计** | 15 | 25 | 5 | 35 | **40** |

### 来源分布

```
共同问题 (双方共识): 5 个 (12.5%)
GLM 独特问题: 10 个 (25%)
Kimi 独特问题: 25 个 (62.5%)
```

---

## 一、P0 问题（立即处理 - 1 周内）

> **定义**: 影响生产环境、安全漏洞、性能瓶颈

### P0-1: 版本号不一致（双方共识）

**发现者**: GLM Agent + Kimi Agent

**位置**:
- `cis-node/src/main.rs:61` - 显示 "1.1.2"
- `cis-core/Cargo.toml:3` - 版本 "1.1.5"
- `cis-node/Cargo.toml:3` - 版本 "1.1.5"

**问题**: CLI 显示的版本号与 crate 版本不一致，导致用户困惑和发布管理混乱

**修复**:
```rust
// cis-node/src/main.rs
const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    println!("CIS v{}", VERSION);
}
```

```toml
# 根 Cargo.toml
[workspace.dependencies]
version = "1.1.6"
```

---

### P0-2: 密钥文件权限设置不完整（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/identity/did.rs:230-240`

**问题**:
1. Windows 系统未设置权限
2. 未验证权限设置成功
3. 密钥明文存储

**风险**: 高 - 未授权访问可能导致密钥泄露

**修复**:
```rust
#[cfg(unix)]
fn set_key_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let key_path = path.with_extension("key");
    let mut perms = fs::metadata(&key_path)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&key_path, perms)?;

    // 验证权限设置成功
    let verified_perms = fs::metadata(&key_path)?.permissions();
    if verified_perms.mode() & 0o777 != 0o600 {
        return Err(CisError::identity("Failed to set key file permissions"));
    }

    Ok(())
}

#[cfg(windows)]
fn set_key_permissions(path: &Path) -> Result<()> {
    use std::process::Command;
    let key_path = path.with_extension("key");
    Command::new("icacls")
        .args(&[key_path.to_str().unwrap(), "/inheritance:r", "/grant:r",
                &format!("{}:F", whoami::username())])
        .output()?;
    Ok(())
}
```

---

### P0-3: 缺少安全的密钥派生函数（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/identity/did.rs:100-120`

**问题**: 种子长度不足时仅使用单次 SHA256，缺少 KDF 和盐值

**风险**: 高 - 弱密钥派生可能导致身份伪造

**修复**:
```rust
use argon2::{Argon2, PasswordHasher, password_hash::SaltString};
use rand::rngs::OsRng;

let seed_bytes: [u8; 32] = if seed.len() >= 32 {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&seed[..32]);
    bytes
} else {
    // 使用 Argon2id 进行密钥派生
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let mut output = [0u8; 32];
    argon2.hash_password_into(seed, salt.as_str().as_bytes(), &mut output)
        .map_err(|e| CisError::identity(format!("Key derivation failed: {}", e)))?;
    output
};
```

---

### P0-4: RwLock 写者饥饿（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/cache/lru.rs:62`

**问题**: 使用 `std::sync::RwLock` 可能导致写者饥饿

**风险**: 高 - 高并发读场景下写操作长时间等待

**修复**:
```rust
// 使用 parking_lot::RwLock 替代 std::sync::RwLock
use parking_lot::RwLock;

pub struct LruCache {
    inner: Arc<RwLock<CacheInner>>,
}

// 或者使用 sharded cache 减少锁竞争
pub struct ShardedLruCache {
    shards: Vec<Arc<RwLock<CacheInner>>>,
    shard_mask: usize,
}
```

---

### P0-5: DAG 执行器顺序执行（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/scheduler/dag_executor.rs:95-110`

**问题**: DAG 节点顺序执行，未充分利用并行性

**风险**: 高 - 性能瓶颈，影响任务吞吐量

**修复**:
```rust
pub async fn execute_parallel(&self, dag: DagDefinition) -> Result<HashMap<String, ExecutionResult>> {
    let mut handles = HashMap::new();
    let completed = Arc::new(Mutex::new(HashSet::new()));

    // 按依赖层级分组并行执行
    for level in dag.topological_levels() {
        let level_futures: Vec<_> = level.iter()
            .map(|node| self.execute_node(node.clone()))
            .collect();

        let results = futures::future::join_all(level_futures).await;
        // 收集结果...
    }
}
```

---

### P0-6: 批量处理无内存上限（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/vector/batch.rs:80-120`

**问题**: 批量处理器没有设置内存使用上限

**风险**: 高 - 大量数据可能导致 OOM

**修复**:
```rust
pub struct BatchProcessor {
    max_memory_mb: usize,
    current_memory_usage: AtomicUsize,
}

async fn submit(&self, item: BatchItem) -> Result<Uuid> {
    // 检查内存使用
    if self.current_memory_usage.load(Ordering::Relaxed) > self.max_memory_mb * 1024 * 1024 {
        return Err(CisError::ResourceExhausted("Memory limit exceeded".to_string()));
    }
    // ...
}
```

---

### P0-7: 删除备份文件（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/memory/weekly_archived.rs.bak2`

**问题**: 版本控制中包含备份文件

**风险**: 高 - 代码库污染，可能泄露敏感信息

**修复**:
```bash
# 删除所有备份文件
find . -name "*.bak*" -delete
find . -name "*.tmp" -delete

# 添加到 .gitignore
echo "*.bak" >> .gitignore
echo "*.bak2" >> .gitignore
echo "*.tmp" >> .gitignore
```

---

## 二、P1 问题（短期处理 - 1 个月内）

> **定义**: 影响开发效率、中等风险、需要优化

### P1-1: cis-core 过于庞大（双方共识）

**发现者**: GLM Agent + Kimi Agent

**位置**: `cis-core/src/` (30+ 模块)

**问题**: 违反单一职责原则，编译时间过长，测试困难

**修复**:
```
将 cis-core 拆分为:
├── cis-core-types/      # 核心类型定义
├── cis-storage/         # 存储层
├── cis-network/         # 网络层
├── cis-wasm/            # WASM 运行时
├── cis-ai/              # AI 集成
└── cis-core/            # 精简后的核心协调层
```

---

### P1-2: 中英文混合注释（双方共识）

**发现者**: GLM Agent + Kimi Agent

**位置**: `memory/mod.rs`, `skill/mod.rs` 等多个文件

**问题**: 影响国际化，降低可读性

**修复**:
```rust
// 当前（不好）
/// 记忆服务模块
/// 提供私域/公域记忆管理，支持加密和访问控制。

// 建议（好）
/// Memory service module
/// Provides private/public memory management with encryption and access control.
```

---

### P1-3: 依赖版本不一致（双方共识）

**发现者**: GLM Agent + Kimi Agent

**位置**: 多个 `Cargo.toml`

**问题**: 同一依赖在不同 crate 中使用不同版本

**修复**:
```toml
[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
axum = "0.7"
```

---

### P1-4: 循环依赖风险（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `crates/cis-mcp-adapter/Cargo.toml`

**问题**: `cis-mcp-adapter` 同时依赖 `cis-capability` 和 `cis-core`，skills 可能又依赖这些 crates

**修复**:
```
crates → cis-types (公共类型) → cis-core → skills
```

---

### P1-5: 文件过大（Kimi 独特）

**发现者**: Kimi Agent

**位置**:
- `cis-core/src/error/unified.rs` (1140 行)
- `cis-core/src/skill/manager.rs` (1038 行)
- `cis-core/src/wasm/sandbox.rs` (904 行)

**问题**: 违反单一职责，难以维护

**修复**:
```rust
// error/unified.rs 拆分为
error/
├── mod.rs           # 导出（< 100 行）
├── types.rs         # 错误类型定义
├── context.rs       # 错误上下文
└── macros.rs        # 错误宏
```

---

### P1-6: WebSocket 防重放保护（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/network/websocket_auth.rs`

**问题**: DID 挑战-响应认证流程中没有明确的 nonce 唯一性验证

**修复**:
```rust
pub struct NonceCache {
    nonces: DashMap<String, Instant>,
    ttl: Duration,
}

impl NonceCache {
    pub fn verify_and_remove(&self, nonce: &str) -> bool {
        self.nonces.remove(nonce).is_some()
    }

    pub fn insert(&self, nonce: String) {
        self.nonces.insert(nonce, Instant::now());
    }
}
```

---

### P1-7: DAG 执行器并行化（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `scheduler/dag_executor.rs`

**问题**: DAG 节点顺序执行

**修复**: 见 P0-5

---

### P1-8: 向量存储连接池（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `vector/storage.rs`

**问题**: 每次向量搜索都创建新连接

**修复**:
```rust
// 实现 sqlite-vec 的连接池
// 使用 r2d2 或 deadpool 进行连接管理
```

---

### P1-9: 添加离线队列（GLM 独特）

**发现者**: GLM Agent

**位置**: P2P 模块

**问题**: 弱网环境下消息无法持久化，断线后丢失

**修复**:
```rust
pub struct OfflineQueue {
    queue: Vec<QueuedMessage>,
    max_size: usize,
    persist_to_disk: bool,
}

impl OfflineQueue {
    pub fn enqueue(&mut self, msg: Message) -> Result<()> {
        if self.queue.len() >= self.max_size {
            return Err(Error::QueueFull);
        }
        self.queue.push(QueuedMessage::new(msg));
        if self.persist_to_disk {
            self.persist()?;
        }
        Ok(())
    }

    pub async fn retry_send(&mut self, p2p: &P2PNetwork) -> Result<()> {
        for msg in self.queue.drain(..) {
            p2p.send(msg.message).await?;
        }
        Ok(())
    }
}
```

---

### P1-10: 异构任务路由（GLM 独特）

**发现者**: GLM Agent

**位置**: DAG 调度器

**问题**: DAG 节点无法指定特定节点执行（如 Mac 编译 vs Windows 编译）

**修复**:
```toml
[dag.tasks]
id = "1"
name = "Mac Metal 编译"
node_selector = { arch = "aarch64", features = ["metal"] }

[dag.tasks]
id = "2"
name = "Windows CUDA 编译"
node_selector = { arch = "x86_64", features = ["cuda"] }
```

---

### P1-11: Feature flags 优化（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/Cargo.toml`

**问题**: Feature flags 标记为 optional 但未充分使用

**修复**:
```toml
[features]
default = ["storage-sqlite", "network-matrix"]
storage-sqlite = ["rusqlite"]
storage-sqlx = ["sqlx"]
vector = ["sqlite-vec"]
p2p = ["quinn", "rcgen", "mdns-sd"]
```

---

### P1-12: 魔法数字和硬编码（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `wasm/sandbox.rs` 等

**问题**: 硬编码数字缺乏语义

**修复**:
```rust
// 当前（不好）
let mut result = Vec::with_capacity(12 + ciphertext.len());

// 建议（好）
const NONCE_SIZE: usize = 12;
const TAG_SIZE: usize = 16;
let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
```

---

### P1-13: 过多的 `#[allow(dead_code)]`（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `skill/manager.rs`

**问题**: 掩盖真正的问题

**修复**:
```rust
// 删除未使用的代码
// 或者添加 TODO 注释说明原因
#[allow(dead_code)]
// TODO: 保留用于未来特性
fn is_active(&self) -> bool {
    self.event_sender.is_some()
}
```

---

### P1-14: 依赖项 atty unmaintained（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `deny.toml`

**问题**: `atty` crate 被标记为 unmaintained (RUSTSEC-2024-0375)

**修复**:
```rust
// 替换 atty 为 std::io::IsTerminal
use std::io::IsTerminal;

if std::io::stdin().is_terminal() {
    // ...
}
```

---

## 三、P2 问题（长期规划 - 3 个月内）

> **定义**: 技术债务、优化建议、时间灵活

### P2-1: 测试结构统一（Kimi 独特）

**发现者**: Kimi Agent

**位置**: 多个 `tests/` 目录

**问题**: 测试代码分散在多个位置

**修复**:
```
tests/
├── unit/              # 单元测试（与源码同目录）
├── integration/       # 集成测试
├── e2e/               # 端到端测试
├── fixtures/          # 测试数据
└── helpers/           # 测试工具
```

---

### P2-2: 文档结构混乱（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `docs/` 目录

**问题**: 文档文件和目录混合存放，命名风格不一致

**修复**:
```
docs/
├── README.md              # 文档入口
├── architecture/          # 架构文档
├── api/                   # API 文档
├── user-guide/            # 用户指南
├── developer/             # 开发者文档
├── designs/               # 设计文档（ADR）
└── archive/               # 归档文档
```

---

### P2-3: 安全响应流程（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `SECURITY.md`

**问题**: 暂无正式的安全响应流程

**修复**:
```markdown
# Security Policy

## Reporting a Vulnerability

Please report security vulnerabilities to: security@cis.example.com

We will respond within 48 hours and provide a fix within 7 days.
```

---

### P2-4: 性能监控（Kimi 独特）

**发现者**: Kimi Agent

**位置**: 全局

**问题**: 缺少持续性能监控

**修复**:
```rust
use metrics::{counter, histogram, gauge};

// 添加 metrics 和 tracing
counter!("cache_hits", cache.get_hits() as u64);
histogram!("cache_latency", latency.as_secs_f64());
gauge!("active_connections", conn_count as f64);
```

---

### P2-5: 断点续传（GLM 独特）

**发现者**: GLM Agent

**位置**: 文件传输模块

**问题**: 大文件传输无法从中断处继续

**修复**:
```rust
pub struct ResumableTransfer {
    file_id: Uuid,
    offset: u64,
    total_size: u64,
    chunks: Vec<Chunk>,
}
```

---

### P2-6: 带宽自适应（GLM 独特）

**发现者**: GLM Agent

**位置**: P2P 模块

**问题**: 弱网环境下无法自动降低同步频率或数据量

**修复**:
```rust
pub struct BandwidthAdaptive {
    current_bandwidth: AtomicUsize,
    sync_interval: Duration,
    batch_size: usize,
}

impl BandwidthAdaptive {
    pub fn adjust(&self, measured_bandwidth: usize) {
        if measured_bandwidth < 100_000 { // < 100KB/s
            self.sync_interval.set(Duration::from_secs(600));
            self.batch_size.set(10);
        }
    }
}
```

---

### P2-7: 基准测试完善（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/benches/`

**问题**: 基准测试覆盖不足

**修复**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_cache_operations(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let cache = LruCache::new(CacheConfig::default());

    c.bench_function("cache_put", |b| {
        b.to_async(&rt).iter(|| async {
            cache.put(black_box("key".to_string()), black_box(vec![1u8; 100]), None).await
        });
    });
}
```

---

### P2-8: TECHNICAL_DEBT.md 文件命名不专业（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/TECHNICAL_DEBT.md`

**问题**: 文件名不够专业

**修复**:
- 迁移内容到 GitHub Issues
- 或使用 `TECHNICAL_DEBT.md` 等更专业的命名

---

### P2-9: 注释中的 emoji（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `memory/mod.rs` 等

**问题**: 代码注释中使用了 emoji

**修复**:
```rust
// 当前（不好）
/// 记忆服务模块（Phase 0: P1.7.0）

// 建议（好）
/// Memory service module (Phase 0: P1.7.0)
```

---

### P2-10: 导入语句格式不一致（Kimi 独特）

**发现者**: Kimi Agent

**位置**: 多个文件

**问题**: 有的文件使用紧凑格式，有的使用展开格式

**修复**:
```bash
# 统一使用 rustfmt 格式化所有代码
cargo fmt --all
```

---

### P2-11: 日志包含敏感信息（Kimi 独特）

**发现者**: Kimi Agent

**位置**: 多个文件

**问题**: 日志可能意外记录敏感信息

**修复**:
```rust
#[derive(Debug)]
struct SensitiveString(String);

impl std::fmt::Display for SensitiveString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "***REDACTED***")
    }
}
```

---

### P2-12: 字符串克隆过多（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/types.rs` (多处)

**问题**: 大量使用 String 类型导致不必要的内存分配

**修复**:
```rust
// 使用 Arc<str> 共享不可变字符串
pub type SharedString = Arc<str>;

pub struct MemoryEntry {
    pub key: SharedString,  // 替代 String
    pub value: Bytes,       // 使用 bytes::Bytes
}
```

---

### P2-13: 序列化使用 JSON 而非二进制（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/Cargo.toml`

**问题**: 使用 serde_json 进行序列化，效率较低

**修复**:
```rust
// 内部通信使用 bincode
pub fn serialize_internal<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    bincode::serialize(value).map_err(|e| CisError::Serialization(e.to_string()))
}

// 外部 API 使用 JSON
pub fn serialize_external<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|e| CisError::Serialization(e.to_string()))
}
```

---

### P2-14: 没有使用 jemalloc（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `.cargo/config.toml`

**问题**: 没有配置 jemalloc 作为全局分配器

**修复**:
```toml
# .cargo/config.toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-ljemalloc"]
```

```rust
use jemallocator::Jemalloc;

#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```

---

### P2-15: SQLite WAL 未优化（Kimi 独特）

**发现者**: Kimi Agent

**位置**: `cis-core/src/storage/connection.rs`

**问题**: WAL 模式已启用但没有优化参数

**修复**:
```rust
fn optimize_wal(conn: &Connection) -> Result<()> {
    conn.execute_batch("
        PRAGMA journal_mode = WAL;
        PRAGMA wal_autocheckpoint = 1000;
        PRAGMA journal_size_limit = 104857600;
        PRAGMA synchronous = NORMAL;
        PRAGMA cache_size = -32768;
        PRAGMA temp_store = MEMORY;
        PRAGMA mmap_size = 268435456;
    ")?;
    Ok(())
}
```

---

### P2-16: 离线合并优化（GLM 独特）

**发现者**: GLM Agent

**位置**: CRDT 同步模块

**问题**: 弱网环境下 CRDT 冲突解决需要优化

**修复**:
```rust
pub struct ConflictResolver {
    strategy: ResolveStrategy,
}

impl ConflictResolver {
    pub fn resolve_lww(&self, left: &CRDT, right: &CRDT) -> CRDT {
        // Last-Write-Wins 策略
        if right.timestamp() > left.timestamp() {
            right.clone()
        } else {
            left.clone()
        }
    }
}
```

---

### P2-17: 节点能力标签（GLM 独特）

**发现者**: GLM Agent

**位置**: P2P 模块

**问题**: 无法识别节点能力（如 Metal、CUDA）

**修复**:
```rust
pub struct NodeCapability {
    arch: String,
    features: Vec<String>,
    resources: Resources,
}

pub struct CapabilityRegistry {
    capabilities: DashMap<NodeId, NodeCapability>,
}
```

---

### P2-18: 编译结果聚合（GLM 独特）

**发现者**: GLM Agent

**位置**: DAG 执行器

**问题**: 多平台编译结果无法聚合

**修复**:
```rust
pub struct AggregatedResult {
    results: HashMap<NodeId, BuildResult>,
    status: AggregateStatus,
}

impl AggregatedResult {
    pub fn merge(&mut self, result: BuildResult) {
        self.results.insert(result.node_id, result);
        self.update_status();
    }
}
```

---

### P2-19: Webhook 接收（GLM 独特）

**发现者**: GLM Agent

**位置**: Git 集成模块

**问题**: 推送代码无法触发编译测试

**修复**:
```rust
pub struct WebhookReceiver {
    router: Router,
}

impl WebhookReceiver {
    pub async fn handle_push(&self, event: PushEvent) -> Result<()> {
        let dag = self.create_build_dag(event.branch)?;
        self.scheduler.execute(dag).await?;
        Ok(())
    }
}
```

---

### P2-20: 事件触发机制（GLM 独特）

**发现者**: GLM Agent

**位置**: 事件总线

**问题**: Git 推送无法触发 DAG 调度

**修复**:
```rust
pub struct EventTrigger {
    scheduler: Arc<DagScheduler>,
}

impl EventTrigger {
    pub async fn on_git_push(&self, event: GitPushEvent) -> Result<()> {
        let dag = self.create_dag_from_event(event)?;
        self.scheduler.execute(dag).await?;
        Ok(())
    }
}
```

---

## 四、问题分类统计

### 4.1 按来源分类

```
共同问题 (双方共识): 5 个
├─ P0: 版本号不一致
├─ P1: cis-core 过于庞大
├─ P1: 中英文混合注释
├─ P1: 依赖版本不一致
└─ P2: 测试覆盖不完整

GLM 独特问题: 10 个
├─ P1: 离线队列缺失
├─ P1: 异构任务路由缺失
├─ P2: 断点续传缺失
├─ P2: 带宽自适应缺失
└─ ...

Kimi 独特问题: 25 个
├─ P0: 密钥文件权限设置不完整
├─ P0: 缺少安全的密钥派生函数
├─ P0: RwLock 写者饥饿
├─ P0: DAG 执行器顺序执行
└─ ...
```

---

### 4.2 按维度分类

```
架构问题: 8 个
├─ P0: 版本号不一致
├─ P1: cis-core 过于庞大
├─ P1: 依赖版本不一致
├─ P1: 循环依赖风险
├─ P1: Feature flags 优化
├─ P2: 测试结构统一
└─ ...

安全问题: 7 个
├─ P0: 密钥文件权限设置不完整
├─ P0: 缺少安全的密钥派生函数
├─ P1: WebSocket 防重放保护
├─ P1: 依赖项 atty unmaintained
├─ P2: 安全响应流程
├─ P2: 日志包含敏感信息
└─ P2: 命令注入防护待完善

性能问题: 10 个
├─ P0: RwLock 写者饥饿
├─ P0: DAG 执行器顺序执行
├─ P0: 批量处理无内存上限
├─ P1: 向量存储连接池
├─ P2: 字符串克隆过多
├─ P2: 序列化使用 JSON 而非二进制
├─ P2: 没有使用 jemalloc
└─ ...

代码质量问题: 10 个
├─ P0: 删除备份文件
├─ P1: 中英文混合注释
├─ P1: 文件过大
├─ P1: 魔法数字和硬编码
├─ P1: 过多的 #[allow(dead_code)]
├─ P2: TECHNICAL_DEBT.md 文件命名不专业
├─ P2: 注释中的 emoji
├─ P2: 导入语句格式不一致
└─ ...

场景适配问题: 5 个 (GLM 独特)
├─ P1: 离线队列缺失
├─ P1: 异构任务路由缺失
├─ P2: 断点续传缺失
├─ P2: 带宽自适应缺失
└─ P2: 离线合并优化
```

---

## 五、修复优先级建议

### 5.1 第一周（P0）

```
Day 1-2:
  ├─ 删除备份文件 (.bak*)
  └─ 修复版本号不一致

Day 3-4:
  ├─ 修复密钥文件权限设置
  └─ 添加安全的密钥派生函数

Day 5-7:
  ├─ 优化 RwLock (使用 parking_lot)
  ├─ 并行化 DAG 执行器
  ├─ 添加批量处理内存上限
  └─ 验证修复效果
```

---

### 5.2 第一个月（P0 + P1）

```
Week 1: P0 问题（见上）

Week 2-3: P1 架构问题
  ├─ 拆分 cis-core
  ├─ 统一依赖版本
  ├─ 解决循环依赖风险
  └─ 优化 Feature flags

Week 4: P1 代码质量问题
  ├─ 统一注释为英文
  ├─ 拆分过大文件
  ├─ 删除魔法数字
  └─ 清理 #[allow(dead_code)]
```

---

### 5.3 三个月（P0 + P1 + P2）

```
Month 1: P0 + P1（见上）

Month 2: P1 场景适配 + 性能优化
  ├─ 添加离线队列
  ├─ 实现异构任务路由
  ├─ 向量存储连接池
  ├─ WebSocket 防重放保护
  └─ 替换 atty 依赖

Month 3: P2 技术债务清理
  ├─ 统一测试结构
  ├─ 整理文档结构
  ├─ 建立安全响应流程
  ├─ 添加性能监控
  └─ 完善基准测试
```

---

## 六、附录

### A. 问题快速索引

| 问题 ID | 问题描述 | 严重程度 | 来源 | 位置 |
|--------|---------|---------|------|------|
| P0-1 | 版本号不一致 | P0 | 共识 | `main.rs:61` |
| P0-2 | 密钥权限设置不完整 | P0 | Kimi | `identity/did.rs:230` |
| P0-3 | 缺少 KDF | P0 | Kimi | `identity/did.rs:100` |
| P0-4 | RwLock 饥饿 | P0 | Kimi | `cache/lru.rs:62` |
| P0-5 | DAG 顺序执行 | P0 | Kimi | `dag_executor.rs:95` |
| P0-6 | 批量处理无内存上限 | P0 | Kimi | `vector/batch.rs:80` |
| P0-7 | 备份文件污染 | P0 | Kimi | `*.bak*` |
| P1-1 | cis-core 过于庞大 | P1 | 共识 | `cis-core/` |
| P1-2 | 中英文混合注释 | P1 | 共识 | 多个文件 |
| ... | ... | ... | ... | ... |

---

### B. 修复状态跟踪

```markdown
| 问题 ID | 问题描述 | 负责人 | 状态 | 完成日期 |
|--------|---------|-------|------|---------|
| P0-1 | 版本号不一致 | @alice | 🟡 进行中 | - |
| P0-2 | 密钥权限设置 | @bob | 🔴 未开始 | - |
| P0-3 | 缺少 KDF | @bob | 🔴 未开始 | - |
| P0-4 | RwLock 饥饿 | @charlie | 🟢 已完成 | 2026-02-18 |
| ... | ... | ... | ... | ... |

图例:
🟢 已完成 | 🟡 进行中 | 🔴 未开始 | 🔵 已验证
```

---

### C. 相关资源

**修复参考**:
- [Rust 错误处理最佳实践](https://doc.rust-lang.org/book/ch09-00-error-handling.html)
- [Tokio 性能优化指南](https://tokio.rs/blog/2020-04-preemption/)
- [Argon2 KDF 规范](https://tools.ietf.org/html/rfc9106)
- [WASM 安全指南](https://webassembly.org/docs/security/)

**工具推荐**:
- `cargo fmt` - 代码格式化
- `cargo clippy` - 代码检查
- `cargo audit` - 安全审计
- `cargo deny` - 依赖检查
- `criterion` - 性能基准测试

---

*问题清单生成时间: 2026-02-17*
*数据来源: GLM Agent + Kimi Agent*
*综合整理: Claude Sonnet 4.5*
*总问题数: 40 个（P0: 6, P1: 14, P2: 20）*
