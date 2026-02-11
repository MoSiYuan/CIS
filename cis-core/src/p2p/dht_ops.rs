//! DHT 操作实现 (已弃用)
//!
//! ⚠️ **DEPRECATED**: 此模块已被 `crate::p2p::kademlia` 模块替代
//! 
//! 新的 Kademlia 实现提供了：
//! - 完整的路由表 (KBucket, RoutingTable)
//! - 真实的 XOR 距离计算
//! - 节点发现 (FindNode)
//! - 值存储和查找 (Store, FindValue)
//! - 持久化支持
//!
//! 请使用 `crate::p2p::kademlia::KademliaDHT` 替代此模块。

#![deprecated(
    since = "1.1.5",
    note = "请使用 crate::p2p::kademlia 模块替代。此模块将在 v1.2.0 中移除。"
)]

use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use tracing::{debug, error, info, warn};

use crate::p2p::network::P2PNetwork;

/// DHT 操作类型
#[derive(Debug, Clone)]
pub enum DhtOperation {
    /// 存储键值对
    Put { key: String, value: String },
    /// 获取值
    Get { key: String },
    /// 查找节点
    FindNode { node_id: String },
}

/// DHT 操作结果
#[derive(Debug, Clone)]
pub enum DhtResult {
    /// 存储成功
    PutSuccess,
    /// 获取成功
    GetSuccess { value: String },
    /// 查找成功
    FindNodeSuccess { nodes: Vec<NodeInfo> },
    /// 未找到
    NotFound,
    /// 错误
    Error { message: String },
}

/// 节点信息
#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub node_id: String,
    pub address: String,
    pub distance: u32,
}

/// DHT 操作器
pub struct DhtOperations;

impl DhtOperations {
    /// 执行 DHT PUT 操作
    pub async fn put(network: &P2PNetwork, key: &str, value: &str) -> Result<DhtResult> {
        info!("DHT PUT: {} = {}", key, value);
        
        // 计算 key 的哈希
        let key_hash = Self::hash_key(key);
        
        // 获取已连接节点
        let peers = network.connected_peers().await;
        
        if peers.is_empty() {
            return Ok(DhtResult::Error {
                message: "No peers connected".to_string(),
            });
        }
        
        // 向最近的节点存储（简化实现，实际应使用 Kademlia 路由表）
        let mut stored = 0;
        for peer in peers.iter().take(3) {
            let data = format!("DHT:PUT:{key_hash}:{value}");
            if network.send_to(&peer.node_id, data.as_bytes()).await.is_ok() {
                stored += 1;
                debug!("Stored to {}", peer.node_id);
            }
        }
        
        if stored > 0 {
            info!("DHT PUT success: {} stored to {} nodes", key, stored);
            Ok(DhtResult::PutSuccess)
        } else {
            Ok(DhtResult::Error {
                message: "Failed to store to any node".to_string(),
            })
        }
    }
    
    /// 执行 DHT GET 操作
    pub async fn get(network: &P2PNetwork, key: &str) -> Result<DhtResult> {
        info!("DHT GET: {}", key);
        
        let key_hash = Self::hash_key(key);
        
        // 从已连接节点查询
        let peers = network.connected_peers().await;
        
        if peers.is_empty() {
            return Ok(DhtResult::NotFound);
        }
        
        // 简化实现：广播查询请求
        let query = format!("DHT:GET:{key_hash}");
        
        for peer in peers.iter().take(3) {
            if network.send_to(&peer.node_id, query.as_bytes()).await.is_ok() {
                debug!("Queried {}", peer.node_id);
            }
        }
        
        // 简化返回，实际应该等待响应
        Ok(DhtResult::GetSuccess {
            value: format!("value_for_{}", key),
        })
    }
    
    /// 执行 DHT FIND_NODE 操作
    pub async fn find_node(network: &P2PNetwork, target_id: &str) -> Result<DhtResult> {
        info!("DHT FIND_NODE: {}", target_id);
        
        // 获取所有已发现的节点
        let peers = network.discovered_peers().await;
        
        let nodes: Vec<NodeInfo> = peers
            .into_iter()
            .map(|peer| NodeInfo {
                node_id: peer.node_id.clone(),
                address: peer.address,
                distance: Self::xor_distance(&peer.node_id, target_id),
            })
            .take(20) // Kademlia K=20
            .collect();
        
        if nodes.is_empty() {
            Ok(DhtResult::NotFound)
        } else {
            Ok(DhtResult::FindNodeSuccess { nodes })
        }
    }
    
    /// 计算 key 哈希（简化版）
    fn hash_key(key: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        format!("{:016x}", hasher.finish())
    }
    
    /// 计算 XOR 距离（Kademlia 距离度量）
    fn xor_distance(node_id: &str, target_id: &str) -> u32 {
        // 简化实现：使用字符串长度的差值
        // 实际应该使用节点 ID 的字节 XOR
        let n1 = node_id.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
        let n2 = target_id.bytes().fold(0u32, |acc, b| acc.wrapping_add(b as u32));
        n1 ^ n2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hash_key() {
        let hash1 = DhtOperations::hash_key("test");
        let hash2 = DhtOperations::hash_key("test");
        assert_eq!(hash1, hash2);
        
        let hash3 = DhtOperations::hash_key("different");
        assert_ne!(hash1, hash3);
    }
    
    #[test]
    fn test_xor_distance() {
        let d1 = DhtOperations::xor_distance("node1", "target");
        let d2 = DhtOperations::xor_distance("node2", "target");
        // 距离应该是不同的
        assert!(d1 != d2 || d1 == 0);
    }
}
