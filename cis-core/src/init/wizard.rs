//! # CIS 初始化向导
//!
//! 引导用户完成 CIS 的初始配置。
//!
//! ## 流程
//! 1. 环境检查 - AI Agent、Git、目录权限
//! 2. 配置生成 - ~/.cis/config.toml、节点密钥
//! 3. 项目初始化 (可选) - .cis/project.toml
//! 4. 验证 - 测试连接、记忆、Skill

use std::io::{self, Write};


use crate::error::{CisError, Result};
use crate::project::Project;
use crate::storage::paths::Paths;

/// 初始化向导
pub struct InitWizard {
    interactive: bool,
    skip_checks: bool,
    force: bool,
    preferred_provider: Option<String>,
}

/// 环境检查结果
#[derive(Debug, Clone)]
pub struct EnvironmentCheck {
    pub git_available: bool,
    pub git_version: Option<String>,
    pub ai_agents: Vec<AgentCheck>,
    pub directory_permissions: bool,
    pub home_dir_writable: bool,
}

/// AI Agent 检查
#[derive(Debug, Clone)]
pub struct AgentCheck {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

/// 向导结果
#[derive(Debug, Clone)]
pub struct WizardResult {
    pub config_created: bool,
    pub project_initialized: bool,
    pub tests_passed: bool,
    pub messages: Vec<String>,
}

impl InitWizard {
    /// 创建新的向导实例
    pub fn new() -> Self {
        Self {
            interactive: true,
            skip_checks: false,
            force: false,
            preferred_provider: None,
        }
    }

    pub fn non_interactive() -> Self {
        Self {
            interactive: false,
            skip_checks: false,
            force: false,
            preferred_provider: None,
        }
    }

    pub fn skip_checks(mut self) -> Self {
        self.skip_checks = true;
        self
    }

    pub fn with_force(mut self) -> Self {
        self.force = true;
        self
    }

    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.preferred_provider = Some(provider.into());
        self
    }

    /// 运行完整向导
    pub async fn run(&self, project_mode: bool) -> Result<WizardResult> {
        let mut result = WizardResult {
            config_created: false,
            project_initialized: false,
            tests_passed: false,
            messages: Vec::new(),
        };

        println!("🚀 CIS 初始化向导\n");

        // Step 1: 环境检查
        if !self.skip_checks {
            self.print_step(1, 4, "环境检查");
            let check = self.check_environment().await?;
            self.display_environment_check(&check);

            if check.ai_agents.iter().all(|a| !a.available) {
                println!("\n⚠️  警告: 未检测到任何 AI Agent");
                println!("   建议安装 OpenCode（DAG 任务推荐）:");
                println!("   https://github.com/your-opencode-repo");
                println!("   或 Claude CLI: https://github.com/anthropics/anthropic-cli");

                if self.interactive && !self.confirm_continue()? {
                    return Ok(result);
                }
            }
        }

        // Step 2: 全局配置
        self.print_step(2, 5, "全局配置");
        let config_content = self.generate_global_config().await?;
        self.save_global_config(&config_content).await?;
        result.config_created = true;
        result.messages.push(format!(
            "全局配置已保存到 {}",
            Paths::config_file().display()
        ));
        println!("✅ 全局配置完成\n");

        // Step 3: 向量引擎配置（记忆、语义搜索必需）
        self.print_step(3, 5, "向量引擎配置");
        self.configure_vector_engine().await?;
        result.messages.push("向量引擎配置完成".to_string());
        println!("✅ 向量引擎配置完成\n");

        // Step 4: 项目初始化 (可选)
        if project_mode {
            self.print_step(4, 5, "项目初始化");
            self.initialize_project().await?;
            result.project_initialized = true;
            let project_toml = std::env::current_dir()?.join(".cis/project.toml");
            result.messages.push(format!(
                "项目配置已保存到 {}",
                project_toml.display()
            ));
            println!("✅ 项目初始化完成\n");
        }

        // Step 5: 验证
        self.print_step(5, 5, "验证");
        let tests = self.run_verification_tests().await?;
        result.tests_passed = tests;

        if tests {
            println!("\n✅ 所有测试通过！CIS 已准备就绪。");
        } else {
            println!("\n⚠️  部分测试未通过，但 CIS 仍可使用。");
        }

        // 显示下一步
        println!("\n📖 下一步:");
        println!("   cis skill list          # 查看可用技能");
        println!("   cis skill do \"...\"      # 使用自然语言调用技能");
        println!("   cis memory search \"...\" # 搜索记忆");

        Ok(result)
    }

