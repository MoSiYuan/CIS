//! # CIS Admin Skill
//!
//! CIS自身的元技能，提供系统管理和核心功能。
//!
//! ## 功能特性
//!
//! - 系统状态查询
//! - 配置管理
//! - 技能列表
//! - 模型切换

use crate::error::{CisError, Result};

use super::router::SkillVectorRouter;
use super::semantics::{SkillIoSignature, SkillScope, SkillSemanticsExt};

/// CIS自身的元技能 - 作为技能注册
pub struct CisAdminSkill;

impl CisAdminSkill {
    /// 获取技能ID
    pub fn skill_id() -> &'static str {
        "cis-local:admin"
    }

    /// 获取技能语义描述
    pub fn semantics() -> SkillSemanticsExt {
        SkillSemanticsExt {
            skill_id: Self::skill_id().to_string(),
            skill_name: "CIS System Administration".to_string(),
            description: "管理CIS系统状态、配置和核心功能".to_string(),
            example_intents: vec![
                "显示系统状态".to_string(),
                "配置向量存储".to_string(),
                "列出所有技能".to_string(),
                "切换AI模型".to_string(),
            ],
            parameter_schema: Some(serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["status", "config", "list-skills", "switch-model"]
                    }
                }
            })),
            io_signature: Some(SkillIoSignature {
                input_types: vec!["command".to_string()],
                output_types: vec!["status".to_string(), "list".to_string()],
                pipeable: false,
                source: false,
                sink: false,
            }),
            scope: SkillScope::Global,
        }
    }

    /// 执行管理命令
    pub async fn execute(
        action: &str,
        _params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        match action {
            "status" => Ok(serde_json::json!({
                "system": "CIS",
                "version": env!("CARGO_PKG_VERSION"),
                "status": "running",
            })),
            "list-skills" => {
                // 返回可用技能列表
                Ok(serde_json::json!({"skills": []}))
            }
            "config" => {
                // 读取/修改配置
                Ok(serde_json::json!({"config": {}}))
            }
            _ => Err(CisError::Skill(format!("Unknown action: {}", action))),
        }
    }
}

/// 便捷函数：注册CIS本地技能
pub fn register_cis_local_skills(registry: &mut SkillVectorRouter) {
    registry.register_global_skill(CisAdminSkill::semantics());
    registry.register_global_skill(CisFileSkill::semantics());
    registry.register_global_skill(CisReadSkill::semantics());
    registry.register_global_skill(CisAnalyzeSkill::semantics());
    registry.register_global_skill(CisCommitSkill::semantics());
}

// CIS本地技能定义

/// 文件列表技能
pub struct CisFileSkill;

impl CisFileSkill {
    /// 获取技能ID
    pub fn skill_id() -> &'static str {
        "cis-local:file-list"
    }

    /// 获取技能语义描述
    pub fn semantics() -> SkillSemanticsExt {
        SkillSemanticsExt {
            skill_id: Self::skill_id().to_string(),
            skill_name: "List Files".to_string(),
            description: "列出项目目录中的文件".to_string(),
            example_intents: vec!["列出文件".to_string(), "显示目录".to_string()],
            parameter_schema: None,
            io_signature: Some(SkillIoSignature {
                input_types: vec![],
                output_types: vec!["file_list".to_string()],
                pipeable: true,
                source: true,
                sink: false,
            }),
            scope: SkillScope::Global,
        }
    }
}

/// 读取文件技能
pub struct CisReadSkill;

impl CisReadSkill {
    /// 获取技能ID
    pub fn skill_id() -> &'static str {
        "cis-local:read"
    }

    /// 获取技能语义描述
    pub fn semantics() -> SkillSemanticsExt {
        SkillSemanticsExt {
            skill_id: Self::skill_id().to_string(),
            skill_name: "Read File".to_string(),
            description: "读取文件内容".to_string(),
            example_intents: vec!["读取文件".to_string(), "显示内容".to_string()],
            parameter_schema: None,
            io_signature: Some(SkillIoSignature {
                input_types: vec!["file_path".to_string()],
                output_types: vec!["content".to_string()],
                pipeable: true,
                source: false,
                sink: false,
            }),
            scope: SkillScope::Global,
        }
    }
}

/// 分析代码技能
pub struct CisAnalyzeSkill;

impl CisAnalyzeSkill {
    /// 获取技能ID
    pub fn skill_id() -> &'static str {
        "cis-local:analyze"
    }

    /// 获取技能语义描述
    pub fn semantics() -> SkillSemanticsExt {
        SkillSemanticsExt {
            skill_id: Self::skill_id().to_string(),
            skill_name: "Analyze Code".to_string(),
            description: "分析代码结构和质量".to_string(),
            example_intents: vec!["分析代码".to_string(), "检查质量".to_string()],
            parameter_schema: None,
            io_signature: Some(SkillIoSignature {
                input_types: vec!["content".to_string()],
                output_types: vec!["analysis".to_string()],
                pipeable: true,
                source: false,
                sink: false,
            }),
            scope: SkillScope::Global,
        }
    }
}

/// 提交变更技能
pub struct CisCommitSkill;

impl CisCommitSkill {
    /// 获取技能ID
    pub fn skill_id() -> &'static str {
        "cis-local:commit"
    }

    /// 获取技能语义描述
    pub fn semantics() -> SkillSemanticsExt {
        SkillSemanticsExt {
            skill_id: Self::skill_id().to_string(),
            skill_name: "Commit Changes".to_string(),
            description: "提交代码变更".to_string(),
            example_intents: vec!["提交代码".to_string(), "创建commit".to_string()],
            parameter_schema: None,
            io_signature: Some(SkillIoSignature {
                input_types: vec!["analysis".to_string()],
                output_types: vec!["commit".to_string()],
                pipeable: true,
                source: false,
                sink: true,
            }),
            scope: SkillScope::Global,
        }
    }
}
