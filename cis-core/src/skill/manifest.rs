//! # Skill Manifest 标准
//!
//! 定义 skill.toml 标准格式，用于 Skill 元数据和配置。
//!
//! ## DAG Skill 示例 (skill.toml)
//!
//! ```toml
//! [skill]
//! name = "comprehensive-code-review"
//! version = "1.0.0"
//! type = "dag"
//! description = "完整的代码审查流程"
//!
//! [dag]
//! policy = "all_success"
//!
//! [[dag.tasks]]
//! id = "1"
//! name = "获取代码变更"
//! skill = "git-diff"
//! level = { type = "mechanical", retry = 3 }
//!
//! [[dag.tasks]]
//! id = "2"
//! name = "AI 分析代码"
//! skill = "ai-code-analyze"
//! deps = ["1"]
//! level = { type = "confirmed" }
//!
//! [[dag.tasks]]
//! id = "3"
//! name = "生成报告"
//! skill = "report-generator"
//! deps = ["2"]
//! level = { type = "mechanical", retry = 3 }
//! rollback = ["rm report.md"]
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{CisError, Result};

/// Skill Manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Skill 基本信息
    pub skill: SkillInfo,
    /// 权限声明
    #[serde(default)]
    pub permissions: SkillPermissions,
    /// 导出函数
    #[serde(default)]
    pub exports: SkillExports,
    /// 配置 Schema
    #[serde(default)]
    pub config: SkillConfigSchema,
    /// 依赖
    #[serde(default)]
    pub dependencies: Vec<SkillDependency>,
    /// DAG 定义（仅当 skill_type = Dag 时有效）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dag: Option<DagDefinition>,
}

/// Skill 基本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    /// 名称（唯一标识）
    pub name: String,
    /// 版本 (semver)
    pub version: String,
    /// 描述
    pub description: String,
    /// 作者
    pub author: String,
    /// 类型
    #[serde(rename = "type")]
    pub skill_type: SkillType,
    /// 入口文件
    #[serde(default = "default_entry")]
    pub entry: String,
    /// 图标
    pub icon: Option<String>,
    /// 主页
    pub homepage: Option<String>,
    /// 仓库
    pub repository: Option<String>,
    /// 许可证
    pub license: Option<String>,
}

fn default_entry() -> String {
    "lib.rs".to_string()
}

/// Skill 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SkillType {
    /// 本地编译的 Skill
    Native,
    /// WASM 沙箱 Skill
    Wasm,
    /// 脚本 Skill
    Script,
    /// DAG 编排 Skill
    Dag,
}

/// Skill 权限
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillPermissions {
    /// 读取记忆
    #[serde(default)]
    pub memory_read: bool,
    /// 写入记忆
    #[serde(default)]
    pub memory_write: bool,
    /// 调用 AI
    #[serde(default)]
    pub ai_call: bool,
    /// 网络访问
    #[serde(default)]
    pub network: bool,
    /// 文件系统访问
    #[serde(default)]
    pub filesystem: bool,
    /// 执行命令
    #[serde(default)]
    pub command: bool,
    /// 自定义权限
    #[serde(default)]
    pub custom: Vec<String>,
}

impl SkillPermissions {
    /// 检查是否有特定权限
    pub fn has(&self, perm: &str) -> bool {
        match perm {
            "memory_read" => self.memory_read,
            "memory_write" => self.memory_write,
            "ai_call" => self.ai_call,
            "network" => self.network,
            "filesystem" => self.filesystem,
            "command" => self.command,
            _ => self.custom.contains(&perm.to_string()),
        }
    }

    /// 验证权限（是否满足最小要求）
    pub fn validate(&self, required: &SkillPermissions) -> bool {
        (!required.memory_read || self.memory_read)
            && (!required.memory_write || self.memory_write)
            && (!required.ai_call || self.ai_call)
            && (!required.network || self.network)
            && (!required.filesystem || self.filesystem)
            && (!required.command || self.command)
    }
}

/// Skill 导出函数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillExports {
    /// 导出函数列表
    #[serde(default)]
    pub functions: Vec<String>,
    /// 事件处理
    #[serde(default)]
    pub events: Vec<String>,
    /// 命令
    #[serde(default)]
    pub commands: Vec<SkillCommand>,
}

/// Skill 命令定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCommand {
    pub name: String,
    pub description: String,
    pub args: Vec<CommandArg>,
}

/// 命令参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandArg {
    pub name: String,
    pub description: String,
    #[serde(rename = "type")]
    pub arg_type: String,
    pub required: bool,
    pub default: Option<serde_json::Value>,
}

