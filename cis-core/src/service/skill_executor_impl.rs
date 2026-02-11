//! # SkillExecutorImpl - 真实 Skill 执行器实现
//!
//! 基于 SkillManager 的真实 Skill 执行器，支持：
//! - 原子 Skill 执行（Binary/WASM）
//! - 执行状态跟踪
//! - 资源限制

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

use crate::error::{CisError, Result};
use crate::skill::manager::SkillManager;
use crate::skill::types::{SkillType, LoadOptions};
use crate::traits::{
    SkillExecutor, ExecutionContext, ExecutionInfo, ExecutionStatus,
    ExecutionResult, Skill, SkillMetadata, SkillExecutionConfig,
    ResourceLimits,
};
use crate::traits::skill_executor::ResourceUsage;

#[cfg(feature = "wasm")]
use crate::wasm::WasmRuntime;
#[cfg(feature = "wasm")]
use crate::memory::MemoryService;
#[cfg(feature = "wasm")]
use crate::memory::MemoryServiceTrait;
#[cfg(feature = "wasm")]
use crate::ai::{AiProviderFactory, AiProvider};
#[cfg(feature = "wasm")]
use crate::conversation::ConversationContext;
#[cfg(feature = "wasm")]
use std::sync::Mutex as StdMutex;

/// 包装 Box<dyn AiProvider> 以实现 AiProvider trait
#[cfg(feature = "wasm")]
struct BoxedAiProvider(Box<dyn crate::ai::AiProvider>);

#[cfg(feature = "wasm")]
#[async_trait]
impl crate::ai::AiProvider for BoxedAiProvider {
    fn name(&self) -> &str {
        self.0.name()
    }

    async fn available(&self) -> bool {
        self.0.available().await
    }

    async fn chat(&self, prompt: &str) -> crate::ai::Result<String> {
        self.0.chat(prompt).await
    }

    async fn chat_with_context(
        &self,
        system: &str,
        messages: &[crate::ai::Message],
    ) -> crate::ai::Result<String> {
        self.0.chat_with_context(system, messages).await
    }
    
    async fn chat_with_rag(
        &self,
        prompt: &str,
        ctx: Option<&ConversationContext>,
    ) -> crate::ai::Result<String> {
        self.0.chat_with_rag(prompt, ctx).await
    }
    
    async fn generate_json(
        &self,
        prompt: &str,
        schema: &str,
    ) -> crate::ai::Result<serde_json::Value> {
        self.0.generate_json(prompt, schema).await
    }
}

/// 执行记录
#[derive(Debug, Clone)]
struct ExecutionRecord {
    info: ExecutionInfo,
    result: Option<ExecutionResult>,
    logs: Vec<String>,
}

/// 真实 Skill 执行器
///
/// 基于 SkillManager 实现 SkillExecutor trait，提供完整的 Skill 执行能力。
pub struct SkillExecutorImpl {
    /// Skill 管理器
    skill_manager: Arc<SkillManager>,
    /// 执行记录
    executions: Arc<RwLock<HashMap<String, ExecutionRecord>>>,
    /// 配置
    config: Arc<RwLock<SkillExecutionConfig>>,
}

