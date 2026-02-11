//! Kademlia 查询管理器
//!
//! 实现并行 α 查询和迭代查找算法。

use super::node_id::NodeId;
use super::kbucket::NodeInfo;
use super::message::{FindNodeRequest, FindValueRequest, NodeInfoMsg, Message};
use super::constants::{ALPHA, MAX_LOOKUP_ITERATIONS, REQUEST_TIMEOUT_MS};
use crate::error::Result;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};
use tokio::time::timeout;

/// 查询结果
#[derive(Debug, Clone)]
pub enum QueryResult {
    /// 找到节点
    Nodes(Vec<NodeInfoMsg>),
    /// 找到值
    Value(Vec<u8>),
    /// 超时
    Timeout,
}

/// 查询管理器
pub struct QueryManager {
    /// 本地节点 ID
    local_id: NodeId,
    /// 活动查询
    active_queries: Arc<RwLock<HashMap<String, QueryHandle>>>,
}

impl QueryManager {
    /// 创建新的查询管理器
    pub fn new(local_id: NodeId) -> Self {
        Self {
            local_id,
            active_queries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 执行节点查找
    ///
    /// 使用 Kademlia 迭代查找算法：
    /// 1. 从本地路由表选择 α 个最近的节点
    /// 2. 并行发送 FIND_NODE 请求
    /// 3. 收集响应，更新候选列表
    /// 4. 重复直到找到 k 个节点或无法更近
    pub async fn find_node<F>(
        &self,
        target: NodeId,
        initial_nodes: Vec<NodeInfo>,
        rpc_call: F,
    ) -> Result<QueryResult>
    where
        F: Fn(NodeInfo, FindNodeRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Message>> + Send>> + Send + Sync,
    {
        let query_id = format!("find_node_{}_{}", hex::encode(target.as_bytes()), rand::random::<u64>());
        let (tx, mut rx) = mpsc::channel(100);
        
        // 候选节点列表
        let mut candidates: Vec<NodeInfo> = initial_nodes;
        // 已查询的节点
        let mut queried: HashSet<NodeId> = HashSet::new();
        // 已发现的节点
        let mut discovered: HashMap<NodeId, NodeInfo> = HashMap::new();
        
        for iteration in 0..MAX_LOOKUP_ITERATIONS {
            // 选择 α 个最近的未查询节点
            candidates.sort_by_key(|n| n.id.distance(&target));
            let to_query: Vec<_> = candidates
                .iter()
                .filter(|n| !queried.contains(&n.id))
                .take(ALPHA)
                .cloned()
                .collect();
            
            if to_query.is_empty() {
                break; // 没有更多节点可查询
            }
            
            // 并行查询
            let mut tasks = Vec::new();
            for node in to_query {
                queried.insert(node.id.clone());
                let request = FindNodeRequest::new(self.local_id.clone(), target.clone());
                let rpc = &rpc_call;
                let tx = tx.clone();
                
                let task = tokio::spawn(async move {
                    let result = timeout(
                        Duration::from_millis(REQUEST_TIMEOUT_MS),
                        rpc(node.clone(), request)
                    ).await;
                    
                    let _ = tx.send((node, result)).await;
                });
                tasks.push(task);
            }
            
            // 等待所有查询完成
            for task in tasks {
                let _ = task.await;
            }
            
            // 处理响应
            let mut new_nodes = false;
            while let Ok((node, result)) = rx.try_recv() {
                match result {
                    Ok(Ok(Message::FindNodeResponse(resp))) => {
                        for node_msg in resp.nodes {
                            let id = NodeId::from_bytes(node_msg.id);
                            if !queried.contains(&id) && !discovered.contains_key(&id) {
                                discovered.insert(id.clone(), NodeInfo::new(id, node_msg.address));
                                new_nodes = true;
                            }
                        }
                    }
                    Ok(Ok(_)) => {}
                    Ok(Err(_)) => {}
                    Err(_) => {} // 超时
                }
            }
            
            // 更新候选列表
            candidates = discovered.values().cloned().collect();
            
            // 如果没有新节点，结束查询
            if !new_nodes {
                break;
            }
        }
        
        // 返回最近的 k 个节点
        let mut nodes: Vec<_> = discovered.values().cloned().collect();
        nodes.sort_by_key(|n| n.id.distance(&target));
        nodes.truncate(super::constants::K);
        
        Ok(QueryResult::Nodes(nodes.into_iter().map(|n| NodeInfoMsg::new(n.id, n.address)).collect()))
    }

    /// 执行值查找
    pub async fn find_value<F>(
        &self,
        key: String,
        target: NodeId,
        initial_nodes: Vec<NodeInfo>,
        rpc_call: F,
    ) -> Result<QueryResult>
    where
        F: Fn(NodeInfo, FindValueRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Message>> + Send>> + Send + Sync,
    {
        let (tx, mut rx) = mpsc::channel(100);
        
        let mut candidates: Vec<NodeInfo> = initial_nodes;
        let mut queried: HashSet<NodeId> = HashSet::new();
        let mut discovered: HashMap<NodeId, NodeInfo> = HashMap::new();
        
        for _ in 0..MAX_LOOKUP_ITERATIONS {
            candidates.sort_by_key(|n| n.id.distance(&target));
            let to_query: Vec<_> = candidates
                .iter()
                .filter(|n| !queried.contains(&n.id))
                .take(ALPHA)
                .cloned()
                .collect();
            
            if to_query.is_empty() {
                break;
            }
            
            let mut tasks = Vec::new();
            for node in to_query {
                queried.insert(node.id.clone());
                let request = FindValueRequest::new(self.local_id.clone(), key.clone());
                let rpc = &rpc_call;
                let tx = tx.clone();
                
                let task = tokio::spawn(async move {
                    let result = timeout(
                        Duration::from_millis(REQUEST_TIMEOUT_MS),
                        rpc(node.clone(), request)
                    ).await;
                    
                    let _ = tx.send((node, result)).await;
                });
                tasks.push(task);
            }
            
            for task in tasks {
                let _ = task.await;
            }
            
            let mut found_value = None;
            while let Ok((node, result)) = rx.try_recv() {
                match result {
                    Ok(Ok(Message::FindValueResponse(resp))) => {
                        if let Some(value) = resp.value {
                            found_value = Some(value);
                            break;
                        }
                        for node_msg in resp.nodes {
                            let id = NodeId::from_bytes(node_msg.id);
                            if !queried.contains(&id) && !discovered.contains_key(&id) {
                                discovered.insert(id.clone(), NodeInfo::new(id, node_msg.address));
                            }
                        }
                    }
                    _ => {}
                }
            }
            
            if let Some(value) = found_value {
                return Ok(QueryResult::Value(value));
            }
            
            candidates = discovered.values().cloned().collect();
        }
        
        // 返回最近的节点
        let mut nodes: Vec<_> = discovered.values().cloned().collect();
        nodes.sort_by_key(|n| n.id.distance(&target));
        nodes.truncate(super::constants::K);
        
        Ok(QueryResult::Nodes(nodes.into_iter().map(|n| NodeInfoMsg::new(n.id, n.address)).collect()))
    }
}

/// 查询句柄
#[derive(Debug)]
struct QueryHandle {
    id: String,
    started_at: std::time::Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::message::{FindNodeResponse, FindValueResponse};

    #[tokio::test]
    async fn test_find_node_local() {
        let local = NodeId::random();
        let manager = QueryManager::new(local.clone());
        let target = NodeId::random();
        
        // 模拟 RPC 调用
        let rpc = |_node: NodeInfo, _req: FindNodeRequest| -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Message>> + Send>> {
            Box::pin(async move {
                Ok(Message::FindNodeResponse(FindNodeResponse::new(
                    NodeId::random(),
                    vec![],
                )))
            })
        };
        
        let result = manager.find_node(target, vec![], rpc).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_query_result() {
        let nodes = QueryResult::Nodes(vec![]);
        let value = QueryResult::Value(vec![1, 2, 3]);
        let timeout = QueryResult::Timeout;
        
        match nodes {
            QueryResult::Nodes(_) => {},
            _ => panic!("Expected Nodes variant"),
        }
        
        match value {
            QueryResult::Value(v) => assert_eq!(v, vec![1, 2, 3]),
            _ => panic!("Expected Value variant"),
        }
        
        match timeout {
            QueryResult::Timeout => {},
            _ => panic!("Expected Timeout variant"),
        }
    }
}
