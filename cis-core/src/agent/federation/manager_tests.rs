//! 联邦管理器测试
//!
//! 测试跨节点 Agent 联邦的管理功能。

use super::{FederationManager, FederatedAddress, FederationTaskRequest, FederationTaskResult};
use std::sync::Arc;
use std::sync::Mutex;
use async_trait::async_trait;
use std::collections::HashMap;
use crate::traits::{NetworkService, SendOptions, PeerInfo, NetworkStatus};
use crate::event_bus::{EventBus, Subscription, EventHandlerFn};
use crate::events::EventWrapper;

/// 模拟网络服务
struct MockNetworkService {
    sent_messages: Mutex<HashMap<String, Vec<Vec<u8>>>>,
}

impl MockNetworkService {
    fn new() -> Self {
        Self {
            sent_messages: Mutex::new(HashMap::new()),
        }
    }

    fn has_sent_to(&self, node_id: &str) -> bool {
        self.sent_messages.lock().unwrap().contains_key(node_id)
    }
}

#[async_trait]
impl NetworkService for MockNetworkService {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> crate::error::Result<()> {
        self.sent_messages
            .lock()
            .unwrap()
            .entry(node_id.to_string())
            .or_default()
            .push(data.to_vec());
        Ok(())
    }

    async fn send_to_with_options(
        &self,
        node_id: &str,
        data: &[u8],
        _options: SendOptions,
    ) -> crate::error::Result<()> {
        self.send_to(node_id, data).await
    }

    async fn broadcast(&self, _data: &[u8]) -> crate::error::Result<usize> {
        Ok(1)
    }

    async fn broadcast_with_options(&self, _data: &[u8], _options: SendOptions) -> crate::error::Result<usize> {
        Ok(1)
    }

    async fn connect(&self, _addr: &str) -> crate::error::Result<()> {
        Ok(())
    }

    async fn disconnect(&self, _node_id: &str) -> crate::error::Result<()> {
        Ok(())
    }

    async fn connected_peers(&self) -> crate::error::Result<Vec<PeerInfo>> {
        Ok(vec![])
    }

    async fn discovered_peers(&self) -> crate::error::Result<Vec<PeerInfo>> {
        Ok(vec![])
    }

    async fn get_peer(&self, _node_id: &str) -> crate::error::Result<Option<PeerInfo>> {
        Ok(None)
    }

    async fn status(&self) -> crate::error::Result<NetworkStatus> {
        Ok(NetworkStatus {
            running: true,
            node_id: "test-node".to_string(),
            listen_addr: "127.0.0.1:0".to_string(),
            uptime_secs: 0,
            connected_peers: 0,
            discovered_peers: 0,
            bytes_sent: 0,
            bytes_received: 0,
            error_count: 0,
        })
    }

    async fn start(&self) -> crate::error::Result<()> {
        Ok(())
    }

    async fn stop(&self) -> crate::error::Result<()> {
        Ok(())
    }

    fn node_id(&self) -> crate::error::Result<String> {
        Ok("test-node".to_string())
    }

    fn did(&self) -> crate::error::Result<String> {
        Ok("did:cis:test".to_string())
    }

    async fn is_connected(&self, _node_id: &str) -> crate::error::Result<bool> {
        Ok(false)
    }
}

/// 模拟事件总线
struct MockEventBus {
    events: Mutex<Vec<String>>,
}

impl MockEventBus {
    fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }

    async fn has_events(&self) -> bool {
        !self.events.lock().unwrap().is_empty()
    }
}

#[async_trait]
impl EventBus for MockEventBus {
    async fn publish(&self, _event: EventWrapper) -> crate::error::Result<()> {
        self.events.lock().unwrap().push("event".to_string());
        Ok(())
    }

    async fn subscribe_boxed(
        &self,
        _topic: &str,
        _handler: EventHandlerFn,
    ) -> crate::error::Result<Subscription> {
        Ok(Subscription::new("sub-1", "topic"))
    }

    async fn unsubscribe(&self, _subscription: &Subscription) -> crate::error::Result<()> {
        Ok(())
    }

