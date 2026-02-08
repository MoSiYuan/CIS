//! CRDT (Conflict-free Replicated Data Types) 实现
//!
//! 用于解决公域记忆在多节点间的冲突。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// LWW (Last-Write-Wins) 注册表
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LWWRegister<T> {
    pub value: T,
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
}

impl<T: Clone> LWWRegister<T> {
    pub fn new(value: T, node_id: String) -> Self {
        Self {
            value,
            timestamp: Utc::now(),
            node_id,
        }
    }

    /// 合并两个注册表（LWW 策略）
    pub fn merge(&self, other: &Self) -> Self {
        if other.timestamp > self.timestamp {
            other.clone()
        } else if other.timestamp == self.timestamp {
            // 时间戳相同，按节点 ID 字典序决定
            if other.node_id > self.node_id {
                other.clone()
            } else {
                self.clone()
            }
        } else {
            self.clone()
        }
    }
}

/// G-Counter (Grow-only Counter)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GCounter {
    pub counters: HashMap<String, u64>,
}

impl GCounter {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    /// 递增
    pub fn increment(&mut self, node_id: &str) {
        *self.counters.entry(node_id.to_string()).or_insert(0) += 1;
    }

    /// 获取总值
    pub fn value(&self) -> u64 {
        self.counters.values().sum()
    }

    /// 合并两个计数器
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for (node, count) in &other.counters {
            let entry = result.counters.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(*count);
        }
        result
    }
}

/// PN-Counter (Increment/Decrement Counter)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PNCounter {
    pub increments: GCounter,
    pub decrements: GCounter,
}

impl PNCounter {
    pub fn new() -> Self {
        Self {
            increments: GCounter::new(),
            decrements: GCounter::new(),
        }
    }

    pub fn increment(&mut self, node_id: &str) {
        self.increments.increment(node_id);
    }

    pub fn decrement(&mut self, node_id: &str) {
        self.decrements.increment(node_id);
    }

    pub fn value(&self) -> i64 {
        self.increments.value() as i64 - self.decrements.value() as i64
    }

    pub fn merge(&self, other: &Self) -> Self {
        Self {
            increments: self.increments.merge(&other.increments),
            decrements: self.decrements.merge(&other.decrements),
        }
    }
}

/// OR-Set (Observed-Remove Set) - 用于记忆键管理
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ORSet<T: Clone + Eq + std::hash::Hash> {
    pub adds: HashMap<T, GCounter>,
    pub removes: HashMap<T, GCounter>,
}

impl<T: Clone + Eq + std::hash::Hash> ORSet<T> {
    pub fn new() -> Self {
        Self {
            adds: HashMap::new(),
            removes: HashMap::new(),
        }
    }

    /// 添加元素
    pub fn add(&mut self, element: T, node_id: &str) {
        self.adds
            .entry(element)
            .or_default()
            .increment(node_id);
    }

    /// 移除元素
    pub fn remove(&mut self, element: &T, node_id: &str) {
        if let Some(counter) = self.adds.get(element) {
            let existing = self.removes.entry(element.clone()).or_default();
            *existing = existing.merge(counter);
            existing.increment(node_id);
        }
    }

    /// 查询元素是否存在
    pub fn contains(&self, element: &T) -> bool {
        let add_count = self.adds.get(element).map(|c| c.value()).unwrap_or(0);
        let remove_count = self.removes.get(element).map(|c| c.value()).unwrap_or(0);
        add_count > remove_count
    }

    /// 获取所有有效元素
    pub fn elements(&self) -> Vec<T> {
        self.adds
            .keys()
            .filter(|k| self.contains(k))
            .cloned()
            .collect()
    }

    /// 合并两个集合
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.clone();

        // 合并 adds
        for (elem, counter) in &other.adds {
            let existing = result.adds.entry(elem.clone()).or_default();
            *existing = existing.merge(counter);
        }

        // 合并 removes
        for (elem, counter) in &other.removes {
            let existing = result.removes.entry(elem.clone()).or_default();
            *existing = existing.merge(counter);
        }

        result
    }
}

