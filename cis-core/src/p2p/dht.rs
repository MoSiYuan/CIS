//! DHT (Distributed Hash Table) 节点发现
//!
//! 使用 Kademlia DHT 协议实现公网节点发现

use crate::error::{CisError, Result};
use crate::service::node_service::NodeInfo;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc, Duration};

/// DHT 消息类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DhtMessage {
    /// Ping 请求
    Ping {
        node_id: String,
        timestamp: i64,
    },
    /// Pong 响应
    Pong {
        node_id: String,
        timestamp: i64,
        /// 已知节点列表
        nodes: Option<Vec<NodeInfo>>,
    },
    /// 查找节点请求
    FindNode {
        node_id: String,
        target_id: String,
        timestamp: i64,
    },
    /// 查找节点响应
    FoundNode {
        node_id: String,
        target_id: String,
        /// 最近的节点列表
        nodes: Vec<NodeInfo>,
        timestamp: i64,
    },
    /// 存储值请求
    Store {
        node_id: String,
        key: String,
        value: Vec<u8>,
        timestamp: i64,
    },
    /// 存储确认
    StoreAck {
        node_id: String,
        key: String,
        timestamp: i64,
    },
}

/// DHT 路由表条目
#[derive(Debug, Clone)]
pub struct RoutingTableEntry {
    pub node_info: NodeInfo,
    pub last_seen: DateTime<Utc>,
    pub last_pinged: Option<DateTime<Utc>>,
    pub ping_count: u32,
    pub failed_pings: u32,
}

impl RoutingTableEntry {
    /// 创建新的路由表条目
    pub fn new(node_info: NodeInfo) -> Self {
        Self {
            node_info,
            last_seen: Utc::now(),
            last_pinged: None,
            ping_count: 0,
            failed_pings: 0,
        }
    }

    /// 更新最后看到时间
    pub fn update_seen(&mut self) {
        self.last_seen = Utc::now();
    }

    /// 记录 ping
    pub fn record_ping(&mut self, success: bool) {
        self.last_pinged = Some(Utc::now());
        self.ping_count += 1;
        if !success {
            self.failed_pings += 1;
        }
    }

    /// 检查是否过期
    pub fn is_expired(&self, timeout_secs: i64) -> bool {
        Utc::now().signed_duration_since(self.last_seen).num_seconds() > timeout_secs
    }

    /// 获取可信度评分 (0-100)
    pub fn reliability_score(&self) -> u8 {
        if self.ping_count == 0 {
            return 50; // 未知，中等可信度
        }
        let success_rate = (self.ping_count - self.failed_pings) as f32 / self.ping_count as f32;
        (success_rate * 100.0) as u8
    }
}

/// DHT 服务
pub struct DhtService {
    node_id: String,
    bootstrap_nodes: Vec<String>,
    /// 本地路由表
    routing_table: Arc<RwLock<HashMap<String, RoutingTableEntry>>>,
    /// 本节点信息
    local_node: Arc<RwLock<Option<NodeInfo>>>,
    /// 键值存储
    kv_store: Arc<RwLock<HashMap<String, Vec<u8>>>>,
    /// 是否运行中
    running: Arc<RwLock<bool>>,
    /// 配置
    config: DhtConfig,
}

impl DhtService {
    /// 创建 DHT 服务
    pub fn new(node_id: String, bootstrap_nodes: Vec<String>) -> Self {
        Self::with_config(node_id, bootstrap_nodes, DhtConfig::default())
    }

    /// 创建带配置的 DHT 服务
    pub fn with_config(
        node_id: String,
        bootstrap_nodes: Vec<String>,
        config: DhtConfig,
    ) -> Self {
        Self {
            node_id,
            bootstrap_nodes,
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            local_node: Arc::new(RwLock::new(None)),
            kv_store: Arc::new(RwLock::new(HashMap::new())),
            running: Arc::new(RwLock::new(false)),
            config,
        }
    }