/// Skill 配置 Schema
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillConfigSchema {
    /// 配置项定义
    #[serde(default)]
    pub schema: HashMap<String, ConfigField>,
    /// 默认值
    #[serde(default)]
    pub defaults: HashMap<String, serde_json::Value>,
}

/// 配置字段定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub description: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub required: bool,
}

/// Skill 依赖
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDependency {
    pub name: String,
    pub version: String,
    pub optional: bool,
}

/// 任务级别定义（用于序列化）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskLevelDefinition {
    Mechanical { retry: u8 },
    Recommended { timeout: u16, default_action: String },
    Confirmed,
    Arbitrated { stakeholders: Vec<String> },
}

fn default_task_level() -> TaskLevelDefinition {
    TaskLevelDefinition::Mechanical { retry: 3 }
}

/// DAG 任务定义（用于复合 Skill）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagTaskDefinition {
    /// 任务 ID
    pub id: String,
    /// 任务名称
    pub name: String,
    /// 要调用的 Skill ID
    pub skill: String,
    /// 依赖的任务 ID 列表
    #[serde(default)]
    pub deps: Vec<String>,
    /// 四级决策级别
    #[serde(default = "default_task_level")]
    pub level: TaskLevelDefinition,
    /// 重试次数（仅 Mechanical 级别有效）
    #[serde(default = "default_retry")]
    pub retry: u8,
    /// 超时（秒）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// 回滚命令
    #[serde(default)]
    pub rollback: Vec<String>,
    /// 是否幂等
    #[serde(default)]
    pub idempotent: bool,
}

fn default_retry() -> u8 {
    3
}

fn default_timeout() -> u64 {
    300
}

/// DAG 执行策略
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DagPolicy {
    /// 所有任务必须成功
    #[default]
    AllSuccess,
    /// 第一个成功即可
    FirstSuccess,
    /// 允许失败（债务模式）
    AllowDebt,
}

/// DAG 定义（复合 Skill）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DagDefinition {
    /// 执行策略
    #[serde(default)]
    pub policy: DagPolicy,
    /// 任务列表
    pub tasks: Vec<DagTaskDefinition>,
}

impl DagDefinition {
    /// 转换为 TaskDag（用于调度器执行）
    pub fn to_dag(&self) -> crate::Result<crate::scheduler::TaskDag> {
        use crate::scheduler::TaskDag;
        use crate::error::CisError;
        
        let mut dag = TaskDag::new();

        for task in &self.tasks {
            // 将 TaskLevelDefinition 转换为 TaskLevel
            let level = task.level.clone().into();
            
            // 处理回滚命令
            let rollback = if task.rollback.is_empty() {
                None
            } else {
                Some(task.rollback.clone())
            };
            
            dag.add_node_with_rollback(
                task.id.clone(),
                task.deps.clone(),
                level,
                rollback,
            )
            .map_err(|e| CisError::scheduler(format!("Failed to add node: {}", e)))?;
        }

        // 验证 DAG
        dag.validate()
            .map_err(|e| CisError::scheduler(format!("DAG validation failed: {}", e)))?;

        // 初始化节点状态
        dag.initialize();

        Ok(dag)
    }
}

impl From<TaskLevelDefinition> for crate::types::TaskLevel {
    fn from(level: TaskLevelDefinition) -> Self {
        use crate::types::{TaskLevel, Action};
        
        match level {
            TaskLevelDefinition::Mechanical { retry } => TaskLevel::Mechanical { retry },
            TaskLevelDefinition::Recommended { timeout, default_action } => {
                let action = match default_action.as_str() {
                    "execute" => Action::Execute,
                    "skip" => Action::Skip,
                    "abort" => Action::Abort,
                    _ => Action::Execute,
                };
                TaskLevel::Recommended { 
                    default_action: action, 
                    timeout_secs: timeout 
                }
            }
            TaskLevelDefinition::Confirmed => TaskLevel::Confirmed,
            TaskLevelDefinition::Arbitrated { stakeholders } => {
                TaskLevel::Arbitrated { stakeholders }
            }
        }
    }
}

