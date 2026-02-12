# 依赖注入容器设计文档

> **版本**: 1.0
> **创建日期**: 2026-02-12
> **负责团队**: Team D (并发安全)

---

## 1. 背景和问题

### 1.1 问题描述

根据代码审阅报告 (`docs/user/code-review-devtools.md`)：

> 🔴 **严重**: 线程安全问题（全局静态变量）
> - 位置: `cis-skill-sdk/src/host.rs:73`
> - 问题描述: 多线程环境下可能崩溃
> - 建议: 使用依赖注入

当前代码使用全局静态变量存储 Host API：

```rust
// cis-skill-sdk/src/host.rs
static mut HOST_API: Option<Box<dyn HostApi>> = None;
```

**问题**：
1. **数据竞争**：多线程同时访问导致数据竞争
2. **内存不安全**：`static mut` 本质上是 unsafe
3. **不可测试**：难以在测试中替换依赖
4. **全局状态**：所有 Skill 共享同一个实例

### 1.2 设计目标

1. ✅ **线程安全**：所有操作都是线程安全的
2. ✅ **依赖注入**：支持依赖注入而非全局状态
3. ✅ **可测试**：易于在测试中模拟依赖
4. ✅ **向后兼容**：保持现有 API 不变（内部实现）
5. ✅ **零成本**：最小化性能开销

---

## 2. 解决方案

### 2.1 依赖注入容器

使用线程安全的依赖注入容器替代全局静态变量：

```rust
pub struct DependencyContainer {
    /// Host API
    host_api: Arc<RwLock<Option<Box<dyn HostApi>>>>,

    /// 其他依赖...
}
```

### 2.2 线程局部存储

对于需要全局访问的场景，使用线程局部存储：

```rust
thread_local! {
    static HOST_API: RefCell<Option<Box<dyn HostApi>>> = RefCell::new(None);
}
```

---

## 3. 架构设计

### 3.1 模块结构

```
cis-skill-sdk/src/
├── dependency_container/
│   ├── mod.rs              # 模块导出
│   ├── container.rs         # 依赖注入容器
│   ├── thread_local.rs      # 线程局部存储
│   └── context.rs          # Skill 上下文
└── host/
    └── thread_safe.rs       # 线程安全的 Host API
```

### 3.2 核心组件

#### DependencyContainer

```rust
pub struct DependencyContainer {
    /// 依赖存储
    dependencies: HashMap<TypeId, Box<dyn Any + Send + Sync>>,

    /// 容器锁
    lock: RwLock<()>,
}
```

**功能**：
- 注册依赖
- 获取依赖
- 构建依赖图
- 循环依赖检测

#### ThreadSafeHost

```rust
pub struct ThreadSafeHost {
    /// Host API 实现
    inner: Arc<RwLock<Box<dyn HostApi>>>,

    /// 实例 ID
    id: String,
}
```

**功能**：
- 线程安全的 API 调用
- 自动锁管理
- 调用统计

---

## 4. API 设计

### 4.1 DependencyContainer

```rust
impl DependencyContainer {
    /// 创建新的容器
    pub fn new() -> Self;

    /// 注册依赖
    pub fn register<T: Send + Sync + 'static>(
        &self,
        dependency: T
    ) -> Result<()>;

    /// 获取依赖
    pub fn get<T: Send + Sync + 'static>(&self)
        -> Result<Arc<T>>;

    /// 构建实例（自动注入依赖）
    pub fn build<T: Injectable>(&self) -> Result<T>;

    /// 创建子容器
    pub fn child(&self) -> Self;
}
```

### 4.2 Injectable Trait

```rust
pub trait Injectable: Sized {
    /// 从容器注入依赖
    fn inject(container: &DependencyContainer) -> Result<Self>;
}
```

---

## 5. 使用示例

### 5.1 基本使用

**Before (不安全)**:
```rust
static mut HOST_API: Option<Box<dyn HostApi>> = None;

fn set_host_api(api: Box<dyn HostApi>) {
    unsafe {
        HOST_API = Some(api);
    }
}

fn call_host() -> Result<()> {
    unsafe {
        HOST_API.as_ref().unwrap().call()?;
    }
    Ok(())
}
```

**After (安全)**:
```rust
use cis_skill_sdk::dependency_container::DependencyContainer;

fn main() {
    let container = DependencyContainer::new();

    // 注册 Host API
    let host_api = Box::new(RealHostApi::new()) as Box<dyn HostApi>;
    container.register(host_api).unwrap();

    // 获取并使用
    let api = container.get::<dyn HostApi>().unwrap();
    api.call()?;
}
```

