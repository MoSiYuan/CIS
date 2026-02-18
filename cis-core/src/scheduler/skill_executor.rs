//! Skill DAG Executor
//!
//! 统一执行 Skill 的执行器，支持：
//! - 原子 Skill（Binary/WASM）直接执行
//! - 复合 Skill（DAG）递归执行
//! - 四级决策检查
//! - 债务累积
//! - 真实用户输入等待（确认、仲裁）

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::decision::{DecisionEngine, DecisionResult};
use crate::error::{CisError, Result};
use crate::scheduler::{DagScheduler, PermissionResult};
use crate::skill::manifest::DagDefinition;
use crate::skill::types::SkillType;
use crate::skill::SkillManager;
use crate::types::{Action, FailureType, SkillExecutionResult, Task, TaskLevel};

/// 用户输入类型
/// 
/// 用于在任务执行过程中接收外部用户输入
#[derive(Debug, Clone)]
pub enum UserInput {
    /// 确认任务继续执行
    Confirm { task_id: String },
    /// 取消任务
    Cancel { task_id: String, reason: String },
    /// 仲裁投票
    ArbitrationVote { 
        task_id: String, 
        stakeholder: String, 
        approve: bool,
        comment: Option<String>,
    },
    /// 跳过任务
    Skip { task_id: String },
}

impl UserInput {
    /// 获取关联的任务 ID
    pub fn task_id(&self) -> &str {
        match self {
            UserInput::Confirm { task_id } => task_id,
            UserInput::Cancel { task_id, .. } => task_id,
            UserInput::ArbitrationVote { task_id, .. } => task_id,
            UserInput::Skip { task_id } => task_id,
        }
    }
}

/// Skill DAG 执行器
pub struct SkillDagExecutor {
    /// DAG 调度器
    scheduler: DagScheduler,
    /// Skill 管理器
    skill_manager: Arc<SkillManager>,
    /// 执行上下文（用于存储中间结果）
    context: HashMap<String, Value>,
    /// 决策引擎（四级决策机制）
    decision_engine: DecisionEngine,
    /// 用户输入接收器（用于确认、仲裁等交互）
    input_rx: mpsc::Receiver<UserInput>,
    /// 用户输入发送器（克隆用于外部发送输入）
    input_tx: mpsc::Sender<UserInput>,
}

#[allow(clippy::await_holding_lock)]
impl SkillDagExecutor {
    /// 创建新的执行器
    /// 
    /// 默认创建容量为 32 的输入通道
    pub fn new(scheduler: DagScheduler, skill_manager: Arc<SkillManager>) -> Self {
        let (input_tx, input_rx) = mpsc::channel(32);
        Self {
            scheduler,
            skill_manager,
            context: HashMap::new(),
            decision_engine: DecisionEngine::new(),
            input_rx,
            input_tx,
        }
    }

    /// 使用自定义决策引擎创建执行器
    pub fn with_decision_engine(
        scheduler: DagScheduler,
        skill_manager: Arc<SkillManager>,
        decision_engine: DecisionEngine,
    ) -> Self {
        let (input_tx, input_rx) = mpsc::channel(32);
        Self {
            scheduler,
            skill_manager,
            context: HashMap::new(),
            decision_engine,
            input_rx,
            input_tx,
        }
    }

    /// 获取输入发送器（用于外部发送用户输入）
    pub fn input_sender(&self) -> mpsc::Sender<UserInput> {
        self.input_tx.clone()
    }

