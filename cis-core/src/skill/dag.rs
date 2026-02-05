//! # Skill DAG Module
//!
//! 将 Skill Manifest 中的 DAG 定义转换为可执行的 Task DAG。
//!
//! ## 功能
//! - DagDefinition 与 TaskDag 的相互转换
//! - Skill DAG 构建器
//! - 输入输出参数传递

use std::collections::HashMap;

use crate::scheduler::{DagError, TaskDag};
use crate::skill::manifest::{DagDefinition, DagPolicy, DagTaskDefinition, TaskLevelDefinition};
use crate::types::{Action, Task, TaskLevel};

/// Skill DAG 上下文
///
/// 存储 DAG 执行过程中的中间结果和全局参数
#[derive(Debug, Clone, Default)]
pub struct SkillDagContext {
    /// 全局输入参数（从外部传入）
    pub global_inputs: serde_json::Value,
    /// 中间结果缓存（task_id -> output）
    pub intermediate_results: HashMap<String, serde_json::Value>,
    /// 父 Skill ID（如果是嵌套 DAG）
    pub parent_skill: Option<String>,
}

impl SkillDagContext {
    /// 创建新的上下文
    pub fn new(inputs: serde_json::Value) -> Self {
        Self {
            global_inputs: inputs,
            intermediate_results: HashMap::new(),
            parent_skill: None,
        }
    }

    /// 存储任务结果
    pub fn store_result(&mut self, task_id: &str, output: serde_json::Value) {
        self.intermediate_results
            .insert(task_id.to_string(), output);
    }

    /// 获取任务结果
    pub fn get_result(&self, task_id: &str) -> Option<&serde_json::Value> {
        self.intermediate_results.get(task_id)
    }

    /// 准备 Skill 输入参数
    ///
    /// 合并全局输入和上游任务输出
    pub fn prepare_skill_inputs(&self, task: &DagTaskDefinition) -> serde_json::Value {
        let mut inputs = self.global_inputs.clone();

        // 如果有依赖，将依赖的输出合并到输入
        for dep_id in &task.deps {
            if let Some(dep_output) = self.get_result(dep_id) {
                if let Some(obj) = inputs.as_object_mut() {
                    obj.insert(format!("_{}_output", dep_id), dep_output.clone());
                }
            }
        }

        inputs
    }
}

/// Skill DAG 转换器
pub struct SkillDagConverter;

impl SkillDagConverter {
    /// 将 DagDefinition 转换为 TaskDag
    pub fn to_task_dag(dag_def: &DagDefinition) -> Result<TaskDag, DagError> {
        let mut task_dag = TaskDag::new();

        // 添加所有任务节点
        for task_def in &dag_def.tasks {
            let level = Self::convert_level(&task_def.level);
            let deps = task_def.deps.clone();
            let rollback = if task_def.rollback.is_empty() {
                None
            } else {
                Some(task_def.rollback.clone())
            };
            task_dag.add_node_with_rollback(task_def.id.clone(), deps, level, rollback)?;
        }

        // 验证 DAG 无环
        task_dag.validate()?;

        Ok(task_dag)
    }

    /// 转换单个任务
    pub fn convert_task(task_def: &DagTaskDefinition) -> Task {
        let level = Self::convert_level(&task_def.level);

        let mut task = Task::new(
            task_def.id.clone(),
            task_def.name.clone(),
            "dag".to_string(),
        )
        .with_skill(&task_def.skill);

        // 设置任务级别
        task.level = level;

        // 设置回滚命令
        if !task_def.rollback.is_empty() {
            task.rollback = Some(task_def.rollback.clone());
        }

        // 设置幂等性
        task.idempotent = task_def.idempotent;

        task
    }

    /// 转换决策级别
    fn convert_level(level_def: &TaskLevelDefinition) -> TaskLevel {
        match level_def {
            TaskLevelDefinition::Mechanical { retry } => {
                TaskLevel::Mechanical { retry: *retry }
            }
            TaskLevelDefinition::Recommended { timeout, default_action } => {
                let action = match default_action.as_str() {
                    "skip" => Action::Skip,
                    "abort" => Action::Abort,
                    _ => Action::Execute,
                };
                TaskLevel::Recommended {
                    default_action: action,
                    timeout_secs: *timeout,
                }
            }
            TaskLevelDefinition::Confirmed => TaskLevel::Confirmed,
            TaskLevelDefinition::Arbitrated { stakeholders } => TaskLevel::Arbitrated {
                stakeholders: stakeholders.clone(),
            },
        }
    }

    /// 将 DagPolicy 转换为执行策略字符串
    pub fn policy_str(policy: &DagPolicy) -> &'static str {
        match policy {
            DagPolicy::AllSuccess => "all_success",
            DagPolicy::FirstSuccess => "first_success",
            DagPolicy::AllowDebt => "allow_debt",
        }
    }
}

/// Skill DAG 构建器（Builder 模式）
#[derive(Debug, Default)]
pub struct SkillDagBuilder {
    policy: DagPolicy,
    tasks: Vec<DagTaskDefinition>,
}

