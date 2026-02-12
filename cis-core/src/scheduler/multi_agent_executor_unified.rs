//! Multi-Agent DAG Executor 的 UnifiedDag 扩展
//!
//! 为 MultiAgentDagExecutor 添加对 UnifiedDag 的支持，
//! 保持向后兼容 TaskDag。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::agent::persistent::{
    AgentAcquireConfig, AgentHandle, AgentPool, RuntimeType as AgentRuntimeType,
    TaskRequest, TaskResult,
};
use crate::agent::cluster::context::ContextStore;
use crate::error::{CisError, Result};
use crate::scheduler::{
    DagNode, DagNodeStatus, DagScheduler, RuntimeType, TaskDag,
    DagRunStatus,
};
use super::multi_agent_executor::{
    MultiAgentDagExecutor, MultiAgentExecutorConfig,
    MultiAgentExecutionReport, TaskExecutionResult,
    SchedulingMode,
};
use super::converters::{
    UnifiedDag, UnifiedTask, AgentTaskConfig, DagValidationError,
};

/// UnifiedDag 扩展 trait
pub trait MultiAgentExecutorUnifiedExt {
    /// 创建 UnifiedDag 运行
    async fn create_unified_run(&self, dag: UnifiedDag) -> Result<String>;

    /// 执行 UnifiedDag 运行
    async fn execute_unified(&self, run_id: &str) -> Result<MultiAgentExecutionReport>;

    /// 创建并执行 UnifiedDag（便捷方法）
    async fn run_unified(&self, dag: UnifiedDag) -> Result<MultiAgentExecutionReport>;
}

impl MultiAgentExecutorUnifiedExt for MultiAgentDagExecutor {
    /// 创建 UnifiedDag 运行
    async fn create_unified_run(&self, dag: UnifiedDag) -> Result<String> {
        // 验证 DAG
        dag.validate().map_err(|e| CisError::validation(format!("Invalid DAG: {}", e)))?;

        // 转换为 TaskDag
        let task_dag = TaskDag::try_from(dag.clone())
            .map_err(|e| CisError::scheduler(format!("Failed to convert UnifiedDag to TaskDag: {}", e)))?;

        // 创建运行
        let mut scheduler = self.scheduler.write().await;
        let run_id = scheduler.create_run(task_dag);

        // 存储 UnifiedDag 元数据（用于后续查询）
        // 注意：实际实现中可能需要扩展 DagRun 来存储 metadata
        info!("Created UnifiedDag run: {} (name: {})", run_id, dag.metadata.name);

        Ok(run_id)
    }

    /// 执行 UnifiedDag 运行
    async fn execute_unified(&self, run_id: &str) -> Result<MultiAgentExecutionReport> {
        let start_time = std::time::Instant::now();
        info!(
            "Starting UnifiedDag execution for run {}",
            run_id
        );

        // 初始化 DAG
        {
            let mut scheduler = self.scheduler.write().await;
            let run = scheduler
                .get_run_mut(run_id)
                .ok_or_else(|| CisError::scheduler("Run not found"))?;
            run.dag.initialize();
            info!("Initialized UnifiedDag with {} tasks", run.dag.node_count());
        }

        // 选择调度模式
        match self.config.scheduling_mode {
            SchedulingMode::EventDriven => {
                self.execute_event_driven(run_id, start_time).await
            }
            SchedulingMode::Polling => {
                self.execute_polling(run_id, start_time).await
            }
        }
    }

    /// 创建并执行 UnifiedDag（便捷方法）
    async fn run_unified(&self, dag: UnifiedDag) -> Result<MultiAgentExecutionReport> {
        let run_id = self.create_unified_run(dag).await?;
        self.execute_unified(&run_id).await
    }
}

/// UnifiedTask 到 DagNode 的转换辅助函数
impl UnifiedTask {
    /// 转换为 DagNode（用于执行）
    pub fn to_dag_node(&self) -> DagNode {
        let (agent_runtime, reuse_agent, keep_agent) = if let Some(agent_config) = &self.agent_config {
            (
                Some(agent_config.runtime),
                agent_config.reuse_agent_id.clone(),
                agent_config.keep_agent,
            )
        } else {
            (None, None, false)
        };

        DagNode {
            task_id: self.id.clone(),
            dependencies: self.dependencies.clone(),
            dependents: Vec::new(), // 将在 add_node 时填充
            status: DagNodeStatus::Pending,
            level: self.level.clone(),
            rollback: self.rollback.clone(),
            agent_runtime,
            reuse_agent,
            keep_agent,
            agent_config: None, // 将在 execute_task 时使用
        }
    }
}

