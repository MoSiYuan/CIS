//! 联邦集成测试
//!
//! 测试 Agent 联邦的完整功能流程。

use cis_core::agent::federation::{
    FederationManager, FederatedAddress, FederationTaskRequest, FederationTaskResult,
    AgentAddress, AgentRoutingTable, AgentRoute, TaskRequestPayload, TaskResultPayload
};
use cis_core::agent::persistent::{AgentStatus, RuntimeType};
use cis_core::agent::federation::protocol::AgentFederationEvent;
use std::sync::Arc;
use std::collections::HashMap;

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

#[async_trait::async_trait]
impl cis_core::traits::NetworkService for MockNetworkService {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> cis_core::error::Result<()> {
        self.sent_messages
            .lock()
            .unwrap()
            .entry(node_id.to_string())
            .or_default()
            .push(data.to_vec());
        Ok(())
    }

    async fn broadcast(&self, _data: &[u8]) -> cis_core::error::Result<()> {
        Ok(())
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

    fn has_events(&self) -> bool {
        !self.events.lock().unwrap().is_empty()
    }

    fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl cis_core::traits::EventBus for MockEventBus {
    async fn publish<E: cis_core::events::DomainEvent>(&self, _event: E) -> cis_core::error::Result<()> {
        self.events.lock().unwrap().push("event".to_string());
        Ok(())
    }

    fn subscribe<E: cis_core::events::DomainEvent>(&self, _handler: Box<dyn Fn(E) + Send + Sync>) {
    }
}

use std::sync::Mutex;
use async_trait::async_trait;

/// 测试联邦管理器完整生命周期
#[tokio::test]
async fn test_federation_manager_full_lifecycle() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network.clone(),
        event_bus.clone(),
    );
    
    // 初始状态
    assert!(manager.list_peers().await.is_empty());
    
    // 注册远程 Agent
    let peer = FederatedAddress {
        node_id: "remote-node".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:remote".to_string(),
    };
    
    manager.register_peer(peer.clone()).await.expect("Failed to register peer");
    
    // 验证注册
    let peers = manager.list_peers().await;
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].did, peer.did);
    
    // 验证事件已发布
    assert!(event_bus.has_events());
}

/// 测试任务分发和结果处理
#[tokio::test]
async fn test_task_dispatch_and_result() {
    let network = Arc::new(MockNetworkService::new());
    let event_bus = Arc::new(MockEventBus::new());
    
    let (manager, _task_rx) = FederationManager::new(
        "local-node".to_string(),
        network.clone(),
        event_bus.clone(),
    );
    
    // 注册远程 Agent
    let peer = FederatedAddress {
        node_id: "remote-node".to_string(),
        agent_id: "agent-1".to_string(),
        did: "did:cis:remote".to_string(),
    };
    
    manager.register_peer(peer.clone()).await.unwrap();
    
    // 分发任务
    let task = FederationTaskRequest {
        task_id: "task-1".to_string(),
        skill_name: "test-skill".to_string(),
        method: "execute".to_string(),
        params: b"{\"test\": true}".to_vec(),
        requester: "local-node".to_string(),
    };
    
    manager.dispatch_task(&peer.did, task).await.expect("Failed to dispatch task");
    
    // 验证任务已发送
    assert!(network.has_sent_to(&peer.node_id));
    
    // 处理结果
    let result = FederationTaskResult {
        task_id: "task-1".to_string(),
        success: true,
        data: b"success".to_vec(),
        error: None,
    };
    
    manager.handle_task_result(result).await.expect("Failed to handle result");
    
    // 验证结果事件已发布
    assert!(event_bus.event_count() >= 2); // 注册 + 任务 + 结果
}

