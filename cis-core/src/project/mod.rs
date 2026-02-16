//! # Project 模块
//!
//! 管理项目级配置和本地 Skill

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{CisError, Result};

pub mod session;

pub use session::ProjectSession;

/// 项目配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// 项目名称
    pub name: String,
    /// 项目 ID
    pub id: String,
    /// 项目根目录
    #[serde(skip)]
    pub root_dir: PathBuf,
    /// AI 配置
    pub ai: AiConfig,
    /// 项目级 Skills
    pub skills: Vec<LocalSkillConfig>,
    /// 记忆配置
    pub memory: MemoryConfig,
    /// 额外配置
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// AI 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// AI 引导提示词
    pub guide: String,
    /// 默认 Agent Provider
    pub provider: String,
    /// 模型配置
    pub model: Option<String>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            guide: "You are working with CIS (Cluster of Independent Systems).".to_string(),
            provider: "claude".to_string(),
            model: None,
        }
    }
}

/// 本地 Skill 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalSkillConfig {
    /// Skill 名称
    pub name: String,
    /// Skill 路径（相对于项目根目录）
    pub path: String,
    /// 是否自动加载
    #[serde(default = "default_auto_load")]
    pub auto_load: bool,
    /// Skill 配置
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

fn default_auto_load() -> bool {
    false
}

/// 记忆配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// 记忆命名空间
    pub namespace: String,

    /// 共享记忆键
    #[serde(default)]
    pub shared_keys: Vec<String>,

    /// 🔥 作用域 ID（v1.1.7: 稳定哈希绑定）
    ///
    /// # 说明
    ///
    /// - **第一次初始化后**：自动生成目录哈希并保存
    /// - **移动/重命名后**：从配置文件读取（哈希不变）
    /// - **用户自定义**：可手动指定人类可读的 ID
    ///
    /// # 示例
    ///
    /// ```toml
    /// [memory]
    /// # 自动生成（第一次初始化后）
    /// scope_id = "a3f7e9c2b1d4f8a5"
    ///
    /// # 或用户自定义
    /// # scope_id = "my-workspace"
    /// ```
    #[serde(default = "default_scope_id")]
    pub scope_id: String,

    /// 🔥 人类可读名称（可选，用于调试和 UI）
    ///
    /// # 示例
    ///
    /// ```toml
    /// [memory]
    /// display_name = "My Project Workspace"
    /// ```
    #[serde(default)]
    pub display_name: Option<String>,
}

fn default_scope_id() -> String {
    "".to_string()  // 默认为空，第一次初始化时生成哈希
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            namespace: "default".to_string(),
            shared_keys: vec![],
            scope_id: default_scope_id(),
            display_name: None,
        }
    }
}

impl ProjectConfig {
    /// 🔥 项目根目录（v1.1.7）
    ///
    /// # 注意
    ///
    /// 此属性用于 MemoryScope 生成哈希时使用。
    pub fn project_root(&self) -> &PathBuf {
        &self.root_dir
    }

    /// 🔥 保存配置文件
    ///
    /// # 使用场景
    ///
    /// - 生成 scope_id 后保存到 `.cis/project.toml`
    /// - 修改其他配置后保存
    pub fn save(&self) -> Result<()> {
        let config_path = self.root_dir.join(".cis").join("project.toml");

        // 1. 序列化为 TOML
        let content = toml::to_string_pretty(self)
            .map_err(|e| CisError::config_validation_error("project_config", format!("Failed to serialize: {}", e)))?;

        // 2. 写入文件
        std::fs::write(&config_path, content)
            .map_err(|e| CisError::config_validation_error("project_config", format!("Failed to write to {:?}: {}", config_path, e)))?;

        println!("[INFO] Saved project config to {:?}", config_path);
        Ok(())
    }
}

/// 项目管理器
pub struct ProjectManager {
    current: Option<Project>,
}

/// 项目实例
#[derive(Debug, Clone)]
pub struct Project {
    pub config: ProjectConfig,
    pub local_skills_dir: PathBuf,
}

impl Project {
    /// 从目录加载项目
    pub fn load(dir: &Path) -> Result<Self> {
        let config_path = dir.join(".cis").join("project.toml");

        if !config_path.exists() {
            return Err(CisError::config_not_found(format!(
                "Project config not found at {:?}",
                config_path
            )));
        }

        let content = std::fs::read_to_string(&config_path)?;
        let mut config: ProjectConfig = toml::from_str(&content)
            .map_err(|e| CisError::config_parse_error(&config_path.display().to_string(), e.to_string()))?;

        config.root_dir = dir.to_path_buf();

        let local_skills_dir = dir.join(".cis").join("skills");

        Ok(Self {
            config,
            local_skills_dir,
        })
    }