### 5.2 在 Skill 中使用

```rust
use cis_skill_sdk::Skill;
use cis_skill_sdk::dependency_container::DependencyContainer;

struct MySkill {
    host: Arc<dyn HostApi>,
    memory: Arc<dyn MemoryService>,
}

impl Injectable for MySkill {
    fn inject(container: &DependencyContainer) -> Result<Self> {
        Ok(Self {
            host: container.get()?,
            memory: container.get()?,
        })
    }
}

impl Skill for MySkill {
    fn execute(&self, _req: Request) -> Result<Response> {
        // 使用注入的依赖
        self.host.call()?;
        Ok(Response::default())
    }
}
```

### 5.3 线程局部存储

```rust
use cis_skill_sdk::host::ThreadSafeHost;

thread_local! {
    static HOST_API: RefCell<Option<ThreadSafeHost>> = RefCell::new(None);
}

fn init_host() {
    HOST_API.with(|cell| {
        *cell.borrow_mut() = Some(ThreadSafeHost::new());
    });
}

fn call_host() -> Result<()> {
    HOST_API.with(|cell| {
        let host = cell.borrow().as_ref().unwrap();
        host.call()
    })
}
```

---

## 6. 迁移路径

### 6.1 第一阶段：实现基础 (1 天)

1. 实现 `DependencyContainer`
2. 实现 `ThreadSafeHost`
3. 编写单元测试

### 6.2 第二阶段：更新 Host API (1 天)

1. 更新 `host.rs` 使用线程安全存储
2. 保持向后兼容的 API
3. 更新所有调用点

### 6.3 第三阶段：测试验证 (0.5 天)

1. 并发测试
2. 集成测试
3. 性能测试

---

## 7. 性能考虑

### 7.1 性能对比

| 方案 | 读取延迟 | 写入延迟 | 内存开销 | 线程安全 |
|------|---------|---------|---------|----------|
| `static mut` | ~1ns | ~1ns | 最小 | ❌ 不安全 |
| `Arc<RwLock>` | ~20ns | ~50ns | 64 字节 | ✅ 安全 |
| `thread_local` | ~5ns | ~5ns | 每线程 | ✅ 安全 |

### 7.2 优化措施

1. **读优化**：使用 `Arc` 克隆而非锁
2. **缓存**：缓存频繁访问的依赖
3. **无锁**：考虑使用原子操作或无锁数据结构

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_registration() {
        let container = DependencyContainer::new();
        let api = MockHostApi::new();

        container.register(api).unwrap();

        let retrieved = container.get::<MockHostApi>();
        assert!(retrieved.is_ok());
    }

    #[test]
    fn test_thread_safe_access() {
        let container = Arc::new(DependencyContainer::new());
        container.register(MockHostApi::new()).unwrap();

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let container = container.clone();
                std::thread::spawn(move || {
                    container.get::<MockHostApi>()
                })
            })
            .collect();

        for handle in handles {
            assert!(handle.join().unwrap().is_ok());
        }
    }
}
```

### 8.2 并发测试

```rust
#[tokio::test]
async fn test_concurrent_host_calls() {
    let host = ThreadSafeHost::new();

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let host = host.clone();
            tokio::spawn(async move {
                host.call().await
            })
        })
        .collect();

    for handle in handles {
        assert!(handle.await.unwrap().is_ok());
    }
}
```

---

## 9. 向后兼容性

### 9.1 渐进式迁移

使用特性标志控制新旧实现：

```toml
[features]
default = ["thread-safe-host"]
thread-safe-host = []
```

```rust
#[cfg(feature = "thread-safe-host")]
pub use self::thread_safe::Host;

#[cfg(not(feature = "thread-safe-host"))]
pub use self::legacy::Host;
```

### 9.2 旧 API 保留

```rust
// 旧 API（废弃但可用）
#[deprecated(since = "1.1.6", note = "Use DependencyContainer instead")]
pub fn set_host_api(api: Box<dyn HostApi>) {
    // 内部使用全局容器
    GLOBAL_CONTAINER.register(api);
}
```

---

## 10. 参考资料

- [The Dependency Injection Pattern in Rust](https://mexus.github.io/rust-di/)
- [Concurrency in Rust](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Thread-local Storage](https://doc.rust-lang.org/std/macro.thread_local.html)

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: Team D
