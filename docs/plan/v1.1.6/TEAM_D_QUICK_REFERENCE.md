# Team D 实现快速参考指南

> **版本**: 1.0
> **更新日期**: 2026-02-12

---

## 锁超时机制快速使用

### 1. 创建带超时的锁

```rust
use cis_core::lock_timeout::{AsyncRwLock, AsyncMutex};
use std::time::Duration;

// 读写锁
let rwlock = AsyncRwLock::new(data, Duration::from_secs(5));

// 互斥锁
let mutex = AsyncMutex::new(state, Duration::from_secs(3));

// 带监控标识
let lock = AsyncRwLock::with_id(
    data,
    Duration::from_secs(5),
    cis_core::lock_timeout::LockId::new("my_file.rs", "my_lock")
);
```

### 2. 使用锁

```rust
// 读锁
let data = lock.read_with_timeout().await?;

// 写锁
let mut data = lock.write_with_timeout().await?;

// 自定义超时
let data = lock.read_with_custom_timeout(Duration::from_secs(30)).await?;
```

### 3. 查看统计

```rust
let stats = lock.stats();
println!("Acquisitions: {}", stats.total_acquisitions);
println!("Timeouts: {}", stats.timeout_count);
println!("Avg wait: {:?}", stats.avg_wait_time());
```

### 4. 监控集成

```rust
use cis_core::lock_timeout::LockMonitor;

let monitor = LockMonitor::new();
monitor.register_lock(lock_id, lock.stats());

// 生成报告
let report = monitor.generate_report();
monitor.log_report(); // 自动输出到日志
```

---

## Agent 资源清理快速使用

### 1. 创建 Agent 守卫

```rust
use cis_core::agent::AgentGuard;

let guard = AgentGuard::new(agent)
    .on_drop(|agent| {
        println!("Cleaning up agent {}", agent.id());
    })
    .on_drop_async(|agent| async move {
        agent.shutdown().await.unwrap();
    });
```

### 2. 使用 Agent

```rust
// 获取 Agent 引用
let result = guard.agent().execute_task().await?;

// 可变引用
guard.agent_mut().update_state();
```

### 3. 自动清理

```rust
async fn example() -> Result<()> {
    let guard = AgentGuard::new(agent);
    // 使用 agent...
    Ok(()) // 离开作用域自动清理
}
```

### 4. 泄漏检测

```rust
use cis_core::agent::AgentLeakDetector;

let mut detector = AgentLeakDetector::new();
detector.start().await?;

// 后台自动检测泄漏
let leaks = detector.detect_leaks();
detector.stop().await;
```

---

## 线程安全 Host API 快速使用

### 1. 创建线程安全 Host

```rust
use cis_skill_sdk::host::thread_safe::ThreadSafeHost;

let host = ThreadSafeHost::new(host_impl);
```

### 2. 调用 Host 函数

```rust
// 同步调用
let result = host.call_function("memory_get", &[key])?;

// 异步调用
let result = host.call_function_async("ai_chat", &[prompt]).await?;
```

### 3. 依赖注入容器

```rust
use cis_skill_sdk::dependency_container::DependencyContainer;

let container = DependencyContainer::new();
container.register(host_impl)?;

let dep = container.get::<HostApi>()?;
```

### 4. 线程局部 Host

```rust
use cis_skill_sdk::host::ThreadLocalHost;

ThreadLocalHost::set(host_impl)?;
ThreadLocalHost::call("test", &[])?;

ThreadLocalHost::reset(); // 清理
```

---

## 错误处理

### 锁超时错误

```rust
use cis_core::lock_timeout::LockTimeoutError;

match lock.read_with_timeout().await {
    Ok(guard) => {
        // 正常使用
    }
    Err(LockTimeoutError::Timeout { lock_type, timeout }) => {
        eprintln!("Lock timeout: {:?} after {:?}", lock_type, timeout);
        // 重试或返回错误
    }
    Err(LockTimeoutError::Cancelled) => {
        // 锁被取消
    }
}
```

### Agent 清理错误

```rust
use cis_core::agent::AgentCleanupError;

match guard.cleanup().await {
    Ok(()) => {
        // 清理成功
    }
    Err(AgentCleanupError::PartialFailure) => {
        // 部分清理失败
    }
    Err(AgentCleanupError::HandlerPanic) => {
        // 清理处理器 panic
    }
}
```

---

## 常见使用场景

### 场景 1: MemoryService 使用锁超时

