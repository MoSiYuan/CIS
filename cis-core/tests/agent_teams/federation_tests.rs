//! # Agent Teams 联邦测试
//!
//! 模拟跨节点场景的集成测试：
//! - 远程 Agent 发现
//! - 跨节点任务分发
//! - 联邦状态同步

use std::collections::HashMap;
use std::sync::Arc;


use cis_core::agent::persistent::{
    AgentPool, PoolConfig, RuntimeType,
};
use cis_core::scheduler::{
    DagScheduler, MultiAgentDagExecutor, MultiAgentExecutorConfig, TaskDag,
};

use super::utils::*;

/// 模拟联邦节点信息
#[derive(Debug, Clone)]
struct FederationNode {
    #[allow(dead_code)]
    node_id: String,
    pool: AgentPool,
}

/// 模拟联邦管理器
struct FederationManager {
    nodes: HashMap<String, FederationNode>,
    local_node_id: String,
}

impl FederationManager {
    fn new(local_node_id: impl Into<String>) -> Self {
        Self {
            nodes: HashMap::new(),
            local_node_id: local_node_id.into(),
        }
    }

    fn add_node(&mut self, node_id: impl Into<String>, pool: AgentPool) {
        let node_id = node_id.into();
        self.nodes.insert(
            node_id.clone(),
            FederationNode {
                node_id,
                pool,
            },
        );
    }

    fn get_node(&self, node_id: &str) -> Option<&FederationNode> {
        self.nodes.get(node_id)
    }

    fn local_node(&self) -> Option<&FederationNode> {
        self.get_node(&self.local_node_id)
    }

    /// 获取所有节点的 Agent 统计
    async fn get_cluster_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        for (node_id, node) in &self.nodes {
            let count = node.pool.agent_count().await;
            stats.insert(node_id.clone(), count);
        }
        stats
    }
}

/// 测试联邦节点间的 Pool 协调
#[tokio::test]
async fn test_federation_pool_coordination() {
    let _ctx = TestContext::new();

    // 创建多个节点的 Pool
    let node1_pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });
    let node2_pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    // 注册 Runtime
    node1_pool
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();
    node2_pool
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 创建联邦管理器
    let mut federation = FederationManager::new("node-1");
    federation.add_node("node-1", node1_pool);
    federation.add_node("node-2", node2_pool);

    // 在 node-1 上创建 Agent
    let node1 = federation.local_node().unwrap();
    let config = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("federated-agent"));
    let agent = node1.pool.acquire(config).await.unwrap();

    // 验证联邦统计
    let stats = federation.get_cluster_stats().await;
    assert_eq!(stats.get("node-1"), Some(&1));
    assert_eq!(stats.get("node-2"), Some(&0));

    // 清理
    node1.pool.release(agent, false).await.unwrap();
}

/// 测试跨节点的 DAG 执行概念
#[tokio::test]
async fn test_federated_dag_execution_concept() {
    let _ctx = TestContext::new();

    // 创建本地执行器
    let scheduler = DagScheduler::new();
    let pool = AgentPool::new(PoolConfig {
        max_agents: 10,
        ..Default::default()
    });

    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();
    pool.register_runtime(Arc::new(MockRuntime::new(RuntimeType::OpenCode)))
        .await
        .unwrap();

    let config = MultiAgentExecutorConfig::default();
    let executor = MultiAgentDagExecutor::new(scheduler, pool, config).unwrap();

    // 创建一个模拟跨节点的 DAG
    // task-1: 本地执行 (Claude)
    // task-2: 本地执行 (OpenCode)，依赖 task-1
    let mut dag = TaskDag::new();

    dag.add_node("task-1".to_string(), vec![]).unwrap();
    if let Some(node) = dag.get_node_mut("task-1") {
        node.agent_runtime = Some(cis_core::scheduler::RuntimeType::Claude);
    }

    dag.add_node("task-2".to_string(), vec!["task-1".to_string()]).unwrap();
    if let Some(node) = dag.get_node_mut("task-2") {
        node.agent_runtime = Some(cis_core::scheduler::RuntimeType::OpenCode);
    }

    // 模拟任务分发决策
    // 在实际联邦系统中，这里会根据节点负载决定在哪里执行
    let _task_assignments: HashMap<String, String> = [
        ("task-1".to_string(), "local".to_string()),
        ("task-2".to_string(), "local".to_string()),
    ]
    .into_iter()
    .collect();

    // 执行 DAG
    let run_id = executor.create_run(dag).await.unwrap();
    let report = executor.execute(&run_id).await.unwrap();

    // 验证结果
    assert_eq!(report.completed, 2);
    assert_eq!(report.failed, 0);
    assert_eq!(report.final_status, "success");

    // 验证任务输出包含分配信息
    assert!(report.task_outputs.contains_key("task-1"));
    assert!(report.task_outputs.contains_key("task-2"));
}

