//! Kademlia 路由表实现

use super::kbucket::{KBucket, NodeInfo};
use super::node_id::NodeId;
use super::constants::NUM_BUCKETS;

/// Kademlia 路由表
pub struct RoutingTable {
    local_id: NodeId,
    buckets: Vec<KBucket>,
}

impl RoutingTable {
    /// 创建新的路由表
    pub fn new(local_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(NUM_BUCKETS);
        for _ in 0..NUM_BUCKETS {
            buckets.push(KBucket::new());
        }
        Self { local_id, buckets }
    }

    /// 获取 bucket 索引
    fn bucket_index(&self, id: &NodeId) -> usize {
        let distance = self.local_id.distance(id);
        match distance.leading_zeros() {
            Some(idx) => idx.min(NUM_BUCKETS - 1),
            None => NUM_BUCKETS - 1,
        }
    }

    /// 插入节点
    pub fn insert(&mut self, node: NodeInfo) -> bool {
        if node.id == self.local_id {
            return false;
        }
        let idx = self.bucket_index(&node.id);
        self.buckets[idx].insert(node)
    }

    /// 移除节点
    pub fn remove(&mut self, id: &NodeId) -> Option<NodeInfo> {
        let idx = self.bucket_index(id);
        self.buckets[idx].remove(id)
    }

    /// 查找节点
    pub fn find(&self, id: &NodeId) -> Option<&NodeInfo> {
        let idx = self.bucket_index(id);
        self.buckets[idx].find(id)
    }

    /// 获取指定 bucket
    pub fn bucket(&self, index: usize) -> Option<&KBucket> {
        self.buckets.get(index)
    }

    /// 获取指定 bucket（可变）
    pub fn bucket_mut(&mut self, index: usize) -> Option<&mut KBucket> {
        self.buckets.get_mut(index)
    }

    /// 获取所有节点数
    pub fn total_nodes(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// 查找最近的 k 个节点
    pub fn find_closest(&self, target: &NodeId, k: usize) -> Vec<&NodeInfo> {
        let mut all_nodes: Vec<_> = self.buckets.iter()
            .flat_map(|b| b.nodes().iter())
            .collect();
        
        all_nodes.sort_by_key(|n| n.id.distance(target));
        all_nodes.into_iter().take(k).collect()
    }

    /// 获取随机节点用于刷新
    pub fn random_nodes(&self, count: usize) -> Vec<&NodeInfo> {
        use rand::seq::SliceRandom;
        let mut all_nodes: Vec<_> = self.buckets.iter()
            .flat_map(|b| b.nodes().iter())
            .collect();
        
        let mut rng = rand::thread_rng();
        all_nodes.shuffle(&mut rng);
        all_nodes.into_iter().take(count).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_routing_table_insert() {
        let local = NodeId::random();
        let mut table = RoutingTable::new(local);
        
        let node = NodeInfo::new(NodeId::random(), "127.0.0.1:8000");
        assert!(table.insert(node));
        assert_eq!(table.total_nodes(), 1);
    }

    #[test]
    fn test_routing_table_find_closest() {
        let local = NodeId::random();
        let mut table = RoutingTable::new(local);
        
        for i in 0..5 {
            let mut bytes = [0u8; 20];
            bytes[0] = i;
            let node = NodeInfo::new(NodeId::from_bytes(bytes), format!("127.0.0.1:800{}", i));
            table.insert(node);
        }
        
        let target = NodeId::random();
        let closest = table.find_closest(&target, 3);
        assert_eq!(closest.len(), 3);
    }
}