impl<T: Clone + Eq + std::hash::Hash> Default for ORSet<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// 记忆向量时钟 - 用于检测并发更新
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VectorClock {
    pub clocks: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self {
            clocks: HashMap::new(),
        }
    }

    /// 递增节点时钟
    pub fn increment(&mut self, node_id: &str) {
        *self.clocks.entry(node_id.to_string()).or_insert(0) += 1;
    }

    /// 比较两个向量时钟
    /// - Some(Ordering::Less): self 发生在 other 之前
    /// - Some(Ordering::Greater): self 发生在 other 之后
    /// - Some(Ordering::Equal): 相同
    /// - None: 并发（不可比较）
    pub fn compare(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let all_nodes: std::collections::HashSet<_> = self.clocks
            .keys()
            .chain(other.clocks.keys())
            .collect();

        let mut less = false;
        let mut greater = false;

        for node in all_nodes {
            let self_val = self.clocks.get(node).copied().unwrap_or(0);
            let other_val = other.clocks.get(node).copied().unwrap_or(0);

            if self_val < other_val {
                less = true;
            } else if self_val > other_val {
                greater = true;
            }
        }

        match (less, greater) {
            (true, true) => None, // 并发
            (true, false) => Some(std::cmp::Ordering::Less),
            (false, true) => Some(std::cmp::Ordering::Greater),
            (false, false) => Some(std::cmp::Ordering::Equal),
        }
    }

    /// 合并向量时钟
    pub fn merge(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for (node, clock) in &other.clocks {
            let entry = result.clocks.entry(node.clone()).or_insert(0);
            *entry = (*entry).max(*clock);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lww_register_merge() {
        let reg1 = LWWRegister::new("value1", "node1".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        let reg2 = LWWRegister::new("value2", "node2".to_string());

        let merged = reg1.merge(&reg2);
        assert_eq!(merged.value, "value2");
    }

    #[test]
    fn test_g_counter() {
        let mut counter1 = GCounter::new();
        counter1.increment("node1");
        counter1.increment("node1");
        counter1.increment("node2");

        assert_eq!(counter1.value(), 3);

        let mut counter2 = GCounter::new();
        counter2.increment("node1");
        counter2.increment("node2");
        counter2.increment("node2");

        let merged = counter1.merge(&counter2);
        // node1 max(2,1)=2, node2 max(1,2)=2, total=4
        assert_eq!(merged.value(), 4);
        assert_eq!(merged.counters.get("node1").copied().unwrap_or(0), 2);
        assert_eq!(merged.counters.get("node2").copied().unwrap_or(0), 2);
    }

    #[test]
    fn test_vector_clock() {
        let mut vc1 = VectorClock::new();
        vc1.increment("node1");
        vc1.increment("node1");
        vc1.increment("node2");

        let mut vc2 = VectorClock::new();
        vc2.increment("node1");
        vc2.increment("node2");
        vc2.increment("node2");

        // vc1: {node1: 2, node2: 1}
        // vc2: {node1: 1, node2: 2}
        // These are concurrent
        assert!(vc1.compare(&vc2).is_none());

        let mut vc3 = VectorClock::new();
        vc3.increment("node1");
        vc3.increment("node2");

        // vc3: {node1: 1, node2: 1}
        // vc1: {node1: 2, node2: 1}
        assert_eq!(vc3.compare(&vc1), Some(std::cmp::Ordering::Less));
        assert_eq!(vc1.compare(&vc3), Some(std::cmp::Ordering::Greater));
    }

    #[test]
    fn test_or_set() {
        let mut set1 = ORSet::new();
        set1.add("a", "node1");
        set1.add("b", "node1");

        let mut set2 = ORSet::new();
        set2.add("b", "node2");
        set2.add("c", "node2");

        let merged = set1.merge(&set2);
        assert!(merged.contains(&"a"));
        assert!(merged.contains(&"b"));
        assert!(merged.contains(&"c"));

        // Test removal
        let mut set3 = ORSet::new();
        set3.add("a", "node1");
        set3.add("a", "node2");
        set3.remove(&"a", "node1");

        // After remove, should not contain "a"
        assert!(!set3.contains(&"a"));
    }
}