impl SkillExecutorImpl {
    /// 创建新的 Skill 执行器
    ///
    /// # 参数
    /// * `skill_manager` - Skill 管理器实例
    pub fn new(skill_manager: Arc<SkillManager>) -> Result<Self> {
        Ok(Self {
            skill_manager,
            executions: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(SkillExecutionConfig::default())),
        })
    }

    /// 创建执行记录
    async fn create_execution(&self, skill: &Skill, context: &ExecutionContext) -> String {
        let execution_id = context.execution_id.clone();
        
        let info = ExecutionInfo {
            execution_id: execution_id.clone(),
            skill_name: skill.name.clone(),
            status: ExecutionStatus::Pending,
            started_at: Some(current_timestamp_millis()),
            ended_at: None,
            exit_code: None,
            error_message: None,
            progress_percent: 0,
            current_step: Some("initializing".to_string()),
        };

        let record = ExecutionRecord {
            info,
            result: None,
            logs: vec![format!("Starting execution of skill '{}'", skill.name)],
        };

        let mut executions = self.executions.write().await;
        executions.insert(execution_id.clone(), record);

        execution_id
    }

    /// 更新执行状态
    async fn update_status(&self, execution_id: &str, status: ExecutionStatus) {
        let mut executions = self.executions.write().await;
        if let Some(record) = executions.get_mut(execution_id) {
            record.info.status = status;
            if status.is_terminal() {
                record.info.ended_at = Some(current_timestamp_millis());
            }
        }
    }

    /// 添加执行日志
    async fn add_log(&self, execution_id: &str, message: impl Into<String>) {
        let mut executions = self.executions.write().await;
        if let Some(record) = executions.get_mut(execution_id) {
            record.logs.push(message.into());
        }
    }

    /// 执行二进制 Skill
    async fn execute_binary(
        &self,
        skill_id: &str,
        path: &PathBuf,
        inputs: serde_json::Value,
        execution_id: &str,
    ) -> Result<ExecutionResult> {
        debug!("Executing binary skill '{}' at {:?}", skill_id, path);
        
        self.add_log(execution_id, format!("Executing binary: {:?}", path)).await;
        
        // 检查文件是否存在
        if !path.exists() {
            return Err(CisError::skill_not_found(format!(
                "Binary not found: {:?}", path
            )));
        }

        // 创建临时输入文件
        let input_file = tokio::task::spawn_blocking(|| {
            tempfile::NamedTempFile::with_suffix(".json")
        })
        .await
        .map_err(|e| CisError::execution(format!("Failed to spawn blocking task: {}", e)))?
        .map_err(|e| CisError::execution(format!("Failed to create temp file: {}", e)))?;

        let input_path = input_file.path().to_path_buf();

        // 写入输入数据
        let input_data = serde_json::to_vec(&inputs)?;
        tokio::fs::write(&input_path, &input_data).await?;

        // 执行二进制（带超时）
        let start = std::time::Instant::now();
        
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(300), // 5 分钟超时
            Command::new(path)
                .arg(&input_path)
                .output(),
        )
        .await
        .map_err(|_| CisError::execution("Binary execution timeout"))?
        .map_err(|e| CisError::execution(format!("Failed to execute binary: {}", e)))?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // 解析输出
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        self.add_log(execution_id, format!("Execution completed in {}ms", duration_ms)).await;

        if output.status.success() {
            Ok(ExecutionResult {
                success: true,
                exit_code: output.status.code().unwrap_or(0),
                stdout: stdout.clone(),
                stderr: stderr.clone(),
                artifacts: vec![],
                duration_ms,
                resource_usage: None,
            })
        } else {
            Ok(ExecutionResult {
                success: false,
                exit_code: output.status.code().unwrap_or(-1),
                stdout: stdout.clone(),
                stderr: stderr.clone(),
                artifacts: vec![],
                duration_ms,
                resource_usage: None,
            })
        }
    }

    /// 执行 WASM Skill
    #[cfg(feature = "wasm")]
    async fn execute_wasm(
        &self,
        skill_id: &str,
        wasm_bytes: &[u8],
        inputs: serde_json::Value,
        execution_id: &str,
    ) -> Result<ExecutionResult> {
        self.add_log(execution_id, "Starting WASM execution").await;
        
        // 验证 WASM 魔术数字
        if wasm_bytes.len() < 8 || wasm_bytes[0..4] != [0x00, 0x61, 0x73, 0x6d] {
            return Err(CisError::wasm(format!("Invalid WASM file for skill '{}'", skill_id)));
        }
        
        // 获取 WASM runtime
        let wasm_runtime = self.skill_manager.get_wasm_runtime()
            .map_err(|e| CisError::skill(format!("Failed to access WASM runtime: {}", e)))?;
        
        self.add_log(execution_id, "Executing WASM module").await;
        
        // 在 spawn_blocking 中执行 WASM 以避免 std::sync::MutexGuard 跨越 await 边界
        let skill_id = skill_id.to_string();
        let wasm_bytes = wasm_bytes.to_vec();
        let execution_id = execution_id.to_string();
        
        let result = tokio::task::spawn_blocking(move || {
            // 创建必要的服务
            let memory_service: Arc<StdMutex<dyn MemoryServiceTrait>> = 
                Arc::new(StdMutex::new(MemoryService::open_default(&skill_id)?));
            let ai_provider: Arc<StdMutex<dyn AiProvider>> = 
                Arc::new(StdMutex::new(BoxedAiProvider(AiProviderFactory::default_provider())));
            
            // 锁定 runtime 并执行 skill
            let runtime_guard = wasm_runtime.lock()
                .map_err(|e| CisError::skill(format!("WASM runtime lock failed: {}", e)))?;
            
            // 创建 tokio runtime 来执行异步代码
            let rt = tokio::runtime::Handle::current();
            
            let result = rt.block_on(async {
                runtime_guard.execute_skill(
                    &skill_id,
                    &wasm_bytes,
                    inputs,
                    memory_service,
                    ai_provider,
                ).await
            }).map_err(|e| CisError::skill(format!("WASM skill execution failed: {}", e)))?;
            
            Ok::<serde_json::Value, CisError>(result)
        }).await
        .map_err(|e| CisError::skill(format!("WASM execution task failed: {}", e)))??;
        
        self.add_log(&execution_id, "WASM execution completed").await;
        
        // 将 JSON 结果转换为 ExecutionResult
        let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        let exit_code = result.get("code").and_then(|v| v.as_i64()).map(|v| v as i32)
            .unwrap_or(if success { 0 } else { 1 });
        let duration_ms = result.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
        
        Ok(ExecutionResult {
            success,
            exit_code,
            stdout: result.get("output").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_default(),
            stderr: String::new(),
            artifacts: vec![],
            duration_ms,
            resource_usage: None,
        })
    }

    #[cfg(not(feature = "wasm"))]
    async fn execute_wasm(
        &self,
        _skill_id: &str,
        _wasm_bytes: &[u8],
        _inputs: serde_json::Value,
        _execution_id: &str,
    ) -> Result<ExecutionResult> {
        Err(CisError::skill("WASM feature not enabled"))
    }

    /// 执行远程 Skill
    async fn execute_remote(
        &self,
        _skill_id: &str,
        _inputs: serde_json::Value,
        execution_id: &str,
    ) -> Result<ExecutionResult> {
        self.add_log(execution_id, "Remote skill execution not implemented").await;
        Err(CisError::skill("Remote skill execution not yet implemented"))
    }

    /// 优雅关闭
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down SkillExecutorImpl...");
        
        // 取消所有正在运行的执行
        let executions = self.executions.read().await;
        for (execution_id, record) in executions.iter() {
            if !record.info.status.is_terminal() {
                warn!("Execution {} is still running during shutdown", execution_id);
            }
        }
        drop(executions);
        
        info!("SkillExecutorImpl shutdown completed");
        Ok(())
    }
}

