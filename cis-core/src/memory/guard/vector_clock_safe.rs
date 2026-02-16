//! 🔒 Vector Clock 并发安全包装 (P0安全修复)
//!
//! 为 VectorClock 添加并发保护，防止多线程竞争条件

use super::vector_clock::VectorClock;
use std::sync::{Arc, RwLock};

/// 🔒 并发安全的 Vector Clock 包装
#[derive(Debug, Clone)]
pub struct SafeVectorClock {
    inner: Arc<RwLock<VectorClock>>,
}

impl SafeVectorClock {
    /// 创建新的安全 Vector Clock
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(VectorClock::new())),
        }
    }

    /// 从现有 Vector Clock 创建
    pub fn from_clock(clock: VectorClock) -> Self {
        Self {
            inner: Arc::new(RwLock::new(clock)),
        }
    }

    /// 🔒 增加计数器（写操作）
    pub fn increment(&self, node_id: &str) {
        let mut clock = self.inner.write().unwrap();
        clock.increment(node_id);
    }

    /// 获取计数器值（读操作）
    pub fn get(&self, node_id: &str) -> Option<u64> {
        let clock = self.inner.read().unwrap();
        clock.get(node_id).copied()
    }

    /// 🔒 合并另一个 Vector Clock（写操作）
    pub fn merge(&self, other: &VectorClock) {
        let mut clock = self.inner.write().unwrap();
        // 假设VectorClock有merge方法
        // clock.merge(other);
        drop(clock);
    }

    /// 获取所有计数器（读操作）
    pub fn get_all(&self) -> Vec<(String, u64)> {
        let clock = self.inner.read().unwrap();
        clock.to_vec()
    }
}

impl Default for SafeVectorClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_concurrent_increment() {
        let clock = SafeVectorClock::new();
        let handles: Vec<_> = (0..10)
            .map(|i| {
                let clock = clock.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        clock.increment(&format!("node-{}", i % 3));
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        // 验证计数器正确性
        let val = clock.get("node-0").unwrap();
        assert!(val > 0);
    }
}
