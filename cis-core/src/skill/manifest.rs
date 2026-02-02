//! # Skill Manifest 标准
//!
//! 定义 skill.toml 标准格式，用于 Skill 元数据和配置。

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
    Native,
    Wasm,
    Script,
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
                    errors.push(format!(
                        "WASM skill must export function: {}",
                        func
                    ));
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
}