    async fn get_history(&self, _topic: &str, _limit: usize) -> crate::error::Result<Vec<EventWrapper>> {
        Ok(vec![])
    }

    async fn subscriber_count(&self, _topic: Option<&str>) -> usize {
        0
    }
}

/// 测试联邦管理器创建
#[tokio::test]
async fn test_federation_manager_creation() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus,
    );
    
    let peers = manager.list_peers().await;
    assert!(peers.is_empty());
}

/// 测试注册远程 Agent
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
    
    let result = manager.register_peer(peer.clone()).await;
    assert!(result.is_ok());
    
    let peers = manager.list_peers().await;
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].did, peer.did);
    
    // 验证事件已发布
    assert!(event_bus.has_events().await);
}

/// 测试重复注册同一 Agent
#[tokio::test]
async fn test_register_duplicate_peer() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus,
    );
    
    let peer = FederatedAddress {
        node_id: "remote-node".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:abc123".to_string(),
    };
    
    manager.register_peer(peer.clone()).await.unwrap();
    
    // 重复注册应该失败
    let result = manager.register_peer(peer).await;
    assert!(result.is_err());
}

/// 测试注册多个 Agent
#[tokio::test]
async fn test_register_multiple_peers() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus,
    );
    
    for i in 0..5 {
        let peer = FederatedAddress {
            node_id: format!("remote-node-{}", i),
            agent_id: format!("agent-{}", i),
            did: format!("did:cis:agent{}", i),
        };
        manager.register_peer(peer).await.unwrap();
    }
    
    let peers = manager.list_peers().await;
    assert_eq!(peers.len(), 5);
}

/// 测试分发任务到已注册 Agent
#[tokio::test]
async fn test_dispatch_task_to_registered_peer() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network.clone(),
        event_bus.clone(),
    );
    
    let peer = FederatedAddress {
        node_id: "remote-node".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:abc123".to_string(),
    };
    
    manager.register_peer(peer.clone()).await.unwrap();
    
    let task = FederationTaskRequest {
        task_id: "task-1".to_string(),
        skill_name: "test-skill".to_string(),
        method: "test".to_string(),
        params: b"{}".to_vec(),
        requester: "local-node".to_string(),
    };
    
    let result = manager.dispatch_task(&peer.did, task).await;
    assert!(result.is_ok());
    
    // 验证网络发送被调用
    assert!(network.has_sent_to(&peer.node_id));
}

/// 测试分发任务到未注册 Agent
#[tokio::test]
async fn test_dispatch_task_to_unregistered_peer() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus,
    );
    
    let task = FederationTaskRequest {
        task_id: "task-1".to_string(),
        skill_name: "test-skill".to_string(),
        method: "test".to_string(),
        params: b"{}".to_vec(),
        requester: "local-node".to_string(),
    };
    
    let result = manager.dispatch_task("did:cis:unknown", task).await;
    assert!(result.is_err());
}

/// 测试处理任务结果
#[tokio::test]
async fn test_handle_task_result() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus.clone(),
    );
    
    let result = FederationTaskResult {
        task_id: "task-1".to_string(),
        success: true,
        data: b"success data".to_vec(),
        error: None,
    };
    
    let handle_result = manager.handle_task_result(result).await;
    assert!(handle_result.is_ok());
    
    // 验证事件已发布
    assert!(event_bus.has_events().await);
}

/// 测试处理失败的任务结果
#[tokio::test]
async fn test_handle_failed_task_result() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus.clone(),
    );
    
    let result = FederationTaskResult {
        task_id: "task-1".to_string(),
        success: false,
        data: vec![],
        error: Some("Task failed".to_string()),
    };
    
    let handle_result = manager.handle_task_result(result).await;
    assert!(handle_result.is_ok());
    
    // 失败结果也应该发布事件
    assert!(event_bus.has_events().await);
}

