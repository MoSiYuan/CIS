//! Agent Bridge Skill
//!
//! 将外部 Agent（Claude 等）包装为 CIS Skill，实现双向集成：
//! - CIS 侧：作为普通 Skill 调用
//! - Agent 侧：Agent 通过此 Skill 访问 CIS 基础设施

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::agent::{AgentProvider, AgentRequest, AgentResponse, AgentContext};
use crate::error::{CisError, Result};
use crate::skill::{Skill, SkillContext, Event};
use crate::memory::MemoryService;
use crate::types::{MemoryDomain, MemoryCategory};
use crate::service::TaskService;

/// Agent Bridge Skill
///
/// 让外部 Agent 成为 CIS 生态系统的一部分
pub struct AgentBridgeSkill {
    name: String,
    agent: Arc<Mutex<Box<dyn AgentProvider>>>,
    context: AgentContext,
}

impl AgentBridgeSkill {
    pub fn new(name: impl Into<String>, agent: Box<dyn AgentProvider>) -> Self {
        Self {
            name: name.into(),
            agent: Arc::new(Mutex::new(agent)),
            context: AgentContext::new(),
        }
    }

    /// 设置上下文
    pub fn with_context(mut self, context: AgentContext) -> Self {
        self.context = context;
        self
    }

    /// 直接调用 Agent
    pub async fn call_agent(&self, prompt: impl Into<String>) -> Result<AgentResponse> {
        let agent = self.agent.lock().await;
        let req = AgentRequest {
            prompt: prompt.into(),
            context: self.context.clone(),
            skills: vec![], // Agent 可以使用所有可用 skills
            system_prompt: None,
            history: vec![],
        };
        agent.execute(req).await
    }

    /// 流式调用 Agent
    pub async fn call_agent_stream(
        &self,
        prompt: impl Into<String>,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse> {
        let agent = self.agent.lock().await;
        let req = AgentRequest {
            prompt: prompt.into(),
            context: self.context.clone(),
            skills: vec![],
            system_prompt: None,
            history: vec![],
        };
        agent.execute_stream(req, tx).await
    }
}

#[async_trait]
impl Skill for AgentBridgeSkill {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Bridge to external AI Agent (Claude, Kimi, etc.)"
    }

    async fn init(&mut self, _config: crate::skill::SkillConfig) -> Result<()> {
        let mut agent = self.agent.lock().await;
        agent.init(self.context.clone()).await
    }