impl SkillDagBuilder {
    /// 创建新的构建器
    pub fn new() -> Self {
        Self {
            policy: DagPolicy::AllSuccess,
            tasks: Vec::new(),
        }
    }

    /// 设置执行策略
    pub fn policy(mut self, policy: DagPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// 添加任务
    pub fn task(mut self, task: DagTaskDefinition) -> Self {
        self.tasks.push(task);
        self
    }

    /// 添加 Mechanical 级别的任务
    pub fn mechanical_task(
        self,
        id: impl Into<String>,
        skill: impl Into<String>,
        retry: u8,
    ) -> Self {
        let task = DagTaskDefinition::new(id, skill).mechanical(retry);
        self.task(task)
    }

    /// 添加 Confirmed 级别的任务
    pub fn confirmed_task(self, id: impl Into<String>, skill: impl Into<String>) -> Self {
        let task = DagTaskDefinition::new(id, skill).confirmed();
        self.task(task)
    }

    /// 构建 DagDefinition
    pub fn build(self) -> DagDefinition {
        DagDefinition {
            policy: self.policy,
            tasks: self.tasks,
        }
    }
}

/// Skill DAG 执行统计
#[derive(Debug, Clone, Default)]
pub struct SkillDagStats {
    /// 总任务数
    pub total_tasks: usize,
    /// 已完成任务数
    pub completed_tasks: usize,
    /// 失败任务数
    pub failed_tasks: usize,
    /// 跳过的任务数（因依赖失败）
    pub skipped_tasks: usize,
    /// 累积的债务数
    pub debt_count: usize,
    /// 总执行时间（毫秒）
    pub total_duration_ms: u64,
}

impl SkillDagStats {
    /// 计算完成百分比
    pub fn completion_percentage(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        (self.completed_tasks as f32 / self.total_tasks as f32) * 100.0
    }

    /// 是否全部完成
    pub fn is_complete(&self) -> bool {
        self.completed_tasks + self.failed_tasks + self.skipped_tasks >= self.total_tasks
    }

    /// 是否有失败
    pub fn has_failures(&self) -> bool {
        self.failed_tasks > 0 || self.skipped_tasks > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_dag_builder() {
        let dag = SkillDagBuilder::new()
            .policy(DagPolicy::AllSuccess)
            .mechanical_task("1", "git-diff", 3)
            .confirmed_task("2", "ai-analyze")
            .mechanical_task("3", "report-gen", 3)
            .build();

        assert_eq!(dag.tasks.len(), 3);
        assert!(matches!(dag.policy, DagPolicy::AllSuccess));
    }

    #[test]
    fn test_dag_context() {
        let mut ctx = SkillDagContext::new(serde_json::json!({
            "repo_path": "/tmp/repo"
        }));

        ctx.store_result("1", serde_json::json!({ "diff": "..." }));

        assert!(ctx.get_result("1").is_some());
        assert_eq!(ctx.global_inputs["repo_path"], "/tmp/repo");
    }

    #[test]
    fn test_skill_dag_converter() {
        let dag_def = SkillDagBuilder::new()
            .mechanical_task("1", "git-diff", 3)
            .mechanical_task("2", "ai-analyze", 3)
            .task(
                DagTaskDefinition::new("3", "report-gen")
                    .with_deps(vec!["1", "2"])
                    .confirmed(),
            )
            .build();

        let task_dag = SkillDagConverter::to_task_dag(&dag_def).unwrap();
        assert_eq!(task_dag.node_count(), 3);
    }

    #[test]
    fn test_dag_stats() {
        let stats = SkillDagStats {
            total_tasks: 10,
            completed_tasks: 5,
            failed_tasks: 2,
            skipped_tasks: 1,
            debt_count: 1,
            total_duration_ms: 1000,
        };

        assert_eq!(stats.completion_percentage(), 50.0);
        assert!(!stats.is_complete());
        assert!(stats.has_failures());

        let complete_stats = SkillDagStats {
            total_tasks: 10,
            completed_tasks: 8,
            failed_tasks: 2,
            skipped_tasks: 0,
            ..Default::default()
        };
        assert!(complete_stats.is_complete());
    }

    #[test]
    fn test_convert_task() {
        let mut task_def = DagTaskDefinition::new("test-1", "test-skill")
            .mechanical(5)
            .with_rollback(vec!["rollback-cmd"]);
        task_def.idempotent = true;

        let task = SkillDagConverter::convert_task(&task_def);

        assert_eq!(task.id, "test-1");
        assert!(task.is_skill_task());
        assert_eq!(task.skill_id(), Some("test-skill"));
        assert!(task.idempotent);
        assert!(task.rollback.is_some());
    }

    #[test]
    fn test_policy_str() {
        assert_eq!(
            SkillDagConverter::policy_str(&DagPolicy::AllSuccess),
            "all_success"
        );
        assert_eq!(
            SkillDagConverter::policy_str(&DagPolicy::FirstSuccess),
            "first_success"
        );
        assert_eq!(
            SkillDagConverter::policy_str(&DagPolicy::AllowDebt),
            "allow_debt"
        );
    }
}