/// 测试联邦地址结构
#[test]
fn test_federated_address() {
    let address = FederatedAddress {
        node_id: "node-1".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:test".to_string(),
    };
    
    assert_eq!(address.node_id, "node-1");
    assert_eq!(address.agent_id, "agent-1");
    assert_eq!(address.did, "did:cis:test");
}

/// 测试联邦地址克隆
#[test]
fn test_federated_address_clone() {
    let address = FederatedAddress {
        node_id: "node-1".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:test".to_string(),
    };
    
    let cloned = address.clone();
    assert_eq!(cloned.node_id, "node-1");
    assert_eq!(cloned.agent_id, "agent-1");
    assert_eq!(cloned.did, "did:cis:test");
}

/// 测试任务请求结构
#[test]
fn test_federation_task_request() {
    let task = FederationTaskRequest {
        task_id: "task-1".to_string(),
        skill_name: "test-skill".to_string(),
        method: "execute".to_string(),
        params: b"{\"key\": \"value\"}".to_vec(),
        requester: "local-node".to_string(),
    };
    
    assert_eq!(task.task_id, "task-1");
    assert_eq!(task.skill_name, "test-skill");
    assert_eq!(task.method, "execute");
}

/// 测试任务结果结构
#[test]
fn test_federation_task_result() {
    let result = FederationTaskResult {
        task_id: "task-1".to_string(),
        success: true,
        data: b"result data".to_vec(),
        error: None,
    };
    
    assert_eq!(result.task_id, "task-1");
    assert!(result.success);
    assert_eq!(result.data, b"result data");
    assert!(result.error.is_none());
}

/// 测试任务结果失败情况
#[test]
fn test_federation_task_result_failure() {
    let result = FederationTaskResult {
        task_id: "task-2".to_string(),
        success: false,
        data: vec![],
        error: Some("Execution failed".to_string()),
    };
    
    assert!(!result.success);
    assert!(result.error.is_some());
    assert_eq!(result.error.unwrap(), "Execution failed");
}

/// 测试并发注册
#[tokio::test]
async fn test_concurrent_peer_registration() {
    use tokio::task::JoinSet;
    
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus,
    );
    
    let manager = Arc::new(manager);
    let mut set = JoinSet::new();
    
    for i in 0..20 {
        let manager = manager.clone();
        set.spawn(async move {
            let peer = FederatedAddress {
                node_id: format!("node-{}", i),
                agent_id: format!("agent-{}", i),
                did: format!("did:cis:{}", i),
            };
            manager.register_peer(peer).await
        });
    }
    
    while set.join_next().await.is_some() {}
    
    let peers = manager.list_peers().await;
    assert_eq!(peers.len(), 20);
}

/// 测试事件总线错误处理
#[tokio::test]
async fn test_event_bus_error_handling() {
    // 创建一个会返回错误的事件总线
    use crate::event_bus::{Subscription, EventHandlerFn};
    
    struct FailingEventBus;
    
    #[async_trait]
    impl crate::event_bus::EventBus for FailingEventBus {
        async fn publish(&self, _event: crate::events::EventWrapper) -> crate::error::Result<()> {
            Err(crate::error::CisError::internal("Event bus error"))
        }
        
        async fn subscribe_boxed(
            &self,
            _topic: &str,
            _handler: EventHandlerFn,
        ) -> crate::error::Result<Subscription> {
            Ok(Subscription::new("sub-1", "topic"))
        }
        
        async fn unsubscribe(&self, _subscription: &Subscription) -> crate::error::Result<()> {
            Ok(())
        }
        
        async fn get_history(&self, _topic: &str, _limit: usize) -> crate::error::Result<Vec<crate::events::EventWrapper>> {
            Ok(vec![])
        }
        
        async fn subscriber_count(&self, _topic: Option<&str>) -> usize {
            0
        }
    }
    
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(FailingEventBus);
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network,
        event_bus,
    );
    
    let peer = FederatedAddress {
        node_id: "remote-node".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:test".to_string(),
    };
    
    // 注册应该失败，因为事件发布失败
    let result = manager.register_peer(peer).await;
    assert!(result.is_err());
}