// ============================================================================
// 测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::converters::DagMetadata;
    use crate::types::TaskLevel;

    fn create_test_unified_dag() -> UnifiedDag {
        use serde_json::Map;

        UnifiedDag {
            metadata: DagMetadata {
                id: "test-unified-dag".to_string(),
                name: "Test Unified DAG".to_string(),
                description: Some("Test DAG for multi-agent executor".to_string()),
                version: "1.0.0".to_string(),
                created_at: None,
                author: None,
                tags: vec!["test".to_string()],
            },
            tasks: vec![
                UnifiedTask {
                    id: "task-1".to_string(),
                    name: Some("First Task".to_string()),
                    description: None,
                    skill: "git-diff".to_string(),
                    method: "execute".to_string(),
                    params: Map::new(),
                    dependencies: vec![],
                    level: TaskLevel::Mechanical { retry: 3 },
                    agent_config: Some(AgentTaskConfig {
                        runtime: RuntimeType::Claude,
                        reuse_agent_id: None,
                        keep_agent: false,
                        model: Some("claude-3-sonnet".to_string()),
                        system_prompt: None,
                        work_dir: None,
                    }),
                    rollback: None,
                    timeout_secs: Some(300),
                    retry: Some(3),
                    condition: None,
                    idempotent: false,
                    outputs: None,
                },
                UnifiedTask {
                    id: "task-2".to_string(),
                    name: Some("Second Task".to_string()),
                    description: None,
                    skill: "ai-review".to_string(),
                    method: "execute".to_string(),
                    params: Map::new(),
                    dependencies: vec!["task-1".to_string()],
                    level: TaskLevel::Confirmed,
                    agent_config: Some(AgentTaskConfig {
                        runtime: RuntimeType::Claude,
                        reuse_agent_id: None,
                        keep_agent: false,
                        model: Some("claude-3-opus".to_string()),
                        system_prompt: None,
                        work_dir: None,
                    }),
                    rollback: None,
                    timeout_secs: Some(600),
                    retry: None,
                    condition: None,
                    idempotent: false,
                    outputs: None,
                },
            ],
            execution_policy: crate::scheduler::converters::ExecutionPolicy::AllSuccess,
        }
    }

    #[test]
    fn test_unified_task_to_dag_node() {
        let task = UnifiedTask {
            id: "test-task".to_string(),
            name: Some("Test Task".to_string()),
            description: None,
            skill: "test-skill".to_string(),
            method: "execute".to_string(),
            params: Map::new(),
            dependencies: vec!["dep-1".to_string()],
            level: TaskLevel::Mechanical { retry: 2 },
            agent_config: Some(AgentTaskConfig {
                runtime: RuntimeType::Kimi,
                reuse_agent_id: Some("agent-123".to_string()),
                keep_agent: true,
                model: Some("kimi-latest".to_string()),
                system_prompt: None,
                work_dir: None,
            }),
            rollback: Some(vec!["rollback-cmd".to_string()]),
            timeout_secs: Some(100),
            retry: Some(2),
            condition: None,
            idempotent: false,
            outputs: None,
        };

        let node = task.to_dag_node();

        assert_eq!(node.task_id, "test-task");
        assert_eq!(node.dependencies, vec!["dep-1".to_string()]);
        assert_eq!(node.agent_runtime, Some(RuntimeType::Kimi));
        assert_eq!(node.reuse_agent, Some("agent-123".to_string()));
        assert!(node.keep_agent);
        assert_eq!(node.rollback, Some(vec!["rollback-cmd".to_string()]));
    }

    #[tokio::test]
    async fn test_create_unified_run() {
        // 注意：这个测试需要真实的 AgentPool 和 Scheduler
        // 在实际环境中需要 mock 这些依赖

        // let config = MultiAgentExecutorConfig::default();
        // let agent_pool = AgentPool::new(...);
        // let executor = MultiAgentDagExecutor::with_pool(agent_pool, config);

        // let dag = create_test_unified_dag();
        // let run_id = executor.create_unified_run(dag).await;

        // assert!(run_id.is_ok());
        // assert!(!run_id.unwrap().is_empty());
    }
}