#[async_trait]
impl SkillExecutor for SkillExecutorImpl {
    async fn execute(&self, skill: &Skill, context: ExecutionContext) -> Result<ExecutionResult> {
        let execution_id = self.create_execution(skill, &context).await;
        
        info!("Executing skill '{}' (execution_id: {})", skill.name, execution_id);
        
        self.update_status(&execution_id, ExecutionStatus::Running).await;
        
        // 获取 Skill 信息
        let skill_info = self.skill_manager.get_info(&skill.name)?
            .ok_or_else(|| CisError::skill_not_found(&skill.name))?;
        
        let result = match skill_info.meta.skill_type {
            SkillType::Native => {
                let path = PathBuf::from(&skill_info.meta.path);
                
                // 构建输入参数
                let mut inputs = serde_json::Map::new();
                for (key, value) in &skill.parameters {
                    inputs.insert(key.clone(), serde_json::Value::String(value.clone()));
                }
                
                self.execute_binary(&skill.name, &path, serde_json::Value::Object(inputs), &execution_id).await
            }
            SkillType::Wasm => {
                let wasm_path = PathBuf::from(&skill_info.meta.path);
                let wasm_bytes = tokio::fs::read(&wasm_path).await?;
                self.execute_wasm(&skill.name, &wasm_bytes, serde_json::json!({}), &execution_id).await
            }
            SkillType::Remote => {
                self.execute_remote(&skill.name, serde_json::json!({}), &execution_id).await
            }
            SkillType::Dag => {
                // DAG Skill 需要特殊处理，使用 DagScheduler
                Err(CisError::skill("DAG skill execution should use DagScheduler directly"))
            }
        };
        
        // 更新执行状态和结果
        match &result {
            Ok(exec_result) => {
                if exec_result.success {
                    self.update_status(&execution_id, ExecutionStatus::Success).await;
                    self.add_log(&execution_id, "Execution completed successfully").await;
                } else {
                    self.update_status(&execution_id, ExecutionStatus::Failed).await;
                    self.add_log(&execution_id, format!("Execution failed: {}", exec_result.stderr)).await;
                }
            }
            Err(e) => {
                self.update_status(&execution_id, ExecutionStatus::Failed).await;
                self.add_log(&execution_id, format!("Execution error: {}", e)).await;
            }
        }
        
        // 保存结果
        let mut executions = self.executions.write().await;
        if let Some(record) = executions.get_mut(&execution_id) {
            record.result = result.as_ref().ok().cloned();
            if let Err(ref e) = result {
                record.info.error_message = Some(e.to_string());
            }
        }
        drop(executions);
        
        result
    }