/// 测试 Agent 发现机制概念
#[tokio::test]
async fn test_agent_discovery_concept() {
    let _ctx = TestContext::new();

    // 创建多个 Pool 模拟多个节点
    let pool1 = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });
    let pool2 = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    pool1
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();
    pool2
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::OpenCode)))
        .await
        .unwrap();

    // 在 pool1 创建 Claude Agent
    let config1 = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("claude-agent"));
    let agent1 = pool1.acquire(config1).await.unwrap();
    let agent1_id = agent1.agent_id().to_string();
    pool1.release(agent1, true).await.unwrap();

    // 在 pool2 创建 OpenCode Agent
    let config2 = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::OpenCode)
        .with_agent_config(test_agent_config("opencode-agent"));
    let agent2 = pool2.acquire(config2).await.unwrap();
    let agent2_id = agent2.agent_id().to_string();
    pool2.release(agent2, true).await.unwrap();

    // 模拟发现服务 - 聚合所有节点的 Agent 列表
    let mut discovered_agents: Vec<(String, String, RuntimeType)> = vec![];

    // 从 pool1 发现
    for agent_info in pool1.list().await {
        discovered_agents.push((
            "node-1".to_string(),
            agent_info.id.clone(),
            agent_info.runtime_type,
        ));
    }

    // 从 pool2 发现
    for agent_info in pool2.list().await {
        discovered_agents.push((
            "node-2".to_string(),
            agent_info.id.clone(),
            agent_info.runtime_type,
        ));
    }

    // 验证发现结果
    assert_eq!(discovered_agents.len(), 2);

    let claude_agents: Vec<_> = discovered_agents
        .iter()
        .filter(|(_, _, rt)| *rt == RuntimeType::Claude)
        .collect();
    assert_eq!(claude_agents.len(), 1);
    assert_eq!(claude_agents[0].0, "node-1");

    let opencode_agents: Vec<_> = discovered_agents
        .iter()
        .filter(|(_, _, rt)| *rt == RuntimeType::OpenCode)
        .collect();
    assert_eq!(opencode_agents.len(), 1);
    assert_eq!(opencode_agents[0].0, "node-2");

    // 清理
    pool1.kill(&agent1_id).await.unwrap();
    pool2.kill(&agent2_id).await.unwrap();
}

/// 测试负载均衡概念
#[tokio::test]
async fn test_load_balancing_concept() {
    let _ctx = TestContext::new();

    // 创建两个不同负载的 Pool
    let loaded_pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });
    let empty_pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    loaded_pool
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();
    empty_pool
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 在 loaded_pool 上创建多个 Agent
    for i in 0..4 {
        let config = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::Claude)
            .with_agent_config(test_agent_config(format!("loaded-agent-{}", i)));
        let agent = loaded_pool.acquire(config).await.unwrap();
        loaded_pool.release(agent, true).await.unwrap();
    }

    // 模拟负载均衡决策
    let loaded_count = loaded_pool.agent_count().await;
    let empty_count = empty_pool.agent_count().await;

    // 简单的负载均衡策略：选择 Agent 数较少的 Pool
    let target_pool = if loaded_count > empty_count {
        &empty_pool
    } else {
        &loaded_pool
    };

    // 在选定的 Pool 上创建新 Agent
    let config = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("balanced-agent"));
    let new_agent = target_pool.acquire(config).await.unwrap();

    // 验证负载均衡结果
    if target_pool as *const _ == &empty_pool as *const _ {
        assert_eq!(empty_pool.agent_count().await, 1);
    } else {
        assert_eq!(loaded_pool.agent_count().await, 5);
    }

    // 清理
    target_pool.release(new_agent, false).await.unwrap();
    loaded_pool.shutdown_all().await.unwrap();
    empty_pool.shutdown_all().await.unwrap();
}

/// 测试故障转移概念
#[tokio::test]
async fn test_failover_concept() {
    let _ctx = TestContext::new();

    // 创建主备 Pool
    let primary_pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });
    let backup_pool = AgentPool::new(PoolConfig {
        max_agents: 5,
        ..Default::default()
    });

    primary_pool
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();
    backup_pool
        .register_runtime(Arc::new(MockRuntime::new(RuntimeType::Claude)))
        .await
        .unwrap();

    // 模拟故障：主 Pool 达到容量上限
    for i in 0..5 {
        let config = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::Claude)
            .with_agent_config(test_agent_config(format!("primary-agent-{}", i)));
        let agent = primary_pool.acquire(config).await.unwrap();
        primary_pool.release(agent, true).await.unwrap();
    }

    // 尝试在主 Pool 上创建新 Agent（应该失败）
    let config = cis_core::agent::persistent::AgentAcquireConfig::new(RuntimeType::Claude)
        .with_agent_config(test_agent_config("overflow-agent"));
    let primary_result = primary_pool.acquire(config.clone()).await;
    assert!(primary_result.is_err());

    // 故障转移：在备 Pool 上创建 Agent
    let failover_agent = backup_pool.acquire(config).await.unwrap();
    assert_eq!(backup_pool.agent_count().await, 1);

    // 清理
    backup_pool.release(failover_agent, false).await.unwrap();
    primary_pool.shutdown_all().await.unwrap();
}

/// 测试分布式上下文共享概念
#[tokio::test]
async fn test_distributed_context_concept() {
    let _ctx = TestContext::new();

    // 创建共享的上下文存储模拟
    let shared_context: Arc<tokio::sync::RwLock<HashMap<String, String>>> =
        Arc::new(tokio::sync::RwLock::new(HashMap::new()));

    // 模拟 node-1 写入上下文
    {
        let mut ctx = shared_context.write().await;
        ctx.insert("task-1-output".to_string(), "Result from node-1".to_string());
    }

    // 模拟 node-2 读取上下文
    {
        let ctx = shared_context.read().await;
        let value = ctx.get("task-1-output");
        assert_eq!(value, Some(&"Result from node-1".to_string()));
    }

    // 模拟 node-2 更新上下文
    {
        let mut ctx = shared_context.write().await;
        ctx.insert("task-2-output".to_string(), "Result from node-2".to_string());
    }

    // 验证最终状态
    let ctx = shared_context.read().await;
    assert_eq!(ctx.len(), 2);
    assert!(ctx.contains_key("task-1-output"));
    assert!(ctx.contains_key("task-2-output"));
}
