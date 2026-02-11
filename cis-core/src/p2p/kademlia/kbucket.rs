//! Kademlia K-Bucket 实现

use serde::{Serialize, Deserialize};
use super::node_id::NodeId;
use std::collections::VecDeque;
use std::time::Instant;

/// 节点信息
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: NodeId,
    pub address: String,
    pub last_seen: Instant,
}

impl NodeInfo {
    pub fn new(id: NodeId, address: impl Into<String>) -> Self {
        Self {
            id,
            address: address.into(),
            last_seen: Instant::now(),
        }
    }
    
    /// 获取节点 ID 字符串表示
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }
}

/// K-Bucket
#[derive(Debug, Clone)]
pub struct KBucket {
    nodes: VecDeque<NodeInfo>,
    capacity: usize,
}

impl KBucket {
    pub fn new() -> Self {
        Self::with_capacity(super::constants::K)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.nodes.len() >= self.capacity
    }

    pub fn nodes(&self) -> &VecDeque<NodeInfo> {
        &self.nodes
    }

    pub fn find(&self, id: &NodeId) -> Option<&NodeInfo> {
        self.nodes.iter().find(|n| n.id == *id)
    }

    pub fn insert(&mut self, node: NodeInfo) -> bool {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == node.id) {
            self.nodes.remove(pos);
            self.nodes.push_back(node);
            return true;
        }
        if !self.is_full() {
            self.nodes.push_back(node);
            return true;
        }
        false
    }

    pub fn remove(&mut self, id: &NodeId) -> Option<NodeInfo> {
        self.nodes.iter().position(|n| n.id == *id)
            .and_then(|pos| self.nodes.remove(pos))
    }

    pub fn closest(&self, target: &NodeId, count: usize) -> Vec<&NodeInfo> {
        let mut nodes: Vec<_> = self.nodes.iter().collect();
        nodes.sort_by_key(|n| n.id.distance(target));
        nodes.into_iter().take(count).collect()
    }
}

impl Default for KBucket {
    fn default() -> Self {
        Self::new()
    }
}
