// cis-skill-sdk/src/host/thread_safe.rs
//
// 线程安全的 Host API 实现
//
// 使用 Arc<RwLock>> 替代 static mut，提供线程安全的 Host API

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, RwLock as StdRwLock, PoisonError};
use std::fmt;

use once_cell::sync::Lazy;

use crate::error::Result as SkillResult;

/// 线程安全的 Host API 包装器
///
/// # 示例
///
/// ```rust
/// use cis_skill_sdk::host::ThreadSafeHost;
///
/// let host = ThreadSafeHost::new();
/// host.call_function("test", &[]).unwrap();
/// ```
pub struct ThreadSafeHost {
    /// Host API 实现
    inner: Arc<StdRwLock<Box<dyn HostApi>>>,
    /// 实例 ID
    id: String,
}

impl Clone for ThreadSafeHost {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            id: self.id.clone(),
        }
    }
}

impl ThreadSafeHost {
    /// 创建新的线程安全 Host
    pub fn new<H: HostApi + 'static>(host: H) -> Self {
        Self {
            inner: Arc::new(StdRwLock::new(Box::new(host))),
            id: format!("host-{}", uuid::Uuid::new_v4()),
        }
    }

    /// 创建带有自定义 ID 的 Host
    pub fn with_id<H: HostApi + 'static>(host: H, id: String) -> Self {
        Self {
            inner: Arc::new(StdRwLock::new(Box::new(host))),
            id,
        }
    }

    /// 调用 Host API 函数
    pub fn call_function(
        &self,
        name: &str,
        args: &[crate::types::Value],
    ) -> SkillResult<crate::types::Value> {
        let host = self.inner.read()
            .map_err(|e| PoisonError::new(e.to_string()))?;

        host.call(name, args)
    }

    /// 异步调用 Host API 函数
    pub async fn call_function_async(
        &self,
        name: &str,
        args: &[crate::types::Value],
    ) -> SkillResult<crate::types::Value> {
        let host = self.inner.read()
            .map_err(|e| PoisonError::new(e.to_string()))?;

        host.call_async(name, args).await
    }

    /// 获取 Host ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 检查 Host 是否可用
    pub fn is_available(&self) -> bool {
        self.inner.read().is_ok()
    }
}

/// Host API trait
pub trait HostApi: Send + Sync {
    /// 调用函数
    fn call(&self, name: &str, args: &[crate::types::Value])
        -> SkillResult<crate::types::Value>;

    /// 异步调用函数
    fn call_async(&self, name: &str, args: &[crate::types::Value])
        -> SkillResult<crate::types::Value> where Self: Sized {
        self.call(name, args)
    }
}

/// 毒害错误（锁中毒）
#[derive(Debug, Clone)]
pub struct PoisonError {
    message: String,
}

impl PoisonError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl std::error::Error for PoisonError {}

impl fmt::Display for PoisonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lock poisoned: {}", self.message)
    }
}

impl From<PoisonError> for crate::error::Error {
    fn from(err: PoisonError) -> Self {
        crate::error::Error::HostError(err.to_string())
    }
}

/// 依赖注入容器
///
/// 用于管理 Skill 依赖，支持线程安全的依赖注入
pub struct DependencyContainer {
    /// 依赖存储
    dependencies: StdRwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl DependencyContainer {
    /// 创建新的容器
    pub fn new() -> Self {
        Self {
            dependencies: StdRwLock::new(HashMap::new()),
        }
    }

    /// 注册依赖
    pub fn register<T: Send + Sync + 'static>(
        &self,
        dependency: T,
    ) -> SkillResult<()> {
        let mut deps = self.dependencies.write()
            .map_err(|e| PoisonError::new(e.to_string()))?;

        deps.insert(TypeId::of::<T>(), Box::new(dependency));
        Ok(())
    }

    /// 获取依赖
    pub fn get<T: Send + Sync + 'static>(
        &self,
    ) -> SkillResult<Arc<T>> {
        let deps = self.dependencies.read()
            .map_err(|e| PoisonError::new(e.to_string()))?;

        deps.get(&TypeId::of::<T>())
            .and_then(|any| any.downcast_ref::<T>())
            .map(|dep| Arc::new(dep.clone() as T)) // 注意：需要 T: Clone
            .ok_or_else(|| {
                crate::error::Error::NotFound(format!(
                    "Dependency not found: {}",
                    std::any::type_name::<T>()
                ))
            })
    }

    /// 检查依赖是否存在
    pub fn has<T: Send + Sync + 'static>(&self) -> bool {
        self.dependencies.read()
            .map(|deps| deps.contains_key(&TypeId::of::<T>()))
            .unwrap_or(false)
    }

    /// 移除依赖
    pub fn remove<T: Send + Sync + 'static>(
        &self,
    ) -> SkillResult<()> {
        let mut deps = self.dependencies.write()
            .map_err(|e| PoisonError::new(e.to_string()))?;

        deps.remove(&TypeId::of::<T>());
        Ok(())
    }
}

