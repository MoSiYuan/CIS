//! 环境检查模块

use crate::error::Result;
use std::process::Command;

/// 环境检查器
pub struct EnvironmentChecker;

/// 检查结果
#[derive(Debug, Clone)]
pub struct CheckResult {
    pub can_proceed: bool,
    pub warnings: Vec<String>,
    pub recommendations: Vec<String>,
    pub tools: Vec<ToolStatus>,
}

/// 工具状态
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

impl EnvironmentChecker {
    pub fn new() -> Self {
        Self
    }

    /// 运行所有检查
    pub fn run_all_checks(&self) -> Result<CheckResult> {
        let mut result = CheckResult {
            can_proceed: true,
            warnings: vec![],
            recommendations: vec![],
            tools: vec![],
        };

        // 检查 AI 工具
        result.tools.push(self.check_claude());
        result.tools.push(self.check_kimi());
        result.tools.push(self.check_aider());
        result.tools.push(self.check_git());

        // 检查是否有至少一个 AI 工具
        let has_ai_tool = result.tools.iter().any(|t| {
            t.installed && ["claude", "kimi", "aider"].contains(&t.name.as_str())
        });

        if !has_ai_tool {
            result.warnings.push(
                "No AI tool found. Please install Claude Code, Kimi, or Aider.".to_string()
            );
            result.can_proceed = false;
        }

        // 推荐默认工具
        if !result.tools.iter().any(|t| t.name == "claude" && t.installed) {
            result.recommendations.push(
                "Install Claude Code for best experience: npm install -g @anthropic-ai/claude-code"
                    .to_string()
            );
        }

        Ok(result)
    }

    /// 检查 Claude Code
    fn check_claude(&self) -> ToolStatus {
        match Command::new("claude").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                ToolStatus {
                    name: "claude".to_string(),
                    installed: true,
                    version: Some(version),
                    path: which::which("claude").ok().map(|p| p.to_string_lossy().to_string()),
                }
            }
            _ => ToolStatus {
                name: "claude".to_string(),
                installed: false,
                version: None,
                path: None,
            },
        }
    }

    /// 检查 Kimi
    fn check_kimi(&self) -> ToolStatus {
        match Command::new("kimi").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                ToolStatus {
                    name: "kimi".to_string(),
                    installed: true,
                    version: Some(version),
                    path: which::which("kimi").ok().map(|p| p.to_string_lossy().to_string()),
                }
            }
            _ => ToolStatus {
                name: "kimi".to_string(),
                installed: false,
                version: None,
                path: None,
            },
        }
    }

    /// 检查 Aider
    fn check_aider(&self) -> ToolStatus {
        match Command::new("aider").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                ToolStatus {
                    name: "aider".to_string(),
                    installed: true,
                    version: Some(version),
                    path: which::which("aider").ok().map(|p| p.to_string_lossy().to_string()),
                }
            }
            _ => ToolStatus {
                name: "aider".to_string(),
                installed: false,
                version: None,
                path: None,
            },
        }
    }

    /// 检查 Git
    fn check_git(&self) -> ToolStatus {
        match Command::new("git").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .to_string();
                ToolStatus {
                    name: "git".to_string(),
                    installed: true,
                    version: Some(version),
                    path: which::which("git").ok().map(|p| p.to_string_lossy().to_string()),
                }
            }
            _ => ToolStatus {
                name: "git".to_string(),
                installed: false,
                version: None,
                path: None,
            },
        }
    }

    /// 检查目录权限
    pub fn check_directory(&self, path: &std::path::Path) -> Result<bool> {
        // 检查是否可写
        match std::fs::metadata(path) {
            Ok(_) => {
                // 尝试创建测试文件
                let test_file = path.join(".cis_write_test");
                match std::fs::write(&test_file, b"") {
                    Ok(_) => {
                        let _ = std::fs::remove_file(&test_file);
                        Ok(true)
                    }
                    Err(_) => Ok(false),
                }
            }
            Err(_) => Ok(false),
        }
    }
}

impl Default for EnvironmentChecker {
    fn default() -> Self {
        Self::new()
    }
}