/// 测试 Agent 地址解析和路由
#[test]
fn test_agent_address_and_routing() {
    // 解析地址
    let addr = AgentAddress::parse("worker-1@kitchen.local").expect("Failed to parse address");
    
    assert_eq!(addr.agent_id(), "worker-1");
    assert_eq!(addr.node_id(), "kitchen.local");
    assert!(addr.is_local("kitchen.local"));
    assert!(!addr.is_local("other.local"));
    
    // 格式化地址
    let formatted = addr.to_string();
    assert_eq!(formatted, "worker-1@kitchen.local");
}

/// 测试路由表功能
#[test]
fn test_routing_table_operations() {
    let mut table = AgentRoutingTable::new();
    
    // 注册远程 Agent
    table.register_remote("agent-1", "node-a");
    table.register_remote("agent-2", "node-b");
    table.register_remote("agent-3", "node-a");
    
    // 测试本地路由
    let route = table.route("agent-1", "node-a");
    assert!(route.is_local());
    
    // 测试远程路由
    let route = table.route("agent-1", "node-b");
    assert!(route.is_remote());
    assert_eq!(route.node_id(), Some("node-a"));
    
    // 测试未知路由
    let route = table.route("unknown", "node-a");
    assert!(route.is_unknown());
    
    // 注册节点 URL
    table.register_node_url("node-a", "https://node-a.example.com");
    assert_eq!(table.node_url("node-a"), Some("https://node-a.example.com"));
    
    // 注销 Agent
    assert!(table.unregister_remote("agent-1"));
    assert!(!table.unregister_remote("nonexistent"));
}

/// 测试任务负载创建和序列化
#[test]
fn test_task_payload_operations() {
    // 创建任务
    let task = TaskRequestPayload::new("task-1", "Write code")
        .with_context("Rust project")
        .with_system_prompt("You are a coder")
        .with_model("claude-3")
        .with_metadata("priority", "high").unwrap();
    
    assert_eq!(task.task_id, "task-1");
    assert_eq!(task.prompt, "Write code");
    assert_eq!(task.context, "Rust project");
    assert_eq!(task.system_prompt, Some("You are a coder".to_string()));
    assert_eq!(task.model, Some("claude-3".to_string()));
    
    // 序列化
    let json = serde_json::to_string(&task).expect("Failed to serialize");
    assert!(json.contains("task-1"));
    assert!(json.contains("Write code"));
    
    // 反序列化
    let deserialized: TaskRequestPayload = serde_json::from_str(&json).expect("Failed to deserialize");
    assert_eq!(deserialized.task_id, "task-1");
}

/// 测试结果负载操作
#[test]
fn test_result_payload_operations() {
    // 成功结果
    let success = TaskResultPayload::success("Task completed")
        .with_metadata("duration_ms", 1000).unwrap();
    
    assert!(success.success);
    assert_eq!(success.output, "Task completed");
    assert_eq!(success.exit_code, 0);
    assert_eq!(success.metadata.get("duration_ms").unwrap(), 1000);
    
    // 失败结果
    let error = TaskResultPayload::error("Task failed", 1);
    assert!(!error.success);
    assert_eq!(error.output, "Task failed");
    assert_eq!(error.exit_code, 1);
    
    // 序列化
    let json = serde_json::to_string(&success).expect("Failed to serialize");
    let deserialized: TaskResultPayload = serde_json::from_str(&json).expect("Failed to deserialize");
    assert!(deserialized.success);
}

