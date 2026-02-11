//! 任务分发测试
//!
//! 测试联邦任务的分发、执行和结果处理。

use super::{FederationManager, FederatedAddress, FederationTaskRequest, FederationTaskResult};
use super::protocol::{
    TaskRequestPayload, TaskResultPayload, AgentAddress, AgentRoute, AgentRoutingTable
};
use std::sync::Arc;
use std::sync::Mutex;
use std::collections::HashMap;
use async_trait::async_trait;
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

    async fn has_sent_to(&self, node_id: &str) -> bool {
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

/// 测试任务请求负载创建
#[test]
fn test_task_request_payload_creation() {
    let task = TaskRequestPayload::new("task-1", "Write a function");
    
    assert_eq!(task.task_id, "task-1");
    assert_eq!(task.prompt, "Write a function");
    assert!(task.context.is_empty());
    assert!(task.system_prompt.is_none());
    assert!(task.model.is_none());
    assert!(task.metadata.is_empty());
}

/// 测试任务请求构建器模式
#[test]
fn test_task_request_payload_builder() {
    let task = TaskRequestPayload::new("task-1", "Write code")
        .with_context("Rust project")
        .with_system_prompt("You are a helpful assistant")
        .with_model("claude-3");
    
    assert_eq!(task.task_id, "task-1");
    assert_eq!(task.prompt, "Write code");
    assert_eq!(task.context, "Rust project");
    assert_eq!(task.system_prompt, Some("You are a helpful assistant".to_string()));
    assert_eq!(task.model, Some("claude-3".to_string()));
}

/// 测试任务请求元数据
#[test]
fn test_task_request_metadata() {
    let task = TaskRequestPayload::new("task-1", "Test")
        .with_metadata("priority", "high").unwrap()
        .with_metadata("timeout", 30).unwrap();
    
    assert_eq!(task.metadata.get("priority").unwrap(), "high");
    assert_eq!(task.metadata.get("timeout").unwrap(), 30);
}

/// 测试任务结果成功
#[test]
fn test_task_result_success() {
    let result = TaskResultPayload::success("Task completed");
    
    assert!(result.success);
    assert_eq!(result.output, "Task completed");
    assert_eq!(result.exit_code, 0);
    assert!(result.metadata.is_empty());
}

/// 测试任务结果失败
#[test]
fn test_task_result_error() {
    let result = TaskResultPayload::error("Task failed", 1);
    
    assert!(!result.success);
    assert_eq!(result.output, "Task failed");
    assert_eq!(result.exit_code, 1);
}

/// 测试任务结果元数据
#[test]
fn test_task_result_metadata() {
    let result = TaskResultPayload::success("Done")
        .with_metadata("duration_ms", 1500).unwrap();
    
    assert_eq!(result.metadata.get("duration_ms").unwrap(), 1500);
}

/// 测试 Agent 地址解析
#[test]
fn test_agent_address_parsing() {
    let addr = AgentAddress::parse("worker-1@kitchen.local").unwrap();
    
    assert_eq!(addr.agent_id(), "worker-1");
    assert_eq!(addr.node_id(), "kitchen.local");
    assert!(addr.is_local("kitchen.local"));
    assert!(!addr.is_local("living.local"));
}

/// 测试 Agent 地址格式化
#[test]
fn test_agent_address_formatting() {
    let addr = AgentAddress::new("worker-1", "kitchen.local");
    
    assert_eq!(addr.to_string(), "worker-1@kitchen.local");
}

/// 测试无效 Agent 地址
#[test]
fn test_agent_address_invalid() {
    // 缺少 @
    assert!(AgentAddress::parse("invalid-address").is_err());
    
    // 多个 @
    assert!(AgentAddress::parse("too@many@parts").is_err());
    
    // 空字符串
    assert!(AgentAddress::parse("").is_err());
}

/// 测试路由表创建
#[test]
fn test_routing_table_creation() {
    let table = AgentRoutingTable::new();
    
    assert!(table.remote_agents().is_empty());
    assert!(table.nodes().is_empty());
}

/// 测试注册远程 Agent
#[test]
fn test_register_remote_agent() {
    let mut table = AgentRoutingTable::new();
    
    table.register_remote("agent-1", "node-a");
    
    assert_eq!(table.remote_agents().len(), 1);
    assert_eq!(table.remote_agents().get("agent-1").unwrap(), "node-a");
}

/// 测试注销远程 Agent
#[test]
fn test_unregister_remote_agent() {
    let mut table = AgentRoutingTable::new();
    
    table.register_remote("agent-1", "node-a");
    let removed = table.unregister_remote("agent-1");
    
    assert!(removed);
    assert!(table.remote_agents().is_empty());
}

/// 测试注销不存在的 Agent
#[test]
fn test_unregister_nonexistent_agent() {
    let mut table = AgentRoutingTable::new();
    
    let removed = table.unregister_remote("nonexistent");
    
    assert!(!removed);
}

/// 测试本地路由
#[test]
fn test_route_local() {
    let mut table = AgentRoutingTable::new();
    
    table.register_remote("agent-1", "local-node");
    
    let route = table.route("agent-1", "local-node");
    assert!(route.is_local());
    assert!(!route.is_remote());
    assert!(!route.is_unknown());
}

/// 测试远程路由
#[test]
fn test_route_remote() {
    let mut table = AgentRoutingTable::new();
    
    table.register_remote("agent-1", "remote-node");
    
    let route = table.route("agent-1", "local-node");
    assert!(!route.is_local());
    assert!(route.is_remote());
    assert!(!route.is_unknown());
    assert_eq!(route.node_id(), Some("remote-node"));
}

/// 测试未知路由
#[test]
fn test_route_unknown() {
    let table = AgentRoutingTable::new();
    
    let route = table.route("unknown-agent", "local-node");
    assert!(!route.is_local());
    assert!(!route.is_remote());
    assert!(route.is_unknown());
    assert!(route.node_id().is_none());
}

/// 测试注册节点 URL
#[test]
fn test_register_node_url() {
    let mut table = AgentRoutingTable::new();
    
    table.register_node_url("node-a", "https://node-a.example.com");
    
    assert_eq!(table.node_url("node-a"), Some("https://node-a.example.com"));
}

/// 测试获取不存在的节点 URL
#[test]
fn test_get_nonexistent_node_url() {
    let table = AgentRoutingTable::new();
    
    assert!(table.node_url("nonexistent").is_none());
}

/// 测试清理节点
#[test]
fn test_cleanup_node() {
    let mut table = AgentRoutingTable::new();
    
    table.register_remote("agent-1", "node-a");
    table.register_remote("agent-2", "node-b");
    table.register_remote("agent-3", "node-a");
    
    table.cleanup_node("node-a");
    
    assert!(table.remote_agents().get("agent-1").is_none());
    assert!(table.remote_agents().get("agent-2").is_some());
    assert!(table.remote_agents().get("agent-3").is_none());
}

/// 测试任务分发
#[tokio::test]
async fn test_task_dispatch() {
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
        did: "did:cis:remote".to_string(),
    };
    
    manager.register_peer(peer.clone()).await.unwrap();
    
    let task = FederationTaskRequest {
        task_id: "task-1".to_string(),
        skill_name: "test-skill".to_string(),
        method: "execute".to_string(),
        params: serde_json::json!({"key": "value"}).to_string().into_bytes(),
        requester: "local-node".to_string(),
    };
    
    let result = manager.dispatch_task(&peer.did, task).await;
    assert!(result.is_ok());
    
    // 验证网络发送
    assert!(network.has_sent_to(&peer.node_id).await);
}

