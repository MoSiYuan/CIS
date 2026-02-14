# AgentGuard 设计文档

> **版本**: 1.0
> **创建日期**: 2026-02-12
> **负责团队**: Team D (并发安全)

---

## 1. 背景和问题

### 1.1 问题描述

根据代码审阅报告 (`docs/user/code-review-execution-layer.md`)：

> 🔴 **严重**: 内存泄漏风险
> - 位置: `scheduler/multi_agent_executor.rs:610-633`
> - 问题描述: Agent 清理逻辑复杂，可能导致泄漏
> - 建议: 改进 Agent 清理逻辑

当前的 Agent 管理存在以下问题：

1. **手动清理**：开发者需要手动调用 `shutdown()`，容易遗漏
2. **异常路径未处理**：panic 或错误时 Agent 可能未正确关闭
3. **资源泄漏**：Agent 持有的资源（文件句柄、网络连接等）可能泄漏
4. **清理顺序复杂**：多个资源需要按特定顺序清理
5. **缺乏监控**：无法追踪哪些 Agent 未正确清理

### 1.2 设计目标

1. ✅ **自动化清理**：使用 RAII 模式自动清理资源
2. ✅ **异常安全**：即使 panic 也能保证清理
3. ✅ **可组合**：支持多个清理回调的组合
4. ✅ **可监控**：追踪 Agent 生命周期
5. ✅ **向后兼容**：最小化对现有代码的改动

---

## 2. 解决方案：AgentGuard

### 2.1 核心概念

**AgentGuard** 是一个 RAII (Resource Acquisition Is Initialization) 守卫，确保 Agent 及其资源在离开作用域时被正确清理。

```rust
pub struct AgentGuard<T> {
    agent: Option<T>,
    cleanup_handlers: Vec<Box<dyn FnOnce(T) + Send>>,
    on_panic: bool,
}
```

### 2.2 设计模式

#### 模式 1: 基本守卫

```rust
use cis_core::agent::guard::AgentGuard;

struct MyService {
    // 不再直接持有 Agent
    // agent: PersistentAgent,
}

impl MyService {
    async fn process(&self) -> Result<()> {
        // 创建守卫，自动管理生命周期
        let agent = PersistentAgent::new(...)?;
        let mut guard = AgentGuard::new(agent);

        // 使用 agent
        guard.agent().execute(task).await?;

        // 离开作用域时自动清理
        Ok(())
    }
}
```

#### 模式 2: 链式清理

```rust
let guard = AgentGuard::new(agent)
    .on_drop(|agent| {
        // 清理回调 1
        tokio::spawn(async move {
            let _ = agent.close_connections().await;
        });
    })
    .on_drop(|agent| {
        // 清理回调 2
        agent.save_state();
    })
    .on_drop(|agent| {
        // 清理回调 3
        tracing::info!("Agent {} cleaned up", agent.id());
    });
```

#### 模式 3: 异步清理

```rust
let guard = AgentGuard::new(agent)
    .on_drop_async(|agent| async move {
        // 异步清理逻辑
        tokio::time::timeout(
            Duration::from_secs(30),
            agent.shutdown()
        ).await
    });
```

---

## 3. API 设计

### 3.1 AgentGuard 结构

```rust
/// Agent 守卫，确保资源自动清理
pub struct AgentGuard<T> {
    /// Agent 实例
    agent: Option<T>,
    /// 清理处理器列表
    cleanup_handlers: Vec<Box<dyn FnOnce(T) + Send>>,
    /// 是否在 panic 时清理
    cleanup_on_panic: bool,
    /// 守卫创建时间
    created_at: Instant,
    /// 守卫标识
    id: GuardId,
}

/// 守卫唯一标识
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GuardId(pub String);
```

### 3.2 核心方法

```rust
impl<T> AgentGuard<T> {
    /// 创建新的守卫
    pub fn new(agent: T) -> Self;

    /// 创建带有自定义 ID 的守卫
    pub fn with_id(agent: T, id: GuardId) -> Self;

    /// 添加同步清理回调
    pub fn on_drop<F>(self, f: F) -> Self
    where
        F: FnOnce(T) + Send + 'static;

    /// 添加异步清理回调
    pub fn on_drop_async<F, Fut>(self, f: F) -> Self
    where
        F: FnOnce(T) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send;

    /// 设置是否在 panic 时清理（默认：true）
    pub fn cleanup_on_panic(mut self, cleanup: bool) -> Self;

    /// 获取 Agent 引用
    pub fn agent(&self) -> &T;

    /// 获取 Agent 可变引用
    pub fn agent_mut(&mut self) -> &mut T;

    /// 手动触发清理（提前释放）
    pub async fn cleanup(mut self) -> Result<(), AgentCleanupError>;

    /// 检查是否已清理
    pub fn is_cleaned(&self) -> bool;

    /// 获取守卫 ID
    pub fn id(&self) -> &GuardId;

    /// 获取守卫存活时间
    pub fn lifetime(&self) -> Duration;
}
```

