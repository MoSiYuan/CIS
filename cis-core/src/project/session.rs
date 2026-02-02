//! 项目会话管理
//!
//! 管理 CIS 与 Agent 的双向集成会话

use std::sync::Arc;

use crate::agent::{AgentBridgeSkill, AgentContext, AgentManager, AgentProvider};
use crate::error::Result;
use crate::project::Project;
use crate::skill::{LoadOptions, SkillManager};
use crate::storage::db::DbManager;

/// 项目会话
///
/// 代表一个活跃的项目工作会话，包含：
/// - 项目配置
/// - Agent 连接
/// - 已加载的 Skills
pub struct ProjectSession {
    project: Arc<Project>,
    agent_manager: Arc<AgentManager>,
    skill_manager: Arc<SkillManager>,
    db_manager: Arc<DbManager>,
}

impl ProjectSession {
    /// 创建新项目会话
    pub fn new(
        project: Project,
        db_manager: Arc<DbManager>,
    ) -> Result<Self> {
        let agent_manager = Arc::new(AgentManager::new());
        let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);

        Ok(Self {
            project: Arc::new(project),
            agent_manager,
            skill_manager,
            db_manager,
        })
    }

    /// 启动会话（双向绑定）
    pub async fn start(&self) -> Result<()> {
        tracing::info!("Starting project session for '{}'", self.project.config.name);

        // 1. 加载项目本地 Skills
        self.load_local_skills().await?;

        // 2. 注册默认 Agent
        self.register_default_agent().await?;

        // 3. 建立双向绑定
        self.establish_bridge().await?;

        tracing::info!("Project session started successfully");
        Ok(())
    }

    /// 加载项目本地 Skills
    async fn load_local_skills(&self) -> Result<()> {
        let local_skills = self.project.list_local_skills()?;

        for skill_info in local_skills {
            tracing::info!("Loading local skill: {}", skill_info.name);

            // 创建本地 skill 数据库
            let skill_db = self.db_manager.load_skill_db(&skill_info.name)?;

            // 注册并加载
            // TODO: 从 manifest 解析 skill 元数据
            let _ = skill_db;
        }

        // 加载配置中标记为 auto_load 的 skills
        for skill_config in &self.project.config.skills {
            if skill_config.auto_load {
                tracing::info!("Auto-loading skill: {}", skill_config.name);

                self.skill_manager
                    .load(&skill_config.name, LoadOptions::default())?;
            }
        }

        Ok(())
    }

    /// 注册默认 Agent
    async fn register_default_agent(&self) -> Result<()> {
        let provider_name = &self.project.config.ai.provider;

        // 尝试创建 provider
        let provider = match provider_name.as_str() {
            "claude" => {
                Some(Box::new(crate::agent::providers::ClaudeProvider::default())
                    as Box<dyn AgentProvider>)
            }
            "kimi" => Some(
                Box::new(crate::agent::providers::KimiProvider::default())
                    as Box<dyn AgentProvider>,
            ),
            "aider" => Some(
                Box::new(crate::agent::providers::AiderProvider::default())
                    as Box<dyn AgentProvider>,
            ),
            _ => None,
        };

        if let Some(provider) = provider {
            self.agent_manager
                .register(provider_name.clone(), provider);
            tracing::info!("Registered agent provider: {}", provider_name);
        }

        Ok(())
    }

    /// 建立 Agent-CIS 双向绑定
    async fn establish_bridge(&self) -> Result<()> {
        let provider_name = self.project.config.ai.provider.clone();

        // 简化处理：只记录 bridge 建立
        if self.agent_manager.list().contains(&provider_name) {
            // 构建 Agent 上下文
            let context = AgentContext::new()
                .with_work_dir(self.project.config.root_dir.clone())
                .with_memory_access(vec![
                    self.project.memory_prefix(),
                    "shared/".to_string(),
                ]);

            // 创建 Bridge Skill
            // 注意：这里需要获取 ownership，所以实际上我们需要不同的方式
            // 简化：直接通过 agent_manager 调用
            let _context = context;
            tracing::info!("Agent bridge established");
        }

        Ok(())
    }

    /// 获取项目引用
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// 获取 Agent 管理器
    pub fn agent_manager(&self) -> &AgentManager {
        &self.agent_manager
    }

    /// 获取 Skill 管理器
    pub fn skill_manager(&self) -> &SkillManager {
        &self.skill_manager
    }

    /// 调用 Agent
    pub async fn call_agent(&self, prompt: impl Into<String>) -> Result<String> {
        let provider_name = self.project.config.ai.provider.clone();

        if let Some(agent) = self.agent_manager.get(&provider_name) {
            let req = crate::agent::AgentRequest {
                prompt: prompt.into(),
                context: crate::agent::AgentContext::new()
                    .with_work_dir(self.project.config.root_dir.clone()),
                skills: vec![],
                system_prompt: Some(self.project.build_ai_guide()),
                history: vec![],
            };

            let response = agent.execute(req).await?;
            Ok(response.content)
        } else {
            Err(crate::error::CisError::skill(
                "No agent provider available"
            ))
        }
    }

    /// 执行 Skill
    pub async fn execute_skill(
        &self,
        skill_name: &str,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // 通过 skill_manager 执行
        // TODO: 实现具体的执行逻辑
        tracing::info!("Executing skill: {}::{}", skill_name, method);
        Ok(serde_json::json!({"status": "ok"}))
    }

    /// 存储项目记忆
    pub fn set_memory(&self, key: &str, value: &[u8]) -> Result<()> {
        let full_key = self.project.memory_key(key);

        // 获取 core_db 并存储
        let core_db = self.db_manager.core();
        let db = core_db.lock()
            .map_err(|e| crate::error::CisError::storage(format!("Lock failed: {}", e)))?;

        // 实际存储逻辑
        // TODO: 实现记忆存储
        tracing::info!("Setting memory: {}", full_key);

        Ok(())
    }

    /// 获取项目记忆
    pub fn get_memory(&self, key: &str) -> Option<Vec<u8>> {
        let full_key = self.project.memory_key(key);

        // TODO: 实现记忆读取
        tracing::debug!("Getting memory: {}", full_key);

        None
    }

    /// 关闭会话
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down project session");

        // 卸载所有 skills
        for skill_name in self.skill_manager.list_loaded()? {
            self.skill_manager.unload(&skill_name)?;
        }

        Ok(())
    }
}

/// 会话构建器
pub struct ProjectSessionBuilder {
    project: Option<Project>,
    db_manager: Option<Arc<DbManager>>,
}

impl ProjectSessionBuilder {
    pub fn new() -> Self {
        Self {
            project: None,
            db_manager: None,
        }
    }

    pub fn project(mut self, project: Project) -> Self {
        self.project = Some(project);
        self
    }

    pub fn db_manager(mut self, db_manager: Arc<DbManager>) -> Self {
        self.db_manager = Some(db_manager);
        self
    }

    pub fn build(self) -> Result<ProjectSession> {
        let project = self.project.ok_or_else(|| {
            crate::error::CisError::configuration("Project not set")
        })?;

        let db_manager = self.db_manager.ok_or_else(|| {
            crate::error::CisError::configuration("DbManager not set")
        })?;

        ProjectSession::new(project, db_manager)
    }
}

impl Default for ProjectSessionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
