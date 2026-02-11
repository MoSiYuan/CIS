//! Agent 命令白名单系统
//!
//! 提供命令分类验证，支持安全/危险/禁止三级分类。
//! 支持从 YAML 配置文件加载规则，支持通配符模式匹配。

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{CisError, Result};

/// 命令分类
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommandClass {
    /// 安全命令：只读操作
    Safe,
    /// 危险命令：需要确认
    Dangerous,
    /// 禁止命令：不允许执行
    Forbidden,
}

impl CommandClass {
    /// 获取分类的显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            CommandClass::Safe => "safe",
            CommandClass::Dangerous => "dangerous",
            CommandClass::Forbidden => "forbidden",
        }
    }

    /// 是否需要用户确认
    pub fn requires_confirmation(&self) -> bool {
        matches!(self, CommandClass::Dangerous)
    }
}

impl std::fmt::Display for CommandClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// 允许命令模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedPattern {
    /// 命令模式（支持通配符 *）
    pub pattern: String,
    /// 命令分类
    #[serde(default = "default_safe_class")]
    pub class: CommandClass,
    /// 是否需要确认（危险命令）
    #[serde(default)]
    pub requires_confirmation: bool,
    /// 可选的描述
    pub description: Option<String>,
}

fn default_safe_class() -> CommandClass {
    CommandClass::Safe
}

/// 禁止命令模式配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeniedPattern {
    /// 命令模式
    pub pattern: String,
    /// 拒绝原因
    pub reason: String,
}

/// 命令白名单配置（YAML 格式）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WhitelistConfig {
    /// 允许的命令模式列表
    #[serde(default)]
    pub allowed: Vec<AllowedPattern>,
    /// 禁止的命令模式列表
    #[serde(default)]
    pub denied: Vec<DeniedPattern>,
}

/// 编译后的命令模式
#[derive(Debug, Clone)]
struct CompiledPattern {
    /// 原始模式字符串
    pattern: String,
    /// 编译后的正则表达式
    regex: Regex,
    /// 命令分类（允许的模式）
    class: Option<CommandClass>,
    /// 是否需要确认
    requires_confirmation: bool,
    /// 拒绝原因（禁止的模式）
    reason: Option<String>,
    /// 描述
    description: Option<String>,
}

/// 命令白名单验证器
#[derive(Debug, Clone)]
pub struct CommandWhitelist {
    /// 允许的命令模式
    allowed: Vec<CompiledPattern>,
    /// 禁止的命令模式
    denied: Vec<CompiledPattern>,
    /// 缓存：命令字符串 -> 验证结果
    cache: HashMap<String, ValidationResult>,
}

/// 验证结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    /// 是否允许执行
    pub allowed: bool,
    /// 命令分类
    pub class: Option<CommandClass>,
    /// 是否需要确认
    pub requires_confirmation: bool,
    /// 拒绝原因（如果不允许）
    pub reason: Option<String>,
    /// 匹配的模式
    pub matched_pattern: Option<String>,
}

impl ValidationResult {
    /// 创建一个允许的结果
    fn allowed(class: CommandClass, requires_confirmation: bool, pattern: String) -> Self {
        Self {
            allowed: true,
            class: Some(class),
            requires_confirmation,
            reason: None,
            matched_pattern: Some(pattern),
        }
    }

    /// 创建一个禁止的结果
    fn denied(reason: String, pattern: String) -> Self {
        Self {
            allowed: false,
            class: Some(CommandClass::Forbidden),
            requires_confirmation: true, // 禁止的命令也要求确认（保守处理）
            reason: Some(reason),
            matched_pattern: Some(pattern),
        }
    }

    /// 创建一个默认拒绝的结果（没有匹配任何规则）
    fn default_denied() -> Self {
        Self {
            allowed: false,
            class: Some(CommandClass::Forbidden),
            requires_confirmation: true, // 未匹配的命令也要求确认（保守处理）
            reason: Some("Command not in whitelist".to_string()),
            matched_pattern: None,
        }
    }
}

impl CommandWhitelist {
    /// 创建空的命令白名单（默认拒绝所有命令）
    pub fn empty() -> Self {
        Self {
            allowed: Vec::new(),
            denied: Vec::new(),
            cache: HashMap::new(),
        }
    }

