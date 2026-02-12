# 锁超时机制迁移指南

> **版本**: 1.0
> **创建日期**: 2026-02-12
> **状态**: 待实施

---

## 概述

本文档提供将现有代码迁移到带超时锁机制的详细指南。迁移分为三个优先级：

1. **P0 - 关键路径**（必须完成）
2. **P1 - 高优先级**（建议完成）
3. **P2 - 一般优化**（可选）

---

## 迁移模式

### 模式 1: Tokio RwLock → AsyncRwLock

**Before**:
```rust
use tokio::sync::RwLock;

struct MyService {
    data: Arc<RwLock<HashMap<String, Value>>>,
}

impl MyService {
    async fn get(&self, key: &str) -> Result<Option<Value>> {
        let data = self.data.read().await;  // 可能永久阻塞
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: String, value: Value) -> Result<()> {
        let mut data = self.data.write().await;  // 可能永久阻塞
        data.insert(key, value);
        Ok(())
    }
}
```

**After**:
```rust
use cis_core::lock_timeout::AsyncRwLock;
use std::time::Duration;

struct MyService {
    data: AsyncRwLock<HashMap<String, Value>>,
}

impl MyService {
    fn new() -> Self {
        Self {
            data: AsyncRwLock::new(
                HashMap::new(),
                Duration::from_secs(5),  // 默认超时 5 秒
            ),
        }
    }

    async fn get(&self, key: &str) -> Result<Option<Value>> {
        let data = self.data.read_with_timeout().await?;  // 5 秒超时
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: String, value: Value) -> Result<()> {
        let mut data = self.data.write_with_timeout().await?;  // 5 秒超时
        data.insert(key, value);
        Ok(())
    }
}
```

### 模式 2: Tokio Mutex → AsyncMutex

**Before**:
```rust
use tokio::sync::Mutex;

struct MyService {
    state: Arc<Mutex<State>>,
}

impl MyService {
    async fn update(&self) -> Result<()> {
        let mut state = self.state.lock().await;  // 可能永久阻塞
        state.counter += 1;
        Ok(())
    }
}
```

**After**:
```rust
use cis_core::lock_timeout::AsyncMutex;
use std::time::Duration;

struct MyService {
    state: AsyncMutex<State>,
}

impl MyService {
    fn new() -> Self {
        Self {
            state: AsyncMutex::new(
                State::default(),
                Duration::from_secs(3),  // 默认超时 3 秒
            ),
        }
    }

    async fn update(&self) -> Result<()> {
        let mut state = self.state.lock_with_timeout().await?;  // 3 秒超时
        state.counter += 1;
        Ok(())
    }
}
```

### 模式 3: 自定义超时

对于需要不同超时时间的操作，使用自定义超时：

```rust
// 快速操作
let data = self.data.read_with_timeout().await?;

// 耗时操作（如批量导入）
let mut data = self.data.write_with_custom_timeout(
    Duration::from_secs(30)
).await?;
```

---

## 具体迁移任务

### P0-1: MemoryService (必须完成)

**文件**: `cis-core/src/memory/service.rs`

**位置**:
- Line 103: `memory_db: Arc<Mutex<MemoryDb>>`
- Line 148: 创建 Mutex
- Line 562: 创建 Mutex

**迁移步骤**:

1. 导入新模块：
```rust
use cis_core::lock_timeout::AsyncMutex;
use std::time::Duration;
```

2. 更新字段类型：
```rust
// Before
memory_db: Arc<Mutex<MemoryDb>>,

// After
memory_db: AsyncMutex<MemoryDb>,
```

3. 更新初始化：
```rust
// Before
Arc::new(tokio::sync::Mutex::new(memory_db)),

// After
AsyncMutex::new(memory_db, Duration::from_secs(5)),
```

4. 更新所有 `.lock().await` 为 `.lock_with_timeout().await?`

**预期工作量**: 2 小时

**超时建议**:
- 数据库读取: 5 秒
- 数据库写入: 10 秒
- 批量操作: 30 秒

---

### P0-2: ArbitrationService (必须完成)

**文件**: `cis-core/src/decision/arbitration.rs`

**位置**:
- Line 7: 导入
- Line 234: `votes: RwLock<HashMap<String, ArbitrationVote>>`
- Line 245: 创建 RwLock
- Line 261, 272, 284, 290, 298, 321, 332, 348, 362, 372, 382, 388, 403: 使用锁

**迁移步骤**:

1. 导入新模块：
```rust
use cis_core::lock_timeout::AsyncRwLock;
use std::time::Duration;
```

2. 更新字段类型：
```rust
// Before
votes: RwLock<HashMap<String, ArbitrationVote>>,

// After
votes: AsyncRwLock<HashMap<String, ArbitrationVote>>,
```

3. 更新初始化：
```rust
// Before
votes: RwLock::new(HashMap::new()),

// After
votes: AsyncRwLock::new(HashMap::new(), Duration::from_secs(3)),
```

4. 更新所有锁使用：
```rust
// Before
let mut votes = self.votes.write().await;

// After
let mut votes = self.votes.write_with_timeout().await?;
```

**预期工作量**: 1.5 小时

**超时建议**:
- 读取投票: 2 秒
- 写入投票: 3 秒

---

### P0-3: SkillManager (必须完成)

**文件**: `cis-core/src/skill/manager.rs`

