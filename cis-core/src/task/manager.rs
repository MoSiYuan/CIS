//! # Task Manager
//!
//! 智能任务分配和调度系统，整合 TaskRepository、DagBuilder、SessionRepository 等组件。

use super::dag::{Dag, DagBuilder};
use super::models::{
    AgentEntity, AgentSessionEntity, TaskEntity, TaskFilter, TaskPriority, TaskResult, TaskType,
};
use super::repository::TaskRepository;
use super::session::SessionRepository;
use crate::error::{CisError, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// 任务分配
#[derive(Debug, Clone)]
pub struct TaskAssignment {
    /// Team ID
    pub team_id: String,
    /// 任务 ID 列表
    pub task_ids: Vec<String>,
    /// 优先级
    pub priority: TaskPriority,
    /// 预估执行时长（秒）
    pub estimated_duration_secs: u64,
}

/// 层级分配
#[derive(Debug, Clone)]
pub struct LevelAssignment {
    /// 层级编号（0 = 根层级）
    pub level: u32,
    /// 该层级的所有任务分配
    pub assignments: Vec<TaskAssignment>,
}

/// 执行计划
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// DAG 结构
    pub dag: Dag,
    /// 按层级分配的任务
    pub levels: Vec<LevelAssignment>,
    /// 预估总执行时长（秒）
    pub estimated_total_duration_secs: u64,
}

/// 编排结果
#[derive(Debug, Clone)]
pub struct TaskOrchestrationResult {
    /// 执行计划
    pub plan: ExecutionPlan,
    /// 编排状态
    pub status: OrchestrationStatus,
}

/// 编排状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestrationStatus {
    /// 就绪
    Ready,
    /// 运行中
    Running,
    /// 已完成
    Completed,
    /// 失败
    Failed(String),
}

/// Task Manager - 核心任务管理和编排器
pub struct TaskManager {
    /// 任务仓储
    repository: Arc<TaskRepository>,
    /// DAG 构建器
    dag_builder: Arc<DagBuilder>,
    /// Session 仓储
    session_repo: Arc<SessionRepository>,
}

impl TaskManager {
    /// 创建新的 TaskManager
    pub fn new(repository: Arc<TaskRepository>, session_repo: Arc<SessionRepository>) -> Self {
        let dag_builder = Arc::new(DagBuilder::new(repository.clone()));
        Self {
            repository,
            dag_builder,
            session_repo,
        }
    }

    /// 创建并注册单个任务
    pub async fn create_task(&self, task: TaskEntity) -> Result<i64> {
        self.repository.create(&task).await.map_err(|e| {
            CisError::database("Failed to create task")
                .with_context("error", e.to_string())
        })
    }

    /// 批量创建任务
    pub async fn create_tasks_batch(&self, tasks: Vec<TaskEntity>) -> Result<Vec<i64>> {
        self.repository
            .batch_create(&tasks)
            .await
            .map_err(|e| {
                CisError::database("Failed to create tasks batch")
                    .with_context("error", e.to_string())
            })
    }

    /// 根据任务 ID 列表构建 DAG
    pub async fn build_dag(&self, task_ids: Vec<String>) -> Result<Dag> {
        self.dag_builder
            .build(&task_ids)
            .await
            .map_err(|e| CisError::scheduler("Failed to build DAG")
                .with_context("error", e.to_string()))
    }

    /// 智能任务分配到 Team
    ///
    /// 分析任务并分配给最合适的 Team
    pub async fn assign_tasks_to_teams(
        &self,
        task_ids: Vec<String>,
    ) -> Result<Vec<TaskAssignment>> {
        // 1. 获取所有任务
        let tasks = self.get_tasks_by_ids(&task_ids).await?;

        // 2. 按类型分组
        let mut team_assignments: HashMap<String, Vec<TaskEntity>> = HashMap::new();

        for task in tasks {
            let team = self.match_team_for_task(&task)?;
            team_assignments
                .entry(team.clone())
                .or_insert_with(Vec::new)
                .push(task);
        }

        // 3. 生成分配结果
        let mut assignments = Vec::new();
        for (team_id, team_tasks) in team_assignments {
            let assignment = TaskAssignment {
                team_id: team_id.clone(),
                task_ids: team_tasks.iter().map(|t| t.task_id.clone()).collect(),
                priority: self.calculate_team_priority(&team_tasks),
                estimated_duration_secs: self.estimate_team_duration(&team_tasks),
            };
            assignments.push(assignment);
        }

        Ok(assignments)
    }