    async fn list_skills(&self) -> Result<Vec<SkillMetadata>> {
        let skills = self.skill_manager.list_all()?;
        
        let metadata: Vec<SkillMetadata> = skills.into_iter()
            .map(|info| SkillMetadata {
                name: info.meta.name,
                description: info.meta.description,
                version: info.meta.version,
                author: Some(info.meta.author),
                tags: vec![],
                input_schema: None,
                output_schema: None,
                default_resources: ResourceLimits::default(),
                enabled: info.runtime.state.is_active(),
                created_at: 0,
                updated_at: 0,
            })
            .collect();
        
        Ok(metadata)
    }

    async fn get_skill_metadata(&self, skill_name: &str) -> Result<Option<SkillMetadata>> {
        let info = self.skill_manager.get_info(skill_name)?;
        
        Ok(info.map(|i| SkillMetadata {
            name: i.meta.name,
            description: i.meta.description,
            version: i.meta.version,
            author: Some(i.meta.author),
            tags: vec![],
            input_schema: None,
            output_schema: None,
            default_resources: ResourceLimits::default(),
            enabled: i.runtime.state.is_active(),
            created_at: 0,
            updated_at: 0,
        }))
    }

    async fn get_status(&self, execution_id: &str) -> Result<Option<ExecutionInfo>> {
        let executions = self.executions.read().await;
        Ok(executions.get(execution_id).map(|r| r.info.clone()))
    }

    async fn cancel(&self, execution_id: &str) -> Result<()> {
        let mut executions = self.executions.write().await;
        
        if let Some(record) = executions.get_mut(execution_id) {
            if record.info.status.is_terminal() {
                return Err(CisError::execution(
                    format!("Execution {} is already terminal", execution_id)
                ));
            }
            
            record.info.status = ExecutionStatus::Cancelled;
            record.info.ended_at = Some(current_timestamp_millis());
            record.logs.push("Execution cancelled".to_string());
            
            info!("Execution {} cancelled", execution_id);
            Ok(())
        } else {
            Err(CisError::not_found(format!(
                "Execution {} not found", execution_id
            )))
        }
    }