    /// 启动 DHT 服务
    pub async fn start(&self, local_node: NodeInfo) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        tracing::info!("Starting DHT service for node {}", self.node_id);

        // 保存本节点信息
        *self.local_node.write().await = Some(local_node.clone());

        // 启动 DHT 维护任务
        self.start_maintenance().await?;

        // 连接到 bootstrap 节点
        self.connect_to_bootstrap().await?;

        tracing::info!("DHT service started");
        Ok(())
    }

    /// 停止 DHT 服务
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        tracing::info!("DHT service stopped");
        Ok(())
    }

    /// 检查是否运行中
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// 连接到 bootstrap 节点
    async fn connect_to_bootstrap(&self) -> Result<()> {
        for bootstrap in &self.bootstrap_nodes {
            match self.try_connect_bootstrap(bootstrap).await {
                Ok(_) => {
                    tracing::info!("Connected to bootstrap node: {}", bootstrap);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect to bootstrap {}: {}", bootstrap, e);
                }
            }
        }
        Ok(())
    }

    /// 尝试连接 bootstrap 节点
    /// 
    /// 使用 TCP 连接尝试与 bootstrap 节点建立连接，
    /// 然后发送 ping 请求获取节点列表
    async fn try_connect_bootstrap(&self, address: &str) -> Result<()> {
        // 解析地址
        let addr: SocketAddr = address
            .parse()
            .map_err(|e| CisError::p2p(format!("Invalid bootstrap address: {}", e)))?;

        tracing::debug!("Connecting to bootstrap: {}", addr);

        // 尝试 TCP 连接
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(addr)
        ).await {
            Ok(Ok(mut stream)) => {
                tracing::info!("TCP connection established to bootstrap {}", addr);
                
                // 发送 DHT ping 请求
                let ping_request = DhtMessage::Ping {
                    node_id: self.node_id.clone(),
                    timestamp: Utc::now().timestamp(),
                };
                
                let request_bytes = serde_json::to_vec(&ping_request)
                    .map_err(|e| CisError::p2p(format!("Failed to serialize ping: {}", e)))?;
                
                // 发送请求
                tokio::io::AsyncWriteExt::write_all(&mut stream, &request_bytes).await
                    .map_err(|e| CisError::p2p(format!("Failed to send ping: {}", e)))?;
                
                // 读取响应
                let mut buffer = vec![0u8; 1024];
                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(5),
                    tokio::io::AsyncReadExt::read(&mut stream, &mut buffer)
                ).await {
                    Ok(Ok(n)) if n > 0 => {
                        buffer.truncate(n);
                        match serde_json::from_slice::<DhtMessage>(&buffer) {
                            Ok(DhtMessage::Pong { node_id, nodes, .. }) => {
                                tracing::info!("Received pong from bootstrap node {}", node_id);
                                
                                // 将返回的节点添加到路由表
                                if let Some(nodes) = nodes {
                                    let mut table = self.routing_table.write().await;
                                    for node in nodes {
                                        let entry = RoutingTableEntry::new(node);
                                        table.insert(entry.node_info.summary.id.clone(), entry);
                                    }
                                    tracing::info!("Added {} nodes from bootstrap", table.len());
                                }
                            }
                            Ok(_) => {
                                tracing::warn!("Unexpected response from bootstrap");
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse bootstrap response: {}", e);
                            }
                        }
                    }
                    Ok(Ok(_)) => {
                        tracing::warn!("Empty response from bootstrap");
                    }
                    Ok(Err(e)) => {
                        tracing::warn!("Failed to read from bootstrap: {}", e);
                    }
                    Err(_) => {
                        tracing::warn!("Timeout waiting for bootstrap response");
                    }
                }
                
                Ok(())
            }
            Ok(Err(e)) => {
                Err(CisError::p2p(format!("Failed to connect to bootstrap {}: {}", addr, e)))
            }
            Err(_) => {
                Err(CisError::p2p(format!("Timeout connecting to bootstrap {}", addr)))
            }
        }
    }

    /// 启动维护任务
    async fn start_maintenance(&self) -> Result<()> {
        let node_id = self.node_id.clone();
        let routing_table = Arc::clone(&self.routing_table);
        let running = Arc::clone(&self.running);
        let config = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                if !*running.read().await {
                    break;
                }

                // 刷新路由表
                let mut table = routing_table.write().await;

                // 移除过期的节点
                let expired: Vec<String> = table
                    .iter()
                    .filter(|(_, entry)| entry.is_expired(config.node_timeout_secs as i64))
                    .map(|(id, _)| id.clone())
                    .collect();

                for id in expired {
                    table.remove(&id);
                    tracing::debug!("Removed expired node: {}", id);
                }

                tracing::debug!(
                    "DHT routing table size: {} (node: {})",
                    table.len(),
                    node_id
                );
            }
        });

        Ok(())
    }

    /// 发布本节点到 DHT
    pub async fn announce(&self) -> Result<()> {
        let local = self
            .local_node
            .read()
            .await
            .clone()
            .ok_or_else(|| CisError::p2p("Local node not set"))?;

        tracing::info!("Announcing node {} to DHT", self.node_id);

        // 将节点信息发布到本地路由表（作为种子节点）
        let entry = RoutingTableEntry::new(local);
        self.routing_table
            .write()
            .await
            .insert(self.node_id.clone(), entry);

        Ok(())
    }

    /// 查找节点
    pub async fn find_node(&self, node_id: &str) -> Result<Option<NodeInfo>> {
        // 首先检查本地路由表
        if let Some(entry) = self.routing_table.read().await.get(node_id) {
            return Ok(Some(entry.node_info.clone()));
        }

        // 在实际实现中，这里会向 DHT 网络查询
        tracing::debug!("Looking up node {} in DHT", node_id);

        // 简化实现：返回 None
        Ok(None)
    }

    /// 查找最近的节点
    pub async fn find_closest_nodes(&self, target: &str, count: usize) -> Vec<NodeInfo> {
        let table = self.routing_table.read().await;

        // 计算距离（使用 XOR 距离）
        let mut nodes: Vec<(String, NodeInfo, u64)> = table
            .iter()
            .map(|(id, entry)| {
                let distance = Self::xor_distance(target, id);
                (id.clone(), entry.node_info.clone(), distance)
            })
            .collect();

        // 按距离排序
        nodes.sort_by_key(|(_, _, dist)| *dist);

        // 返回最近的 N 个节点
        nodes.into_iter().take(count).map(|(_, info, _)| info).collect()
    }

    /// 获取所有已知节点
    pub async fn get_all_nodes(&self) -> Vec<NodeInfo> {
        self.routing_table
            .read()
            .await
            .values()
            .map(|e| e.node_info.clone())
            .collect()
    }

    /// 获取路由表统计
    pub async fn get_stats(&self) -> DhtStats {
        let table = self.routing_table.read().await;
        let kv = self.kv_store.read().await;

        let total_reliability: u32 = table
            .values()
            .map(|e| e.reliability_score() as u32)
            .sum();

        DhtStats {
            routing_table_size: table.len(),
            kv_store_size: kv.len(),
            average_reliability: if table.is_empty() {
                0
            } else {
                (total_reliability / table.len() as u32) as u8
            },
            bootstrap_nodes: self.bootstrap_nodes.len(),
        }
    }

    /// 添加节点到路由表
    pub async fn add_node(&self, node: NodeInfo) -> Result<()> {
        tracing::debug!("Adding node {} to routing table", node.summary.id);
        
        let mut table = self.routing_table.write().await;
        
        // 如果节点已存在，更新最后看到时间
        if let Some(entry) = table.get_mut(&node.summary.id) {
            entry.update_seen();
            entry.node_info = node;
        } else {
            // 添加新条目
            table.insert(node.summary.id.clone(), RoutingTableEntry::new(node));
        }
        
        Ok(())
    }

    /// 存储键值对到 DHT
    pub async fn put(&self, key: &str, value: Vec<u8>) -> Result<()> {
        tracing::debug!("Storing key {} in DHT ({} bytes)", key, value.len());
        
        // 存储到本地
        self.kv_store.write().await.insert(key.to_string(), value.clone());
        
        // 在实际实现中，这里会将数据复制到最近的 k 个节点
        let closest = self.find_closest_nodes(key, self.config.replication_factor).await;
        tracing::debug!("Replicating key {} to {} nodes", key, closest.len());
        
        Ok(())
    }

    /// 从 DHT 获取键值对
    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        tracing::debug!("Getting key {} from DHT", key);
        
        // 首先检查本地存储
        if let Some(value) = self.kv_store.read().await.get(key) {
            return Ok(Some(value.clone()));
        }
        
        // 在实际实现中，这里会向最近的节点查询
        // 简化实现：返回 None
        Ok(None)
    }

    /// 删除键值对
    pub async fn delete(&self, key: &str) -> Result<bool> {
        tracing::debug!("Deleting key {} from DHT", key);
        Ok(self.kv_store.write().await.remove(key).is_some())
    }

    /// XOR 距离计算（Kademlia 使用）
    fn xor_distance(a: &str, b: &str) -> u64 {
        // 简化的距离计算
        // 实际应该使用节点 ID 的字节表示
        let a_bytes = a.as_bytes();
        let b_bytes = b.as_bytes();

        let mut distance: u64 = 0;
        for i in 0..a_bytes.len().min(b_bytes.len()).min(8) {
            distance = (distance << 8) | ((a_bytes[i] ^ b_bytes[i]) as u64);
        }
        distance
    }

    /// 向特定节点查询目标节点
    /// 
    /// 发送 FindNode 消息到指定节点，返回该节点知道的最近节点列表
    async fn query_node_for_target(&self, node: &NodeInfo, target_id: &str) -> Result<Vec<NodeInfo>> {
        let addr = node.summary.endpoint.parse::<SocketAddr>()
            .map_err(|e| CisError::p2p(format!("Invalid node address: {}", e)))?;
        
        // 尝试 TCP 连接
        match tokio::time::timeout(
            tokio::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(addr)
        ).await {
            Ok(Ok(mut stream)) => {
                // 发送 FindNode 请求
                let request = DhtMessage::FindNode {
                    node_id: self.node_id.clone(),
                    target_id: target_id.to_string(),
                    timestamp: Utc::now().timestamp(),
                };
                
                let request_bytes = serde_json::to_vec(&request)
                    .map_err(|e| CisError::p2p(format!("Failed to serialize request: {}", e)))?;
                
                tokio::io::AsyncWriteExt::write_all(&mut stream, &request_bytes).await
                    .map_err(|e| CisError::p2p(format!("Failed to send request: {}", e)))?;
                
                // 读取响应
                let mut buffer = vec![0u8; 65536];
                match tokio::time::timeout(
                    tokio::time::Duration::from_secs(5),
                    tokio::io::AsyncReadExt::read(&mut stream, &mut buffer)
                ).await {
                    Ok(Ok(n)) if n > 0 => {
                        buffer.truncate(n);
                        match serde_json::from_slice::<DhtMessage>(&buffer) {
                            Ok(DhtMessage::FoundNode { nodes, .. }) => {
                                tracing::debug!("Received {} nodes from {}", nodes.len(), node.summary.id);
                                Ok(nodes)
                            }
                            Ok(_) => {
                                Err(CisError::p2p("Unexpected response type".to_string()))
                            }
                            Err(e) => {
                                Err(CisError::p2p(format!("Failed to parse response: {}", e)))
                            }
                        }
                    }
                    Ok(Ok(_)) => {
                        Err(CisError::p2p("Empty response".to_string()))
                    }
                    Ok(Err(e)) => {
                        Err(CisError::p2p(format!("Failed to read response: {}", e)))
                    }
                    Err(_) => {
                        Err(CisError::p2p("Timeout waiting for response".to_string()))
                    }
                }
            }
            Ok(Err(e)) => {
                Err(CisError::p2p(format!("Failed to connect to node: {}", e)))
            }
            Err(_) => {
                Err(CisError::p2p("Connection timeout".to_string()))
            }
        }
    }

    /// 获取配置
    pub fn config(&self) -> &DhtConfig {
        &self.config
    }

    /// 获取 bootstrap 节点列表
    pub fn bootstrap_nodes(&self) -> &[String] {
        &self.bootstrap_nodes
    }

    /// 执行节点查找迭代（模拟 Kademlia 查找过程）
    pub async fn iterative_find_node(&self, target_id: &str) -> Result<Vec<NodeInfo>> {
        let alpha = self.config.alpha; // 并发查询数
        let k = self.config.k; // 桶大小
        
        tracing::debug!(
            "Starting iterative find node for {} (alpha={}, k={})",
            target_id, alpha, k
        );

        // 从本地路由表获取初始节点（获取 k 个最近节点）
        let mut closest = self.find_closest_nodes(target_id, k).await;
        let mut queried = std::collections::HashSet::new();
        let mut found = Vec::new();

        while !closest.is_empty() {
            // 选择 alpha 个未查询的最近节点
            let to_query: Vec<NodeInfo> = closest
                .into_iter()
                .filter(|n| queried.insert(n.summary.id.clone()))
                .take(alpha)
                .collect();

            if to_query.is_empty() {
                break;
            }

            // 并行查询节点
            for node in &to_query {
                tracing::debug!("Querying node {} for target {}", node.summary.id, target_id);
                // 发送 FindNode RPC 查询
                match self.query_node_for_target(node, target_id).await {
                    Ok(nodes) => {
                        // 将新发现的节点加入待查询列表
                        for new_node in nodes {
                            if !queried.contains(&new_node.summary.id) {
                                // 添加到路由表
                                self.add_node(new_node.clone()).await?;
                                // 添加到 found 列表
                                found.push(new_node);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to query node {}: {}", node.summary.id, e);
                    }
                }
            }

            // 添加已查询的节点到结果
            found.extend(to_query);
            
            // 获取下一批最近的节点（获取 k 个，然后过滤已查询的）
            closest = self.find_closest_nodes(target_id, k).await
                .into_iter()
                .filter(|n| !queried.contains(&n.summary.id))
                .collect();
        }

        // 返回 k 个最近的节点
        found.truncate(k);
        Ok(found)
    }
}

/// DHT 配置
#[derive(Debug, Clone)]
pub struct DhtConfig {
    pub bootstrap_nodes: Vec<String>,
    pub listen_addr: String,
    pub announce_interval_secs: u64,
    /// 节点超时时间（秒）
    pub node_timeout_secs: u64,
    /// 路由表桶大小 (k)
    pub k: usize,
    /// 并发查询数 (alpha)
    pub alpha: usize,
    /// 数据复制因子
    pub replication_factor: usize,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![
                // 默认 bootstrap 节点
                // "bootstrap.cis.dev:6767".to_string(),
            ],
            listen_addr: "0.0.0.0:7678".to_string(),
            announce_interval_secs: 300,
            node_timeout_secs: 600, // 10 分钟
            k: 20,
            alpha: 3,
            replication_factor: 3,
        }
    }
}