    /// 匹配任务到合适的 Team
    ///
    /// 基于任务类型和名称智能匹配
    fn match_team_for_task(&self, task: &TaskEntity) -> Result<String> {
        match task.task_type {
            TaskType::ModuleRefactoring => {
                // 根据任务名称细分
                if task.name.contains("CLI") || task.name.contains("cli") {
                    Ok("Team-V-CLI".to_string())
                } else if task.name.contains("scheduler") || task.name.contains("core") {
                    Ok("Team-Q-Core".to_string())
                } else if task.name.contains("memory") {
                    Ok("Team-V-Memory".to_string())
                } else if task.name.contains("skill") {
                    Ok("Team-T-Skill".to_string())
                } else {
                    Ok("Team-U-Other".to_string())
                }
            }
            TaskType::EngineCodeInjection => {
                // 引擎代码注入通常由专门的 Team 处理
                if let Some(engine_type) = &task.engine_type {
                    match engine_type.as_str() {
                        "Unreal5.7" | "Unreal5.6" => Ok("Team-E-Unreal".to_string()),
                        "Unity" => Ok("Team-E-Unity".to_string()),
                        "Godot" => Ok("Team-E-Godot".to_string()),
                        _ => Ok("Team-E-Engine".to_string()),
                    }
                } else {
                    Ok("Team-E-Engine".to_string())
                }
            }
            TaskType::PerformanceOptimization => {
                // 性能优化任务
                if task.name.contains("database") || task.name.contains("storage") {
                    Ok("Team-Q-Core".to_string())
                } else if task.name.contains("network") || task.name.contains("p2p") {
                    Ok("Team-N-Network".to_string())
                } else {
                    Ok("Team-O-Optimization".to_string())
                }
            }
            TaskType::CodeReview => {
                // 代码审查根据模块
                if task.name.contains("CLI") || task.name.contains("cli") {
                    Ok("Team-V-CLI".to_string())
                } else if task.name.contains("scheduler") {
                    Ok("Team-Q-Core".to_string())
                } else {
                    Ok("Team-R-Review".to_string())
                }
            }
            TaskType::TestWriting => {
                // 测试编写
                Ok("Team-T-Test".to_string())
            }
            TaskType::Documentation => {
                // 文档编写
                Ok("Team-D-Docs".to_string())
            }
        }
    }

    /// 计算 Team 优先级
    ///
    /// 取所有任务中的最高优先级
    fn calculate_team_priority(&self, tasks: &[TaskEntity]) -> TaskPriority {
        tasks
            .iter()
            .map(|t| t.priority)
            .max_by_key(|p| match p {
                TaskPriority::P0 => 4,
                TaskPriority::P1 => 3,
                TaskPriority::P2 => 2,
                TaskPriority::P3 => 1,
            })
            .unwrap_or(TaskPriority::P3)
    }

    /// 估算 Team 执行时间
    ///
    /// 简单估算：基于任务数量和预估工作量
    fn estimate_team_duration(&self, tasks: &[TaskEntity]) -> u64 {
        let total_days: f64 = tasks
            .iter()
            .map(|t| t.estimated_effort_days.unwrap_or(1.0))
            .sum();

        // 1 人日 = 8 小时 = 28800 秒
        (total_days * 28800.0) as u64
    }

