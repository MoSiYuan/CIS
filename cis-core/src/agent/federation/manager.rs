//! Agent 联邦实现

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use crate::error::{CisError, Result};
use crate::events::domain::{AgentOnlineEvent, FederationTaskEvent, SkillCompletedEvent};
use crate::events::{EventWrapper, Task, ExecutionResult, EventMetadata};
use crate::event_bus::EventBus;
use crate::traits::NetworkService;

/// 联邦 Agent 地址
#[derive(Debug, Clone)]
pub struct FederatedAddress {
    pub node_id: String,
    pub agent_id: String,
    pub did: String,
}

/// 联邦任务请求
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FederationTaskRequest {
    pub task_id: String,
    pub skill_name: String,
    pub method: String,
    pub params: Vec<u8>,
    pub requester: String,
}

/// 联邦任务结果
#[derive(Debug, Clone)]
pub struct FederationTaskResult {
    pub task_id: String,
    pub success: bool,
    pub data: Vec<u8>,
    pub error: Option<String>,
}

/// 联邦管理器
pub struct FederationManager {
    node_id: String,
    network: Arc<dyn NetworkService>,
    event_bus: Arc<dyn EventBus>,
    peers: Arc<RwLock<Vec<FederatedAddress>>>,
    task_tx: mpsc::Sender<FederationTaskRequest>,
}

impl FederationManager {
    pub fn new(
        node_id: String,
        network: Arc<dyn NetworkService>,
        event_bus: Arc<dyn EventBus>,
    ) -> (Self, mpsc::Receiver<FederationTaskRequest>) {
        let (task_tx, task_rx) = mpsc::channel(100);
        
        let manager = Self {
            node_id,
            network,
            event_bus,
            peers: Arc::new(RwLock::new(Vec::new())),
            task_tx,
        };
        
        (manager, task_rx)
    }
    
    /// 注册远程 Agent
    pub async fn register_peer(&self, address: FederatedAddress) -> Result<()> {
        let mut peers = self.peers.write().await;
        
        // 检查是否已存在
        if peers.iter().any(|p| p.did == address.did) {
            return Err(CisError::federation(format!(
                "Peer {} already registered", address.did
            )));
        }
        
        peers.push(address.clone());
        
        // 发布 Agent 上线事件
        let event = EventWrapper::AgentOnline(AgentOnlineEvent::new(
            &address.node_id,
            &address.agent_id,
            vec![],
        ));
        self.event_bus.publish(event).await?;
        
        tracing::info!("Registered federated peer: {}", address.did);
        Ok(())
    }
    
    /// 分发任务到远程节点
    pub async fn dispatch_task(&self, target_did: &str, task: FederationTaskRequest) -> Result<()> {
        let peers = self.peers.read().await;
        
        let target = peers.iter()
            .find(|p| p.did == target_did)
            .ok_or_else(|| CisError::federation(format!(
                "Peer {} not found", target_did
            )))?;
        
        // 创建 Task 结构
        let task_def = Task {
            task_type: format!("{}.{}", task.skill_name, task.method),
            parameters: serde_json::json!({
                "params": task.params,
                "requester": task.requester,
            }),
            priority: 5,
            timeout_secs: 300,
        };
        
        // 发布联邦任务事件
        let event = EventWrapper::FederationTask(FederationTaskEvent::new(
            &task.task_id,
            &self.node_id,
            &target.node_id,
            task_def,
        ));
        self.event_bus.publish(event).await?;
        
        // 通过网络发送任务
        let task_bytes = serde_json::to_vec(&task)?;
        self.network.send_to(&target.node_id, &task_bytes).await?;
        
        Ok(())
    }
    
    /// 处理接收到的任务结果
    pub async fn handle_task_result(&self, result: FederationTaskResult) -> Result<()> {
        // 构建 ExecutionResult
        let exec_result = if result.success {
            ExecutionResult::success(serde_json::json!({
                "data": result.data,
            }))
        } else {
            ExecutionResult::error(result.error.clone().unwrap_or_default())
        };
        
        let event = EventWrapper::SkillCompleted(SkillCompletedEvent::new(
            &result.task_id,
            "federation-task",
            exec_result,
            &self.node_id,
        ));
        self.event_bus.publish(event).await?;
        
        Ok(())
    }
    
    /// 获取所有对等节点
    pub async fn list_peers(&self) -> Vec<FederatedAddress> {
        self.peers.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::mocks::{MockNetworkService, MockEventBus};
    
    #[tokio::test]
    async fn test_register_peer() {
        let network = Arc::new(MockNetworkService::new());
        let event_bus = Arc::new(MockEventBus::new());
        
        let (manager, _task_rx) = FederationManager::new(
            "local-node".to_string(),
            network,
            event_bus.clone(),
        );
        
        let peer = FederatedAddress {
            node_id: "remote-node".to_string(),
            agent_id: "agent-1".to_string(),
            did: "did:cis:abc123".to_string(),
        };
        
        manager.register_peer(peer.clone()).await.unwrap();
        
        let peers = manager.list_peers().await;
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].did, peer.did);
    }
}