**位置**:
- Line 6: 导入
- Line 36: `db: Arc<Mutex<SkillDb>>`
- Line 159: `registry: Arc<Mutex<SkillRegistry>>`
- Line 161: `active_skills: Arc<Mutex<HashMap<String, ActiveSkill>>>`
- Line 164: `wasm_runtime: Arc<Mutex<WasmRuntime>>`
- Line 170-173: 初始化
- Line 178: 初始化
- Line 754-756, 761: 使用锁

**迁移步骤**:

1. 导入新模块：
```rust
use cis_core::lock_timeout::AsyncMutex;
use std::time::Duration;
```

2. 更新字段类型：
```rust
// Before
registry: Arc<Mutex<SkillRegistry>>,
active_skills: Arc<Mutex<HashMap<String, ActiveSkill>>>,

// After
registry: AsyncMutex<SkillRegistry>,
active_skills: AsyncMutex<HashMap<String, ActiveSkill>>,
```

3. 更新初始化：
```rust
// Before
let registry = Arc::new(Mutex::new(SkillRegistry::load()?));

// After
let registry = AsyncMutex::new(
    SkillRegistry::load()?,
    Duration::from_secs(5)
);
```

4. 更新所有锁使用：
```rust
// Before
let mut registry = self.registry.lock().await;

// After
let mut registry = self.registry.lock_with_timeout().await?;
```

**预期工作量**: 2 小时

**超时建议**:
- 注册表读取: 3 秒
- 注册表写入: 5 秒
- Skill 状态更新: 2 秒

---

## 错误处理

所有锁操作现在返回 `Result`，需要处理超时错误：

### 基本错误处理

```rust
use cis_core::lock_timeout::LockTimeoutError;
use cis_core::error::CisError;

async fn example(&self) -> Result<()> {
    match self.data.read_with_timeout().await {
        Ok(guard) => {
            // 使用数据
            Ok(())
        }
        Err(LockTimeoutError::Timeout { .. }) => {
            // 锁超时，返回错误
            Err(CisError::LockTimeout("Database locked, try again later".to_string()))
        }
        Err(LockTimeoutError::Cancelled) => {
            // 锁被取消
            Err(CisError::Cancelled)
        }
    }
}
```

### 自动转换（推荐）

在 `CisError` 中添加自动转换：

```rust
// cis-core/src/error.rs
impl From<LockTimeoutError> for CisError {
    fn from(err: LockTimeoutError) -> Self {
        CisError::LockTimeout(err.to_string())
    }
}
```

然后可以简化为：

```rust
async fn example(&self) -> Result<()> {
    let data = self.data.read_with_timeout().await?;  // 自动转换为 CisError
    Ok(())
}
```

---

## 监控集成

### 1. 注册锁到监控器

```rust
use cis_core::lock_timeout::{LockMonitor, LockId};

// 在服务初始化时
fn setup_monitoring(&self) {
    let monitor = LockMonitor::new();

    // 注册关键锁
    monitor.register_lock(
        LockId::new("memory/service.rs", "data"),
        self.data.stats().clone(),
    );

    monitor.register_lock(
        LockId::new("decision/arbitration.rs", "votes"),
        self.votes.stats().clone(),
    );

    // 定期生成报告
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(300)); // 每 5 分钟
        loop {
            interval.tick().await;
            monitor.log_report();
        }
    });
}
```

### 2. 查看统计

```rust
// 获取统计信息
let stats = self.data.stats();
println!("Acquisitions: {}", stats.total_acquisitions);
println!("Timeouts: {}", stats.timeout_count);
println!("Avg wait: {:?}", stats.avg_wait_time());

// 生成完整报告
let report = monitor.generate_report();
println!("High contention locks: {:?}", report.summary.high_contention);
```

---

## 验证清单

迁移完成后，使用以下清单验证：

- [ ] 所有 `tokio::sync::RwLock` 已替换为 `AsyncRwLock`
- [ ] 所有 `tokio::sync::Mutex` 已替换为 `AsyncMutex`
- [ ] 所有 `.lock().await` 已替换为 `.lock_with_timeout().await?`
- [ ] 所有 `.read().await` 已替换为 `.read_with_timeout().await?`
- [ ] 所有 `.write().await` 已替换为 `.write_with_timeout().await?`
- [ ] 所有函数返回类型包含 `Result`
- [ ] 所有错误已正确处理
- [ ] 超时时间根据操作特点合理设置
- [ ] 关键锁已注册到监控器
- [ ] 单元测试已更新
- [ ] 集成测试已通过

---

## 性能测试

迁移后进行性能测试：

```bash
# 运行基准测试
cargo bench --bench lock_performance

# 检查超时统计
cargo run --example lock_stats

# 压力测试
cargo test --release --test concurrency -- --ignored
```

---

## 回滚计划

如果迁移遇到问题，可以快速回滚：

1. 保留原始锁类型定义的注释
2. 使用特性标志控制：
   ```toml
   [features]
   default = ["lock-timeout"]
   lock-timeout = []
   ```

3. 条件编译：
   ```rust
   #[cfg(feature = "lock-timeout")]
   use cis_core::lock_timeout::AsyncRwLock;

   #[cfg(not(feature = "lock-timeout"))]
   use tokio::sync::RwLock;
   ```

---

## 下一步

1. 实施 P0-1, P0-2, P0-3
2. 运行测试套件验证
3. 更新文档
4. 部署到测试环境
5. 监控超时统计
6. 根据实际数据调整超时时间
7. 迁移其他模块（P1, P2）

---

**联系人**: Team D
**最后更新**: 2026-02-12
