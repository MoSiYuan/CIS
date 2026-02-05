//! Skill DAG Executor
//!
//! 统一执行 Skill 的执行器，支持：
//! - 原子 Skill（Binary/WASM）直接执行
//! - 复合 Skill（DAG）递归执行
//! - 四级决策检查
//! - 债务累积

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tracing::{debug, info, warn};

use crate::error::{CisError, Result};
use crate::scheduler::{DagScheduler, PermissionResult};
use crate::skill::manifest::DagDefinition;
use crate::skill::types::SkillType;
use crate::skill::SkillManager;
use crate::types::{Action, FailureType, SkillExecutionResult, Task, TaskLevel};

/// Skill DAG 执行器
pub struct SkillDagExecutor {
    /// DAG 调度器
    scheduler: DagScheduler,
    /// Skill 管理器
    skill_manager: Arc<SkillManager>,
    /// 执行上下文（用于存储中间结果）
    context: HashMap<String, Value>,
}

impl SkillDagExecutor {
    /// 创建新的执行器
    pub fn new(scheduler: DagScheduler, skill_manager: Arc<SkillManager>) -> Self {
        Self {
            scheduler,
            skill_manager,
            context: HashMap::new(),
        }
    }

    /// 执行单个 Skill（入口方法）
    ///
    /// 根据 Skill 类型决定执行方式：
    /// - Binary: 直接执行二进制
    /// - Wasm: WASM 运行时执行
    /// - Dag: 递归执行 DAG
    pub async fn execute_skill(
        &mut self,
        skill_id: &str,
        inputs: Value,
    ) -> Result<SkillExecutionResult> {
        let skill_info = self
            .skill_manager
            .get_info(skill_id)?
            .ok_or_else(|| CisError::skill_not_found(skill_id))?;

        let start_time = std::time::Instant::now();

        let result = match skill_info.meta.skill_type {
            SkillType::Native => {
                let path = std::path::PathBuf::from(&skill_info.meta.path);
                self.execute_binary(&path, inputs).await
            }
            SkillType::Wasm => {
                // WASM 执行需要读取 WASM 文件
                let wasm_path = std::path::PathBuf::from(&skill_info.meta.path);
                let wasm_bytes = tokio::fs::read(&wasm_path).await?;
                self.execute_wasm(&wasm_bytes, inputs).await
            }
            SkillType::Remote => {
                // 远程 Skill 暂未实现
                Err(CisError::skill("Remote skill execution not yet implemented"))
            }
            SkillType::Dag => {
                // DAG Skill 执行 - 需要加载 DAG 定义并递归执行
                Err(CisError::skill("DAG skill execution not yet implemented - use execute_dag_skill"))
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(output) => Ok(SkillExecutionResult {
                success: true,
                output: Some(output),
                error: None,
                duration_ms,
            }),
            Err(e) => Ok(SkillExecutionResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                duration_ms,
            }),
        }
    }

    /// 执行二进制 Skill
    async fn execute_binary(
        &self,
        path: &std::path::Path,
        inputs: Value,
    ) -> Result<Value> {
        // 1. 创建临时输入文件
        let input_file = tokio::task::spawn_blocking(|| {
            tempfile::NamedTempFile::with_suffix(".json")
        })
        .await
        .map_err(|e| CisError::execution(format!("Failed to spawn blocking task: {}", e)))??;

        let input_path = input_file.path().to_path_buf();

        // 写入输入数据
        let input_data = serde_json::to_vec(&inputs)?;
        tokio::fs::write(&input_path, &input_data).await?;

        // 2. 执行二进制（带超时）
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5 分钟超时
            tokio::process::Command::new(path)
                .arg(&input_path)
                .output(),
        )
        .await
        .map_err(|_| CisError::execution("Binary execution timeout"))?
        .map_err(|e| CisError::execution(format!("Failed to execute binary: {}", e)))?;

