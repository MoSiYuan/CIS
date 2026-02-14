//! Skill DAG Executor 的 UnifiedDag 扩展
//!
//! 为 SkillDagExecutor 添加对 UnifiedDag 的支持，
//! 保持向后兼容 DagDefinition。

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::decision::{DecisionEngine, DecisionResult};
use crate::error::{CisError, Result};
use crate::scheduler::{
    DagScheduler, DagTask, DagRunStatus,
    DagNode, DagNodeStatus, RuntimeType, TaskDag,
    PermissionResult,
};
use crate::skill::SkillManager;
use crate::types::{Action, FailureType, SkillExecutionResult, Task, TaskLevel};

use super::skill_executor::{SkillDagExecutor, UserInput};
use super::converters::{UnifiedDag, UnifiedTask, DagValidationError, ExecutionPolicy};

/// UnifiedDag 扩展 trait for SkillDagExecutor
pub trait SkillExecutorUnifiedExt {
    /// 执行 UnifiedDag
    async fn execute_unified_dag(
        &mut self,
        dag: UnifiedDag,
        inputs: Value,
    ) -> Result<Value>;

    /// 从文件加载并执行 UnifiedDag
    async fn load_and_execute_unified_dag(
        &mut self,
        path: &std::path::Path,
        inputs: Value,
    ) -> Result<Value>;
}

impl SkillExecutorUnifiedExt for SkillDagExecutor {
    /// 执行 UnifiedDag
    async fn execute_unified_dag(
        &mut self,
        dag: UnifiedDag,
        inputs: Value,
    ) -> Result<Value> {
        // 1. 验证 DAG
        dag.validate()
            .map_err(|e| CisError::validation(format!("Invalid UnifiedDag: {}", e)))?;

        info!(
            "Executing UnifiedDag: {} (version: {})",
            dag.metadata.name,
            dag.metadata.version
        );

        // 2. 转换为 TaskDag
        let task_dag = TaskDag::try_from(dag.clone())
            .map_err(|e| CisError::scheduler(format!("Failed to convert UnifiedDag: {}", e)))?;

        // 3. 创建 DAG 运行
        let run_id = self.scheduler.create_run(task_dag);

        // 4. 注入全局输入到第一个任务
        self.inject_inputs(&run_id, &inputs)?;

        // 5. 执行 DAG 循环
        loop {
            // 获取就绪任务
            let ready_tasks = self.scheduler.get_ready_tasks(&run_id)?;

            if ready_tasks.is_empty() {
                // 检查是否完成
                if self.scheduler.is_completed(&run_id)? {
                    break;
                }
                // 等待任务完成或新任务就绪
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                continue;
            }

            // 处理每个就绪任务
            for task_id in ready_tasks {
                let task = self.scheduler.get_task(&run_id, &task_id)?;

                // 查找对应的 UnifiedTask（获取额外配置）
                let unified_task = dag.tasks.iter()
                    .find(|t| t.id == task_id);

                // 四级决策检查 - 使用决策引擎
                match self.decision_engine.process_decision(&task, &run_id).await {
                    DecisionResult::Allow => {
                        // 直接执行
                    }
                    DecisionResult::Skip => {
                        // 跳过任务
                        warn!("Task '{}' skipped by decision engine", task_id);
                        self.scheduler.mark_skipped(&run_id, &task_id)
                            .map_err(|e| CisError::scheduler(format!("Failed to mark skipped: {}", e)))?;
                        continue;
                    }
                    DecisionResult::Abort => {
                        // 中止执行
                        warn!("Task '{}' aborted by decision engine", task_id);
                        return Err(CisError::execution(format!(
                            "Task '{}' aborted by decision engine", task_id
                        )));
                    }
                    DecisionResult::Pending(request_id) => {
                        // 等待异步决策结果
                        info!("Task '{}' waiting for decision: {}", task_id, request_id);
                        // 实际实现中应该在这里等待异步结果
                    }
                }

                // 执行 Skill
                if let Some(skill_id) = task.skill_id() {
                    let skill_inputs = self.prepare_skill_inputs(&task)?;

                    // 合并 UnifiedTask 的 params
                    let final_inputs = if let Some(unified_task) = unified_task {
                        self.merge_inputs(&skill_inputs, &unified_task.params)?
                    } else {
                        skill_inputs
                    };

                    match self.execute_skill(skill_id, final_inputs).await {
                        Ok(result) => {
                            if result.success {
                                self.scheduler.mark_completed(&run_id, &task_id)?;
                                if let Some(output) = result.output {
                                    self.store_result(&run_id, &task_id, output)?;
                                }
                            } else {
                                // 判断失败类型
                                let failure_type = self.classify_error(&result);
                                self.scheduler.mark_failed_with_type(
                                    &run_id,
                                    &task_id,
                                    failure_type,
                                    result.error.unwrap_or_default(),
                                ).map_err(|e| CisError::scheduler(format!("Failed to mark failed: {}", e)))?;
                            }
                        }
                        Err(e) => {
                            self.scheduler
                                .mark_failed_with_type(&run_id, &task_id, FailureType::Blocking, e.to_string())
                                .map_err(|e| CisError::scheduler(format!("Failed to mark failed: {}", e)))?;
                        }
                    }
                }
            }
        }

        // 6. 收集最终输出
        self.collect_outputs(&run_id)
    }