    /// 从配置文件加载白名单
    pub fn from_file(path: &str) -> Result<Self> {
        let path = Path::new(path);
        if !path.exists() {
            return Err(CisError::configuration(format!(
                "Whitelist config file not found: {}",
                path.display()
            )));
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| CisError::configuration(format!("Failed to read whitelist config: {}", e)))?;

        Self::from_yaml(&content)
    }

    /// 从 YAML 字符串加载白名单
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        let config: WhitelistConfig = serde_yaml::from_str(yaml)
            .map_err(|e| CisError::configuration(format!("Failed to parse whitelist YAML: {}", e)))?;

        Self::from_config(config)
    }

    /// 从配置结构创建白名单
    pub fn from_config(config: WhitelistConfig) -> Result<Self> {
        let mut allowed = Vec::new();
        let mut denied = Vec::new();

        // 编译允许的模式
        for pattern in config.allowed {
            let regex = Self::pattern_to_regex(&pattern.pattern)
                .map_err(|e| CisError::configuration(format!("Invalid pattern '{}': {}", pattern.pattern, e)))?;
            
            allowed.push(CompiledPattern {
                pattern: pattern.pattern,
                regex,
                class: Some(pattern.class),
                requires_confirmation: pattern.requires_confirmation || pattern.class.requires_confirmation(),
                reason: None,
                description: pattern.description,
            });
        }

        // 编译禁止的模式
        for pattern in config.denied {
            let regex = Self::pattern_to_regex(&pattern.pattern)
                .map_err(|e| CisError::configuration(format!("Invalid pattern '{}': {}", pattern.pattern, e)))?;
            
            denied.push(CompiledPattern {
                pattern: pattern.pattern,
                regex,
                class: Some(CommandClass::Forbidden),
                requires_confirmation: false,
                reason: Some(pattern.reason),
                description: None,
            });
        }

        Ok(Self {
            allowed,
            denied,
            cache: HashMap::new(),
        })
    }

    /// 将通配符模式转换为正则表达式
    ///
    /// 支持：
    /// - `*` 匹配任意字符序列（包括空）
    /// - `?` 匹配单个字符
    fn pattern_to_regex(pattern: &str) -> Result<Regex> {
        // 转义正则特殊字符
        let mut regex_str = String::with_capacity(pattern.len() * 2);
        regex_str.push('^');

        for ch in pattern.chars() {
            match ch {
                '*' => regex_str.push_str(".*"),
                '?' => regex_str.push('.'),
                '.' => regex_str.push_str("\\."),
                '+' => regex_str.push_str("\\+"),
                '(' => regex_str.push_str("\\("),
                ')' => regex_str.push_str("\\)"),
                '[' => regex_str.push_str("\\["),
                ']' => regex_str.push_str("\\]"),
                '{' => regex_str.push_str("\\{"),
                '}' => regex_str.push_str("\\}"),
                '^' => regex_str.push_str("\\^"),
                '$' => regex_str.push_str("\\$"),
                '\\' => regex_str.push_str("\\\\"),
                '|' => regex_str.push_str("\\|"),
                c => regex_str.push(c),
            }
        }

        regex_str.push('$');

        Regex::new(&regex_str)
            .map_err(|e| CisError::configuration(format!("Regex compilation failed: {}", e)))
    }

    /// 验证命令是否允许执行
    ///
    /// # Arguments
    /// * `command` - 命令名称（如 "ls", "rm"）
    /// * `args` - 命令参数列表
    ///
    /// # Returns
    /// * `Ok(ValidationResult)` - 验证通过，包含分类和确认要求
    /// * `Err(CisError)` - 验证失败，包含拒绝原因
    pub fn validate(&self, command: &str, args: &[&str]) -> Result<ValidationResult> {
        let full_command = Self::build_command_string(command, args);
        
        // 检查缓存
        if let Some(result) = self.cache.get(&full_command) {
            return Ok(result.clone());
        }

        // 首先检查禁止列表（优先级最高）
        for pattern in &self.denied {
            if pattern.regex.is_match(&full_command) {
                let reason = pattern.reason.clone().unwrap_or_else(|| "Command denied".to_string());
                let result = ValidationResult::denied(reason, pattern.pattern.clone());
                return Ok(result);
            }
        }

        // 然后检查允许列表
        for pattern in &self.allowed {
            if pattern.regex.is_match(&full_command) {
                let class = pattern.class.unwrap_or(CommandClass::Safe);
                let result = ValidationResult::allowed(
                    class,
                    pattern.requires_confirmation,
                    pattern.pattern.clone(),
                );
                return Ok(result);
            }
        }

        // 没有匹配任何规则，默认拒绝
        Ok(ValidationResult::default_denied())
    }

    /// 验证命令并返回详细的用户友好错误信息
    pub fn validate_with_explanation(&self, command: &str, args: &[&str]) -> Result<ValidationResult> {
        let result = self.validate(command, args)?;
        
        if !result.allowed {
            let full_command = Self::build_command_string(command, args);
            let reason = result.reason.as_deref().unwrap_or("Command not allowed");
            
            return Err(CisError::execution(format!(
                "Command '{}' rejected: {}. Matched pattern: {:?}",
                full_command,
                reason,
                result.matched_pattern
            )));
        }

        Ok(result)
    }

    /// 检查命令是否需要用户确认
    pub fn requires_confirmation(&self, command: &str, args: &[&str]) -> bool {
        match self.validate(command, args) {
            Ok(result) => result.requires_confirmation,
            Err(_) => true, // 验证出错时保守处理，要求确认
        }
    }

    /// 获取命令的分类
    pub fn get_command_class(&self, command: &str, args: &[&str]) -> CommandClass {
        match self.validate(command, args) {
            Ok(result) => result.class.unwrap_or(CommandClass::Forbidden),
            Err(_) => CommandClass::Forbidden,
        }
    }

    /// 构建完整命令字符串
    fn build_command_string(command: &str, args: &[&str]) -> String {
        if args.is_empty() {
            command.to_string()
        } else {
            format!("{} {}", command, args.join(" "))
        }
    }

    /// 获取默认白名单配置
    pub fn default_config() -> WhitelistConfig {
        serde_yaml::from_str(DEFAULT_CONFIG).expect("Default config should be valid YAML")
    }

    /// 加载默认白名单
    pub fn default() -> Self {
        Self::from_config(Self::default_config())
            .expect("Default config should compile successfully")
    }

    /// 添加允许的命令模式
    pub fn allow(&mut self, pattern: &str, class: CommandClass) -> Result<()> {
        let regex = Self::pattern_to_regex(pattern)?;
        self.allowed.push(CompiledPattern {
            pattern: pattern.to_string(),
            regex,
            class: Some(class),
            requires_confirmation: class.requires_confirmation(),
            reason: None,
            description: None,
        });
        Ok(())
    }

    /// 添加禁止的命令模式
    pub fn deny(&mut self, pattern: &str, reason: &str) -> Result<()> {
        let regex = Self::pattern_to_regex(pattern)?;
        self.denied.push(CompiledPattern {
            pattern: pattern.to_string(),
            regex,
            class: Some(CommandClass::Forbidden),
            requires_confirmation: false,
            reason: Some(reason.to_string()),
            description: None,
        });
        Ok(())
    }

    /// 列出所有允许的模式
    pub fn list_allowed(&self) -> Vec<(String, CommandClass, bool)> {
        self.allowed
            .iter()
            .map(|p| (
                p.pattern.clone(),
                p.class.unwrap_or(CommandClass::Safe),
                p.requires_confirmation,
            ))
            .collect()
    }

    /// 列出所有禁止的模式
    pub fn list_denied(&self) -> Vec<(String, String)> {
        self.denied
            .iter()
            .map(|p| (p.pattern.clone(), p.reason.clone().unwrap_or_default()))
            .collect()
    }
}

