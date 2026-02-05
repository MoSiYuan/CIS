//! DHT (Distributed Hash Table) 节点发现
//!
//! 使用 Kademlia DHT 协议实现公网节点发现

use crate::error::{CisError, Result};
use crate::p2p::NodeInfo;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

/// DHT 服务
pub struct DhtService {
    node_id: String,
    bootstrap_nodes: Vec<String>,
    /// 本地路由表
    routing_table: Arc<RwLock<HashMap<String, NodeInfo>>>,
    /// 本节点信息
    local_node: Arc<RwLock<Option<NodeInfo>>>,
}

impl DhtService {
    /// 创建 DHT 服务
    pub fn new(node_id: String, bootstrap_nodes: Vec<String>) -> Self {
        Self {
            node_id,
            bootstrap_nodes,
            routing_table: Arc::new(RwLock::new(HashMap::new())),
            local_node: Arc::new(RwLock::new(None)),
        }
    }

    /// 启动 DHT 服务
    pub async fn start(&self, local_node: NodeInfo) -> Result<()> {
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
    async fn try_connect_bootstrap(&self, address: &str) -> Result<()> {
        // 解析地址
        let addr: SocketAddr = address
            .parse()
            .map_err(|e| CisError::p2p(format!("Invalid bootstrap address: {}", e)))?;

        // 在实际实现中，这里会建立连接并获取节点列表
        tracing::debug!("Connecting to bootstrap: {}", addr);

        // 模拟获取节点列表
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        Ok(())
    }

    /// 启动维护任务
    async fn start_maintenance(&self) -> Result<()> {
        let _node_id = self.node_id.clone();
        let routing_table = Arc::clone(&self.routing_table);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                // 刷新路由表
                let mut table = routing_table.write().await;

                // 移除过期的节点
                let expired: Vec<String> = table
                    .iter()
                    .filter(|(_, _info)| {
                        // 检查节点是否过期（超过 10 分钟未更新）
                        // 实际实现中应该有 last_seen 字段
                        false
                    })
                    .map(|(id, _)| id.clone())
                    .collect();

                for id in expired {
                    table.remove(&id);
                    tracing::debug!("Removed expired node: {}", id);
                }

                tracing::debug!("DHT routing table size: {}", table.len());
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

        // 在实际实现中，这里会将节点信息发布到 DHT
        // 简化实现：存储到本地路由表
        self.routing_table
            .write()
            .await
            .insert(self.node_id.clone(), local);

        Ok(())
    }

    /// 查找节点
    pub async fn find_node(&self, node_id: &str) -> Result<Option<NodeInfo>> {
        // 首先检查本地路由表
        if let Some(info) = self.routing_table.read().await.get(node_id).cloned() {
            return Ok(Some(info));
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
            .map(|(id, info)| {
                let distance = Self::xor_distance(target, id);
                (id.clone(), info.clone(), distance)
            })
            .collect();

        // 按距离排序
        nodes.sort_by_key(|(_, _, dist)| *dist);

        // 返回最近的 N 个节点
        nodes.into_iter().take(count).map(|(_, info, _)| info).collect()
    }

    /// 获取所有已知节点
    pub async fn get_all_nodes(&self) -> Vec<NodeInfo> {
        self.routing_table.read().await.values().cloned().collect()
    }

    /// 添加节点到路由表
    pub async fn add_node(&self, node: NodeInfo) -> Result<()> {
        tracing::debug!("Adding node {} to routing table", node.node_id);
        self.routing_table
            .write()
            .await
            .insert(node.node_id.clone(), node);
        Ok(())
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
}

/// DHT 配置
#[derive(Debug, Clone)]
pub struct DhtConfig {
    pub bootstrap_nodes: Vec<String>,
    pub listen_addr: String,
    pub announce_interval_secs: u64,
}

impl Default for DhtConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![
                // 默认 bootstrap 节点
                // "bootstrap.cis.dev:7676".to_string(),
            ],
            listen_addr: "0.0.0.0:7678".to_string(),
            announce_interval_secs: 300,
        }
    }
}
