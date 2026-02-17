//! # Vector Clock 实现 (P1.7.0 任务组 0.2)
//!
//! **分布式版本控制**
//!
//! # 核心机制
//!
//! - **Vector Clock**：跟踪分布式系统中的事件因果关系
//! - **LWW (Last-Write-Wins)**：基于时间戳的决胜策略
//! - **冲突检测**：检测并发写入（无因果关系）
//!
//! # 使用场景
//!
//! ```text
//! Node A: [1, 0, 0]  →  写入 key1
//! Node B: [1, 1, 0]  →  写入 key1 (基于 A 的版本)
//! Node C: [0, 0, 1]  →  写入 key1 (并发，冲突!)
//! ```
//!
//! # Vector Clock 比较规则
//!
//! - **Equal**: VC1 == VC2 (所有分量相等)
//! - **Happens-Before**: VC1 < VC2 (所有分量 <= 且至少一个 <)
//! - **Concurrent**: VC1 || VC2 (既不相等也不是 happens-before)
//!   - **冲突**: 需要解决

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Vector Clock (分布式版本控制)
///
/// # 表示
///
/// 使用 `HashMap<NodeId, Counter>` 表示：
/// - `NodeId`: 节点 ID (String)
/// - `Counter`: 版本号 (u64)
///
/// # 示例
///
/// ```
/// let mut vc = VectorClock::new();
/// vc.increment("node-a");
/// vc.increment("node-b");
/// // vc = {"node-a": 1, "node-b": 1}
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VectorClock {
    /// 节点 ID → 版本号
    counters: HashMap<String, u64>,
}

impl VectorClock {
    /// 创建空的 Vector Clock
    ///
    /// # 示例
    ///
    /// ```
    /// let vc = VectorClock::new();
    /// assert_eq!(vc.len(), 0);
    /// ```
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    /// 增加指定节点的计数器
    ///
    /// # 参数
    ///
    /// - `node_id`: 节点 ID
    ///
    /// # 示例
    ///
    /// ```
    /// let mut vc = VectorClock::new();
    /// vc.increment("node-a");
    /// vc.increment("node-a");
    /// assert_eq!(vc.get("node-a"), Some(&2));
    /// ```
    pub fn increment(&mut self, node_id: &str) {
        *self.counters.entry(node_id.to_string()).or_insert(0) += 1;
    }

    /// 获取指定节点的计数器值
    ///
    /// # 参数
    ///
    /// - `node_id`: 节点 ID
    ///
    /// # 返回
    ///
    /// 返回计数器值，如果节点不存在则返回 `None`。
    pub fn get(&self, node_id: &str) -> Option<&u64> {
        self.counters.get(node_id)
    }

    /// 获取所有节点计数器
    ///
    /// # 返回
    ///
    /// 返回计数器的不可变引用。
    pub fn counters(&self) -> &HashMap<String, u64> {
        &self.counters
    }

    /// 获取计数器数量
    ///
    /// # 返回
    ///
    /// 返回节点数量。
    pub fn len(&self) -> usize {
        self.counters.len()
    }

    /// 判断是否为空
    ///
    /// # 返回
    ///
    /// 返回 `true` 如果没有任何节点计数器。
    pub fn is_empty(&self) -> bool {
        self.counters.is_empty()
    }

    /// 合并两个 Vector Clock (取最大值)
    ///
    /// # 用途
    ///
    /// 当合并两个分支时使用，取每个节点的最大计数器值。
    ///
    /// # 参数
    ///
    /// - `other`: 另一个 Vector Clock
    ///
    /// # 示例
    ///
    /// ```
    /// let mut vc1 = VectorClock::new();
    /// vc1.increment("node-a");
    ///
    /// let mut vc2 = VectorClock::new();
    /// vc2.increment("node-b");
    /// vc2.increment("node-b");
    ///
    /// let merged = vc1.merge(&vc2);
    /// // merged = {"node-a": 1, "node-b": 2}
    /// ```
    pub fn merge(&self, other: &VectorClock) -> VectorClock {
        let mut merged = self.clone();

        for (node_id, counter) in other.counters.iter() {
            let entry = merged.counters.entry(node_id.clone()).or_insert(0);
            *entry = (*entry).max(*counter);
        }

        merged
    }

