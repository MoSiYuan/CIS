# 锁超时机制设计文档

> **版本**: 1.0
> **创建日期**: 2026-02-12
> **负责团队**: Team D (并发安全)

---

## 1. 背景和目标

### 1.1 问题陈述

根据代码审阅报告，CIS 系统中存在以下并发安全问题：

1. **死锁风险**：长期持有数据库锁可能导致死锁
   - 位置：`memory/service.rs:344`
   - 位置：`skill/manager.rs:746-751`
   - 位置：`decision/arbitration.rs`

2. **锁竞争**：多个协程竞争同一锁，性能下降

3. **资源泄漏**：锁获取失败后资源未正确释放

### 1.2 设计目标

1. ✅ **超时保护**：所有锁操作都有超时限制
2. ✅ **性能监控**：收集锁竞争和等待时间统计
3. ✅ **向后兼容**：最小化对现有代码的改动
4. ✅ **类型安全**：利用 Rust 类型系统保证正确使用
5. ✅ **可配置性**：支持不同场景的超时配置

---

## 2. 架构设计

### 2.1 模块结构

```
cis-core/src/lock_timeout/
├── mod.rs              # 模块导出
├── async_rwlock.rs     # RwLock 超时包装器
├── async_mutex.rs      # Mutex 超时包装器
├── monitor.rs          # 锁竞争监控
└── stats.rs           # 统计数据收集
```

### 2.2 核心组件

#### 2.2.1 AsyncRwLock<T>

带超时的读写锁包装器，基于 `tokio::sync::RwLock`。

```rust
pub struct AsyncRwLock<T> {
    inner: Arc<tokio::sync::RwLock<T>>,
    default_timeout: Duration,
    stats: Arc<LockStats>,
}
```

**功能**：
- 读锁超时获取：`read_with_timeout()`
- 写锁超时获取：`write_with_timeout()`
- 自定义超时：`read_with_custom_timeout()`
- 统计收集：自动记录等待时间

#### 2.2.2 AsyncMutex<T>

带超时的互斥锁包装器，基于 `tokio::sync::Mutex`。

```rust
pub struct AsyncMutex<T> {
    inner: Arc<tokio::sync::Mutex<T>>,
    default_timeout: Duration,
    stats: Arc<LockStats>,
}
```

**功能**：
- 锁超时获取：`lock_with_timeout()`
- 自定义超时：`lock_with_custom_timeout()`
- 统计收集：自动记录等待时间

#### 2.2.3 LockMonitor

锁竞争监控器，用于收集和分析锁使用情况。

```rust
pub struct LockMonitor {
    stats: HashMap<LockId, LockStats>,
}
```

**功能**：
- 记录锁获取次数
- 记录锁等待时间
- 记录锁超时次数
- 生成性能报告

---

## 3. API 设计

### 3.1 AsyncRwLock API

```rust
impl<T> AsyncRwLock<T> {
    /// 创建新的带超时的读写锁
    pub fn new(value: T, default_timeout: Duration) -> Self;

    /// 创建带有监控标识的锁
    pub fn with_id(value: T, default_timeout: Duration, id: LockId) -> Self;

    /// 使用默认超时获取读锁
    pub async fn read_with_timeout(&self)
        -> Result<AsyncRwLockReadGuard<'_, T>, LockTimeoutError>;

    /// 使用自定义超时获取读锁
    pub async fn read_with_custom_timeout(&self, timeout: Duration)
        -> Result<AsyncRwLockReadGuard<'_, T>, LockTimeoutError>;

    /// 使用默认超时获取写锁
    pub async fn write_with_timeout(&self)
        -> Result<AsyncRwLockWriteGuard<'_, T>, LockTimeoutError>;

    /// 使用自定义超时获取写锁
    pub async fn write_with_custom_timeout(&self, timeout: Duration)
        -> Result<AsyncRwLockWriteGuard<'_, T>, LockTimeoutError>;

    /// 获取锁统计信息
    pub fn stats(&self) -> &LockStats;

    /// 重置统计信息
    pub fn reset_stats(&self);
}
```

### 3.2 AsyncMutex API

```rust
impl<T> AsyncMutex<T> {
    /// 创建新的带超时的互斥锁
    pub fn new(value: T, default_timeout: Duration) -> Self;

    /// 创建带有监控标识的锁
    pub fn with_id(value: T, default_timeout: Duration, id: LockId) -> Self;

    /// 使用默认超时获取锁
    pub async fn lock_with_timeout(&self)
        -> Result<AsyncMutexGuard<'_, T>, LockTimeoutError>;

    /// 使用自定义超时获取锁
    pub async fn lock_with_custom_timeout(&self, timeout: Duration)
        -> Result<AsyncMutexGuard<'_, T>, LockTimeoutError>;

    /// 获取锁统计信息
    pub fn stats(&self) -> &LockStats;

    /// 重置统计信息
    pub fn reset_stats(&self);
}
```