    /// 创建新项目
    pub fn init(dir: &Path, name: impl Into<String>) -> Result<Self> {
        let cis_dir = dir.join(".cis");
        std::fs::create_dir_all(&cis_dir)?;

        let config = ProjectConfig {
            name: name.into(),
            id: uuid::Uuid::new_v4().to_string(),
            root_dir: dir.to_path_buf(),
            ai: AiConfig::default(),
            skills: vec![],
            memory: MemoryConfig {
                namespace: format!("project/{}", dir.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")),
                shared_keys: vec!["conventions".to_string(), "architecture".to_string()],
                scope_id: default_scope_id(),  // 🔥 v1.1.7: 默认为空（第一次初始化时生成）
                display_name: None,       // 🔥 v1.1.7: 可选
            },
            extra: HashMap::new(),
        };

        // 保存配置
        let config_path = cis_dir.join("project.toml");
        let content = toml::to_string_pretty(&config)
            .map_err(|e| CisError::configuration(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(&config_path, content)?;

        // 创建本地 skills 目录
        let local_skills_dir = cis_dir.join("skills");
        std::fs::create_dir_all(&local_skills_dir)?;

        Ok(Self {
            config,
            local_skills_dir,
        })
    }

    /// 保存配置
    pub fn save(&self) -> Result<()> {
        let config_path = self.config.root_dir.join(".cis").join("project.toml");
        let content = toml::to_string_pretty(&self.config)
            .map_err(|e| CisError::configuration(format!("Failed to serialize config: {}", e)))?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    /// 获取记忆键前缀
    pub fn memory_prefix(&self) -> String {
        format!("{}/", self.config.memory.namespace)
    }

    /// 构建完整记忆键
    pub fn memory_key(&self, key: &str) -> String {
        if key.starts_with(&self.config.memory.namespace) {
            key.to_string()
        } else {
            format!("{}{}", self.memory_prefix(), key)
        }
    }

    /// 获取本地 Skill 路径
    pub fn local_skill_path(&self, name: &str) -> PathBuf {
        self.local_skills_dir.join(name)
    }

    /// 列出本地 Skills
    pub fn list_local_skills(&self) -> Result<Vec<LocalSkillInfo>> {
        let mut skills = vec![];

        if !self.local_skills_dir.exists() {
            return Ok(skills);
        }

        for entry in std::fs::read_dir(&self.local_skills_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                let name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                let manifest_path = path.join("skill.toml");
                let manifest = if manifest_path.exists() {
                    std::fs::read_to_string(&manifest_path).ok()
                } else {
                    None
                };

                skills.push(LocalSkillInfo {
                    name,
                    path,
                    manifest,
                });
            }
        }

        Ok(skills)
    }

    /// 获取 AI 引导上下文
    pub fn build_ai_guide(&self) -> String {
        let mut guide = self.config.ai.guide.clone();

        guide.push_str("\n\n=== Project Context ===\n");
        guide.push_str(&format!("Project: {} ({}\n", self.config.name, self.config.id));
        guide.push_str(&format!("Root: {}\n", self.config.root_dir.display()));

        if !self.config.memory.shared_keys.is_empty() {
            guide.push_str("\nShared Memory Keys:\n");
            for key in &self.config.memory.shared_keys {
                guide.push_str(&format!("- {}\n", key));
            }
        }

        if !self.config.skills.is_empty() {
            guide.push_str("\nAvailable Local Skills:\n");
            for skill in &self.config.skills {
                guide.push_str(&format!("- {} (auto_load: {})\n", skill.name, skill.auto_load));
            }
        }

        guide
    }
}

/// 本地 Skill 信息
#[derive(Debug, Clone)]
pub struct LocalSkillInfo {
    pub name: String,
    pub path: PathBuf,
    pub manifest: Option<String>,
}

impl ProjectManager {
    pub fn new() -> Self {
        Self { current: None }
    }

    /// 查找最近的父目录中的项目
    pub fn find_project(start_dir: &Path) -> Option<Project> {
        let mut current = Some(start_dir);

        while let Some(dir) = current {
            let cis_dir = dir.join(".cis");
            if cis_dir.join("project.toml").exists() {
                return Project::load(dir).ok();
            }
            current = dir.parent();
        }

        None
    }

    /// 加载项目
    pub fn load(&mut self, dir: &Path) -> Result<&Project> {
        let project = Project::load(dir)?;
        self.current = Some(project);
        Ok(self.current.as_ref().unwrap())
    }

    /// 初始化新项目
    pub fn init(&mut self, dir: &Path, name: impl Into<String>) -> Result<&Project> {
        let project = Project::init(dir, name)?;
        self.current = Some(project);
        Ok(self.current.as_ref().unwrap())
    }

    /// 获取当前项目
    pub fn current(&self) -> Option<&Project> {
        self.current.as_ref()
    }

    /// 获取当前项目（可变）
    pub fn current_mut(&mut self) -> Option<&mut Project> {
        self.current.as_mut()
    }

    /// 清除当前项目
    pub fn clear(&mut self) {
        self.current = None;
    }
}

impl Default for ProjectManager {
    fn default() -> Self {
        Self::new()
    }
}