/// DHT 统计信息
#[derive(Debug, Clone)]
pub struct DhtStats {
    pub routing_table_size: usize,
    pub kv_store_size: usize,
    pub average_reliability: u8,
    pub bootstrap_nodes: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_node_info(node_id: &str) -> NodeInfo {
        use crate::service::node_service::{NodeSummary, NodeStatus};
        
        NodeInfo {
            summary: NodeSummary {
                id: node_id.to_string(),
                did: format!("did:cis:{}", node_id),
                name: node_id.to_string(),
                status: NodeStatus::Online,
                endpoint: format!("127.0.0.1:767{}", node_id.len()),
                version: "1.0.0".to_string(),
                last_seen: Utc::now(),
                capabilities: vec!["memory_sync".to_string()],
            },
            public_key: "mock_key".to_string(),
            metadata: HashMap::new(),
            trust_score: 1.0,
            is_blacklisted: false,
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_dht_service_creation() {
        let bootstrap = vec!["127.0.0.1:7678".to_string()];
        let service = DhtService::new("test-node".to_string(), bootstrap.clone());
        
        assert!(!service.is_running().await);
        assert_eq!(service.bootstrap_nodes(), &bootstrap);
    }

    #[tokio::test]
    async fn test_dht_start_stop() {
        let service = DhtService::new("test-node".to_string(), vec![]);
        let local_node = create_test_node_info("test-node");

        // 启动服务
        service.start(local_node.clone()).await.unwrap();
        assert!(service.is_running().await);

        // 停止服务
        service.stop().await.unwrap();
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_dht_announce_and_find() {
        let service = DhtService::new("test-node".to_string(), vec![]);
        let local_node = create_test_node_info("test-node");

        service.start(local_node.clone()).await.unwrap();
        service.announce().await.unwrap();

        // 查找自己
        let found = service.find_node("test-node").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().summary.id, "test-node");

        // 查找不存在的节点
        let not_found = service.find_node("non-existent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_routing_table_entry() {
        let node_info = create_test_node_info("test-node");
        let mut entry = RoutingTableEntry::new(node_info);

        assert_eq!(entry.reliability_score(), 50);
        assert!(!entry.is_expired(1));

        // 记录成功的 ping
        entry.record_ping(true);
        assert_eq!(entry.ping_count, 1);
        assert_eq!(entry.failed_pings, 0);
        assert_eq!(entry.reliability_score(), 100);

        // 记录失败的 ping
        entry.record_ping(false);
        assert_eq!(entry.ping_count, 2);
        assert_eq!(entry.failed_pings, 1);
        assert_eq!(entry.reliability_score(), 50);
    }

    #[tokio::test]
    async fn test_dht_add_node() {
        let service = DhtService::new("local-node".to_string(), vec![]);
        let local_node = create_test_node_info("local-node");
        service.start(local_node).await.unwrap();

        // 添加新节点
        let peer = create_test_node_info("peer-node");
        service.add_node(peer.clone()).await.unwrap();

        // 验证节点已添加
        let nodes = service.get_all_nodes().await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].summary.id, "peer-node");

        // 再次添加同一节点（应更新）
        let mut updated_peer = peer.clone();
        updated_peer.summary.endpoint = "192.168.1.1:7677".to_string();
        service.add_node(updated_peer).await.unwrap();

        let nodes = service.get_all_nodes().await;
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].summary.endpoint, "192.168.1.1:7677");
    }

    #[tokio::test]
    async fn test_dht_kv_store() {
        let service = DhtService::new("test-node".to_string(), vec![]);
        let local_node = create_test_node_info("test-node");
        service.start(local_node).await.unwrap();

        // 存储键值对
        let key = "test-key";
        let value = b"test-value".to_vec();
        service.put(key, value.clone()).await.unwrap();

        // 获取键值对
        let retrieved = service.get(key).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), value);

        // 获取不存在的键
        let not_found = service.get("non-existent").await.unwrap();
        assert!(not_found.is_none());

        // 删除键值对
        let deleted = service.delete(key).await.unwrap();
        assert!(deleted);

        // 再次删除
        let not_deleted = service.delete(key).await.unwrap();
        assert!(!not_deleted);
    }

    #[test]
    fn test_xor_distance() {
        // 相同的 ID，距离应为 0
        assert_eq!(DhtService::xor_distance("abc", "abc"), 0);

        // 不同的 ID，距离应大于 0
        let dist1 = DhtService::xor_distance("abc", "def");
        let dist2 = DhtService::xor_distance("abc", "xyz");
        assert!(dist1 > 0);
        assert!(dist2 > 0);

        // 距离是对称的
        assert_eq!(
            DhtService::xor_distance("node1", "node2"),
            DhtService::xor_distance("node2", "node1")
        );
    }

    #[tokio::test]
    async fn test_find_closest_nodes() {
        let service = DhtService::new("local-node".to_string(), vec![]);
        let local_node = create_test_node_info("local-node");
        service.start(local_node).await.unwrap();

        // 添加多个节点
        for i in 0..10 {
            let node = create_test_node_info(&format!("node{}", i));
            service.add_node(node).await.unwrap();
        }

        // 查找最近的 3 个节点
        let closest = service.find_closest_nodes("target", 3).await;
        assert_eq!(closest.len(), 3);

        // 查找最近的 20 个节点（但只有 10 个存在）
        let closest = service.find_closest_nodes("target", 20).await;
        assert_eq!(closest.len(), 10);
    }

    #[tokio::test]
    async fn test_dht_stats() {
        let service = DhtService::new("test-node".to_string(), vec![]);
        let local_node = create_test_node_info("test-node");
        service.start(local_node).await.unwrap();

        // 初始状态
        let stats = service.get_stats().await;
        assert_eq!(stats.routing_table_size, 0);
        assert_eq!(stats.kv_store_size, 0);
        assert_eq!(stats.bootstrap_nodes, 0);

        // 添加节点和数据
        service.add_node(create_test_node_info("peer1")).await.unwrap();
        service.add_node(create_test_node_info("peer2")).await.unwrap();
        service.put("key1", b"value1".to_vec()).await.unwrap();

        let stats = service.get_stats().await;
        assert_eq!(stats.routing_table_size, 2);
        assert_eq!(stats.kv_store_size, 1);
    }

    #[tokio::test]
    async fn test_iterative_find_node() {
        let service = DhtService::new("local-node".to_string(), vec![]);
        let local_node = create_test_node_info("local-node");
        service.start(local_node).await.unwrap();

        // 添加一些节点
        for i in 0..5 {
            let node = create_test_node_info(&format!("node{}", i));
            service.add_node(node).await.unwrap();
        }

        // 执行迭代查找
        let found = service.iterative_find_node("target").await.unwrap();
        // 由于都是本地节点，应该返回所有节点
        assert_eq!(found.len(), 5);
    }

    #[test]
    fn test_dht_config_default() {
        let config = DhtConfig::default();
        assert_eq!(config.listen_addr, "0.0.0.0:7678");
        assert_eq!(config.announce_interval_secs, 300);
        assert_eq!(config.node_timeout_secs, 600);
        assert_eq!(config.k, 20);
        assert_eq!(config.alpha, 3);
        assert_eq!(config.replication_factor, 3);
    }

    #[tokio::test]
    async fn test_routing_entry_expiration() {
        let node_info = create_test_node_info("test-node");
        let entry = RoutingTableEntry::new(node_info);

        // 检查最近创建的条目没有过期
        assert!(!entry.is_expired(3600)); // 1 小时

        // 创建过期的条目（手动修改 last_seen）
        let mut expired_entry = RoutingTableEntry {
            node_info: create_test_node_info("expired"),
            last_seen: Utc::now() - Duration::seconds(7200), // 2 小时前
            last_pinged: None,
            ping_count: 0,
            failed_pings: 0,
        };

        assert!(expired_entry.is_expired(3600));

        // 更新 seen 时间
        expired_entry.update_seen();
        assert!(!expired_entry.is_expired(3600));
    }
}