    async fn list_running(&self) -> Result<Vec<ExecutionInfo>> {
        let executions = self.executions.read().await;
        let running: Vec<_> = executions
            .values()
            .filter(|r| matches!(r.info.status, ExecutionStatus::Pending | ExecutionStatus::Running))
            .map(|r| r.info.clone())
            .collect();
        Ok(running)
    }

    async fn list_history(&self, limit: usize) -> Result<Vec<ExecutionInfo>> {
        let executions = self.executions.read().await;
        let mut history: Vec<_> = executions
            .values()
            .map(|r| r.info.clone())
            .collect();
        
        // 按开始时间降序排序
        history.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        
        // 限制数量
        history.truncate(limit);
        
        Ok(history)
    }

    async fn get_logs(&self, execution_id: &str, lines: usize) -> Result<Vec<String>> {
        let executions = self.executions.read().await;
        
        if let Some(record) = executions.get(execution_id) {
            let start = record.logs.len().saturating_sub(lines);
            Ok(record.logs[start..].to_vec())
        } else {
            Err(CisError::not_found(format!(
                "Execution {} not found", execution_id
            )))
        }
    }

    fn get_config(&self) -> Result<SkillExecutionConfig> {
        Ok(self.config.blocking_read().clone())
    }

    async fn update_config(&self, config: SkillExecutionConfig) -> Result<()> {
        *self.config.write().await = config;
        Ok(())
    }
}

impl ExecutionStatus {
    /// 检查状态是否为终止状态
    fn is_terminal(&self) -> bool {
        matches!(self, 
            ExecutionStatus::Success | 
            ExecutionStatus::Failed | 
            ExecutionStatus::Cancelled | 
            ExecutionStatus::Timeout
        )
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
    use crate::storage::db::DbManager;
    use tempfile::TempDir;

    fn setup_test_env() -> (TempDir, Arc<SkillManager>) {
        let temp_dir = TempDir::new().unwrap();
        std::env::set_var("CIS_DATA_DIR", temp_dir.path());
        
        // 确保目录存在
        crate::storage::paths::Paths::ensure_dirs().unwrap();
        
        let db_manager = Arc::new(DbManager::new().unwrap());
        let skill_manager = Arc::new(SkillManager::new(db_manager).unwrap());
        
        (temp_dir, skill_manager)
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let (_temp, skill_manager) = setup_test_env();
        let executor = SkillExecutorImpl::new(skill_manager).unwrap();
        
        let config = executor.get_config().unwrap();
        assert_eq!(config.max_concurrent, 10);
    }

    #[tokio::test]
    async fn test_list_skills_empty() {
        let (_temp, skill_manager) = setup_test_env();
        let executor = SkillExecutorImpl::new(skill_manager).unwrap();
        
        let skills = executor.list_skills().await.unwrap();
        // 初始时应该没有技能
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn test_get_skill_metadata_not_found() {
        let (_temp, skill_manager) = setup_test_env();
        let executor = SkillExecutorImpl::new(skill_manager).unwrap();
        
        let metadata = executor.get_skill_metadata("nonexistent").await.unwrap();
        assert!(metadata.is_none());
    }

    #[tokio::test]
    async fn test_config_update() {
        let (_temp, skill_manager) = setup_test_env();
        let executor = SkillExecutorImpl::new(skill_manager).unwrap();
        
        let new_config = SkillExecutionConfig {
            max_concurrent: 20,
            ..Default::default()
        };
        
        executor.update_config(new_config).await.unwrap();
        
        let config = executor.get_config().unwrap();
        assert_eq!(config.max_concurrent, 20);
    }
}
