//! # 测试工具模块
//!
//! 提供 Mock 实现和测试辅助函数

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use cis_core::agent::persistent::{
    AgentConfig, AgentInfo, AgentRuntime, AgentStatus, PersistentAgent, RuntimeType, TaskRequest,
    TaskResult,
};
use cis_core::scheduler::{DagNode, TaskDag, DagNodeStatus};
use cis_core::types::TaskLevel;

/// 创建测试用的 AgentConfig
pub fn test_agent_config(name: impl Into<String>) -> AgentConfig {
    AgentConfig::new(name, std::env::temp_dir().join(format!("test-{}", Uuid::new_v4())))
        .with_model("test-model")
        .with_system_prompt("You are a test agent")
        .with_timeout(60)
}

/// 创建测试用的 TaskRequest
pub fn test_task_request(task_id: impl Into<String>, prompt: impl Into<String>) -> TaskRequest {
    TaskRequest::new(task_id, prompt).with_timeout(60)
}

/// 创建简单的测试 DAG
pub fn create_test_dag() -> TaskDag {
    let mut dag = TaskDag::new();

    // 添加任务节点
    dag.add_node(
        "task-1".to_string(),
        vec![], // 无依赖
    )
    .unwrap();

    dag.add_node(
        "task-2".to_string(),
        vec!["task-1".to_string()], // 依赖 task-1
    )
    .unwrap();

    dag
}

/// 创建多 Agent 测试 DAG
pub fn create_multi_agent_dag() -> TaskDag {
    let mut dag = TaskDag::new();

    // Claude task
    dag.add_node("claude-task".to_string(), vec![]).unwrap();
    if let Some(node) = dag.get_node_mut("claude-task") {
        node.agent_runtime = Some(cis_core::scheduler::RuntimeType::Claude);
        node.keep_agent = true;
    }

    // OpenCode task (depends on claude-task)
    dag.add_node(
        "opencode-task".to_string(),
        vec!["claude-task".to_string()],
    )
    .unwrap();
    if let Some(node) = dag.get_node_mut("opencode-task") {
        node.agent_runtime = Some(cis_core::scheduler::RuntimeType::OpenCode);
    }

    dag
}

/// 创建带 Agent 复用的测试 DAG
pub fn create_reuse_agent_dag() -> TaskDag {
    let mut dag = TaskDag::new();

    // 第一个任务，创建并保留 Agent
    dag.add_node("task-1".to_string(), vec![]).unwrap();
    if let Some(node) = dag.get_node_mut("task-1") {
        node.agent_runtime = Some(cis_core::scheduler::RuntimeType::Claude);
        node.keep_agent = true;
    }

    // 第二个任务，复用第一个任务的 Agent
    dag.add_node("task-2".to_string(), vec!["task-1".to_string()]).unwrap();
    if let Some(node) = dag.get_node_mut("task-2") {
        node.agent_runtime = Some(cis_core::scheduler::RuntimeType::Claude);
        node.reuse_agent = Some("task-1".to_string());
    }

    dag
}

/// Mock Agent 状态
struct MockAgentState {
    status: AgentStatus,
    executed_tasks: Vec<String>,
}

/// Mock Agent 用于测试（不依赖外部 CLI）
pub struct MockAgent {
    agent_id: String,
    runtime_type: RuntimeType,
    state: Arc<RwLock<MockAgentState>>,
}

impl MockAgent {
    pub fn new(runtime_type: RuntimeType) -> Self {
        Self {
            agent_id: format!("mock-agent-{}", Uuid::new_v4()),
            runtime_type,
            state: Arc::new(RwLock::new(MockAgentState {
                status: AgentStatus::Idle,
                executed_tasks: vec![],
            })),
        }
    }

    #[allow(dead_code)]
    pub async fn executed_tasks(&self) -> Vec<String> {
        self.state.read().await.executed_tasks.clone()
    }
}

#[async_trait]
impl PersistentAgent for MockAgent {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn runtime_type(&self) -> RuntimeType {
        self.runtime_type
    }

    async fn execute(&self, task: TaskRequest) -> cis_core::Result<TaskResult> {
        // 模拟执行
        tokio::time::sleep(Duration::from_millis(50)).await;

        let mut state = self.state.write().await;
        state.executed_tasks.push(task.task_id.clone());
        state.status = AgentStatus::Busy;
        drop(state);

        // 模拟处理
        tokio::time::sleep(Duration::from_millis(50)).await;

        self.state.write().await.status = AgentStatus::Idle;

        Ok(TaskResult::success(
            task.task_id,
            format!("Executed: {}", task.prompt),
        ))
    }

    async fn status(&self) -> AgentStatus {
        self.state.read().await.status.clone()
    }

    async fn attach(&self) -> cis_core::Result<()> {
        Ok(())
    }

    async fn detach(&self) -> cis_core::Result<()> {
        Ok(())
    }

    async fn shutdown(&self) -> cis_core::Result<()> {
        self.state.write().await.status = AgentStatus::Shutdown;
        Ok(())
    }
}

/// Mock Runtime
pub struct MockRuntime {
    runtime_type: RuntimeType,
}

impl MockRuntime {
    pub fn new(runtime_type: RuntimeType) -> Self {
        Self { runtime_type }
    }
}

#[async_trait]
impl AgentRuntime for MockRuntime {
    fn runtime_type(&self) -> RuntimeType {
        self.runtime_type
    }

    async fn create_agent(
        &self,
        _config: AgentConfig,
    ) -> cis_core::Result<Box<dyn PersistentAgent>> {
        let agent = MockAgent::new(self.runtime_type);
        // 使用 config 中的名称，但保持 agent_id
        Ok(Box::new(agent))
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        vec![]
    }
}

/// 测试上下文，用于管理测试资源
pub struct TestContext {
    pub temp_dir: PathBuf,
}

impl TestContext {
    pub fn new() -> Self {
        let temp_dir = std::env::temp_dir().join(format!("cis-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        Self { temp_dir }
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        // 清理临时目录
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

/// 创建 DAG 节点辅助函数
#[allow(dead_code)]
pub fn create_dag_node(
    task_id: impl Into<String>,
    dependencies: Vec<String>,
    level: TaskLevel,
) -> DagNode {
    DagNode {
        task_id: task_id.into(),
        dependencies,
        dependents: vec![],
        status: DagNodeStatus::Pending,
        level,
        rollback: None,
        agent_runtime: None,
        reuse_agent: None,
        keep_agent: false,
        agent_config: None,
    }
}

/// 等待条件满足
#[allow(dead_code)]
pub async fn wait_for<F>(mut condition: F, timeout_ms: u64)
where
    F: FnMut() -> bool,
{
    let start = std::time::Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    while !condition() && start.elapsed() < timeout {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// 断言最终状态
#[macro_export]
macro_rules! assert_eventually {
    ($condition:expr, $timeout_ms:expr, $msg:expr) => {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis($timeout_ms);
        let mut last_result = false;

        while start.elapsed() < timeout {
            if $condition {
                last_result = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        assert!(last_result, $msg);
    };
}