    async fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        match event {
            Event::Custom { name, data } => {
                match name.as_str() {
                    "agent:execute" => {
                        // 解析 prompt
                        let prompt = data.get("prompt")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| CisError::invalid_input("Missing prompt"))?;

                        // 调用 Agent
                        let response = self.call_agent(prompt).await?;

                        // 记录结果到记忆
                        ctx.memory_set(
                            &format!("agent/{}/last_response", self.name),
                            response.content.as_bytes()
                        )?;

                        ctx.log_info(&format!("Agent executed, response length: {}", 
                            response.content.len()));
                    }
                    "agent:execute_with_context" => {
                        // 带 CIS 上下文的执行
                        let prompt = data.get("prompt")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| CisError::invalid_input("Missing prompt"))?;

                        // 获取相关记忆作为上下文
                        let context_keys: Vec<String> = data.get("context_keys")
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect())
                            .unwrap_or_default();

                        let mut full_prompt = String::new();
                        full_prompt.push_str("=== CIS Context ===\n");
                        for key in context_keys {
                            if let Some(value) = ctx.memory_get(&key) {
                                full_prompt.push_str(&format!("{}: {}\n", key, 
                                    String::from_utf8_lossy(&value)));
                            }
                        }
                        full_prompt.push_str("\n=== User Request ===\n");
                        full_prompt.push_str(prompt);

                        // 执行
                        let response = self.call_agent(full_prompt).await?;

                        // 存储结果
                        if let Some(output_key) = data.get("output_key").and_then(|v| v.as_str()) {
                            ctx.memory_set(output_key, response.content.as_bytes())?;
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Agent 调用 CIS 的接口
///
/// 供外部 Agent（如 Claude）调用 CIS 功能
pub struct AgentCisClient {
    memory_service: Option<Arc<Mutex<MemoryService>>>,
}

impl AgentCisClient {
    /// 创建新的客户端实例
    pub fn new() -> Self {
        // 尝试初始化 MemoryService
        let memory_service = match MemoryService::open_default("agent") {
            Ok(service) => {
                tracing::info!("AgentCisClient: MemoryService initialized");
                Some(Arc::new(Mutex::new(service)))
            }
            Err(e) => {
                tracing::warn!("AgentCisClient: Failed to initialize MemoryService: {}", e);
                None
            }
        };
        
        Self { memory_service }
    }

    /// 获取记忆
    pub async fn memory_get(&self, key: &str) -> Option<Vec<u8>> {
        match &self.memory_service {
            Some(service) => {
                let service = service.lock().await;
                match service.get(key).await {
                    Ok(Some(item)) => Some(item.value),
                    Ok(None) => {
                        tracing::debug!("AgentCisClient: Key '{}' not found", key);
                        None
                    }
                    Err(e) => {
                        tracing::warn!("AgentCisClient: Failed to get key '{}': {}", key, e);
                        None
                    }
                }
            }
            None => {
                tracing::warn!("AgentCisClient: MemoryService not available");
                None
            }
        }
    }

    /// 设置记忆
    pub async fn memory_set(&self, key: &str, value: &[u8]) -> Result<()> {
        match &self.memory_service {
            Some(service) => {
                let service = service.lock().await;
                service.set(key, value, MemoryDomain::Public, MemoryCategory::Context).await
            }
            None => {
                Err(CisError::Memory("MemoryService not available".to_string()))
            }
        }
    }

    /// 调用 Skill
    pub async fn skill_call(&self, skill: &str, method: &str, params: &[u8]) -> Result<Vec<u8>> {
        // 尝试通过本地 socket 或 HTTP 调用本地 CIS 服务
        tracing::info!("AgentCisClient: Calling skill {}::{} ({} bytes params)", 
            skill, method, params.len());
        
        // 当前实现：直接返回错误，提示使用 HTTP API
        Err(CisError::skill(format!(
            "Direct skill call not implemented. Use CIS HTTP API: POST /api/v1/skills/{}/{}",
            skill, method
        )))
    }

    /// 获取任务列表
    pub async fn task_list(&self) -> Result<Vec<TaskInfo>> {
        // 从 CoreDb 读取任务列表
        match TaskService::new() {
            Ok(task_service) => {
                match task_service.list(crate::service::ListOptions::default()).await {
                    Ok(result) => {
                        let tasks: Vec<TaskInfo> = result.items.into_iter()
                            .map(|t| TaskInfo {
                                id: t.id,
                                title: t.name,
                                status: format!("{:?}", t.status),
                            })
                            .collect();
                        Ok(tasks)
                    }
                    Err(e) => Err(e)
                }
            }
            Err(e) => Err(e)
        }
    }

    /// 创建任务
    pub async fn task_create(&self, title: &str, description: Option<&str>) -> Result<String> {
        match TaskService::new() {
            Ok(task_service) => {
                use crate::service::task_service::{CreateTaskOptions, TaskPriority};
                
                let input = if let Some(desc) = description {
                    serde_json::json!({ "description": desc })
                } else {
                    serde_json::json!({})
                };
                
                let options = CreateTaskOptions {
                    name: title.to_string(),
                    task_type: "agent".to_string(),
                    input,
                    priority: TaskPriority::Normal,
                    dag_id: None,
                    worker_id: None,
                    timeout: 300,
                    max_retries: 1,
                };
                
                match task_service.create(options).await {
                    Ok(info) => Ok(info.summary.id),
                    Err(e) => Err(e)
                }
            }
            Err(e) => Err(e)
        }
    }
}

impl Default for AgentCisClient {
    fn default() -> Self {
        Self::new()
    }
}

/// 任务信息
#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub id: String,
    pub title: String,
    pub status: String,
}

/// CLI 接口供 Agent 调用
///
/// 这些命令通过 `cis agent` 子命令暴露
pub mod cli {
    //! Agent 可调用的 CIS CLI 命令

    /// 导出当前上下文给 Agent
    pub fn export_context() -> String {
        // 收集当前项目状态、记忆、任务等
        serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "memory_keys": list_memory_keys(),
            "pending_tasks": list_pending_tasks(),
        }).to_string()
    }

    fn list_memory_keys() -> Vec<String> {
        vec![]
    }

    fn list_pending_tasks() -> Vec<String> {
        vec![]
    }
}