    /// 等待用户输入（带超时）
    /// 
    /// # Arguments
    /// * `task_id` - 任务 ID，用于过滤输入
    /// * `timeout_secs` - 超时时间（秒）
    /// 
    /// # Returns
    /// - `Ok(UserInput)` - 收到匹配的用户输入
    /// - `Err(CisError)` - 超时或通道关闭
    async fn wait_for_input(&mut self, task_id: &str, timeout_secs: u64) -> Result<UserInput> {
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        
        tokio::time::timeout(timeout, async {
            while let Some(input) = self.input_rx.recv().await {
                if input.task_id() == task_id {
                    return Ok(input);
                }
                // 忽略不匹配的输入（可能是其他任务的）
                warn!("Received input for different task: {}", input.task_id());
            }
            Err(CisError::execution("Input channel closed"))
        })
        .await
        .map_err(|_| CisError::execution(format!("Timeout waiting for input on task '{}'", task_id)))?
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
    /// 1. 等待用户输入（通过 input_rx 通道）
    /// 2. 支持 Confirm、Cancel、Skip 三种操作
    /// 3. 超时后自动继续（默认行为）
    /// 
    /// # Arguments
    /// * `task` - 需要确认的任务
    /// * `timeout_secs` - 超时时间（默认 300 秒 = 5 分钟）
    /// 
    /// # Returns
    /// - `Ok(())` - 用户确认或超时默认继续
    /// - `Err(CisError)` - 用户取消或发生错误
    async fn wait_confirmation(&mut self, task: &Task, timeout_secs: Option<u64>) -> Result<()> {
        let timeout = timeout_secs.unwrap_or(300); // 默认 5 分钟超时
        info!("Task '{}' requires user confirmation (timeout: {}s)", task.id, timeout);
        
        match self.wait_for_input(&task.id, timeout).await {
            Ok(UserInput::Confirm { .. }) => {
                info!("Task '{}' confirmed by user", task.id);
                Ok(())
            }
            Ok(UserInput::Cancel { reason, .. }) => {
                warn!("Task '{}' cancelled by user: {}", task.id, reason);
                Err(CisError::execution(format!("Task cancelled: {}", reason)))
            }
            Ok(UserInput::Skip { .. }) => {
                info!("Task '{}' skipped by user", task.id);
                Err(CisError::execution("Task skipped by user"))
            }
            Ok(UserInput::ArbitrationVote { .. }) => {
                warn!("Received arbitration vote for task '{}' in confirmation phase, treating as confirm", task.id);
                Ok(())
            }
            Err(e) => {
                // 超时 - 默认继续执行
                warn!("Confirmation timeout for task '{}', auto-continuing: {}", task.id, e);
                Ok(())
            }
        }
    }

    /// 等待仲裁
    /// 
    /// 实现方式：
    /// 1. 等待利益相关者投票（通过 input_rx 通道）
    /// 2. 收集各方投票/意见
    /// 3. 根据多数决规则决定结果（简单多数通过）
    /// 
    /// # Arguments
    /// * `task` - 需要仲裁的任务
    /// * `stakeholders` - 利益相关者列表
    /// * `timeout_secs` - 超时时间（默认 600 秒 = 10 分钟）
    /// 
    /// # Returns
    /// - `Ok(())` - 多数通过或超时默认继续
    /// - `Err(CisError)` - 多数反对或发生错误
    async fn wait_arbitration(
        &mut self, 
        task: &Task, 
        stakeholders: Vec<String>,
        timeout_secs: Option<u64>,
    ) -> Result<()> {
        let timeout = timeout_secs.unwrap_or(600); // 默认 10 分钟超时
        info!(
            "Task '{}' requires arbitration from {:?} (timeout: {}s)", 
            task.id, stakeholders, timeout
        );
        
        let mut approvals = 0u32;
        let mut rejections = 0u32;
        let total = stakeholders.len() as u32;
        let majority_threshold = (total / 2) + 1; // 简单多数
        
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout);
        
        // 循环收集投票直到超时或形成决议
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            
            if remaining.is_zero() {
                info!("Arbitration timeout for task '{}'", task.id);
                break;
            }
            
            match tokio::time::timeout(remaining, self.input_rx.recv()).await {
                Ok(Some(input)) => {
                    if input.task_id() != task.id {
                        warn!("Received input for different task: {}", input.task_id());
                        continue;
                    }
                    
                    match input {
                        UserInput::ArbitrationVote { stakeholder, approve, comment, .. } => {
                            if !stakeholders.contains(&stakeholder) {
                                warn!("Vote from non-stakeholder '{}' ignored", stakeholder);
                                continue;
                            }
                            
                            if approve {
                                approvals += 1;
                                info!("Stakeholder '{}' approved task '{}' {:?}", 
                                    stakeholder, task.id, comment);
                            } else {
                                rejections += 1;
                                warn!("Stakeholder '{}' rejected task '{}' {:?}", 
                                    stakeholder, task.id, comment);
                            }
                            
                            // 检查是否已形成决议
                            if approvals >= majority_threshold {
                                info!("Task '{}' approved by majority ({} approvals)", 
                                    task.id, approvals);
                                return Ok(());
                            }
                            if rejections >= majority_threshold {
                                return Err(CisError::execution(
                                    format!("Task rejected by majority ({} rejections)", rejections)
                                ));
                            }
                        }
                        UserInput::Cancel { reason, .. } => {
                            warn!("Task '{}' cancelled during arbitration: {}", task.id, reason);
                            return Err(CisError::execution(format!("Cancelled: {}", reason)));
                        }
                        _ => {
                            warn!("Unexpected input type for arbitration on task '{}'", task.id);
                        }
                    }
                }
                Ok(None) => {
                    // 通道关闭
                    warn!("Input channel closed during arbitration for task '{}'", task.id);
                    break;
                }
                Err(_) => {
                    // 超时
                    info!("Arbitration timeout for task '{}'", task.id);
                    break;
                }
            }
        }
        
        // 超时后根据已收集的投票决定
        info!(
            "Arbitration ended for task '{}': {} approvals, {} rejections out of {} stakeholders",
            task.id, approvals, rejections, total
        );
        
        if approvals >= rejections {
            info!("Task '{}' auto-approved (more approvals than rejections)", task.id);
            Ok(())
        } else {
            Err(CisError::execution(
                format!("Task rejected ({} approvals vs {} rejections)", approvals, rejections)
            ))
        }
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