```rust
use cis_core::lock_timeout::AsyncRwLock;
use std::time::Duration;

pub struct MemoryService {
    data: AsyncRwLock<HashMap<String, Vec<u8>>>,
}

impl MemoryService {
    pub fn new() -> Self {
        Self {
            data: AsyncRwLock::new(
                HashMap::new(),
                Duration::from_secs(5), // 5 秒超时
            ),
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let data = self.data.read_with_timeout().await?;
        Ok(data.get(key).cloned())
    }

    pub async fn set(&self, key: String, value: Vec<u8>) -> Result<()> {
        let mut data = self.data.write_with_timeout().await?;
        data.insert(key, value);
        Ok(())
    }
}
```

### 场景 2: SkillManager 使用 AgentGuard

```rust
use cis_core::agent::{AgentGuard, PersistentAgent};

pub async fn execute_with_agent(
    pool: &AgentPool,
    task: Task,
) -> Result<Response> {
    // 获取 Agent（自动管理生命周期）
    let mut agent_guard = pool.acquire_agent()
        .await?
        .on_drop(|agent| {
            println!("Agent {} cleaned up", agent.id());
        });

    // 执行任务
    let response = agent_guard.agent_mut()
        .execute(task)
        .await?;

    // agent_guard 离开作用域自动清理
    Ok(response)
}
```

### 场景 3: Skill 使用线程安全 Host

```rust
use cis_skill_sdk::{Skill, Request};
use cis_skill_sdk::host::ThreadSafeHost;

struct MySkill {
    host: ThreadSafeHost,
}

impl Skill for MySkill {
    fn execute(&self, req: Request) -> Result<Response> {
        // 线程安全调用 Host
        let result = self.host.call_function(
            "memory_get",
            &[req.params.get("key").unwrap()],
        )?;

        Ok(Response::new(result))
    }
}
```

---

## 调试和监控

### 启用详细日志

```rust
// RUST_LOG=trace
RUST_LOG=cis_core::lock_timeout=trace,cis_core::agent=debug
```

### 查看锁统计

```bash
# 定期生成报告
curl http://localhost:3000/api/locks/stats

# 查看高竞争锁
curl http://localhost:3000/api/locks/contention
```

### 检测 Agent 泄漏

```bash
# 查看活跃 Agent
curl http://localhost:3000/api/agents/active

# 查看泄漏报告
curl http://localhost:3000/api/agents/leaks
```

---

## 性能优化建议

### 1. 合理设置超时时间

| 操作类型 | 建议超时 | 说明 |
|---------|----------|------|
| 数据库读取 | 3-5 秒 | 普通查询 |
| 数据库写入 | 5-10 秒 | 考虑网络延迟 |
| 向量搜索 | 5-10 秒 | 耗时操作 |
| 批量导入 | 30-60 秒 | 大量数据 |
| Agent 调用 | 30-300 秒 | 根据任务复杂度 |

### 2. 减少锁持有时间

```rust
// ❌ 错误：长时间持有锁
let mut data = lock.write_with_timeout().await?;
process_data(&data); // 耗时操作
tokio::time::sleep(Duration::from_secs(10)).await; // 阻塞
drop(data);

// ✅ 正确：快速释放锁
{
    let data = lock.read_with_timeout().await?;
    let cloned = data.clone();
}

// 在锁外处理
process_data(&cloned).await;
```

### 3. 使用读锁优先

```rust
// ✅ 优先使用读锁（允许并发）
let data = lock.read_with_timeout().await?;

// 只在需要修改时使用写锁
let mut data = lock.write_with_timeout().await?;
```

---

## 故障排查

### 问题 1: 锁频繁超时

**可能原因**:
- 超时时间设置过短
- 锁竞争过于激烈
- 持锁时间过长

**解决方案**:
1. 增加超时时间
2. 检查锁使用统计
3. 优化持锁逻辑

### 问题 2: Agent 泄漏

**可能原因**:
- 未使用 AgentGuard
- 清理回调 panic
- 循环引用

**解决方案**:
1. 使用 AgentGuard 包装
2. 添加清理日志
3. 运行泄漏检测器

### 问题 3: Host 线程不安全

**可能原因**:
- 使用旧的 static mut
- 多线程访问

**解决方案**:
1. 迁移到 ThreadSafeHost
2. 使用 DependencyContainer
3. 使用 ThreadLocalHost

---

## 参考资料

- **完整设计文档**: `design.md`
- **迁移指南**: `MIGRATION_GUIDE.md`
- **API 文档**: `/Users/jiangxiaolong/work/project/CIS/cis-core/src/lock_timeout/`
- **测试示例**: 查看各文件的 `#[cfg(test)]` 模块

---

**维护者**: Team D
**最后更新**: 2026-02-12