### 3.3 锁守卫 (Lock Guards)

```rust
/// 读锁守卫，实现 Deref
pub struct AsyncRwLockReadGuard<'a, T> {
    inner: tokio::sync::RwLockReadGuard<'a, T>,
    acquired_at: Instant,
}

/// 写锁守卫，实现 DerefMut
pub struct AsyncRwLockWriteGuard<'a, T> {
    inner: tokio::sync::RwLockWriteGuard<'a, T>,
    acquired_at: Instant,
}

/// 互斥锁守卫，实现 DerefMut
pub struct AsyncMutexGuard<'a, T> {
    inner: tokio::sync::MutexGuard<'a, T>,
    acquired_at: Instant,
}
```

所有守卫在 Drop 时自动更新持有时间统计。

---

## 4. 错误处理

### 4.1 错误类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockTimeoutError {
    /// 锁获取超时
    Timeout {
        lock_type: LockType,
        timeout: Duration,
    },
    /// 锁被取消
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockType {
    ReadLock,
    WriteLock,
    Mutex,
}

impl Error for LockTimeoutError {}
impl Display for LockTimeoutError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { lock_type, timeout } => {
                write!(f, "{:?} acquisition timeout after {:?}", lock_type, timeout)
            }
            Self::Cancelled => {
                write!(f, "Lock acquisition cancelled")
            }
        }
    }
}
```

### 4.2 错误处理策略

1. **日志记录**：超时发生时自动记录警告日志
2. **统计更新**：更新超时计数器
3. **优雅降级**：调用者可以选择重试或放弃
4. **上下文保留**：错误信息包含锁类型和超时时间

---

## 5. 监控和统计

### 5.1 LockStats 结构

```rust
#[derive(Debug, Clone)]
pub struct LockStats {
    /// 锁获取总次数
    pub total_acquisitions: u64,

    /// 锁超时次数
    pub timeout_count: u64,

    /// 平均等待时间
    pub avg_wait_time: Duration,

    /// 最大等待时间
    pub max_wait_time: Duration,

    /// 平均持有时间
    pub avg_hold_time: Duration,

    /// 最大持有时间
    pub max_hold_time: Duration,

    /// 当前等待的协程数
    pub current_waiters: u32,
}
```

### 5.2 LockMonitor 接口

```rust
impl LockMonitor {
    /// 创建新的监控器
    pub fn new() -> Self;

    /// 注册一个锁
    pub fn register_lock(&self, id: LockId, stats: Arc<LockStats>);

    /// 获取锁统计
    pub fn get_stats(&self, id: &LockId) -> Option<&LockStats>;

    /// 生成性能报告
    pub fn generate_report(&self) -> LockReport;

    /// 检测异常锁竞争
    pub fn detect_contention(&self, threshold: Duration) -> Vec<LockId>;

    /// 打印报告到日志
    pub fn log_report(&self);
}
```

### 5.3 报告格式

```
=== Lock Performance Report ===
Generated at: 2026-02-12 10:30:00

Lock: memory/service.rs:data
  Total acquisitions: 15234
  Timeouts: 12 (0.08%)
  Avg wait time: 2.3ms
  Max wait time: 145ms
  Avg hold time: 890us
  Max hold time: 5.2s
  Current waiters: 3
  Status: ⚠️  High contention detected

Lock: skill/manager.rs:skills
  Total acquisitions: 8921
  Timeouts: 0 (0.00%)
  Avg wait time: 1.1ms
  Max wait time: 23ms
  Avg hold time: 2.1ms
  Max hold time: 450ms
  Current waiters: 0
  Status: ✅ Healthy

Summary:
  Total locks: 45
  Total acquisitions: 234567
  Total timeouts: 23
  Critical locks: 2
```

---

## 6. 使用示例

### 6.1 替换普通 RwLock

**Before (不安全)**:
```rust
use tokio::sync::RwLock;

struct MemoryService {
    data: RwLock<HashMap<String, MemoryItem>>,
}

impl MemoryService {
    async fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        let data = self.data.read().await; // 可能永久阻塞
        Ok(data.get(key).cloned())
    }
}
```

**After (安全)**:
```rust
use crate::lock_timeout::AsyncRwLock;