    /// 执行任务编排（完整流程）
    ///
    /// 这是 TaskManager 的核心方法，负责：
    /// 1. 构建 DAG
    /// 2. 拓扑排序
    /// 3. 按层级分配任务
    /// 4. 生成执行计划
    pub async fn orchestrate_tasks(&self, task_ids: Vec<String>) -> Result<TaskOrchestrationResult> {
        // 1. 构建 DAG
        let dag = self.build_dag(task_ids.clone()).await?;

        // 2. 拓扑排序
        let levels = dag.get_execution_levels();

        // 3. 按层级分配任务
        let mut all_assignments = Vec::new();
        for (level_idx, level_node_ids) in levels.iter().enumerate() {
            // 将 node IDs 转换为 task_ids
            let level_task_ids: Vec<String> = level_node_ids
                .iter()
                .filter_map(|&node_id| {
                    dag.nodes.get(&node_id).map(|node| node.task_id.clone())
                })
                .collect();

            if !level_task_ids.is_empty() {
                let assignments = self.assign_tasks_to_teams(level_task_ids).await?;
                all_assignments.push(LevelAssignment {
                    level: level_idx as u32,
                    assignments,
                });
            }
        }

        // 4. 生成执行计划
        let estimated_total_duration_secs =
            self.estimate_total_duration(&all_assignments);

        let plan = ExecutionPlan {
            dag,
            levels: all_assignments,
            estimated_total_duration_secs,
        };

        Ok(TaskOrchestrationResult {
            plan,
            status: OrchestrationStatus::Ready,
        })
    }

    /// 估算总执行时间
    ///
    /// 考虑层级间的时间依赖
    fn estimate_total_duration(&self, assignments: &[LevelAssignment]) -> u64 {
        // 总时间 = 各层级中最长时间之和（串行）
        // 每个层级内部是并行的，取最长的 Team
        assignments
            .iter()
            .map(|level| {
                level
                    .assignments
                    .iter()
                    .map(|a| a.estimated_duration_secs)
                    .max()
                    .unwrap_or(0)
            })
            .sum()
    }

    /// 获取任务列表
    async fn get_tasks_by_ids(&self, task_ids: &[String]) -> Result<Vec<TaskEntity>> {
        let mut tasks = Vec::new();
        for task_id in task_ids {
            let task = self
                .repository
                .get_by_task_id(task_id)
                .await
                .map_err(|e| {
                    CisError::database("Failed to get task")
                        .with_context("task_id", task_id)
                        .with_context("error", e.to_string())
                })?
                .ok_or_else(|| {
                    CisError::not_found("Task not found")
                        .with_context("task_id", task_id)
                })?;
            tasks.push(task);
        }
        Ok(tasks)
    }

    /// 更新任务状态
    pub async fn update_task_status(
        &self,
        task_id: i64,
        status: super::models::TaskStatus,
        error_message: Option<String>,
    ) -> Result<()> {
        self.repository
            .update_status(task_id, status, error_message)
            .await
            .map_err(|e| {
                CisError::database("Failed to update task status")
                    .with_context("error", e.to_string())
            })
    }

    /// 分配任务到 Team
    pub async fn assign_task_to_team(
        &self,
        task_id: i64,
        team_id: String,
        agent_id: Option<i64>,
    ) -> Result<()> {
        self.repository
            .update_assignment(task_id, Some(team_id), agent_id)
            .await
            .map_err(|e| {
                CisError::database("Failed to assign task to team")
                    .with_context("error", e.to_string())
            })
    }

    /// 更新任务执行结果
    pub async fn update_task_result(
        &self,
        task_id: i64,
        result: &TaskResult,
        duration_seconds: f64,
    ) -> Result<()> {
        self.repository
            .update_result(task_id, result, duration_seconds)
            .await
            .map_err(|e| {
                CisError::database("Failed to update task result")
                    .with_context("error", e.to_string())
            })
    }

    /// 查询任务
    pub async fn query_tasks(&self, filter: TaskFilter) -> Result<Vec<TaskEntity>> {
        self.repository
            .query(filter)
            .await
            .map_err(|e| {
                CisError::database("Failed to query tasks")
                    .with_context("error", e.to_string())
            })
    }

    /// 统计任务数量
    pub async fn count_tasks(&self, filter: TaskFilter) -> Result<i64> {
        self.repository
            .count(filter)
            .await
            .map_err(|e| {
                CisError::database("Failed to count tasks")
                    .with_context("error", e.to_string())
            })
    }