    /// 从文件加载并执行 UnifiedDag
    async fn load_and_execute_unified_dag(
        &mut self,
        path: &std::path::Path,
        inputs: Value,
    ) -> Result<Value> {
        // 读取文件内容
        let content = tokio::fs::read_to_string(path).await
            .map_err(|e| CisError::io(format!("Failed to read DAG file: {}", e)))?;

        // 检测文件格式并解析
        let dag: UnifiedDag = match path.extension().and_then(|e| e.to_str()) {
            Some("toml") => {
                toml::from_str(&content)
                    .map_err(|e| CisError::validation(format!("Failed to parse TOML: {}", e)))?
            }
            Some("json") => {
                serde_json::from_str(&content)
                    .map_err(|e| CisError::validation(format!("Failed to parse JSON: {}", e)))?
            }
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(&content)
                    .map_err(|e| CisError::validation(format!("Failed to parse YAML: {}", e)))?
            }
            _ => {
                return Err(CisError::validation(format!(
                    "Unsupported DAG file format: {:?}",
                    path.extension()
                )));
            }
        };

        // 执行 DAG
        self.execute_unified_dag(dag, inputs).await
    }
}

impl SkillDagExecutor {
    /// 合并输入参数
    fn merge_inputs(&self, base: &Value, params: &serde_json::Map<String, Value>) -> Result<Value> {
        match base {
            Value::Object(base_map) => {
                let mut merged = base_map.clone();
                for (key, value) in params {
                    merged.insert(key.clone(), value.clone());
                }
                Ok(Value::Object(merged))
            }
            _ => {
                // 如果 base 不是对象，直接使用 params
                Ok(Value::Object(params.clone()))
            }
        }
    }

    /// 超时时间（从 UnifiedTask 或使用默认值）
    fn get_task_timeout(&self, unified_task: Option<&UnifiedTask>) -> u64 {
        unified_task
            .and_then(|t| t.timeout_secs)
            .unwrap_or(300) // 默认 5 分钟
    }
}

// ============================================================================
// 从 TaskDag 到 UnifiedDag 的辅助函数
// ============================================================================