### 3.3 Drop 实现

```rust
impl<T> Drop for AgentGuard<T> {
    fn drop(&mut self) {
        // 检查是否需要清理
        if let Some(agent) = self.agent.take() {
            let lifetime = self.created_at.elapsed();

            // 记录清理
            tracing::debug!(
                "AgentGuard {:?} cleaning up after {:?}",
                self.id,
                lifetime
            );

            // 执行所有清理回调
            for handler in self.cleanup_handlers.drain(..).rev() {
                // 反向执行（后进先出）
                handler(agent.clone());
            }

            // 记录清理完成
            tracing::debug!("AgentGuard {:?} cleaned up successfully", self.id);
        }
    }
}
```

---

## 4. Agent Pool 集成

### 4.1 Pool 返回守卫

```rust
impl AgentPool {
    /// 获取 Agent（返回守卫）
    pub async fn acquire(&self)
        -> Result<AgentGuard<PersistentAgent>, PoolError>
    {
        let agent = self.acquire_agent().await?;

        // 创建守卫，自动归还到 Pool
        let guard = AgentGuard::new(agent)
            .on_drop_async(|agent| async move {
                // 归还 Agent 到 Pool
                if let Err(e) = self.return_agent(agent).await {
                    tracing::error!("Failed to return agent to pool: {}", e);
                }
            });

        Ok(guard)
    }
}
```

### 4.2 使用示例

```rust
async fn process_task(pool: &AgentPool, task: Task) -> Result<()> {
    // 获取 Agent（自动管理生命周期）
    let mut agent_guard = pool.acquire().await?;

    // 执行任务
    let result = agent_guard.agent_mut()
        .execute(task)
        .await?;

    // agent_guard 离开作用域时自动归还到 Pool
    Ok(result)
}
```

---

## 5. 泄漏检测

### 5.1 LeakDetector

```rust
pub struct LeakDetector {
    /// 活跃的守卫
    active_guards: Arc<RwLock<HashMap<GuardId, GuardInfo>>>,
    /// 泄漏阈值（秒）
    leak_threshold: Duration,
}

#[derive(Debug, Clone)]
struct GuardInfo {
    id: GuardId,
    created_at: Instant,
    location: &'static std::panic::Location<'static>,
}
```

### 5.2 注册守卫

```rust
impl LeakDetector {
    /// 注册守卫
    pub fn register_guard(
        &self,
        id: GuardId,
        location: &'static std::panic::Location<'static>
    ) {
        self.active_guards.write().unwrap().insert(
            id.clone(),
            GuardInfo {
                id,
                created_at: Instant::now(),
                location,
            },
        );
    }

    /// 注销守卫
    pub fn unregister_guard(&self, id: &GuardId) {
        self.active_guards.write().unwrap().remove(id);
    }

    /// 检测泄漏
    pub fn detect_leaks(&self) -> Vec<LeakedGuard> {
        let guards = self.active_guards.read().unwrap();
        let now = Instant::now();

        guards.values()
            .filter(|info| {
                now.duration_since(info.created_at) > self.leak_threshold
            })
            .map(|info| LeakedGuard {
                id: info.id.clone(),
                lifetime: now.duration_since(info.created_at),
                location: info.location,
            })
            .collect()
    }
}
```

### 5.3 定期检查

```rust
// 在应用启动时
let detector = LeakDetector::new(Duration::from_secs(300)); // 5 分钟阈值

tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;

        let leaked = detector.detect_leaks();
        if !leaked.is_empty() {
            tracing::warn!("Detected {} leaked guards:", leaked.len());
            for leak in leaked {
                tracing::warn!(
                    "Guard {:?} at {:?} alive for {:?}",
                    leak.id,
                    leak.location,
                    leak.lifetime
                );
            }
        }
    }
});
```

---

## 6. 监控和统计

### 6.1 GuardStats

```rust
#[derive(Debug, Clone)]
pub struct GuardStats {
    /// 创建的守卫总数
    pub total_created: u64,
    /// 正常清理的守卫数
    pub cleaned_normally: u64,
    /// 因 panic 清理的守卫数
    pub cleaned_on_panic: u64,
    /// 当前活跃的守卫数
    pub active_guards: u64,
    /// 平均存活时间
    pub avg_lifetime: Duration,
    /// 最大存活时间
    pub max_lifetime: Duration,
}
```