    /// 比较 Vector Clock 关系
    ///
    /// # 返回
    ///
    /// - `VectorClockRelation::Equal`: 两个 Vector Clock 相等
    /// - `VectorClockRelation::HappensBefore`: self < other (self 发生在 other 之前)
    /// - `VectorClockRelation::HappensAfter`: self > other (self 发生在 other 之后)
    /// - `VectorClockRelation::Concurrent`: 并发（冲突）
    ///
    /// # 示例
    ///
    /// ```
    /// let mut vc1 = VectorClock::new();
    /// vc1.increment("node-a");
    ///
    /// let mut vc2 = VectorClock::new();
    /// vc2.increment("node-a");
    /// vc2.increment("node-b");
    ///
    /// assert_eq!(vc1.compare(&vc2), VectorClockRelation::HappensBefore);
    /// ```
    pub fn compare(&self, other: &VectorClock) -> VectorClockRelation {
        // 获取所有节点 ID
        let all_nodes: std::collections::HashSet<&str> = self.counters
            .keys()
            .map(|k| k.as_str())
            .chain(other.counters.keys().map(|k| k.as_str()))
            .collect();

        let mut self_less_or_equal = false;
        let mut self_greater_or_equal = false;
        let mut has_difference = false;

        for node_id in all_nodes {
            let self_counter = self.counters.get(node_id).copied().unwrap_or(0);
            let other_counter = other.counters.get(node_id).copied().unwrap_or(0);

            if self_counter < other_counter {
                self_less_or_equal = true;
                has_difference = true;
            } else if self_counter > other_counter {
                self_greater_or_equal = true;
                has_difference = true;
            }
        }

        if !has_difference {
            VectorClockRelation::Equal
        } else if self_less_or_equal && !self_greater_or_equal {
            VectorClockRelation::HappensBefore
        } else if self_greater_or_equal && !self_less_or_equal {
            VectorClockRelation::HappensAfter
        } else {
            VectorClockRelation::Concurrent
        }
    }

    /// 判断是否有冲突（并发）
    ///
    /// # 返回
    ///
    /// 返回 `true` 如果两个 Vector Clock 并发（无因果关系）。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut vc1 = VectorClock::new();
    /// vc1.increment("node-a");
    ///
    /// let mut vc2 = VectorClock::new();
    /// vc2.increment("node-b");
    ///
    /// assert!(vc1.is_concurrent_with(&vc2));  // 并发 = 冲突
    /// ```
    pub fn is_concurrent_with(&self, other: &VectorClock) -> bool {
        matches!(self.compare(other), VectorClockRelation::Concurrent)
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for VectorClock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut pairs: Vec<_> = self.counters.iter().collect();
        pairs.sort_by(|a, b| a.0.cmp(b.0));

        write!(f, "{{")?;
        for (i, (node_id, counter)) in pairs.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", node_id, counter)?;
        }
        write!(f, "}}")
    }
}

/// Vector Clock 关系
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorClockRelation {
    /// 相等
    Equal,

    /// self 发生在 other 之前 (self < other)
    HappensBefore,

    /// self 发生在 other 之后 (self > other)
    HappensAfter,

    /// 并发（冲突）
    Concurrent,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 Vector Clock 创建和递增
    #[test]
    fn test_vector_clock_increment() {
        let mut vc = VectorClock::new();
        assert_eq!(vc.len(), 0);

        vc.increment("node-a");
        assert_eq!(vc.get("node-a"), Some(&1));
        assert_eq!(vc.len(), 1);

        vc.increment("node-a");
        assert_eq!(vc.get("node-a"), Some(&2));

        vc.increment("node-b");
        assert_eq!(vc.get("node-b"), Some(&1));
        assert_eq!(vc.len(), 2);
    }

    /// 测试 Vector Clock 相等关系
    #[test]
    fn test_vector_clock_equal() {
        let mut vc1 = VectorClock::new();
        vc1.increment("node-a");
        vc1.increment("node-b");

        let mut vc2 = VectorClock::new();
        vc2.increment("node-b");
        vc2.increment("node-a");

        assert_eq!(vc1.compare(&vc2), VectorClockRelation::Equal);
    }

    /// 测试 Vector Clock Happens-Before 关系
    #[test]
    fn test_vector_clock_happens_before() {
        let mut vc1 = VectorClock::new();
        vc1.increment("node-a");

        let mut vc2 = VectorClock::new();
        vc2.increment("node-a");
        vc2.increment("node-b");

        assert_eq!(vc1.compare(&vc2), VectorClockRelation::HappensBefore);
        assert_eq!(vc2.compare(&vc1), VectorClockRelation::HappensAfter);
    }

    /// 测试 Vector Clock 并发关系
    #[test]
    fn test_vector_clock_concurrent() {
        let mut vc1 = VectorClock::new();
        vc1.increment("node-a");

        let mut vc2 = VectorClock::new();
        vc2.increment("node-b");

        assert_eq!(vc1.compare(&vc2), VectorClockRelation::Concurrent);
        assert!(vc1.is_concurrent_with(&vc2));
    }

    /// 测试 Vector Clock 合并
    #[test]
    fn test_vector_clock_merge() {
        let mut vc1 = VectorClock::new();
        vc1.increment("node-a");
        vc1.increment("node-a");

        let mut vc2 = VectorClock::new();
        vc2.increment("node-b");
        vc2.increment("node-b");

        let merged = vc1.merge(&vc2);
        assert_eq!(merged.get("node-a"), Some(&2));
        assert_eq!(merged.get("node-b"), Some(&2));
    }

    /// 测试 Vector Clock Display
    #[test]
    fn test_vector_clock_display() {
        let mut vc = VectorClock::new();
        vc.increment("node-a");
        vc.increment("node-b");

        let display = format!("{}", vc);
        assert!(display.contains("node-a: 1"));
        assert!(display.contains("node-b: 1"));
    }
}