impl Default for CommandWhitelist {
    fn default() -> Self {
        Self::default()
    }
}

/// 默认安全配置
const DEFAULT_CONFIG: &str = r#"
allowed:
  - pattern: "ls *"
    class: safe
    description: "List directory contents"
  - pattern: "cat *"
    class: safe
    description: "Display file contents"
  - pattern: "grep *"
    class: safe
    description: "Search text patterns"
  - pattern: "find *"
    class: safe
    description: "Find files and directories"
  - pattern: "cargo *"
    class: safe
    description: "Rust build tool"
  - pattern: "git *"
    class: safe
    description: "Git version control"
  - pattern: "pwd"
    class: safe
    description: "Print working directory"
  - pattern: "echo *"
    class: safe
    description: "Print text"
  - pattern: "head *"
    class: safe
    description: "Output first part of files"
  - pattern: "tail *"
    class: safe
    description: "Output last part of files"
  - pattern: "less *"
    class: safe
    description: "View file contents"
  - pattern: "more *"
    class: safe
    description: "View file contents"
  - pattern: "file *"
    class: safe
    description: "Determine file type"
  - pattern: "which *"
    class: safe
    description: "Locate a command"
  - pattern: "whereis *"
    class: safe
    description: "Locate binary, source, and manual page files"
  - pattern: "stat *"
    class: safe
    description: "Display file or file system status"
  - pattern: "du *"
    class: safe
    description: "Estimate file space usage"
  - pattern: "df *"
    class: safe
    description: "Report file system disk space usage"
  - pattern: "ps *"
    class: safe
    description: "Report process status"
  - pattern: "top"
    class: safe
    description: "Display processes"
  - pattern: "htop"
    class: safe
    description: "Interactive process viewer"
  - pattern: "uptime"
    class: safe
    description: "Show system uptime"
  - pattern: "whoami"
    class: safe
    description: "Print effective username"
  - pattern: "id"
    class: safe
    description: "Print user and group information"
  - pattern: "uname *"
    class: safe
    description: "Print system information"
  - pattern: "date"
    class: safe
    description: "Print system date and time"
  - pattern: "cal"
    class: safe
    description: "Display calendar"
  - pattern: "clear"
    class: safe
    description: "Clear terminal screen"
  - pattern: "history"
    class: safe
    description: "Show command history"
  - pattern: "man *"
    class: safe
    description: "Open manual pages"
  - pattern: "help"
    class: safe
    description: "Display help information"
  - pattern: "mkdir *"
    class: dangerous
    requires_confirmation: true
    description: "Create directories"
  - pattern: "cp *"
    class: dangerous
    requires_confirmation: true
    description: "Copy files and directories"
  - pattern: "mv *"
    class: dangerous
    requires_confirmation: true
    description: "Move/rename files and directories"
  - pattern: "touch *"
    class: dangerous
    requires_confirmation: true
    description: "Create empty files or update timestamps"
  - pattern: "chmod *"
    class: dangerous
    requires_confirmation: true
    description: "Change file permissions"
  - pattern: "chown *"
    class: dangerous
    requires_confirmation: true
    description: "Change file owner and group"
  - pattern: "rm *"
    class: dangerous
    requires_confirmation: true
    description: "Remove files or directories"