    // ==================== 环境检查 ====================

    async fn check_environment(&self) -> Result<EnvironmentCheck> {
        println!("  检查 Git...");
        let git_available = self.check_git().await;

        println!("  检查 AI Agents...");
        let ai_agents = self.check_ai_agents().await;

        println!("  检查目录权限...");
        let directory_permissions = self.check_directory_permissions().await?;
        let home_dir_writable = self.check_home_writable().await?;

        Ok(EnvironmentCheck {
            git_available: git_available.is_ok(),
            git_version: git_available.ok(),
            ai_agents,
            directory_permissions,
            home_dir_writable,
        })
    }

    async fn check_git(&self) -> Result<String> {
        let output = tokio::process::Command::new("git")
            .args(["--version"])
            .output()
            .await
            .map_err(|e| CisError::other(format!("Git check failed: {}", e)))?;

        if output.status.success() {
            let version = String::from_utf8_lossy(&output.stdout);
            Ok(version.trim().to_string())
        } else {
            Err(CisError::other("Git not found"))
        }
    }

    async fn check_ai_agents(&self) -> Vec<AgentCheck> {
        let mut agents = Vec::new();

        // Check OpenCode (推荐，DAG 任务默认)
        let opencode = self.check_agent("opencode", &["--version"]).await;
        agents.push(AgentCheck {
            name: "OpenCode (推荐)".to_string(),
            available: opencode.is_ok(),
            version: opencode.as_ref().ok().cloned(),
            path: which::which("opencode").ok().map(|p| p.to_string_lossy().to_string()),
        });

        // Check Claude CLI
        let claude = self.check_agent("claude", &["--version"]).await;
        agents.push(AgentCheck {
            name: "Claude CLI".to_string(),
            available: claude.is_ok(),
            version: claude.as_ref().ok().cloned(),
            path: which::which("claude").ok().map(|p| p.to_string_lossy().to_string()),
        });

        // Check Kimi Code
        let kimi = self.check_agent("kimi", &["--version"]).await;
        agents.push(AgentCheck {
            name: "Kimi Code".to_string(),
            available: kimi.is_ok(),
            version: kimi.as_ref().ok().cloned(),
            path: which::which("kimi").ok().map(|p| p.to_string_lossy().to_string()),
        });

        // Check Aider
        let aider = self.check_agent("aider", &["--version"]).await;
        agents.push(AgentCheck {
            name: "Aider".to_string(),
            available: aider.is_ok(),
            version: aider.as_ref().ok().cloned(),
            path: which::which("aider").ok().map(|p| p.to_string_lossy().to_string()),
        });

        agents
    }

