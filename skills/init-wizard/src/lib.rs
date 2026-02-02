//! Init Wizard Skill
//!
//! 初始化引导：帮助用户配置 AI 环境
//! 检查可用工具，生成配置建议

use std::process::Command;

/// AI 工具信息
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub command: String,
    pub install_url: String,
    pub description: String,
}

/// 初始化向导
pub struct InitWizard;

impl InitWizard {
    pub fn new() -> Self { Self }
    
    /// 运行完整检查
    pub fn run_check(&self) -> InitReport {
        let tools = vec![
            self.check_claude(),
            self.check_kimi(),
            self.check_aider(),
            self.check_codex(),
        ];
        
        InitReport { tools }
    }
    
    fn check_claude(&self) -> ToolCheck {
        let found = self.command_exists("claude");
        ToolCheck {
            tool: ToolInfo {
                name: "Claude Code".to_string(),
                command: "claude".to_string(),
                install_url: "https://docs.anthropic.com/en/docs/agents-and-tools/claude-code/overview".to_string(),
                description: "Anthropic Claude CLI - 默认推荐".to_string(),
            },
            found,
            suggestion: if found {
                "已安装，可直接使用".to_string()
            } else {
                "安装: npm install -g @anthropic-ai/claude-code".to_string()
            },
        }
    }
    
    fn check_kimi(&self) -> ToolCheck {
        let found = self.command_exists("kimi");
        ToolCheck {
            tool: ToolInfo {
                name: "Kimi Code".to_string(),
                command: "kimi".to_string(),
                install_url: "https://www.moonshot.cn/".to_string(),
                description: "Moonshot Kimi CLI".to_string(),
            },
            found,
            suggestion: if found {
                "已安装".to_string()
            } else {
                "请参考官方文档安装".to_string()
            },
        }
    }
    
    fn check_aider(&self) -> ToolCheck {
        let found = self.command_exists("aider");
        ToolCheck {
            tool: ToolInfo {
                name: "Aider".to_string(),
                command: "aider".to_string(),
                install_url: "https://aider.chat/".to_string(),
                description: "多模型 AI 编程助手".to_string(),
            },
            found,
            suggestion: if found {
                "已安装".to_string()
            } else {
                "安装: pip install aider-chat".to_string()
            },
        }
    }
    
    fn check_codex(&self) -> ToolCheck {
        let found = self.command_exists("codex");
        ToolCheck {
            tool: ToolInfo {
                name: "OpenAI Codex".to_string(),
                command: "codex".to_string(),
                install_url: "https://github.com/openai/codex".to_string(),
                description: "OpenAI CLI".to_string(),
            },
            found,
            suggestion: if found {
                "已安装".to_string()
            } else {
                "安装: npm install -g @openai/codex".to_string()
            },
        }
    }
    
    fn command_exists(&self, cmd: &str) -> bool {
        Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    /// 生成配置建议
    pub fn generate_config(&self, report: &InitReport) -> String {
        let mut config = String::from("# CIS AI Configuration\n\n[ai]\n");
        
        // 选择默认 provider
        let default_agent = report.tools.iter()
            .find(|t| t.found && t.tool.command == "claude")
            .map(|_| "claude")
            .or_else(|| report.tools.iter().find(|t| t.found).map(|t| t.tool.command.as_str()))
            .unwrap_or("claude");
        
        config.push_str(&format!("default_provider = \"{}\"\n\n", default_agent));
        
        // 添加每个工具的注释
        for check in &report.tools {
            if check.found {
                config.push_str(&format!(
                    "# {} - 可用\n# {}\n\n",
                    check.tool.name,
                    check.tool.description
                ));
            }
        }
        
        // 未安装的工具
        config.push_str("# 其他可用工具:\n");
        for check in &report.tools {
            if !check.found {
                config.push_str(&format!(
                    "# {}: {}\n#   安装: {}\n",
                    check.tool.name,
                    check.tool.description,
                    check.suggestion
                ));
            }
        }
        
        config
    }
}

impl Default for InitWizard {
    fn default() -> Self { Self::new() }
}

pub struct ToolCheck {
    pub tool: ToolInfo,
    pub found: bool,
    pub suggestion: String,
}

pub struct InitReport {
    pub tools: Vec<ToolCheck>,
}

// WASM 导出
#[no_mangle]
pub extern "C" fn skill_init() -> i32 {
    let wizard = InitWizard::new();
    let report = wizard.run_check();
    let config = wizard.generate_config(&report);
    
    // 输出到日志
    eprintln!("=== CIS AI Environment Check ===");
    for check in &report.tools {
        let status = if check.found { "✓" } else { "✗" };
        eprintln!("{} {}: {}", status, check.tool.name, check.suggestion);
    }
    eprintln!("\n=== Generated Config ===");
    eprintln!("{}", config);
    
    0
}

#[no_mangle]
pub extern "C" fn skill_check() -> i32 {
    skill_init()
}