/// 测试联邦事件创建
#[test]
fn test_federation_events() {
    use cis_core::agent::persistent::AgentStatus;
    
    // Agent 注册事件
    let registered = AgentFederationEvent::registered(
        "agent-1",
        "node-1",
        RuntimeType::Claude,
        vec!["coding".to_string(), "analysis".to_string()],
    );
    
    assert_eq!(registered.event_type_str(), "io.cis.agent.registered");
    
    // Agent 注销事件
    let unregistered = AgentFederationEvent::unregistered(
        "agent-1",
        "node-1",
        Some("Shutdown".to_string()),
    );
    
    assert_eq!(unregistered.event_type_str(), "io.cis.agent.unregistered");
    
    // 任务请求事件
    let task_req = AgentFederationEvent::task_request(
        "req-1",
        "agent-a",
        "agent-b",
        TaskRequestPayload::new("task-1", "Do something"),
        Some(300),
    );
    
    assert_eq!(task_req.event_type_str(), "io.cis.agent.task.request");
    
    // 心跳事件
    let heartbeat = AgentFederationEvent::heartbeat(
        "agent-1",
        "node-1",
        AgentStatus::Idle,
    );
    
    assert_eq!(heartbeat.event_type_str(), "io.cis.agent.heartbeat");
}

/// 测试并发任务分发
#[tokio::test]
async fn test_concurrent_task_dispatch() {
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
    
    let manager = Arc::new(manager);
    let mut handles = vec![];
    
    for i in 0..20 {
        let manager = manager.clone();
        let did = peer.did.clone();
        let handle = tokio::spawn(async move {
            let task = FederationTaskRequest {
                task_id: format!("task-{}", i),
                skill_name: "test-skill".to_string(),
                method: "execute".to_string(),
                params: vec![],
                requester: "local-node".to_string(),
            };
            manager.dispatch_task(&did, task).await
        });
        handles.push(handle);
    }
    
    let mut success_count = 0;
    for handle in handles {
        if handle.await.unwrap().is_ok() {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 20);
}

/// 测试路由表清理
#[test]
fn test_routing_table_cleanup() {
    let mut table = AgentRoutingTable::new();
    
    // 注册多个 Agent
    table.register_remote("agent-1", "node-a");
    table.register_remote("agent-2", "node-b");
    table.register_remote("agent-3", "node-a");
    table.register_remote("agent-4", "node-c");
    
    assert_eq!(table.remote_agents().len(), 4);
    
    // 清理 node-a 的所有 Agent
    table.cleanup_node("node-a");
    
    assert_eq!(table.remote_agents().len(), 2);
    assert!(!table.remote_agents().contains_key("agent-1"));
    assert!(table.remote_agents().contains_key("agent-2"));
    assert!(!table.remote_agents().contains_key("agent-3"));
    assert!(table.remote_agents().contains_key("agent-4"));
}

/// 测试错误路径：重复注册
#[tokio::test]
async fn test_duplicate_registration_error() {
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
    
    // 重复注册应该失败
    let result = manager.register_peer(peer).await;
    assert!(result.is_err());
}

/// 测试错误路径：分发到未注册 Agent
#[tokio::test]
async fn test_dispatch_to_unregistered_peer() {
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
        method: "execute".to_string(),
        params: vec![],
        requester: "local-node".to_string(),
    };
    
    // 分发到未注册的 Agent 应该失败
    let result = manager.dispatch_task("did:cis:unknown", task).await;
    assert!(result.is_err());
}

/// 测试大规模路由表
#[test]
fn test_large_routing_table() {
    let mut table = AgentRoutingTable::new();
    
    // 注册 1000 个 Agent
    for i in 0..1000 {
        table.register_remote(
            &format!("agent-{}", i),
            &format!("node-{}", i % 10),
        );
    }
    
    assert_eq!(table.remote_agents().len(), 1000);
    
    // 验证所有路由
    for i in 0..1000 {
        let route = table.route(&format!("agent-{}", i), "local-node");
        assert!(matches!(route, AgentRoute::Remote { .. }));
    }
}

/// 测试无效地址解析
#[test]
fn test_invalid_address_parsing() {
    // 缺少 @
    assert!(AgentAddress::parse("invalid").is_err());
    
    // 多个 @
    assert!(AgentAddress::parse("a@b@c").is_err());
    
    // 空字符串
    assert!(AgentAddress::parse("").is_err());
    
    // 只有 @
    assert!(AgentAddress::parse("@").is_err());
}