    /// 创建 Agent Session
    pub async fn create_session(
        &self,
        agent_id: i64,
        runtime_type: &str,
        context_capacity: i64,
        ttl_minutes: i64,
    ) -> Result<i64> {
        self.session_repo
            .create(agent_id, runtime_type, context_capacity, ttl_minutes)
            .await
            .map_err(|e| {
                CisError::database("Failed to create session")
                    .with_context("error", e.to_string())
            })
    }

    /// 获取可复用的 Session
    pub async fn acquire_session(
        &self,
        agent_id: i64,
        min_capacity: i64,
    ) -> Result<Option<AgentSessionEntity>> {
        self.session_repo
            .acquire_session(agent_id, min_capacity)
            .await
            .map_err(|e| {
                CisError::database("Failed to acquire session")
                    .with_context("error", e.to_string())
            })
    }

    /// 归还 Session
    pub async fn release_session(&self, session_id: i64) -> Result<()> {
        self.session_repo
            .release_session(session_id)
            .await
            .map_err(|e| {
                CisError::database("Failed to release session")
                    .with_context("error", e.to_string())
            })
    }

    /// 清理过期 Sessions
    pub async fn cleanup_expired_sessions(&self) -> Result<usize> {
        self.session_repo
            .cleanup_expired()
            .await
            .map_err(|e| {
                CisError::database("Failed to cleanup sessions")
                    .with_context("error", e.to_string())
            })
    }

    /// 获取可用的 Agents
    pub async fn get_available_agents(&self) -> Result<Vec<AgentEntity>> {
        // 注意：这里我们需要从 AgentRepository 获取
        // 但由于当前设计限制，我们通过 SessionRepository 间接访问
        // 实际实现中可能需要重构
        Ok(vec![]) // 暂时返回空，后续实现
    }

    /// 标记任务为运行中
    pub async fn mark_task_running(&self, task_id: i64) -> Result<()> {
        self.repository
            .mark_running(task_id)
            .await
            .map_err(|e| {
                CisError::database("Failed to mark task running")
                    .with_context("error", e.to_string())
            })
    }

    /// 删除任务
    pub async fn delete_task(&self, task_id: i64) -> Result<()> {
        self.repository
            .delete(task_id)
            .await
            .map_err(|e| {
                CisError::database("Failed to delete task")
                    .with_context("error", e.to_string())
            })
    }

    /// 批量删除任务
    pub async fn delete_tasks_batch(&self, task_ids: &[i64]) -> Result<usize> {
        self.repository
            .batch_delete(task_ids)
            .await
            .map_err(|e| {
                CisError::database("Failed to delete tasks batch")
                    .with_context("error", e.to_string())
            })
    }

    /// 搜索任务
    pub async fn search_tasks(&self, query: &str, limit: usize) -> Result<Vec<TaskEntity>> {
        self.repository
            .search(query, limit)
            .await
            .map_err(|e| {
                CisError::database("Failed to search tasks")
                    .with_context("error", e.to_string())
            })
    }

    /// 获取任务详情
    pub async fn get_task_by_id(&self, id: i64) -> Result<Option<TaskEntity>> {
        self.repository
            .get_by_id(id)
            .await
            .map_err(|e| {
                CisError::database("Failed to get task")
                    .with_context("error", e.to_string())
            })
    }

