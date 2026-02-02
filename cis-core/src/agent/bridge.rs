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
pub struct AgentCisClient;

impl AgentCisClient {
    /// 获取记忆
    pub fn memory_get(_key: &str) -> Option<Vec<u8>> {
        // 通过环境变量或命名管道与 CIS 通信
        // 简化实现：直接读取数据库
        // 实际应该通过 CIS 服务进程
        None
    }

    /// 设置记忆
    pub fn memory_set(_key: &str, _value: &[u8]) -> Result<()> {
        Ok(())
    }

    /// 调用 Skill
    pub fn skill_call(_skill: &str, _method: &str, _params: &[u8]) -> Result<Vec<u8>> {
        Ok(vec![])
    }

    /// 获取任务列表
    pub fn task_list() -> Vec<TaskInfo> {
        vec![]
    }

    /// 创建任务
    pub fn task_create(_title: &str, _description: Option<&str>) -> Result<String> {
        Ok(String::new())
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
