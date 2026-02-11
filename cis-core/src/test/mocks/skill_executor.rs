//! # Mock Skill Executor
//!
//! Skill 执行器的 Mock 实现，用于测试 Skill 执行相关功能。

use async_trait::async_trait;
use crate::error::{CisError, Result};
use crate::traits::{
    SkillExecutor, ExecutionContext, ExecutionInfo, ExecutionStatus,
    ExecutionResult, Skill, SkillMetadata, SkillExecutionConfig,
    ResourceLimits,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

/// Mock Skill 执行器
#[derive(Debug, Clone)]
pub struct MockSkillExecutor {
    executions: Arc<RwLock<HashMap<String, MockExecution>>>,
    results: Arc<Mutex<HashMap<String, Result<ExecutionResult>>>>,
    logs: Arc<RwLock<HashMap<String, Vec<String>>>>,
    should_fail: Arc<Mutex<Option<String>>>,
    delay_ms: Arc<Mutex<u64>>,
    call_records: Arc<Mutex<Vec<CallRecord>>>,
    available_skills: Arc<RwLock<Vec<SkillMetadata>>>,
    config: Arc<Mutex<SkillExecutionConfig>>,
}

/// 调用记录
#[derive(Debug, Clone)]
pub struct CallRecord {
    pub skill_name: String,
    pub execution_id: String,
    pub dag_run_id: String,
    pub task_id: String,
}

/// Mock 执行状态
#[derive(Debug, Clone)]
struct MockExecution {
    info: ExecutionInfo,
    status: ExecutionStatus,
    logs: Vec<String>,
}

impl MockSkillExecutor {
    /// 创建新的 Mock
    pub fn new() -> Self {
        let mut default_skills = Vec::new();
        default_skills.push(SkillMetadata {
            name: "test-skill".to_string(),
            description: "Test skill for mocking".to_string(),
            version: "1.0.0".to_string(),
            author: Some("Test".to_string()),
            tags: vec!["test".to_string()],
            input_schema: None,
            output_schema: None,
            default_resources: ResourceLimits::default(),
            enabled: true,
            created_at: 0,
            updated_at: 0,
        });

        Self {
            executions: Arc::new(RwLock::new(HashMap::new())),
            results: Arc::new(Mutex::new(HashMap::new())),
            logs: Arc::new(RwLock::new(HashMap::new())),
            should_fail: Arc::new(Mutex::new(None)),
            delay_ms: Arc::new(Mutex::new(0)),
            call_records: Arc::new(Mutex::new(Vec::new())),
            available_skills: Arc::new(RwLock::new(default_skills)),
            config: Arc::new(Mutex::new(SkillExecutionConfig::default())),
        }
    }

    /// 预设执行结果
    pub fn preset_result(&self, execution_id: impl Into<String>, result: Result<ExecutionResult>) {
        let mut results = self.results.lock().unwrap();
        results.insert(execution_id.into(), result);
    }

    /// 预设执行日志
    pub async fn preset_logs(&self, execution_id: impl Into<String>, logs: Vec<String>) {
        let mut log_map = self.logs.write().await;
        log_map.insert(execution_id.into(), logs);
    }

    /// 设置执行延迟
    pub fn with_delay(&self, delay_ms: u64) {
        *self.delay_ms.lock().unwrap() = delay_ms;
    }

    /// 设置下次执行失败
    pub fn will_fail(&self, message: impl Into<String>) {
        *self.should_fail.lock().unwrap() = Some(message.into());
    }

    /// 添加可用 Skill
    pub async fn add_skill(&self, metadata: SkillMetadata) {
        let mut skills = self.available_skills.write().await;
        skills.push(metadata);
    }

    /// 获取调用记录
    pub fn get_calls(&self) -> Vec<CallRecord> {
        self.call_records.lock().unwrap().clone()
    }

    /// 断言：Skill 被调用
    pub fn assert_called(&self, skill_name: &str) {
        let calls = self.call_records.lock().unwrap();
        let called = calls.iter().any(|c| c.skill_name == skill_name);
        assert!(called, "Expected skill '{}' to be called", skill_name);
    }

    /// 断言：执行特定次数
    pub fn assert_call_count(&self, expected: usize) {
        let calls = self.call_records.lock().unwrap();
        assert_eq!(
            calls.len(), expected,
            "Expected {} calls, but got {}",
            expected, calls.len()
        );
    }

    /// 清空调用记录
    pub fn clear(&self) {
        self.call_records.lock().unwrap().clear();
    }

    /// 模拟创建执行记录
    async fn create_execution(&self, skill: &Skill, context: &ExecutionContext) -> String {
        let execution_id = context.execution_id.clone();
        let info = ExecutionInfo {
            execution_id: execution_id.clone(),
            skill_name: skill.name.clone(),
            status: ExecutionStatus::Pending,
            started_at: None,
            ended_at: None,
            exit_code: None,
            error_message: None,
            progress_percent: 0,
            current_step: None,
        };

        let execution = MockExecution {
            info,
            status: ExecutionStatus::Pending,
            logs: Vec::new(),
        };

        self.executions.write().await.insert(execution_id.clone(), execution);

        // 记录调用
        self.call_records.lock().unwrap().push(CallRecord {
            skill_name: skill.name.clone(),
            execution_id: execution_id.clone(),
            dag_run_id: context.dag_run_id.clone(),
            task_id: context.task_id.clone(),
        });

        execution_id
    }
}

impl Default for MockSkillExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SkillExecutor for MockSkillExecutor {
    async fn execute(&self, skill: &Skill, context: ExecutionContext) -> Result<ExecutionResult> {
        let execution_id = self.create_execution(skill, &context).await;

        // 应用延迟
        let delay = *self.delay_ms.lock().unwrap();
        if delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // 检查是否应该失败
        let fail_msg = self.should_fail.lock().unwrap().take();
        if let Some(msg) = fail_msg {
            let mut executions = self.executions.write().await;
            if let Some(exec) = executions.get_mut(&execution_id) {
                exec.status = ExecutionStatus::Failed;
                exec.info.status = ExecutionStatus::Failed;
                exec.info.ended_at = Some(current_timestamp_millis());
                exec.info.error_message = Some(msg.clone());
            }
            return Err(CisError::execution(format!("Mock execution failed: {}", msg)));
        }

        // 返回预设结果或默认成功
        let result = {
            let results = self.results.lock().unwrap();
            results.get(&execution_id).map(|r| match r {
                Ok(ok) => Ok(ok.clone()),
                Err(e) => Err(CisError::execution(e.to_string())),
            })
        };

        // 更新执行状态
        let mut executions = self.executions.write().await;
        if let Some(exec) = executions.get_mut(&execution_id) {
            exec.status = ExecutionStatus::Success;
            exec.info.status = ExecutionStatus::Success;
            exec.info.started_at = Some(current_timestamp_millis());
            exec.info.ended_at = Some(current_timestamp_millis());
            exec.info.exit_code = Some(0);
        }
        drop(executions);

        match result {
            Some(r) => r,
            None => Ok(ExecutionResult::success("Mock execution completed successfully")
                .with_duration(100)),
        }
    }

    async fn list_skills(&self) -> Result<Vec<SkillMetadata>> {
        let skills = self.available_skills.read().await;
        Ok(skills.clone())
    }

    async fn get_skill_metadata(&self, skill_name: &str) -> Result<Option<SkillMetadata>> {
        let skills = self.available_skills.read().await;
        Ok(skills.iter().find(|s| s.name == skill_name).cloned())
    }

    async fn get_status(&self, execution_id: &str) -> Result<Option<ExecutionInfo>> {
        let executions = self.executions.read().await;
        Ok(executions.get(execution_id).map(|e| e.info.clone()))
    }

    async fn cancel(&self, execution_id: &str) -> Result<()> {
        let mut executions = self.executions.write().await;
        if let Some(exec) = executions.get_mut(execution_id) {
            exec.status = ExecutionStatus::Cancelled;
            exec.info.status = ExecutionStatus::Cancelled;
            exec.info.ended_at = Some(current_timestamp_millis());
            Ok(())
        } else {
            Err(CisError::not_found(format!("Execution {} not found", execution_id)))
        }
    }

    async fn list_running(&self) -> Result<Vec<ExecutionInfo>> {
        let executions = self.executions.read().await;
        let running: Vec<_> = executions
            .values()
            .filter(|e| matches!(e.status, ExecutionStatus::Pending | ExecutionStatus::Running))
            .map(|e| e.info.clone())
            .collect();
        Ok(running)
    }

    async fn list_history(&self, _limit: usize) -> Result<Vec<ExecutionInfo>> {
        let executions = self.executions.read().await;
        let history: Vec<_> = executions
            .values()
            .map(|e| e.info.clone())
            .collect();
        Ok(history)
    }

    async fn get_logs(&self, execution_id: &str, _lines: usize) -> Result<Vec<String>> {
        let logs = self.logs.read().await;
        Ok(logs.get(execution_id).cloned().unwrap_or_default())
    }

    fn get_config(&self) -> Result<SkillExecutionConfig> {
        Ok(self.config.lock().unwrap().clone())
    }

    async fn update_config(&self, config: SkillExecutionConfig) -> Result<()> {
        *self.config.lock().unwrap() = config;
        Ok(())
    }
}

fn current_timestamp_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_skill() -> Skill {
        Skill::new("test-skill")
            .with_description("Test skill")
    }

    fn create_test_context() -> ExecutionContext {
        ExecutionContext::new("dag-1", "task-1", "/tmp")
    }

    #[tokio::test]
    async fn test_mock_execute() {
        let executor = MockSkillExecutor::new();
        let skill = create_test_skill();
        let context = create_test_context();

        let result = executor.execute(&skill, context).await.unwrap();
        assert!(result.success);
        assert_eq!(result.exit_code, 0);

        executor.assert_called("test-skill");
    }

    #[tokio::test]
    async fn test_mock_execute_failure() {
        let executor = MockSkillExecutor::new();
        executor.will_fail("Simulated error");

        let skill = create_test_skill();
        let context = create_test_context();

        let result = executor.execute(&skill, context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_preset_result() {
        let executor = MockSkillExecutor::new();
        let context = create_test_context();
        let execution_id = context.execution_id.clone();

        let preset_result = ExecutionResult::success("Custom output")
            .with_artifact("artifact.txt");

        executor.preset_result(&execution_id, Ok(preset_result.clone()));

        let skill = create_test_skill();
        let result = executor.execute(&skill, context).await.unwrap();

        assert_eq!(result.stdout, "Custom output");
    }

    #[tokio::test]
    async fn test_mock_get_status() {
        let executor = MockSkillExecutor::new();
        let skill = create_test_skill();
        let context = create_test_context();
        let execution_id = context.execution_id.clone();

        // 执行前状态为 None
        let status = executor.get_status(&execution_id).await.unwrap();
        assert!(status.is_none());

        // 执行后
        executor.execute(&skill, context).await.unwrap();

        let status = executor.get_status(&execution_id).await.unwrap().unwrap();
        assert_eq!(status.execution_id, execution_id);
        assert_eq!(status.skill_name, "test-skill");
    }

    #[tokio::test]
    async fn test_mock_cancel() {
        let executor = MockSkillExecutor::new();
        let skill = create_test_skill();
        let context = create_test_context();
        let execution_id = context.execution_id.clone();

        executor.execute(&skill, context).await.unwrap();
        executor.cancel(&execution_id).await.unwrap();

        let status = executor.get_status(&execution_id).await.unwrap().unwrap();
        assert!(matches!(status.status, ExecutionStatus::Cancelled));
    }

    #[tokio::test]
    async fn test_mock_logs() {
        let executor = MockSkillExecutor::new();
        let execution_id = "exec-1";

        executor.preset_logs(execution_id, vec![
            "Log line 1".to_string(),
            "Log line 2".to_string(),
        ]).await;

        let logs = executor.get_logs(execution_id, 10).await.unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0], "Log line 1");
    }

    #[tokio::test]
    async fn test_mock_call_count() {
        let executor = MockSkillExecutor::new();
        let skill = create_test_skill();

        executor.assert_call_count(0);

        executor.execute(&skill, create_test_context()).await.unwrap();
        executor.assert_call_count(1);

        executor.execute(&skill, create_test_context()).await.unwrap();
        executor.assert_call_count(2);

        executor.clear();
        executor.assert_call_count(0);
    }

    #[tokio::test]
    async fn test_list_skills() {
        let executor = MockSkillExecutor::new();

        let skills = executor.list_skills().await.unwrap();
        assert!(!skills.is_empty());
        assert_eq!(skills[0].name, "test-skill");
    }

    #[tokio::test]
    async fn test_get_skill_metadata() {
        let executor = MockSkillExecutor::new();

        let metadata = executor.get_skill_metadata("test-skill").await.unwrap();
        assert!(metadata.is_some());
        assert_eq!(metadata.unwrap().name, "test-skill");

        let metadata = executor.get_skill_metadata("non-existent").await.unwrap();
        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn test_config() {
        let executor = MockSkillExecutor::new();

        let config = executor.get_config().unwrap();
        assert_eq!(config.max_concurrent, 10);

        let new_config = SkillExecutionConfig {
            max_concurrent: 20,
            ..Default::default()
        };
        executor.update_config(new_config).await.unwrap();

        let config = executor.get_config().unwrap();
        assert_eq!(config.max_concurrent, 20);
    }
}