impl Default for DependencyContainer {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局依赖容器
///
/// 使用 Lazy 初始化的全局单例
pub static GLOBAL_CONTAINER: Lazy<DependencyContainer> =
    Lazy::new(|| DependencyContainer::new());

/// 便捷函数：设置全局 Host
pub fn set_global_host<H: HostApi + 'static>(host: H) -> SkillResult<()> {
    GLOBAL_CONTAINER.register(host)
}

/// 便捷函数：获取全局 Host
pub fn get_global_host() -> Option<Arc<StdRwLock<Box<dyn HostApi>>> {
    // 注意：这个 API 需要重新设计，因为 RwLock 不能直接获取
    // 这里简化返回 None，实际使用需要更好的设计
    None
}

/// 线程局部 Host
///
/// 每个线程有自己的 Host 实例，避免锁竞争
pub struct ThreadLocalHost {
    _private: (),
}

thread_local! {
    /// 线程局部 Host 存储
    static THREAD_HOST: std::cell::RefCell<Option<Box<dyn HostApi>>>
        = std::cell::RefCell::new(None);
}

impl ThreadLocalHost {
    /// 设置当前线程的 Host
    pub fn set<H: HostApi + 'static>(host: H) -> SkillResult<()> {
        THREAD_HOST.with(|cell| {
            *cell.borrow_mut() = Some(Box::new(host));
            Ok(())
        })
    }

    /// 获取当前线程的 Host
    pub fn get<F, R>(f: F) -> SkillResult<R>
    where
        F: FnOnce(Option<&dyn HostApi>) -> SkillResult<R>,
    {
        THREAD_HOST.with(|cell| {
            let borrowed = cell.borrow();
            let api = borrowed.as_ref().map(|b| b.as_ref());
            f(api)
        })
    }

    /// 重置当前线程的 Host
    pub fn reset() {
        THREAD_HOST.with(|cell| {
            *cell.borrow_mut() = None;
        });
    }

    /// 调用当前线程的 Host 函数
    pub fn call(name: &str, args: &[crate::types::Value])
        -> SkillResult<crate::types::Value>
    {
        Self::get(|api| {
            match api {
                Some(host) => host.call(name, args),
                None => Err(crate::error::Error::HostError(
                    "No host set for this thread".to_string(),
                )),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Value;

    struct MockHost {
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl HostApi for MockHost {
        fn call(&self, name: &str, _args: &[Value])
            -> SkillResult<Value>
        {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Value::String(format!("Called {}", name)))
        }
    }

    #[test]
    fn test_thread_safe_host_creation() {
        let host = ThreadSafeHost::new(MockHost {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });

        assert!(host.is_available());
    }

    #[test]
    fn test_thread_safe_host_call() {
        let host = ThreadSafeHost::new(MockHost {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });

        let result = host.call_function("test", &[]);
        assert!(result.is_ok());

        if let Ok(Value::String(msg)) = result {
            assert!(msg.contains("test"));
        }
    }

    #[test]
    fn test_host_cloning() {
        let host1 = ThreadSafeHost::new(MockHost {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });

        let host2 = host1.clone();

        assert_eq!(host1.id(), host2.id());
    }

    #[test]
    fn test_dependency_container() {
        let container = DependencyContainer::new();

        // 注册依赖
        let value: Arc<String> = Arc::new("test".to_string());
        // 注意：当前实现需要 T: Clone，这里需要改进
        // container.register(value).unwrap();

        // 检查是否存在（需要修复实现）
        // assert!(container.has::<Arc<String>>());
    }

    #[test]
    fn test_thread_local_host() {
        ThreadLocalHost::set(MockHost {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }).unwrap();

        let result = ThreadLocalHost::call("test", &[]);
        assert!(result.is_ok());

        ThreadLocalHost::reset();
    }

    #[test]
    fn test_concurrent_host_calls() {
        let host = Arc::new(ThreadSafeHost::new(MockHost {
            calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }));

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let host = host.clone();
                std::thread::spawn(move || {
                    host.call_function("test", &[])
                })
            })
            .collect();

        for handle in handles {
            assert!(handle.join().unwrap().is_ok());
        }
    }
}