impl SkillManifest {
    /// 从文件加载
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CisError::storage(format!("Failed to read manifest: {}", e)))?;

        Self::from_str(&content)
    }

    /// 从字符串解析
    pub fn from_str(content: &str) -> Result<Self> {
        toml::from_str(content)
            .map_err(|e| CisError::configuration(format!("Failed to parse manifest: {}", e)))
    }

    /// 从 DAG 文件加载 Skill
    /// 
    /// 支持两种格式：
    /// 1. TOML 格式的 skill.toml（包含 [skill] 和 [dag] 部分）
    /// 2. JSON 格式的纯 DAG 定义（仅包含 DAG 结构）
    pub fn from_dag_file(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| CisError::storage(format!("Failed to read DAG file: {}", e)))?;

        // 尝试作为 TOML skill.toml 解析
        if path.extension().map(|e| e == "toml").unwrap_or(false) {
            let manifest: SkillManifest = toml::from_str(&content)
                .map_err(|e| CisError::configuration(format!("Failed to parse TOML: {}", e)))?;

            if !manifest.is_dag_skill() {
                return Err(CisError::configuration(
                    "Skill type is not 'dag'".to_string()
                ));
            }

            return Ok(manifest);
        }

        // 尝试作为 JSON 解析（纯 DAG 定义）
        let dag_def: DagDefinition = serde_json::from_str(&content)
            .map_err(|e| CisError::configuration(format!("Failed to parse DAG JSON: {}", e)))?;

        // 从文件名生成 skill 名称
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unnamed-dag")
            .to_string();

        Ok(Self {
            skill: SkillInfo {
                name: name.clone(),
                version: "1.0.0".to_string(),
                description: format!("DAG skill: {}", name),
                author: "Anonymous".to_string(),
                skill_type: SkillType::Dag,
                entry: path.to_string_lossy().to_string(),
                icon: None,
                homepage: None,
                repository: None,
                license: None,
            },
            permissions: SkillPermissions::default(),
            exports: SkillExports::default(),
            config: SkillConfigSchema::default(),
            dependencies: Vec::new(),
            dag: Some(dag_def),
        })
    }

    /// 保存到文件
    pub fn to_file(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .map_err(|e| CisError::configuration(format!("Failed to serialize manifest: {}", e)))?;

        std::fs::write(path, content)
            .map_err(|e| CisError::storage(format!("Failed to write manifest: {}", e)))?;

        Ok(())
    }

    /// 生成默认 Manifest
    pub fn default_native(name: impl Into<String>) -> Self {
        Self {
            skill: SkillInfo {
                name: name.into(),
                version: "0.1.0".to_string(),
                description: "A CIS skill".to_string(),
                author: "Anonymous".to_string(),
                skill_type: SkillType::Native,
                entry: "lib.rs".to_string(),
                icon: None,
                homepage: None,
                repository: None,
                license: Some("MIT".to_string()),
            },
            permissions: SkillPermissions {
                memory_read: true,
                memory_write: false,
                ai_call: false,
                network: false,
                filesystem: false,
                command: false,
                custom: vec![],
            },
            exports: SkillExports::default(),
            config: SkillConfigSchema::default(),
            dependencies: vec![],
            dag: None,
        }
    }

    /// 生成 WASM Skill 默认 Manifest
    pub fn default_wasm(name: impl Into<String>) -> Self {
        let mut manifest = Self::default_native(name);
        manifest.skill.skill_type = SkillType::Wasm;
        manifest.skill.entry = "skill.wasm".to_string();
        manifest.exports = SkillExports {
            functions: vec![
                "skill_init".to_string(),
                "skill_handle_event".to_string(),
                "skill_shutdown".to_string(),
            ],
            events: vec![],
            commands: vec![],
        };
        manifest
    }

    /// 设置 Skill 为 DAG 类型
    pub fn as_dag(mut self, dag: DagDefinition) -> Self {
        self.skill.skill_type = SkillType::Dag;
        self.dag = Some(dag);
        self
    }

    /// 添加 DAG 任务
    pub fn with_dag_task(mut self, task: DagTaskDefinition) -> Self {
        if let Some(ref mut dag) = self.dag {
            dag.tasks.push(task);
        } else {
            self.dag = Some(DagDefinition {
                policy: DagPolicy::AllSuccess,
                tasks: vec![task],
            });
            self.skill.skill_type = SkillType::Dag;
        }
        self
    }

    /// 检查是否为 DAG Skill
    pub fn is_dag_skill(&self) -> bool {
        self.skill.skill_type == SkillType::Dag
    }

    /// 获取 DAG 定义
    pub fn dag(&self) -> Option<&DagDefinition> {
        self.dag.as_ref()
    }
}