struct MemoryService {
    data: AsyncRwLock<HashMap<String, MemoryItem>>,
}

impl MemoryService {
    fn new() -> Self {
        Self {
            data: AsyncRwLock::new(
                HashMap::new(),
                Duration::from_secs(5),
            ),
        }
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        let data = self.data.read_with_timeout().await?;
        Ok(data.get(key).cloned())
    }
}
```

### 6.2 自定义超时时间

```rust
async fn critical_section(&self) -> Result<()> {
    // 对耗时操作使用更长的超时
    let mut data = self.data.write_with_custom_timeout(
        Duration::from_secs(30)
    ).await?;

    // 执行耗时操作
    self.process_data(&mut data).await?;

    Ok(())
}
```

### 6.3 使用监控器

```rust
use crate::lock_timeout::{LockMonitor, LockId};

fn setup_monitoring() {
    let monitor = LockMonitor::new();

    // 注册关键锁
    monitor.register_lock(
        LockId::new("memory/service.rs", "data"),
        memory_service.data.stats().clone(),
    );

    monitor.register_lock(
        LockId::new("skill/manager.rs", "skills"),
        skill_manager.skills.stats().clone(),
    );

    // 定期生成报告
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            monitor.log_report();

            // 检测高竞争锁
            let contested = monitor.detect_contention(Duration::from_millis(100));
            if !contested.is_empty() {
                warn!("High contention detected on locks: {:?}", contested);
            }
        }
    });
}
```

---

## 7. 迁移计划

### 7.1 第一阶段：核心替换 (2 天)

替换以下关键文件的锁：

1. **cis-core/src/memory/service.rs**
   - `MemoryService.data` 锁
   - `MemoryService.sync_markers` 锁
   - 建议超时：5 秒

2. **cis-core/src/decision/arbitration.rs**
   - `ArbitrationService.votes` 锁
   - 建议超时：3 秒

3. **cis-core/src/skill/manager.rs**
   - `SkillManager.skills` 锁
   - 建议超时：5 秒

### 7.2 第二阶段：扩展替换 (1 天)

替换其他模块的锁：

1. **cis-core/src/storage/db.rs**
2. **cis-core/src/vector/storage.rs**
3. **cis-core/src/p2p/*.rs**

### 7.3 第三阶段：监控集成 (1 天)

1. 添加全局 LockMonitor
2. 注册所有关键锁
3. 设置定期报告
4. 配置告警阈值

---

## 8. 性能考虑

### 8.1 性能影响

1. **额外开销**：
   - 时间记录：每次约 50-100ns
   - 统计更新：每次约 200ns (原子操作)
   - 总体影响：< 1% (可接受)

2. **优化措施**：
   - 使用 `Arc` 共享统计结构
   - 原子操作更新计数器
   - 条件编译：release 模式可禁用详细统计

### 8.2 内存开销

每个锁增加的内存：
- `AsyncRwLock`: 约 64 字节
- `AsyncMutex`: 约 48 字节
- `LockStats`: 约 128 字节

对于系统中的 100+ 个锁，总增加约 24KB (可忽略)。

---

## 9. 测试策略

### 9.1 单元测试

1. **基本功能测试**
   - 超时正常工作
   - 自定义超时正常工作
   - 守卫正确释放

2. **并发测试**
   - 多个读者
   - 读者/写者竞争
   - 超时后恢复

3. **统计测试**
   - 计数准确
   - 时间测量准确
   - 并发更新安全

### 9.2 集成测试

1. **端到端测试**
   - 在 MemoryService 中使用
   - 在 SkillManager 中使用
   - 在 ArbitrationService 中使用

2. **性能测试**
   - 基准测试
   - 与原生锁对比

3. **压力测试**
   - 高并发场景
   - 长时间运行

---

## 10. 未来扩展

### 10.1 可能的增强

1. **自适应超时**：根据历史数据自动调整超时时间
2. **死锁检测**：检测循环等待
3. **锁降级**：写锁自动降级为读锁
4. **优先级锁**：支持优先级继承
5. **分布式锁**：支持跨进程锁

### 10.2 监控集成

1. **Prometheus 集成**：导出指标到 Prometheus
2. **可视化仪表板**：Grafana 面板
3. **告警规则**：基于阈值的自动告警

---

## 11. 参考资料

- [Tokio Sync Primitives](https://docs.rs/tokio/latest/tokio/sync/)
- [Rust Concurrency Patterns](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Lock Contention Analysis](https://medium.com/@arjunkrishnabh/lock-contention-analysis-7e7b6b3e3e3e)

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: Team D