### 6.2 统计收集

```rust
impl AgentGuard<T> {
    fn record_lifetime(&self) {
        let lifetime = self.created_at.elapsed();

        // 更新全局统计
        GLOBAL_GUARD_STATS.record_lifetime(lifetime);
    }
}
```

---

## 7. 测试策略

### 7.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_basic_cleanup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        {
            let _guard = AgentGuard::new(())
                .on_drop(move |_| {
                    cleaned_clone.store(true, Ordering::SeqCst);
                });
        }

        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_guard_async_cleanup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        {
            let _guard = AgentGuard::new(())
                .on_drop_async(|_| async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    cleaned_clone.store(true, Ordering::SeqCst);
                });
        }

        // 等待异步清理完成
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(cleaned.load(Ordering::SeqCst));
    }

    #[test]
    fn test_guard_panic_cleanup() {
        let cleaned = Arc::new(AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        let result = std::panic::catch_unwind(|| {
            let _guard = AgentGuard::new(())
                .on_drop(move |_| {
                    cleaned_clone.store(true, Ordering::SeqCst);
                });

            panic!("Intentional panic");
        });

        assert!(result.is_err());
        assert!(cleaned.load(Ordering::SeqCst));
    }
}
```

### 7.2 集成测试

```rust
#[tokio::test]
async fn test_pool_with_guard() {
    let pool = AgentPool::new(2);

    // 获取 Agent
    let guard1 = pool.acquire().await.unwrap();
    assert_eq!(pool.available_count(), 1);

    // 离开作用域，自动归还
    drop(guard1);

    // 等待归还完成
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(pool.available_count(), 2);
}

#[tokio::test]
async fn test_leak_detection() {
    let detector = LeakDetector::new(Duration::from_secs(1));
    let id = GuardId::new("test-guard");

    detector.register_guard(id.clone());

    // 立即检查，不应泄漏
    assert!(detector.detect_leaks().is_empty());

    // 等待超过阈值
    tokio::time::sleep(Duration::from_secs(2)).await;

    // 应该检测到泄漏
    let leaked = detector.detect_leaks();
    assert_eq!(leaked.len(), 1);
    assert_eq!(leaked[0].id, id);
}
```

---

## 8. 迁移计划

### 8.1 第一阶段：实现基础（1.5 天）

1. 实现 `AgentGuard` 基础结构
2. 实现 `Drop` trait
3. 实现基本的单元测试

### 8.2 第二阶段：Pool 集成（1 天）

1. 更新 `AgentPool::acquire()` 返回守卫
2. 更新所有调用点
3. 添加集成测试

### 8.3 第三阶段：泄漏检测（1 天）

1. 实现 `LeakDetector`
2. 添加全局监控
3. 添加告警机制

### 8.4 第四阶段：测试验证（1 天）

1. 编写全面的测试套件
2. 进行压力测试
3. 验证资源清理

---

## 9. 性能考虑

### 9.1 性能影响

1. **守卫创建**：约 100ns（可忽略）
2. **清理回调**：取决于回调数量和复杂度
3. **内存开销**：每个守卫约 200 字节

### 9.2 优化措施

1. **零成本抽象**：守卫本身开销极小
2. **内联优化**：简单回调会被内联
3. **批量清理**：多个守卫可以并行清理

---

## 10. 向后兼容性

### 10.1 渐进式迁移

```rust
// 旧代码仍然可以工作
let agent = pool.acquire_agent().await?;
// ... 使用 agent
pool.return_agent(agent).await?;

// 新代码使用守卫
let guard = pool.acquire().await?;
// ... 使用 guard.agent()
// 自动归还
```

### 10.2 特性标志

```toml
[features]
default = ["agent-guard"]
agent-guard = []
```

```rust
#[cfg(feature = "agent-guard")]
pub type AcquiredAgent = AgentGuard<PersistentAgent>;

#[cfg(not(feature = "agent-guard"))]
pub type AcquiredAgent = PersistentAgent;
```

---

## 11. 参考资料

- [RAII in Rust](https://doc.rust-lang.org/stable/book/ch15-3-drop.html)
- [Effective Rust: Resource Management](https://www.lurklurk.org/effective-rust/resources/raii.html)
- [Zero-cost Abstractions](https://doc.rust-lang.org/stable/book/ch10-00-generics.html)

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: Team D