impl DagTaskDefinition {
    /// 创建新的 DAG 任务定义
    pub fn new(id: impl Into<String>, skill: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            skill: skill.into(),
            deps: Vec::new(),
            level: default_task_level(),
            retry: default_retry(),
            timeout_secs: default_timeout(),
            rollback: Vec::new(),
            idempotent: false,
        }
    }

    /// 设置名称
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// 设置依赖
    pub fn with_deps(mut self, deps: Vec<impl Into<String>>) -> Self {
        self.deps = deps.into_iter().map(|s| s.into()).collect();
        self
    }

    /// 设置为 Mechanical 级别
    pub fn mechanical(mut self, retry: u8) -> Self {
        self.level = TaskLevelDefinition::Mechanical { retry };
        self
    }

    /// 设置为 Confirmed 级别
    pub fn confirmed(mut self) -> Self {
        self.level = TaskLevelDefinition::Confirmed;
        self
    }

    /// 设置回滚命令
    pub fn with_rollback(mut self, commands: Vec<impl Into<String>>) -> Self {
        self.rollback = commands.into_iter().map(|s| s.into()).collect();
        self
    }
}

/// Skill Manifest 验证器
pub struct ManifestValidator;

impl ManifestValidator {
    /// 验证 Manifest
    pub fn validate(manifest: &SkillManifest) -> Result<Vec<String>> {
        let mut errors = vec![];

        // 验证名称
        if manifest.skill.name.is_empty() {
            errors.push("Skill name cannot be empty".to_string());
        }
        if manifest.skill.name.contains(' ') {
            errors.push("Skill name cannot contain spaces".to_string());
        }

        // 验证版本
        if !Self::is_valid_semver(&manifest.skill.version) {
            errors.push(format!(
                "Invalid version format: {} (expected semver)",
                manifest.skill.version
            ));
        }

        // 验证导出函数
        if manifest.skill.skill_type == SkillType::Wasm {
            let required = vec!["skill_init", "skill_handle_event"];
            for func in required {
                if !manifest.exports.functions.contains(&func.to_string()) {
                    errors.push(format!("WASM skill must export function: {}", func));
                }
            }
        }

        Ok(errors)
    }

    /// 检查是否为有效 semver
    fn is_valid_semver(version: &str) -> bool {
        // 简单检查: x.y.z
        version.split('.').count() == 3
            && version.split('.').all(|s| s.parse::<u32>().is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_parse() {
        let toml = r#"
[skill]
name = "test-skill"
version = "1.0.0"
description = "A test skill"
author = "Test Author"
type = "wasm"

[permissions]
memory_read = true
memory_write = true
ai_call = false

[exports]
functions = ["skill_init", "skill_handle_event"]
"#;

        let manifest = SkillManifest::from_str(toml).unwrap();
        assert_eq!(manifest.skill.name, "test-skill");
        assert_eq!(manifest.skill.skill_type, SkillType::Wasm);
        assert!(manifest.permissions.memory_read);
    }

    #[test]
    fn test_manifest_validation() {
        let manifest = SkillManifest::default_wasm("my-skill");
        let errors = ManifestValidator::validate(&manifest).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_dag_skill_manifest() {
        let toml = r#"
[skill]
name = "comprehensive-code-review"
version = "1.0.0"
type = "dag"
description = "完整的代码审查流程"
author = "Test Author"

[dag]
policy = "all_success"

[[dag.tasks]]
id = "1"
name = "获取代码变更"
skill = "git-diff"
level = { type = "mechanical", retry = 3 }

[[dag.tasks]]
id = "2"
name = "AI 分析代码"
skill = "ai-code-analyze"
deps = ["1"]
level = { type = "confirmed" }
"#;

        let manifest = SkillManifest::from_str(toml).unwrap();
        assert_eq!(manifest.skill.name, "comprehensive-code-review");
        assert_eq!(manifest.skill.skill_type, SkillType::Dag);
        assert!(manifest.is_dag_skill());
        
        let dag = manifest.dag().unwrap();
        assert_eq!(dag.tasks.len(), 2);
        assert_eq!(dag.tasks[0].id, "1");
        assert_eq!(dag.tasks[1].deps, vec!["1"]);
    }

    #[test]
    fn test_dag_task_definition_builder() {
        let task = DagTaskDefinition::new("task1", "test-skill")
            .with_name("测试任务")
            .with_deps(vec!["dep1", "dep2"])
            .mechanical(5)
            .with_rollback(vec!["rollback-cmd"]);
        
        assert_eq!(task.id, "task1");
        assert_eq!(task.skill, "test-skill");
        assert_eq!(task.name, "测试任务");
        assert_eq!(task.deps, vec!["dep1", "dep2"]);
        assert_eq!(task.rollback, vec!["rollback-cmd"]);
    }
}