/// 测试任务结果处理
#[tokio::test]
async fn test_task_result_handling() {
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
        data: b"result".to_vec(),
        error: None,
    };
    
    let handle_result = manager.handle_task_result(result).await;
    assert!(handle_result.is_ok());
}

/// 测试并发任务分发
#[tokio::test]
async fn test_concurrent_task_dispatch() {
    use tokio::task::JoinSet;
    
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
        did: "did:cis:remote".to_string(),
    };
    
    manager.register_peer(peer.clone()).await.unwrap();
    
    let manager = Arc::new(manager);
    let mut set = JoinSet::new();
    
    for i in 0..10 {
        let manager = manager.clone();
        let did = peer.did.clone();
        set.spawn(async move {
            let task = FederationTaskRequest {
                task_id: format!("task-{}", i),
                skill_name: "test-skill".to_string(),
                method: "execute".to_string(),
                params: vec![],
                requester: "local-node".to_string(),
            };
            manager.dispatch_task(&did, task).await
        });
    }
    
    let mut success_count = 0;
    while let Some(result) = set.join_next().await {
        if result.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 10);
}

/// 测试路由表多个 Agent
#[test]
fn test_routing_table_multiple_agents() {
    let mut table = AgentRoutingTable::new();
    
    // 注册多个 Agent
    for i in 0..100 {
        table.register_remote(&format!("agent-{}", i), &format!("node-{}", i % 10));
    }
    
    assert_eq!(table.remote_agents().len(), 100);
    
    // 验证路由
    for i in 0..100 {
        let route = table.route(&format!("agent-{}", i), "local");
        if i % 10 == 0 {
            // 假设 node-0 是本地节点
        }
        assert!(matches!(route, AgentRoute::Remote { .. }));
    }
}

/// 测试重复注册相同 Agent（更新节点）
#[test]
fn test_register_same_agent_different_node() {
    let mut table = AgentRoutingTable::new();
    
    table.register_remote("agent-1", "node-a");
    table.register_remote("agent-1", "node-b"); // 应该更新
    
    assert_eq!(table.remote_agents().get("agent-1").unwrap(), "node-b");
}

/// 测试任务负载序列化
#[test]
fn test_task_payload_serialization() {
    let task = TaskRequestPayload::new("task-1", "Test prompt")
        .with_context("Test context")
        .with_model("gpt-4");
    
    let json = serde_json::to_string(&task).unwrap();
    assert!(json.contains("task-1"));
    assert!(json.contains("Test prompt"));
    assert!(json.contains("Test context"));
    assert!(json.contains("gpt-4"));
    
    let deserialized: TaskRequestPayload = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.task_id, "task-1");
}

/// 测试结果负载序列化
#[test]
fn test_result_payload_serialization() {
    let result = TaskResultPayload::success("Output")
        .with_metadata("key", "value").unwrap();
    
    let json = serde_json::to_string(&result).unwrap();
    let deserialized: TaskResultPayload = serde_json::from_str(&json).unwrap();
    
    assert!(deserialized.success);
    assert_eq!(deserialized.output, "Output");
}
