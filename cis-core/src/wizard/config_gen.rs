//! 配置生成模块

use crate::error::{CisError, Result};
use crate::storage::paths::Paths;
use std::collections::HashMap;

/// 配置生成器
pub struct ConfigGenerator;

impl ConfigGenerator {
    pub fn new() -> Self {
        Self
    }

    /// 生成全局配置
    pub fn generate_global_config(&self, preferred_provider: Option<&str>) -> Result<String> {
        // 检测可用的 AI Provider
        let provider = preferred_provider
            .map(|p| p.to_string())
            .or_else(|| self.detect_default_provider())
            .unwrap_or_else(|| "claude".to_string());

        let config = format!(r#"# CIS Global Configuration
# Generated at: {}

[node]
# 节点唯一标识（自动生成）
id = "{}"
# 节点名称
name = "{}"

[ai]
# 默认 AI Provider: claude | kimi | aider
default_provider = "{}"

[ai.claude]
# Claude Code 配置
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

[ai.kimi]
# Kimi Code 配置
model = "kimi-k2"
max_tokens = 8192

[storage]
# 自动备份数量
max_backups = 10
# 备份间隔（天）
backup_interval_days = 7

[sync]
# P2P 网络配置（预留）
enabled = false
bootstrap_nodes = []
"#,
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            uuid::Uuid::new_v4(),
            whoami::username(),
            provider
        );

        Ok(config)
    }

    /// 生成项目配置
    pub fn generate_project_config(
        &self,
        project_name: &str,
        project_id: &str,
    ) -> Result<String> {
        let config = format!(r#"# CIS Project Configuration
# Project: {}
# ID: {}

[project]
name = "{}"
id = "{}"

[ai]
# 项目级 AI 引导提示词
guide = """
You are working on the '{}' project with CIS integration.
This project uses CIS for task management, memory organization, and AI-assisted development.

Available Skills:
- memory-search: Search project memory
- task-manage: Create and manage tasks
- code-review: Review code changes

Project Conventions:
- Memory namespace: project/{}
- Shared keys: conventions, architecture
"""
# 覆盖全局默认 Provider
# provider = "claude"

[[skills]]
# 项目本地 Skill 示例
# name = "custom-linter"
# path = "./.cis/skills/custom-linter"
# auto_load = true

[memory]
# 记忆命名空间
namespace = "project/{}"
# 共享记忆键（可被 P2P 同步）
shared_keys = ["conventions", "architecture", "decisions"]
# 自动分类规则
auto_categorize = true

[task]
# 默认任务优先级
default_priority = "medium"
# 自动归档完成的任务
auto_archive = true
"#,
            project_name,
            project_id,
            project_name,
            project_id,
            project_name,
            project_id,
            project_id
        );

        Ok(config)
    }

    /// 生成 Skill Manifest 模板
    pub fn generate_skill_manifest(
        &self,
        name: &str,
        skill_type: &str,
    ) -> Result<String> {
        let manifest = format!(r#"[skill]
name = "{}"
version = "0.1.0"
description = "A CIS skill that..."
author = "{}"
type = "{}"  # native | wasm
entry = "{}"
license = "MIT"

[permissions]
memory_read = true
memory_write = true
ai_call = false
network = false
filesystem = false
command = false

[exports]
# WASM Skill 导出函数
functions = [
    "skill_init",
    "skill_handle_event",
    "skill_shutdown",
]

# 事件订阅
events = [
    "memory:write",
    "task:complete",
]

# CLI 命令
[[exports.commands]]
name = "example"
description = "An example command"
args = [
    {{ name = "input", description = "Input parameter", type = "string", required = true }},
]

[config]
# 配置项定义
[config.schema.example_option]
description = "An example configuration option"
type = "string"
required = false

[config.defaults]
# 默认值
example_option = "default_value"
"#,
            name,
            whoami::username(),
            skill_type,
            if skill_type == "wasm" { "skill.wasm" } else { "lib.rs" }
        );

        Ok(manifest)
    }

    /// 检测默认 Provider
    fn detect_default_provider(&self) -> Option<String> {
        let providers = vec![("claude", "claude"), ("kimi", "kimi"), ("aider", "aider")];

        for (cmd, name) in providers {
            if which::which(cmd).is_ok() {
                return Some(name.to_string());
            }
        }

        None
    }
}

impl Default for ConfigGenerator {
    fn default() -> Self {
        Self::new()
    }
}