denied:
  - pattern: "sudo *"
    reason: "Privilege escalation not allowed for security reasons"
  - pattern: "su *"
    reason: "User switching not allowed for security reasons"
  - pattern: "rm -rf /"
    reason: "Destructive command that can destroy the system"
  - pattern: "rm -rf /*"
    reason: "Destructive command that can destroy the system"
  - pattern: ":(){ :|:& };:"
    reason: "Fork bomb that can crash the system"
  - pattern: "mkfs *"
    reason: "Filesystem creation can destroy data"
  - pattern: "dd *"
    reason: "Low-level disk operations can destroy data"
  - pattern: "fdisk *"
    reason: "Disk partitioning can destroy data"
  - pattern: "format *"
    reason: "Formatting can destroy data"
  - pattern: "> /dev/*"
    reason: "Direct device writes can destroy data"
  - pattern: "chmod 777 *"
    reason: "Insecure permissions that grant full access to everyone"
  - pattern: "chmod -R 777 *"
    reason: "Insecure recursive permissions that grant full access to everyone"
  - pattern: "wget * | sh"
    reason: "Piping remote content directly to shell is dangerous"
  - pattern: "curl * | sh"
    reason: "Piping remote content directly to shell is dangerous"
  - pattern: "nc *"
    reason: "Netcat can be used for unauthorized network access"
  - pattern: "netcat *"
    reason: "Netcat can be used for unauthorized network access"
  - pattern: "telnet *"
    reason: "Telnet transmits data in plain text"
  - pattern: "ssh *"
    reason: "SSH connections require explicit user authorization"
  - pattern: "scp *"
    reason: "Secure copy requires explicit user authorization"
  - pattern: "rsync *"
    reason: "Remote sync requires explicit user authorization"
  - pattern: "eval *"
    reason: "Eval can execute arbitrary code"
  - pattern: "exec *"
    reason: "Exec replaces the current process and can be dangerous"
  - pattern: "bash *"
    reason: "Explicit shell execution requires explicit authorization"
  - pattern: "sh *"
    reason: "Explicit shell execution requires explicit authorization"
  - pattern: "zsh *"
    reason: "Explicit shell execution requires explicit authorization"
  - pattern: "source *"
    reason: "Sourcing arbitrary files can execute malicious code"
  - pattern: ". *"
    reason: "Sourcing arbitrary files can execute malicious code"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_commands_allowed() {
        let whitelist = CommandWhitelist::default();

        // 测试安全命令
        let result = whitelist.validate("ls", &["-la"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));
        assert!(!result.requires_confirmation);

        let result = whitelist.validate("cat", &["file.txt"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));

        let result = whitelist.validate("grep", &["pattern", "file.txt"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));

        let result = whitelist.validate("cargo", &["build"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));

        let result = whitelist.validate("git", &["status"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));

        let result = whitelist.validate("pwd", &[]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));
    }

    #[test]
    fn test_dangerous_commands_require_confirmation() {
        let whitelist = CommandWhitelist::default();

        // 测试危险命令需要确认
        let result = whitelist.validate("rm", &["file.txt"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Dangerous));
        assert!(result.requires_confirmation);

        let result = whitelist.validate("cp", &["src", "dst"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Dangerous));
        assert!(result.requires_confirmation);

        let result = whitelist.validate("mv", &["old", "new"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Dangerous));
        assert!(result.requires_confirmation);

        let result = whitelist.validate("chmod", &["755", "file"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Dangerous));
        assert!(result.requires_confirmation);
    }

    #[test]
    fn test_forbidden_commands_denied() {
        let whitelist = CommandWhitelist::default();

        // 测试禁止命令
        let result = whitelist.validate("sudo", &["ls"]).unwrap();
        assert!(!result.allowed);
        assert_eq!(result.class, Some(CommandClass::Forbidden));
        assert!(result.reason.is_some());
        assert!(result.reason.as_ref().unwrap().contains("Privilege escalation"));

        let result = whitelist.validate("rm", &["-rf", "/"]).unwrap();
        assert!(!result.allowed);
        assert_eq!(result.class, Some(CommandClass::Forbidden));

        let result = whitelist.validate("chmod", &["777", "file"]).unwrap();
        assert!(!result.allowed);
        assert_eq!(result.class, Some(CommandClass::Forbidden));
        assert!(result.reason.as_ref().unwrap().contains("Insecure permissions"));

        let result = whitelist.validate("eval", &["echo hello"]).unwrap();
        assert!(!result.allowed);
        assert_eq!(result.class, Some(CommandClass::Forbidden));
    }

    #[test]
    fn test_unknown_commands_denied() {
        let whitelist = CommandWhitelist::default();

        // 测试未知命令默认拒绝
        let result = whitelist.validate("unknown_command", &["arg1"]).unwrap();
        assert!(!result.allowed);
        assert_eq!(result.class, Some(CommandClass::Forbidden));
        assert!(result.reason.as_ref().unwrap().contains("not in whitelist"));
    }

    #[test]
    fn test_from_yaml() {
        let yaml = r#"
allowed:
  - pattern: "test *"
    class: safe
  - pattern: "danger *"
    class: dangerous
    requires_confirmation: true
denied:
  - pattern: "forbid *"
    reason: "Test forbidden"
"#;

        let whitelist = CommandWhitelist::from_yaml(yaml).unwrap();

        let result = whitelist.validate("test", &["arg"]).unwrap();
        assert!(result.allowed);
        assert_eq!(result.class, Some(CommandClass::Safe));

        let result = whitelist.validate("danger", &["arg"]).unwrap();
        assert!(result.allowed);
        assert!(result.requires_confirmation);

        let result = whitelist.validate("forbid", &["arg"]).unwrap();
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("Test forbidden"));
    }

    #[test]
    fn test_pattern_matching() {
        let whitelist = CommandWhitelist::empty();
        let mut whitelist = whitelist;
        whitelist.allow("cmd *", CommandClass::Safe).unwrap();
        whitelist.allow("cmd2 arg1 *", CommandClass::Safe).unwrap();
        whitelist.deny("cmd dangerous *", "Test deny").unwrap();

        // 应该匹配
        assert!(whitelist.validate("cmd", &["arg"]).unwrap().allowed);
        assert!(whitelist.validate("cmd", &["arg1", "arg2"]).unwrap().allowed);
        assert!(whitelist.validate("cmd2", &["arg1", "arg2"]).unwrap().allowed);

        // 不应该匹配
        assert!(!whitelist.validate("cmd2", &["other"]).unwrap().allowed);
    }

    #[test]
    fn test_validate_with_explanation() {
        let whitelist = CommandWhitelist::default();

        // 允许的命令应该成功
        let result = whitelist.validate_with_explanation("ls", &["-la"]);
        assert!(result.is_ok());

        // 禁止的命令应该返回详细的错误
        let result = whitelist.validate_with_explanation("sudo", &["ls"]);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("rejected"));
        assert!(err_msg.contains("Privilege escalation"));
    }

    #[test]
    fn test_get_command_class() {
        let whitelist = CommandWhitelist::default();

        assert_eq!(whitelist.get_command_class("ls", &["-la"]), CommandClass::Safe);
        assert_eq!(whitelist.get_command_class("rm", &["file"]), CommandClass::Dangerous);
        assert_eq!(whitelist.get_command_class("sudo", &["ls"]), CommandClass::Forbidden);
        assert_eq!(whitelist.get_command_class("unknown", &[]), CommandClass::Forbidden);
    }

    #[test]
    fn test_requires_confirmation() {
        let whitelist = CommandWhitelist::default();

        assert!(!whitelist.requires_confirmation("ls", &["-la"]));
        assert!(whitelist.requires_confirmation("rm", &["file"]));
        assert!(whitelist.requires_confirmation("sudo", &["ls"])); // 禁止的命令也返回 true（保守处理）
    }

    #[test]
    fn test_list_allowed_and_denied() {
        let whitelist = CommandWhitelist::default();

        let allowed = whitelist.list_allowed();
        assert!(!allowed.is_empty());
        // 验证包含预期的模式
        assert!(allowed.iter().any(|(p, _, _)| p == "ls *"));

        let denied = whitelist.list_denied();
        assert!(!denied.is_empty());
        // 验证包含预期的模式
        assert!(denied.iter().any(|(p, _)| p == "sudo *"));
    }

    #[test]
    fn test_command_class_display() {
        assert_eq!(CommandClass::Safe.to_string(), "safe");
        assert_eq!(CommandClass::Dangerous.to_string(), "dangerous");
        assert_eq!(CommandClass::Forbidden.to_string(), "forbidden");
    }

    #[test]
    fn test_command_class_requires_confirmation() {
        assert!(!CommandClass::Safe.requires_confirmation());
        assert!(CommandClass::Dangerous.requires_confirmation());
        assert!(!CommandClass::Forbidden.requires_confirmation());
    }

    #[test]
    fn test_empty_whitelist_denies_all() {
        let whitelist = CommandWhitelist::empty();

        let result = whitelist.validate("ls", &["-la"]).unwrap();
        assert!(!result.allowed);
        assert!(result.reason.as_ref().unwrap().contains("not in whitelist"));
    }

    #[test]
    fn test_invalid_yaml() {
        let yaml = "invalid: yaml: [";
        let result = CommandWhitelist::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_nonexistent_file() {
        let result = CommandWhitelist::from_file("/nonexistent/path/config.yaml");
        assert!(result.is_err());
    }
}