    /// 根据 task_id 获取任务
    pub async fn get_task_by_task_id(&self, task_id: &str) -> Result<Option<TaskEntity>> {
        self.repository
            .get_by_task_id(task_id)
            .await
            .map_err(|e| {
                CisError::database("Failed to get task")
                    .with_context("error", e.to_string())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::db::create_database_pool;
    use chrono::Utc;
    use tempfile::TempDir;

    /// 创建测试任务
    fn create_test_task(
        task_id: &str,
        name: &str,
        task_type: TaskType,
        priority: TaskPriority,
        dependencies: Vec<String>,
    ) -> TaskEntity {
        TaskEntity {
            id: 0,
            task_id: task_id.to_string(),
            name: name.to_string(),
            task_type,
            priority,
            prompt_template: "Test prompt".to_string(),
            context_variables: serde_json::json!({}),
            description: Some("Test description".to_string()),
            estimated_effort_days: Some(1.0),
            dependencies,
            engine_type: None,
            engine_context_id: None,
            status: super::models::TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            metadata: None,
            created_at_ts: Utc::now().timestamp(),
            updated_at_ts: Utc::now().timestamp(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_task() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let task = create_test_task(
            "test-1",
            "Test Task",
            TaskType::CodeReview,
            TaskPriority::P0,
            vec![],
        );

        let id = manager.create_task(task).await.unwrap();
        assert!(id > 0);

        let retrieved = manager.get_task_by_id(id).await.unwrap().unwrap();
        assert_eq!(retrieved.task_id, "test-1");
        assert_eq!(retrieved.name, "Test Task");
        assert_eq!(retrieved.priority, TaskPriority::P0);
    }

    #[tokio::test]
    async fn test_batch_create_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let tasks = vec![
            create_test_task("batch-1", "Batch Task 1", TaskType::CodeReview, TaskPriority::P0, vec![]),
            create_test_task("batch-2", "Batch Task 2", TaskType::TestWriting, TaskPriority::P1, vec![]),
            create_test_task("batch-3", "Batch Task 3", TaskType::Documentation, TaskPriority::P2, vec![]),
        ];

        let ids = manager.create_tasks_batch(tasks).await.unwrap();
        assert_eq!(ids.len(), 3);
        assert!(ids.iter().all(|&id| id > 0));
    }

    #[tokio::test]
    async fn test_team_matching_cli_task() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let task = create_test_task(
            "cli-task",
            "CLI handler refactoring",
            TaskType::ModuleRefactoring,
            TaskPriority::P0,
            vec![],
        );

        let team = manager.match_team_for_task(&task).unwrap();
        assert_eq!(team, "Team-V-CLI");
    }

    #[tokio::test]
    async fn test_team_matching_memory_task() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let task = create_test_task(
            "memory-task",
            "Memory module optimization",
            TaskType::ModuleRefactoring,
            TaskPriority::P1,
            vec![],
        );

        let team = manager.match_team_for_task(&task).unwrap();
        assert_eq!(team, "Team-V-Memory");
    }

    #[tokio::test]
    async fn test_team_matching_engine_task() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let mut task = create_test_task(
            "engine-task",
            "Unreal code injection",
            TaskType::EngineCodeInjection,
            TaskPriority::P0,
            vec![],
        );
        task.engine_type = Some("Unreal5.7".to_string());

        let team = manager.match_team_for_task(&task).unwrap();
        assert_eq!(team, "Team-E-Unreal");
    }

    #[tokio::test]
    async fn test_assign_tasks_to_teams() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        // 创建多个任务
        let task1 = create_test_task(
            "cli-1",
            "CLI refactoring",
            TaskType::ModuleRefactoring,
            TaskPriority::P0,
            vec![],
        );
        let task2 = create_test_task(
            "mem-1",
            "Memory optimization",
            TaskType::ModuleRefactoring,
            TaskPriority::P1,
            vec![],
        );

        manager.create_task(task1).await.unwrap();
        manager.create_task(task2).await.unwrap();

        // 分配任务
        let assignments = manager
            .assign_tasks_to_teams(vec!["cli-1".to_string(), "mem-1".to_string()])
            .await
            .unwrap();

        assert_eq!(assignments.len(), 2);

        // 验证分配
        let cli_assignment = assignments
            .iter()
            .find(|a| a.team_id == "Team-V-CLI")
            .unwrap();
        assert_eq!(cli_assignment.task_ids.len(), 1);
        assert_eq!(cli_assignment.priority, TaskPriority::P0);

        let mem_assignment = assignments
            .iter()
            .find(|a| a.team_id == "Team-V-Memory")
            .unwrap();
        assert_eq!(mem_assignment.task_ids.len(), 1);
        assert_eq!(mem_assignment.priority, TaskPriority::P1);
    }