    async fn check_agent(&self, name: &str, args: &[&str]) -> Result<String> {
        let output = tokio::process::Command::new(name)
            .args(args)
            .output()
            .await
            .map_err(|e| CisError::other(format!("{} check failed: {}", name, e)))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(CisError::other(format!("{} not available", name)))
        }
    }

    async fn check_directory_permissions(&self) -> Result<bool> {
        let data_dir = Paths::data_dir();
        std::fs::create_dir_all(&data_dir)?;

        let test_file = data_dir.join(".permission_test");
        match std::fs::write(&test_file, b"test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    async fn check_home_writable(&self) -> Result<bool> {
        let home = dirs::home_dir().ok_or_else(|| CisError::other("Home directory not found"))?;

        let test_file = home.join(".cis_write_test");
        match std::fs::write(&test_file, b"test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                Ok(true)
            }
            Err(_) => Ok(false),
        }
    }

    fn display_environment_check(&self, check: &EnvironmentCheck) {
        println!("\n  📊 环境检查结果:");

        if check.git_available {
            println!(
                "    ✅ Git: {}",
                check.git_version.as_ref().unwrap_or(&"unknown".to_string())
            );
        } else {
            println!("    ❌ Git: 未安装");
        }

        println!("    🤖 AI Agents:");
        for agent in &check.ai_agents {
            let status = if agent.available { "✅" } else { "❌" };
            let version = agent
                .version
                .as_ref()
                .map(|v| format!(" ({})", v))
                .unwrap_or_default();
            println!("      {} {}{}", status, agent.name, version);
        }

        if check.directory_permissions {
            println!("    ✅ 目录权限: 正常");
        } else {
            println!("    ❌ 目录权限: 无法写入数据目录");
        }

        if check.home_dir_writable {
            println!("    ✅ 主目录: 可写");
        } else {
            println!("    ❌ 主目录: 不可写");
        }

        println!();
    }

    // ==================== 配置生成 ====================

    async fn generate_global_config(&self) -> Result<String> {
        // 检测或选择 AI Provider
        let provider = if let Some(ref p) = self.preferred_provider {
            p.clone()
        } else if self.interactive {
            println!("  选择默认 AI Provider:");
            println!("    1) OpenCode (推荐，DAG 任务优化)");
            println!("    2) Claude CLI");
            println!("    3) Kimi Code");
            println!("    4) Aider");

            let choice = self.prompt_input("请输入选项 (1-4, 默认1): ")?;

            match choice.trim() {
                "2" => "claude".to_string(),
                "3" => "kimi".to_string(),
                "4" => "aider".to_string(),
                _ => "opencode".to_string(),
            }
        } else {
            // 非交互模式，自动检测（优先 OpenCode）
            self.detect_default_provider()
                .unwrap_or_else(|| "opencode".to_string())
        };

        // 生成节点密钥
        let node_key = self.generate_node_key()?;

        // 构建配置
        let config = format!(
            r#"# CIS Global Configuration
# Generated at: {}

[node]
# 节点唯一标识（自动生成）
id = "{}"
# 节点名称
name = "{}"
# 节点密钥（用于联邦网络身份验证）
key = "{}"

[ai]
# 默认 AI Provider: opencode | claude | kimi | aider
default_provider = "{}"

[ai.opencode]
# OpenCode 配置（DAG 任务推荐）
# 可用模型：
#   - opencode/glm-4.7-free (免费)
#   - opencode/kimi-k2.5-free (免费)
#   - opencode/gpt-5-nano (免费)
#   - anthropic/claude-3-opus-20240229 (付费)
#   - openai/gpt-4 (付费)
model = "opencode/glm-4.7-free"
max_tokens = 4096
temperature = 0.7

[ai.claude]
# Claude Code 配置
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

[ai.kimi]
# Kimi Code 配置
model = "kimi-k2"
max_tokens = 8192

[vector]
# 向量引擎配置（用于语义搜索和记忆）
# 嵌入维度: 768 (Nomic Embed), 1536 (OpenAI), 384 (MiniLM)
embedding_dim = 768

# 是否启用 HNSW 索引（推荐启用）
use_hnsw = true

# 相似度阈值（0-1，越高越严格）
default_threshold = 0.7

# 向量存储路径（默认使用数据目录）
# storage_path = "/var/lib/cis/vectors"

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
            node_key,
            provider
        );

        Ok(config)
    }

    fn generate_node_key(&self) -> Result<String> {
        use rand::RngCore;

        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);

        Ok(hex::encode(key))
    }

    fn detect_default_provider(&self) -> Option<String> {
        // 优先检测 OpenCode（DAG 任务推荐）
        let providers = vec![
            ("opencode", "opencode"),
            ("claude", "claude"),
            ("kimi", "kimi"),
            ("aider", "aider"),
        ];

        for (cmd, name) in providers {
            if which::which(cmd).is_ok() {
                return Some(name.to_string());
            }
        }

        None
    }

    async fn save_global_config(&self, config: &str) -> Result<()> {
        let config_path = Paths::config_file();

        // 确保目录存在
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 检查是否已存在
        if config_path.exists() && !self.force {
            if self.interactive {
                let input = self.prompt_input(&format!(
                    "配置文件已存在: {}\n是否覆盖? (y/N): ",
                    config_path.display()
                ))?;
                if input.trim().to_lowercase() != "y" {
                    println!("  跳过配置文件写入");
                    return Ok(());
                }
            } else {
                println!("  配置文件已存在，使用 --force 覆盖");
                return Ok(());
            }
        }

        std::fs::write(&config_path, config)?;

        // 设置权限 (仅当前用户可读写)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&config_path)?.permissions();
            permissions.set_mode(0o600);
            std::fs::set_permissions(&config_path, permissions)?;
        }

        Ok(())
    }

    // ==================== 向量引擎配置 ====================

    async fn configure_vector_engine(&self) -> Result<()> {
        use crate::ai::embedding_init::{interactive_init, EmbeddingInitOption, needs_init};
        
        println!("  检查向量引擎状态...");
        
        // 检查是否已配置
        if !needs_init() {
            println!("  ✓ 向量引擎已配置");
            return Ok(());
        }
        
        println!("\n  📚 CIS 向量引擎用于：");
        println!("     • 语义记忆检索（自然语言搜索）");
        println!("     • 智能技能匹配");
        println!("     • 对话上下文理解");
        println!("     • 项目知识库搜索\n");
        
        if self.interactive {
            // 交互式配置
            println!("  是否现在配置向量引擎? (推荐)");
            print!("  (Y/n): ");
            std::io::stdout().flush()?;
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            
            if input.trim().to_lowercase() == "n" {
                println!("  ⚠️  已跳过向量引擎配置");
                println!("     记忆和语义搜索功能将受限");
                println!("     稍后可通过 `cis config vector` 重新配置\n");
                return Ok(());
            }
            
            // 调用交互式 embedding 初始化
            match interactive_init() {
                Ok(config) => {
                    match config.option {
                        EmbeddingInitOption::DownloadLocalModel => {
                            println!("  ✓ 已配置本地向量模型 (Nomic Embed v1.5)");
                        }
                        EmbeddingInitOption::UseOpenAI => {
                            println!("  ✓ 已配置 OpenAI Embedding API");
                        }
                        EmbeddingInitOption::UseClaudeCli => {
                            println!("  ✓ 已配置 Claude CLI 代理");
                        }
                        EmbeddingInitOption::UseSqlFallback => {
                            println!("  ⚠️  已配置 SQL 回退模式（无语义搜索）");
                        }
                        EmbeddingInitOption::Skip => {
                            println!("  ⚠️  已跳过向量引擎配置");
                            println!("     稍后可通过 `cis config vector` 重新配置");
                        }
                    }
                }
                Err(e) => {
                    println!("  ⚠️  向量引擎配置失败: {}", e);
                    println!("     稍后可通过 `cis config vector` 重新配置");
                }
            }
        } else {
            // 非交互模式：使用自动配置
            println!("  非交互模式：使用自动配置...");
            use crate::ai::embedding_init::auto_init;
            
            match auto_init() {
                Ok(config) => {
                    match config.option {
                        EmbeddingInitOption::DownloadLocalModel => {
                            println!("  ✓ 自动配置：本地向量模型");
                        }
                        EmbeddingInitOption::UseOpenAI => {
                            println!("  ✓ 自动配置：OpenAI API");
                        }
                        EmbeddingInitOption::UseClaudeCli => {
                            println!("  ✓ 自动配置：Claude CLI 代理");
                        }
                        _ => {
                            println!("  ⚠️  自动配置：SQL 回退模式");
                            println!("     记忆和语义搜索功能将受限");
                        }
                    }
                }
                Err(e) => {
                    println!("  ⚠️  自动配置失败: {}", e);
                }
            }
        }
        
        Ok(())
    }

    // ==================== 项目初始化 ====================

    async fn initialize_project(&self) -> Result<()> {
        let project_dir = std::env::current_dir()?;
        let cis_dir = project_dir.join(".cis");

        // 检查是否已存在
        if cis_dir.join("project.toml").exists() && !self.force {
            if self.interactive {
                let input = self.prompt_input(&format!(
                    "项目配置已存在: {}\n是否覆盖? (y/N): ",
                    cis_dir.join("project.toml").display()
                ))?;
                if input.trim().to_lowercase() != "y" {
                    println!("  跳过项目初始化");
                    return Ok(());
                }
            } else {
                println!("  项目配置已存在，使用 --force 覆盖");
                return Ok(());
            }
        }

        // 确保全局配置已存在
        if !Paths::config_file().exists() {
            println!("  先创建全局配置...");
        }

        // 创建项目
        let project_name = project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed");

        let _project = Project::init(&project_dir, project_name)?;

        // 创建 .gitignore
        let gitignore = cis_dir.join(".gitignore");
        std::fs::write(&gitignore, "data/\n*.db\n*.log\n")?;

        println!("  创建: {}", cis_dir.display());
        println!("  创建: {}", cis_dir.join("project.toml").display());
        println!("  创建: {}", gitignore.display());

        Ok(())
    }

    // ==================== 验证 ====================

    async fn run_verification_tests(&self) -> Result<bool> {
        let mut all_passed = true;

        println!("  运行验证测试...\n");

        // Test 1: 配置读取
        print!("  [1/5] 配置读取... ");
        match self.test_config_read().await {
            Ok(_) => println!("✅ 通过"),
            Err(e) => {
                println!("❌ 失败: {}", e);
                all_passed = false;
            }
        }

        // Test 2: 目录写入
        print!("  [2/5] 目录写入... ");
        match self.test_directory_write().await {
            Ok(_) => println!("✅ 通过"),
            Err(e) => {
                println!("❌ 失败: {}", e);
                all_passed = false;
            }
        }

        // Test 3: 节点密钥
        print!("  [3/5] 节点密钥... ");
        match self.test_node_key().await {
            Ok(_) => println!("✅ 通过"),
            Err(e) => {
                println!("❌ 失败: {}", e);
                all_passed = false;
            }
        }

        // Test 4: 向量引擎
        print!("  [4/5] 向量引擎... ");
        match self.test_vector_engine().await {
            Ok(_) => println!("✅ 通过"),
            Err(e) => {
                println!("⚠️  警告: {}", e);
                // 向量引擎失败不视为整体失败，但提醒用户
            }
        }

        // Test 5: AI Provider
        print!("  [5/5] AI Provider... ");
        match self.test_ai_provider().await {
            Ok(_) => println!("✅ 通过"),
            Err(e) => {
                println!("⚠️  跳过: {}", e);
                // AI 测试失败不视为整体失败
            }
        }

        Ok(all_passed)
    }

    async fn test_config_read(&self) -> Result<()> {
        let config_path = Paths::config_file();
        if !config_path.exists() {
            return Err(CisError::other("Config file not found"));
        }

        let content = std::fs::read_to_string(&config_path)?;
        // 验证是有效的 TOML
        let _: toml::Value = toml::from_str(&content)
            .map_err(|e| CisError::other(format!("Invalid config: {}", e)))?;

        Ok(())
    }

    async fn test_directory_write(&self) -> Result<()> {
        let test_file = Paths::data_dir().join(".write_test");
        std::fs::write(&test_file, b"test")?;
        std::fs::remove_file(&test_file)?;
        Ok(())
    }

    async fn test_node_key(&self) -> Result<()> {
        // 检查配置文件中是否包含密钥
        let config_path = Paths::config_file();
        let content = std::fs::read_to_string(&config_path)?;
        
        if !content.contains("key =") {
            return Err(CisError::other("Node key not found in config"));
        }

        Ok(())
    }

    async fn test_vector_engine(&self) -> Result<()> {
        use crate::ai::embedding_init::needs_init;
        
        // 检查向量引擎是否需要初始化
        if needs_init() {
            return Err(CisError::other(
                "向量引擎未配置。运行 `cis config vector` 进行配置"
            ));
        }
        
        Ok(())
    }

    async fn test_ai_provider(&self) -> Result<()> {
        // 检查 AI Provider 是否可调用
        let config_path = Paths::config_file();
        let content = std::fs::read_to_string(&config_path)?;
        let config: toml::Value = toml::from_str(&content)
            .map_err(|e| CisError::other(format!("Invalid config: {}", e)))?;

        let provider = config
            .get("ai")
            .and_then(|ai| ai.get("default_provider"))
            .and_then(|p| p.as_str())
            .unwrap_or("opencode");

        // 检查 provider 是否可用
        if which::which(provider).is_err() {
            return Err(CisError::other(format!(
                "Provider '{}' not found in PATH",
                provider
            )));
        }

        Ok(())
    }

    // ==================== 辅助函数 ====================

    fn print_step(&self, current: usize, total: usize, title: &str) {
        println!("\n┌─ 步骤 {}/{}: {}", current, total, title);
        println!("│");
    }

    fn prompt_input(&self, prompt: &str) -> Result<String> {
        print!("{}", prompt);
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input)
    }

    fn confirm_continue(&self) -> Result<bool> {
        let input = self.prompt_input("是否继续? (y/N): ")?;
        Ok(input.trim().to_lowercase() == "y")
    }
}

impl Default for InitWizard {
    fn default() -> Self {
        Self::new()
    }
}

/// 快速初始化（使用默认值）
pub async fn quick_init(project_mode: bool) -> Result<WizardResult> {
    let wizard = InitWizard::new();
    wizard.run(project_mode).await
}

/// 非交互式初始化
pub async fn init_non_interactive(project_mode: bool, _force: bool) -> Result<WizardResult> {
    let wizard = InitWizard::non_interactive()
        .skip_checks()
        .with_force();
    wizard.run(project_mode).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_creation() {
        let wizard = InitWizard::new();
        assert!(wizard.interactive);
        assert!(!wizard.skip_checks);
    }

    #[test]
    fn test_non_interactive_wizard() {
        let wizard = InitWizard::non_interactive();
        assert!(!wizard.interactive);
    }
}
