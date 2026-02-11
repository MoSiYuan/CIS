//! Kademlia DHT 实现
//!
//! 完整的 Kademlia 分布式哈希表实现。
//!
//! ## 模块结构
//!
//! - `constants`: Kademlia 常量定义（K, α, ID 长度等）
//! - `node_id`: 160-bit 节点 ID
//! - `distance`: XOR 距离计算
//! - `kbucket`: K-bucket 路由表条目
//! - `routing_table`: Kademlia 路由表
//! - `message`: Kademlia RPC 消息
//! - `query`: 查询管理器
//! - `storage`: 本地键值存储
//! - `transport`: DHT 网络传输接口
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::p2p::kademlia::{KademliaDht, NodeId, KademliaConfig};
//!
//! # async fn example() {
//! // 创建本地节点 ID
//! let local_id = NodeId::random();
//!
//! // 配置 DHT
//! let config = KademliaConfig::default();
//!
//! // 创建 DHT 服务
//! let dht = KademliaDht::new(local_id, config).await;
//!
//! // 启动 DHT
//! dht.start().await.unwrap();
//!
//! // 存储值
//! dht.put("key", b"value").await.unwrap();
//!
//! // 查找值
//! let value = dht.get("key").await.unwrap();
//! # }
//! ```

pub mod constants;
pub mod distance;
pub mod kbucket;
pub mod message;
pub mod node_id;
// Temporarily disabled due to lifetime issues
// pub mod query;
pub mod routing_table;
pub mod storage;
pub mod transport;

// 重新导出主要类型
pub use constants::{K, ALPHA, ID_LENGTH, ID_BITS};
pub use distance::Distance;
pub use node_id::NodeId;
pub use kbucket::NodeInfo;
pub use routing_table::RoutingTable;
pub use transport::{DhtTransport, P2PNetworkTransport};

use crate::error::{CisError, Result};
use message::{KademliaMessage, MessagePayload, NodeInfoMsg};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;

/// Kademlia 配置
#[derive(Debug, Clone)]
pub struct KademliaConfig {
    /// K 参数 - 每个 bucket 的节点数
    pub k: usize,
    /// α 参数 - 并行查询数
    pub alpha: usize,
    /// 请求超时（毫秒）
    pub request_timeout_ms: u64,
    /// Bucket 刷新间隔（秒）
    pub bucket_refresh_interval_secs: u64,
    /// 值过期时间（秒）
    pub value_expiration_secs: u64,
}

impl Default for KademliaConfig {
    fn default() -> Self {
        Self {
            k: constants::K,
            alpha: constants::ALPHA,
            request_timeout_ms: constants::REQUEST_TIMEOUT_MS,
            bucket_refresh_interval_secs: constants::BUCKET_REFRESH_INTERVAL_SECS,
            value_expiration_secs: constants::VALUE_EXPIRATION_SECS,
        }
    }
}

/// Kademlia DHT 服务
pub struct KademliaDht<T: DhtTransport + 'static> {
    /// 本地节点 ID
    local_id: NodeId,
    /// 配置
    config: KademliaConfig,
    /// 路由表
    routing_table: Arc<RwLock<RoutingTable>>,
    /// 本地存储
    storage: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// 传输层
    transport: Arc<T>,
    /// 运行状态
    running: Arc<RwLock<bool>>,
}