    #[tokio::test]
    async fn test_orchestrate_simple_dag() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        // 创建独立任务
        let task1 = create_test_task(
            "task-1",
            "Independent Task 1",
            TaskType::CodeReview,
            TaskPriority::P0,
            vec![],
        );
        let task2 = create_test_task(
            "task-2",
            "Independent Task 2",
            TaskType::TestWriting,
            TaskPriority::P1,
            vec![],
        );

        manager.create_task(task1).await.unwrap();
        manager.create_task(task2).await.unwrap();

        // 编排任务
        let result = manager
            .orchestrate_tasks(vec!["task-1".to_string(), "task-2".to_string()])
            .await
            .unwrap();

        assert_eq!(result.status, OrchestrationStatus::Ready);
        assert_eq!(result.plan.levels.len(), 1); // 应该只有 1 个层级（并行）

        let level0 = &result.plan.levels[0];
        assert_eq!(level0.level, 0);
        assert!(!level0.assignments.is_empty());
    }

    #[tokio::test]
    async fn test_orchestrate_dependent_dag() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        // 创建有依赖的任务：task-2 依赖 task-1
        let task1 = create_test_task(
            "task-1",
            "Base Task",
            TaskType::ModuleRefactoring,
            TaskPriority::P0,
            vec![],
        );
        let task2 = create_test_task(
            "task-2",
            "Dependent Task",
            TaskType::ModuleRefactoring,
            TaskPriority::P1,
            vec!["task-1".to_string()],
        );

        manager.create_task(task1).await.unwrap();
        manager.create_task(task2).await.unwrap();

        // 编排任务
        let result = manager
            .orchestrate_tasks(vec!["task-1".to_string(), "task-2".to_string()])
            .await
            .unwrap();

        assert_eq!(result.status, OrchestrationStatus::Ready);
        assert_eq!(result.plan.levels.len(), 2); // 应该有 2 个层级（串行）

        // Level 0: task-1
        assert_eq!(result.plan.levels[0].level, 0);

        // Level 1: task-2
        assert_eq!(result.plan.levels[1].level, 1);
    }

    #[tokio::test]
    async fn test_query_tasks_with_filter() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        // 创建不同优先级的任务
        for i in 0..3 {
            let priority = match i {
                0 => TaskPriority::P0,
                1 => TaskPriority::P1,
                _ => TaskPriority::P2,
            };
            let task = create_test_task(
                &format!("query-{}", i),
                &format!("Query Task {}", i),
                TaskType::CodeReview,
                priority,
                vec![],
            );
            manager.create_task(task).await.unwrap();
        }

        // 查询 P0 任务
        let filter = TaskFilter {
            min_priority: Some(TaskPriority::P0),
            max_priority: Some(TaskPriority::P0),
            ..Default::default()
        };

        let results = manager.query_tasks(filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].priority, TaskPriority::P0);
    }

    #[tokio::test]
    async fn test_update_task_status() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let task = create_test_task(
            "status-test",
            "Status Test Task",
            TaskType::CodeReview,
            TaskPriority::P0,
            vec![],
        );

        let id = manager.create_task(task).await.unwrap();

        // 更新状态为运行中
        manager
            .update_task_status(id, super::models::TaskStatus::Running, None)
            .await
            .unwrap();

        let retrieved = manager.get_task_by_id(id).await.unwrap().unwrap();
        assert_eq!(retrieved.status, super::models::TaskStatus::Running);
    }

    #[tokio::test]
    async fn test_delete_task() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);
        let repo = Arc::new(TaskRepository::new(pool.clone()));
        let session_repo = Arc::new(SessionRepository::new(pool));
        let manager = TaskManager::new(repo, session_repo);

        let task = create_test_task(
            "delete-test",
            "Delete Test Task",
            TaskType::CodeReview,
            TaskPriority::P0,
            vec![],
        );

        let id = manager.create_task(task).await.unwrap();

        // 删除任务
        manager.delete_task(id).await.unwrap();

        // 验证删除
        let retrieved = manager.get_task_by_id(id).await.unwrap();
        assert!(retrieved.is_none());
    }
}