impl UnifiedDag {
    /// 从 Skill manifest 的 DagDefinition 创建
    ///
    /// 这是一个便捷方法，用于从现有的 Skill DAG 定义创建 UnifiedDag
    pub fn from_skill_manifest(
        id: String,
        name: String,
        tasks: Vec<DagTask>,
    ) -> Self {
        let unified_tasks = tasks.into_iter()
            .map(|dag_task| UnifiedTask {
                id: dag_task.task_id.clone(),
                name: Some(dag_task.task_id.clone()),
                description: None,
                skill: dag_task.skill_id.unwrap_or_default(),
                method: "execute".to_string(),
                params: serde_json::Map::new(),
                dependencies: dag_task.dependencies,
                level: dag_task.level,
                agent_config: if let Some(runtime) = dag_task.agent_runtime {
                    Some(crate::scheduler::converters::AgentTaskConfig {
                        runtime,
                        reuse_agent_id: dag_task.reuse_agent,
                        keep_agent: dag_task.keep_agent,
                        model: dag_task.agent_config.as_ref().and_then(|c| c.model.clone()),
                        system_prompt: None,
                        work_dir: None,
                    })
                } else {
                    None
                },
                rollback: None,
                timeout_secs: None,
                retry: None,
                condition: None,
                idempotent: false,
                outputs: None,
            })
            .collect();

        Self {
            metadata: crate::scheduler::converters::DagMetadata {
                id,
                name,
                description: None,
                version: "1.0.0".to_string(),
                created_at: Some(chrono::Utc::now()),
                author: None,
                tags: vec!["skill-manifest".to_string()],
            },
            tasks: unified_tasks,
            execution_policy: ExecutionPolicy::AllSuccess,
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
    use serde_json::json;

    fn create_test_skill_dag_executor() -> SkillDagExecutor {
        use crate::storage::db::DbManager;

        let db_manager = Arc::new(DbManager::new().unwrap());
        let skill_manager = Arc::new(SkillManager::new(db_manager).unwrap());
        let scheduler = DagScheduler::new();

        SkillDagExecutor::new(scheduler, skill_manager)
    }

    fn create_test_unified_dag() -> UnifiedDag {
        use serde_json::Map;

        UnifiedDag {
            metadata: DagMetadata {
                id: "test-skill-dag".to_string(),
                name: "Test Skill DAG".to_string(),
                description: Some("Test DAG for skill executor".to_string()),
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
                    skill: "echo".to_string(),
                    method: "execute".to_string(),
                    params: {
                        let mut map = Map::new();
                        map.insert("message".to_string(), json!("Hello World"));
                        map
                    },
                    dependencies: vec![],
                    level: TaskLevel::Mechanical { retry: 3 },
                    agent_config: None,
                    rollback: None,
                    timeout_secs: Some(60),
                    retry: Some(3),
                    condition: None,
                    idempotent: true,
                    outputs: None,
                },
            ],
            execution_policy: ExecutionPolicy::AllSuccess,
        }
    }

    #[test]
    fn test_merge_inputs() {
        let executor = create_test_skill_dag_executor();

        let base = json!({
            "key1": "value1",
            "key2": "old_value"
        });

        let params = {
            let mut map = Map::new();
            map.insert("key2".to_string(), json!("new_value"));
            map.insert("key3".to_string(), json!("value3"));
            map
        };

        let merged = executor.merge_inputs(&base, &params).unwrap();

        assert_eq!(
            merged,
            json!({
                "key1": "value1",
                "key2": "new_value",
                "key3": "value3"
            })
        );
    }

    #[test]
    fn test_get_task_timeout() {
        let executor = create_test_skill_dag_executor();

        let unified_task = UnifiedTask {
            timeout_secs: Some(120),
            ..Default::default()
        };

        let timeout = executor.get_task_timeout(Some(&unified_task));
        assert_eq!(timeout, 120);

        let default_timeout = executor.get_task_timeout(None);
        assert_eq!(default_timeout, 300);
    }

    #[test]
    fn test_unified_dag_from_skill_manifest() {
        use crate::types::TaskLevel;

        let tasks = vec![
            DagTask {
                task_id: "task-1".to_string(),
                dependencies: vec![],
                skill_id: Some("test-skill".to_string()),
                level: TaskLevel::Mechanical { retry: 3 },
                agent_runtime: None,
                reuse_agent: None,
                keep_agent: false,
                agent_config: None,
            },
        ];

        let dag = UnifiedDag::from_skill_manifest(
            "test-dag".to_string(),
            "Test DAG".to_string(),
            tasks,
        );

        assert_eq!(dag.metadata.id, "test-dag");
        assert_eq!(dag.metadata.name, "Test DAG");
        assert_eq!(dag.tasks.len(), 1);
        assert_eq!(dag.tasks[0].id, "task-1");
        assert_eq!(dag.tasks[0].skill, "test-skill");
    }
}