        // 3. 读取输出
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CisError::execution(format!(
                "Binary execution failed: {}",
                stderr
            )));
        }

        // 4. 解析输出为 JSON
        let stdout = String::from_utf8_lossy(&output.stdout);
        let result: Value = serde_json::from_str(&stdout)
            .map_err(|e| CisError::execution(format!("Failed to parse output: {}", e)))?;

        Ok(result)
    }

    /// 执行 WASM Skill
    async fn execute_wasm(
        &self,
        _wasm_bytes: &[u8],
        _inputs: Value,
    ) -> Result<Value> {
        // WASM 执行需要集成 wasmtime 或其他 WASM 运行时
        // 这是一个复杂的功能，需要:
        // 1. 添加 wasmtime 依赖
        // 2. 实现 WASM 模块加载和实例化
        // 3. 实现 host function 绑定
        // 4. 实现输入/输出序列化
        //
        // 当前返回错误提示用户使用 native skill
        Err(CisError::skill("WASM execution not yet implemented. Please use native skill type for now."))
    }

    /// 执行 DAG 类型的 Skill（核心方法）
    async fn execute_dag_skill(
        &mut self,
        dag_def: &DagDefinition,
        inputs: Value,
    ) -> Result<Value> {
        // 1. 创建 DAG 运行
        let run_id = self.scheduler.create_run(dag_def.to_dag()?);

        // 2. 注入全局输入到第一个任务
        self.inject_inputs(&run_id, &inputs)?;

        // 3. 执行 DAG 循环
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

                // 四级决策检查
                match self.check_permission(&task).await? {
                    PermissionResult::AutoApprove => {
                        // 直接执行
                    }
                    PermissionResult::Countdown {
                        seconds,
                        default_action,
                    } => {
                        // 倒计时后执行
                        self.wait_countdown(seconds, default_action).await?;
                    }
                    PermissionResult::NeedsConfirmation => {
                        // 等待确认
                        self.wait_confirmation(&task).await?;
                    }
                    PermissionResult::NeedsArbitration { stakeholders: _ } => {
                        // 暂停等待仲裁 - 简化处理，直接继续
                        println!("⚠️ Arbitration required but not implemented, continuing...");
                    }
                }

                // 执行 Skill
                if let Some(skill_id) = task.skill_id() {
                    let skill_inputs = self.prepare_skill_inputs(&task)?;

                    match self.execute_skill(skill_id, skill_inputs).await {
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

        // 4. 收集最终输出
        self.collect_outputs(&run_id)
    }

    /// 检查任务权限（四级决策）
    async fn check_permission(&self, task: &Task) -> Result<PermissionResult> {
        match &task.level {
            TaskLevel::Mechanical { .. } => Ok(PermissionResult::AutoApprove),
            TaskLevel::Recommended {
                default_action,
                timeout_secs,
            } => Ok(PermissionResult::Countdown {
                seconds: *timeout_secs,
                default_action: *default_action,
            }),
            TaskLevel::Confirmed => Ok(PermissionResult::NeedsConfirmation),
            TaskLevel::Arbitrated { stakeholders } => Ok(PermissionResult::NeedsArbitration {
                stakeholders: stakeholders.clone(),
            }),
        }
    }

    /// 等待倒计时
    async fn wait_countdown(&self, seconds: u16, default_action: Action) -> Result<()> {
        match default_action {
            Action::Execute => {
                tokio::time::sleep(std::time::Duration::from_secs(seconds as u64)).await;
                Ok(())
            }
            Action::Skip => Err(CisError::execution("Task skipped by countdown default")),
            Action::Abort => Err(CisError::execution("Task aborted by countdown default")),
        }
    }

    /// 等待用户确认
    /// 
    /// 实现方式：
    /// 1. 在 Matrix Room 中发送确认请求消息
    /// 2. 等待用户回复（有超时）
    /// 3. 根据用户回复决定继续或取消
    /// 
    /// 注意：当前实现为简化版，直接继续。
    /// 完整实现需要集成 Matrix 客户端来收发确认消息。
    async fn wait_confirmation(&self, task: &Task) -> Result<()> {
        info!("Task '{}' requires user confirmation", task.id);
        
        // 在实际实现中，这里应该：
        // 1. 发送确认请求到 Matrix Room
        // 2. 设置超时等待用户响应
        // 3. 根据响应决定继续或取消
        
        warn!("Confirmation mechanism simplified - auto-approving task '{}'", task.id);
        warn!("Full implementation requires Matrix integration for interactive confirmation");
        
        // 模拟等待时间（实际应用中这里会等待用户输入）
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        
        info!("Auto-approved task '{}' after waiting period", task.id);
        Ok(())
    }

    /// 等待仲裁
    /// 
    /// 实现方式：
    /// 1. 向多个利益相关者发送仲裁请求
    /// 2. 收集各方投票/意见
    /// 3. 根据仲裁规则决定结果
    /// 
    /// 注意：当前实现为简化版，直接继续。
    /// 完整实现需要多方投票机制。
    async fn wait_arbitration(&self, task: &Task, stakeholders: Vec<String>) -> Result<()> {
        info!("Task '{}' requires arbitration from {:?}", task.id, stakeholders);
        
        // 在实际实现中，这里应该：
        // 1. 向所有 stakeholders 发送仲裁请求
        // 2. 收集投票/意见
        // 3. 根据多数决或其他规则决定
        
        warn!("Arbitration mechanism simplified - auto-approving task '{}'", task.id);
        warn!("Stakeholders: {:?}", stakeholders);
        warn!("Full implementation requires voting protocol");
        
        // 模拟等待时间
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        
        info!("Auto-approved task '{}' after arbitration period", task.id);
        Ok(())
    }

    /// 注入输入到 DAG
    fn inject_inputs(&mut self, run_id: &str, inputs: &Value) -> Result<()> {
        // 存储全局输入到上下文
        self.context
            .insert(format!("{}/global", run_id), inputs.clone());
        Ok(())
    }

    /// 准备 Skill 输入
    fn prepare_skill_inputs(&self, task: &Task) -> Result<Value> {
        // 优先使用 task 的 skill_params
        if let Some(params) = &task.skill_params {
            return Ok(params.clone());
        }

        // 否则返回空对象
        Ok(Value::Object(serde_json::Map::new()))
    }

    /// 存储任务结果
    fn store_result(&mut self, run_id: &str, task_id: &str, output: Value) -> Result<()> {
        self.context
            .insert(format!("{}/{}", run_id, task_id), output);
        Ok(())
    }

    /// 收集 DAG 最终输出
    fn collect_outputs(&self, run_id: &str) -> Result<Value> {
        // 收集所有任务的结果
        let key_prefix = format!("{}/", run_id);
        let outputs: HashMap<String, Value> = self
            .context
            .iter()
            .filter(|(k, _)| k.starts_with(&key_prefix) && !k.ends_with("/global"))
            .map(|(k, v)| (k[key_prefix.len()..].to_string(), v.clone()))
            .collect();

        Ok(serde_json::to_value(outputs)?)
    }

    /// 错误分类（判断是 Ignorable 还是 Blocking）
    fn classify_error(&self, result: &SkillExecutionResult) -> FailureType {
        // 根据错误类型判断
        if let Some(error) = &result.error {
            if error.contains("timeout") || error.contains("rate limit") {
                FailureType::Ignorable
            } else {
                FailureType::Blocking
            }
        } else {
            FailureType::Blocking
        }
    }
}



/// 为 DagScheduler 添加辅助方法
impl DagScheduler {
    /// 获取就绪任务列表
    fn get_ready_tasks(&self, run_id: &str) -> Result<Vec<String>> {
        let run = self
            .get_run(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;
        Ok(run.dag.get_ready_tasks())
    }

    /// 检查 DAG 是否已完成
    fn is_completed(&self, run_id: &str) -> Result<bool> {
        let run = self
            .get_run(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;
        Ok(run.dag.nodes().values().all(|n| n.is_terminal()))
    }

    /// 获取任务
    fn get_task(&self, run_id: &str, task_id: &str) -> Result<Task> {
        let run = self
            .get_run(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;

        let node = run
            .dag
            .get_node(task_id)
            .ok_or_else(|| CisError::scheduler("Task not found"))?;

        // 将 DagNode 转换为 Task
        Ok(Task {
            id: node.task_id.clone(),
            parent_id: None,
            title: node.task_id.clone(),
            description: None,
            group_name: "dag".to_string(),
            completion_criteria: None,
            status: crate::types::TaskStatus::Pending,
            priority: crate::types::TaskPriority::Medium,
            dependencies: node.dependencies.clone(),
            result: None,
            error: None,
            workspace_dir: None,
            sandboxed: true,
            allow_network: false,
            created_at: chrono::Utc::now(),
            started_at: None,
            completed_at: None,
            node_id: None,
            metadata: HashMap::new(),
            level: node.level.clone(),
            on_ambiguity: crate::types::AmbiguityPolicy::AutoBest,
            inputs: Vec::new(),
            outputs: Vec::new(),
            rollback: None,
            idempotent: false,
            failure_type: None,
            skill_id: None,
            skill_params: None,
            skill_result: None,
        })
    }

    /// 标记任务完成
    pub fn mark_completed(&mut self, run_id: &str, task_id: &str) -> Result<()> {
        let run = self
            .get_run_mut(run_id)
            .ok_or_else(|| CisError::scheduler("Run not found"))?;

        run.dag
            .mark_completed(task_id.to_string())
            .map_err(|e| CisError::scheduler(format!("Failed to mark completed: {}", e)))?;

        run.update_status();
        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::manifest::{DagPolicy, DagTaskDefinition};

    #[test]
    fn test_dag_definition_to_dag() {
        let dag_def = DagDefinition {
            tasks: vec![
                DagTaskDefinition::new("task1", "test-skill").with_name("Task 1"),
                DagTaskDefinition::new("task2", "test-skill").with_name("Task 2"),
            ],
            policy: DagPolicy::AllSuccess,
        };

        let dag = dag_def.to_dag().unwrap();
        assert_eq!(dag.node_count(), 2);
    }

    #[test]
    fn test_classify_error() {
        let executor = SkillDagExecutor::new(
            DagScheduler::new(),
            Arc::new(SkillManager::new(Arc::new(crate::storage::db::DbManager::new().unwrap())).unwrap()),
        );

        let ignorable_result = SkillExecutionResult {
            success: false,
            output: None,
            error: Some("Connection timeout".to_string()),
            duration_ms: 1000,
        };

        assert_eq!(executor.classify_error(&ignorable_result), FailureType::Ignorable);

        let blocking_result = SkillExecutionResult {
            success: false,
            output: None,
            error: Some("File not found".to_string()),
            duration_ms: 1000,
        };

        assert_eq!(executor.classify_error(&blocking_result), FailureType::Blocking);
    }
}