impl<T: DhtTransport + 'static> KademliaDht<T> {
    /// 创建新的 Kademlia DHT 服务
    pub async fn new(local_id: NodeId, config: KademliaConfig, transport: Arc<T>) -> Self {
        let routing_table = Arc::new(RwLock::new(RoutingTable::new(local_id.clone())));
        
        Self {
            local_id,
            config,
            routing_table,
            storage: Arc::new(RwLock::new(HashMap::new())),
            transport,
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// 启动 DHT 服务
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Kademlia DHT starting, node_id={}", self.local_id);
        
        // 设置运行状态
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // 启动路由表维护任务
        self.start_maintenance_tasks().await;
        
        tracing::info!("Kademlia DHT started successfully");
        Ok(())
    }

    /// 停止 DHT 服务
    pub async fn stop(&self) -> Result<()> {
        tracing::info!("Kademlia DHT stopping");
        
        let mut running = self.running.write().await;
        *running = false;
        
        tracing::info!("Kademlia DHT stopped");
        Ok(())
    }

    /// 启动维护任务（定期刷新路由表）
    async fn start_maintenance_tasks(&self) {
        let routing_table = Arc::clone(&self.routing_table);
        let transport = Arc::clone(&self.transport);
        let running = Arc::clone(&self.running);
        let refresh_interval_secs = self.config.bucket_refresh_interval_secs;
        let k = self.config.k;
        
        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(refresh_interval_secs));
            
            loop {
                ticker.tick().await;
                
                // 检查是否仍在运行
                if !*running.read().await {
                    break;
                }
                
                // 执行路由表刷新
                Self::refresh_routing_table(&routing_table, &transport, k).await;
            }
            
            tracing::info!("Routing table maintenance task stopped");
        });
        
        tracing::debug!("Routing table maintenance task started (interval: {}s)", 
            refresh_interval_secs);
    }

    /// 刷新路由表 - 随机选择节点进行查找以保持路由表新鲜
    async fn refresh_routing_table(
        routing_table: &Arc<RwLock<RoutingTable>>,
        transport: &Arc<T>,
        k: usize,
    ) {
        // 获取需要刷新的 bucket 的随机节点 ID
        let random_targets: Vec<NodeId> = {
            let rt = routing_table.read().await;
            (0..constants::NUM_BUCKETS)
                .filter_map(|i| {
                    if rt.bucket(i).map(|b| b.len()).unwrap_or(0) > 0 {
                        Some(NodeId::random())
                    } else {
                        None
                    }
                })
                .take(3) // 每次刷新最多 3 个随机目标
                .collect()
        };
        
        for target in random_targets {
            // 查找最近的节点
            let closest = {
                let rt = routing_table.read().await;
                rt.find_closest(&target, k)
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>()
            };
            
            // 向这些节点发送 FindNode 请求
            for node in closest {
                let msg = KademliaMessage::find_node(
                    transport.local_node().id,
                    target.clone(),
                );
                
                if let Err(e) = transport.send_to(&node, &msg).await {
                    tracing::debug!("Failed to send refresh ping to {}: {}", node.id, e);
                }
            }
        }
    }

    /// 添加引导节点
    pub async fn add_bootstrap_node(&self, node_info: NodeInfo) -> Result<()> {
        tracing::info!("Adding bootstrap node: {} at {}", node_info.id, node_info.address);
        
        // 添加到路由表
        {
            let mut rt = self.routing_table.write().await;
            rt.insert(node_info.clone());
        }
        
        // 发送 Ping 请求验证节点
        let ping_msg = KademliaMessage::ping(self.local_id.clone());
        let timeout = Duration::from_millis(self.config.request_timeout_ms);
        
        match self.transport.send_request(&node_info, &ping_msg, timeout).await {
            Ok(response) => {
                if let MessagePayload::Pong { nodes } = response.payload {
                    tracing::info!("Bootstrap node {} responded, received {} nodes", 
                        node_info.id, nodes.len());
                    
                    // 将返回的节点也加入路由表
                    let mut rt = self.routing_table.write().await;
                    for node_msg in nodes {
                        {
                            let node_id = NodeId::from_bytes(node_msg.id);
                            rt.insert(NodeInfo::new(node_id, node_msg.address));
                        }
                    }
                }
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Bootstrap node {} did not respond: {}", node_info.id, e);
                // 即使引导节点暂时不可用，也保留在路由表中，稍后刷新时会重试
                Ok(())
            }
        }
    }

    /// 存储键值对（分布式存储）
    pub async fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        tracing::debug!("Storing key={}, value_len={}", key, value.len());
        
        // 1. 先存储到本地
        {
            let mut storage = self.storage.write().await;
            storage.insert(key.to_string(), value.to_vec());
        }
        
        // 2. 计算目标节点 ID（使用 key 的哈希）
        let target_id = Self::key_to_node_id(key);
        
        // 3. 查找最近的 K 个节点
        let closest = {
            let rt = self.routing_table.read().await;
            rt.find_closest(&target_id, self.config.k)
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        };
        
        // 4. 向这些节点发送存储请求
        for node in closest {
            let store_msg = KademliaMessage::store(
                self.local_id.clone(),
                key.to_string(),
                value.to_vec(),
            );
            
            if let Err(e) = self.transport.send_to(&node, &store_msg).await {
                tracing::debug!("Failed to send store request to {}: {}", node.id, e);
            }
        }
        
        Ok(())
    }

    /// 获取键对应的值（分布式查找）
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        tracing::debug!("Looking up key={}", key);
        
        // 1. 先检查本地存储
        {
            let storage = self.storage.read().await;
            if let Some(value) = storage.get(key) {
                tracing::debug!("Key {} found in local storage", key);
                return Ok(Some(value.clone()));
            }
        }
        
        // 2. 计算目标节点 ID
        let target_id = Self::key_to_node_id(key);
        
        // 3. 执行节点查找迭代
        let value = self.iterative_find_value(key, &target_id).await?;
        
        if value.is_some() {
            tracing::debug!("Key {} found in remote storage", key);
        } else {
            tracing::debug!("Key {} not found", key);
        }
        
        Ok(value)
    }

    /// 迭代查找值
    async fn iterative_find_value(&self, key: &str, target_id: &NodeId) -> Result<Option<Vec<u8>>> {
        let mut queried = std::collections::HashSet::new();
        let mut closest: Vec<NodeInfo> = {
            let rt = self.routing_table.read().await;
            rt.find_closest(target_id, self.config.alpha)
                .into_iter()
                .cloned()
                .collect()
        };
        
        for iteration in 0..constants::MAX_LOOKUP_ITERATIONS {
            if closest.is_empty() {
                break;
            }
            
            // 选择未查询的节点
            let to_query: Vec<&NodeInfo> = closest.iter()
                .filter(|n| !queried.contains(&n.id))
                .take(self.config.alpha)
                .collect();
            
            if to_query.is_empty() {
                break;
            }
            
            // 并行发送 FindValue 请求
            let mut found_value = None;
            let mut new_nodes = Vec::new();
            
            for node in to_query {
                queried.insert(node.id.clone());
                
                let find_msg = KademliaMessage::find_value(
                    self.local_id.clone(),
                    key.to_string(),
                );
                
                let timeout = Duration::from_millis(self.config.request_timeout_ms);
                
                match self.transport.send_request(node, &find_msg, timeout).await {
                    Ok(response) => {
                        match response.payload {
                            MessagePayload::FindValueResponse { value, .. } => {
                                if let Some(v) = value {
                                    found_value = Some(v);
                                    break;
                                }
                            }
                            MessagePayload::FindNodeResponse { nodes } => {
                                // 收集新节点
                                for node_msg in nodes {
                                    let node_id = NodeId::from_bytes(node_msg.id);
                                    new_nodes.push(NodeInfo::new(node_id, node_msg.address));
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        tracing::debug!("FindValue request to {} failed: {}", node.id, e);
                    }
                }
            }
            
            // 更新 closest 列表
            for new_node in new_nodes {
                if !closest.iter().any(|n| n.id == new_node.id) {
                    closest.push(new_node);
                }
            }
            // 按距离排序
            closest.sort_by_key(|n| n.id.distance(target_id));
            closest.truncate(self.config.k);
            
            if found_value.is_some() {
                return Ok(found_value);
            }
        }
        
        Ok(None)
    }

    /// 查找节点
    pub async fn find_node(&self, target_id: &NodeId) -> Result<Vec<NodeInfo>> {
        let rt = self.routing_table.read().await;
        let closest = rt.find_closest(target_id, self.config.k)
            .into_iter()
            .cloned()
            .collect();
        Ok(closest)
    }
    
    /// 列出带有指定前缀的所有键（仅本地存储）
    /// 
    /// 注意：此方法只查询本地存储，不会向网络中的其他节点查询
    pub async fn list_keys_with_prefix(&self, prefix: &str) -> Result<Vec<String>> {
        let storage = self.storage.read().await;
        let keys: Vec<String> = storage
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        Ok(keys)
    }

    /// 处理传入的 Kademlia 消息
    pub async fn handle_message(&self, message: KademliaMessage) -> Result<Option<KademliaMessage>> {
        let sender_id = message.sender_id.clone();
        
        // 更新路由表（如果知道发送者地址）
        if let Some(node_info) = self.get_node_info(&sender_id).await {
            let mut rt = self.routing_table.write().await;
            rt.insert(node_info);
        }
        
        match message.payload {
            MessagePayload::Ping => {
                // 返回 Pong，附带一些已知节点
                let nodes = {
                    let rt = self.routing_table.read().await;
                    rt.find_closest(&sender_id, constants::K)
                        .into_iter()
                        .map(|n| NodeInfoMsg::new(n.id.clone(), n.address.clone()))
                        .collect()
                };
                
                let response = KademliaMessage::pong(self.local_id.clone(), nodes);
                Ok(Some(response))
            }
            
            MessagePayload::FindNode { target } => {
                let nodes = {
                    let rt = self.routing_table.read().await;
                    rt.find_closest(&target, constants::K)
                        .into_iter()
                        .map(|n| NodeInfoMsg::new(n.id.clone(), n.address.clone()))
                        .collect()
                };
                
                let response = KademliaMessage::find_node_response(
                    self.local_id.clone(),
                    nodes,
                );
                Ok(Some(response))
            }
            
            MessagePayload::FindValue { key } => {
                // 检查本地存储
                let (value, nodes) = {
                    let storage = self.storage.read().await;
                    if let Some(v) = storage.get(&key) {
                        (Some(v.clone()), vec![])
                    } else {
                        // 返回最近的节点
                        let rt = self.routing_table.read().await;
                        let closest = rt.find_closest(&Self::key_to_node_id(&key), constants::K)
                            .into_iter()
                            .map(|n| NodeInfoMsg::new(n.id.clone(), n.address.clone()))
                            .collect();
                        (None, closest)
                    }
                };
                
                let response = KademliaMessage::find_value_response(
                    self.local_id.clone(),
                    value,
                    nodes,
                );
                Ok(Some(response))
            }
            
            MessagePayload::Store { key, value } => {
                // 存储到本地
                let mut storage = self.storage.write().await;
                storage.insert(key, value);
                // Store 不需要响应
                Ok(None)
            }
            
            _ => {
                // 其他消息类型不需要响应
                Ok(None)
            }
        }
    }

    /// 获取节点信息（从路由表查找）
    async fn get_node_info(&self, node_id: &NodeId) -> Option<NodeInfo> {
        let rt = self.routing_table.read().await;
        rt.find(node_id).cloned()
    }

    /// 将字符串 key 转换为 NodeId
    fn key_to_node_id(key: &str) -> NodeId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        let hash = hasher.finish();
        
        // 将 64-bit hash 扩展到 160-bit
        let mut bytes = [0u8; constants::ID_LENGTH];
        bytes[0..8].copy_from_slice(&hash.to_be_bytes());
        // 添加一些变化
        for i in 1..constants::ID_LENGTH / 8 {
            bytes[i * 8..(i + 1) * 8].copy_from_slice(&(hash.wrapping_add(i as u64)).to_be_bytes());
        }
        
        NodeId::from_bytes(bytes)
    }

    /// 获取本地节点 ID
    pub fn local_id(&self) -> &NodeId {
        &self.local_id
    }

    /// 获取路由表统计
    pub async fn routing_table_stats(&self) -> (usize, usize) {
        let rt = self.routing_table.read().await;
        let total_nodes = rt.total_nodes();
        let num_buckets = constants::NUM_BUCKETS;
        (total_nodes, num_buckets)
    }

    /// 获取本地存储统计
    pub async fn storage_stats(&self) -> usize {
        let storage = self.storage.read().await;
        storage.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_kademlia_basic() {
        let node_id = NodeId::random();
        
        // 创建一个模拟的传输层用于测试
        use transport::MockTransport;
        let transport = Arc::new(MockTransport::new(NodeInfo::new(node_id.clone(), "127.0.0.1:0")));
        
        let dht = KademliaDht::new(node_id, KademliaConfig::default(), transport).await;
        
        // 测试本地存储
        dht.put("test_key", b"test_value").await.unwrap();
        let value = dht.get("test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));
    }
}
