//! # Embedding 服务初始化向导
//!
//! 提供渐进式的 embedding 服务配置：
//! 1. 下载本地模型 (Nomic Embed Text v1.5)
//! 2. 配置 OpenAI API Key
//! 3. 使用 Claude CLI 代理
//! 4. 回退到 SQL LIKE 搜索 (无向量功能)

use std::io::{self, Write};
use std::path::PathBuf;
use tracing::{info, warn, error};

use crate::error::{CisError, Result};
use crate::storage::paths::Paths;

/// 模型下载信息
const MODEL_INFO: &str = r#"
╔══════════════════════════════════════════════════════════════╗
║              CIS Embedding 模型配置                          ║
╠══════════════════════════════════════════════════════════════╣
║  CIS 使用文本向量嵌入来实现语义搜索和记忆检索。                ║
║  需要配置 embedding 服务才能使用高级记忆功能。                 ║
╚══════════════════════════════════════════════════════════════╝
"#;

/// 模型下载配置
pub struct ModelDownloadConfig {
    /// 模型名称
    pub name: &'static str,
    /// 模型下载 URL
    pub url: &'static str,
    /// 模型文件大小 (MB)
    pub size_mb: f32,
    /// 本地路径
    pub local_path: PathBuf,
}

impl Default for ModelDownloadConfig {
    fn default() -> Self {
        Self {
            name: "nomic-embed-text-v1.5",
            url: "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/onnx/model.onnx",
            size_mb: 130.0,
            local_path: Paths::models_dir().join("nomic-embed-text-v1.5").join("model.onnx"),
        }
    }
}

impl ModelDownloadConfig {
    /// 获取 tokenizer URL
    pub fn tokenizer_url(&self) -> &'static str {
        "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/tokenizer.json"
    }
    
    /// 获取 tokenizer 本地路径
    pub fn tokenizer_path(&self) -> PathBuf {
        self.local_path.parent().unwrap().join("tokenizer.json")
    }
    
    /// 检查模型是否已存在
    pub fn exists(&self) -> bool {
        self.local_path.exists() && self.tokenizer_path().exists()
    }
}

/// Embedding 初始化选项
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingInitOption {
    /// 下载本地模型
    DownloadLocalModel,
    /// 使用 OpenAI API
    UseOpenAI,
    /// 使用 Claude CLI 代理
    UseClaudeCli,
    /// 回退到 SQL LIKE 搜索
    UseSqlFallback,
    /// 跳过配置
    Skip,
}

/// 交互式初始化 embedding 服务
pub fn interactive_init() -> Result<EmbeddingInitConfig> {
    println!("{}", MODEL_INFO);
    
    let config = ModelDownloadConfig::default();
    
    // 检查模型是否已存在
    if config.exists() {
        println!("✓ 检测到本地模型已存在: {}", config.local_path.display());
        return Ok(EmbeddingInitConfig::local());
    }
    
    // 交互式选择
    loop {
        println!("\n请选择 embedding 服务配置方式：\n");
        println!("  [1] 下载本地模型 (Nomic Embed v1.5, ~130MB) ⭐ 推荐");
        println!("      - 优点：离线使用，无需 API Key，隐私性好，语义搜索质量高");
        println!("      - 缺点：需要下载模型文件 (~130MB)\n");
        
        println!("  [2] 使用 Claude CLI 代理");
        println!("      - 优点：利用已安装的 Claude CLI，无需下载");
        println!("      - 缺点：速度较慢，启发式嵌入（非真正语义）\n");
        
        println!("  [3] 使用 OpenAI API (text-embedding-3-small)");
        println!("      - 优点：高质量嵌入");
        println!("      - 缺点：需要 API Key，消耗 token，需要联网\n");
        
        println!("  [4] 回退到简单搜索 (SQL LIKE)");
        println!("      - 优点：无需任何配置，完全离线");
        println!("      - 缺点：仅支持关键词匹配，无语义搜索能力\n");
        
        println!("  [5] 跳过配置 (稍后手动设置)\n");
        
        print!("请输入选项 (1-5): ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        
        match input.trim() {
            "1" => return handle_download_model(&config),
            "2" => return handle_claude_cli(),
            "3" => return handle_openai_config(),
            "4" => return Ok(EmbeddingInitConfig::sql_fallback()),
            "5" => {
                println!("已跳过配置。稍后可以通过 `cis config embedding` 重新配置。");
                return Ok(EmbeddingInitConfig::skip());
            }
            _ => println!("无效选项，请重新选择。\n"),
        }
    }
}

/// 处理模型下载
fn handle_download_model(config: &ModelDownloadConfig) -> Result<EmbeddingInitConfig> {
    println!("\n📥 准备下载模型: {}", config.name);
    println!("   大小: ~{:.1} MB", config.size_mb);
    println!("   保存位置: {}", config.local_path.display());
    
    print!("\n确认下载? (y/n): ");
    io::stdout().flush().unwrap();
    
    let mut confirm = String::new();
    io::stdin().read_line(&mut confirm).unwrap();
    
    if confirm.trim().to_lowercase() != "y" {
        println!("已取消下载。");
        return Ok(EmbeddingInitConfig::skip());
    }
    
    // 创建目录
    if let Some(parent) = config.local_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CisError::io(format!("Failed to create model directory: {}", e)))?;
    }
    
    // 下载模型
    println!("\n📥 正在下载模型文件...");
    match download_file(config.url, &config.local_path) {
        Ok(_) => {
            println!("✓ 模型文件下载完成");
        }
        Err(e) => {
            error!("模型下载失败: {}", e);
            println!("✗ 模型下载失败: {}", e);
            println!("\n是否尝试使用 OpenAI API 作为替代? (y/n): ");
            io::stdout().flush().unwrap();
            
            let mut retry = String::new();
            io::stdin().read_line(&mut retry).unwrap();
            
            if retry.trim().to_lowercase() == "y" {
                return handle_openai_config();
            } else {
                return Ok(EmbeddingInitConfig::sql_fallback());
            }
        }
    }
    
    // 下载 tokenizer
    println!("📥 正在下载 tokenizer...");
    match download_file(config.tokenizer_url(), &config.tokenizer_path()) {
        Ok(_) => {
            println!("✓ Tokenizer 下载完成");
        }
        Err(e) => {
            error!("Tokenizer 下载失败: {}", e);
            println!("⚠ Tokenizer 下载失败，但模型可能仍可用。");
        }
    }
    
    println!("\n✅ 本地模型配置完成！");
    println!("   模型路径: {}", config.local_path.display());
    
    Ok(EmbeddingInitConfig::local())
}

/// 处理 OpenAI 配置
fn handle_openai_config() -> Result<EmbeddingInitConfig> {
    println!("\n🔑 OpenAI API 配置");
    
    // 检查是否已有环境变量
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        if !api_key.is_empty() {
            println!("✓ 检测到 OPENAI_API_KEY 已设置");
            print!("是否使用现有配置? (y/n): ");
            io::stdout().flush().unwrap();
            
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm).unwrap();
            
            if confirm.trim().to_lowercase() == "y" {
                return Ok(EmbeddingInitConfig::openai());
            }
        }
    }
    
    println!("\n请输入 OpenAI API Key:");
    println!("  (输入不会显示在屏幕上，输入完成后按回车)");
    print!("> ");
    io::stdout().flush().unwrap();
    
    let api_key = rpassword::read_password().unwrap_or_default();
    
    if api_key.is_empty() {
        println!("✗ API Key 不能为空");
        return Ok(EmbeddingInitConfig::skip());
    }
    
    if !api_key.starts_with("sk-") {
        println!("⚠ 警告: API Key 格式不正确，应以 'sk-' 开头");
        print!("是否继续? (y/n): ");
        io::stdout().flush().unwrap();
        
        let mut confirm = String::new();
        io::stdin().read_line(&mut confirm).unwrap();
        
        if confirm.trim().to_lowercase() != "y" {
            return Ok(EmbeddingInitConfig::skip());
        }
    }
    
    // 保存到配置文件
    match save_openai_config(&api_key) {
        Ok(_) => {
            println!("✅ OpenAI API Key 已保存到配置文件");
            Ok(EmbeddingInitConfig::openai())
        }
        Err(e) => {
            error!("保存配置失败: {}", e);
            println!("⚠ 保存配置失败，但可以使用环境变量临时设置");
            println!("   export OPENAI_API_KEY='{}'", &api_key[..10.min(api_key.len())]);
            Ok(EmbeddingInitConfig::openai_temp(api_key))
        }
    }
}

/// 处理 Claude CLI 配置
fn handle_claude_cli() -> Result<EmbeddingInitConfig> {
    println!("\n🤖 Claude CLI 代理配置 (实验性)");
    println!("\n此选项使用已安装的 Claude CLI 来生成文本嵌入。");
    println!("注意：这是一个实验性功能，速度较慢。\n");
    
    // 检查 Claude CLI 是否可用
    match std::process::Command::new("claude").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            println!("✓ 检测到 Claude CLI: {}", version.trim());
        }
        _ => {
            println!("✗ 未检测到 Claude CLI");
            println!("   请先安装: https://github.com/anthropics/claude-cli");
            println!("\n按回车键返回主菜单...");
            io::stdin().read_line(&mut String::new()).unwrap();
            return Ok(EmbeddingInitConfig::skip());
        }
    }
    
    println!("\n✅ Claude CLI 代理配置完成！");
    println!("   将使用 `claude` 命令生成嵌入。\n");
    
    Ok(EmbeddingInitConfig::claude_cli())
}

/// 下载文件
fn download_file(url: &str, path: &PathBuf) -> Result<()> {
    use std::fs::File;
    use std::io::copy;
    
    let response = reqwest::blocking::get(url)
        .map_err(|e| CisError::network(format!("Failed to download {}: {}", url, e)))?;
    
    if !response.status().is_success() {
        return Err(CisError::network(format!(
            "Download failed with status: {}", 
            response.status()
        )));
    }
    
    let mut file = File::create(path)
        .map_err(|e| CisError::io(format!("Failed to create file: {}", e)))?;
    
    let content = response.bytes()
        .map_err(|e| CisError::network(format!("Failed to read response: {}", e)))?;
    
    copy(&mut content.as_ref(), &mut file)
        .map_err(|e| CisError::io(format!("Failed to write file: {}", e)))?;
    
    Ok(())
}

/// 保存 OpenAI 配置
fn save_openai_config(api_key: &str) -> Result<()> {
    let config_dir = Paths::config_dir();
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| CisError::io(format!("Failed to create config dir: {}", e)))?;
    
    let config_path = config_dir.join("embedding.toml");
    let config_content = format!(
        r#"[openai]
api_key = "{}"
model = "text-embedding-3-small"
"#,
        api_key
    );
    
    std::fs::write(&config_path, config_content)
        .map_err(|e| CisError::io(format!("Failed to write config: {}", e)))?;
    
    // 设置文件权限 (仅用户可读)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&config_path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&config_path, perms)?;
    }
    
    Ok(())
}

/// Embedding 初始化配置
#[derive(Debug, Clone)]
pub struct EmbeddingInitConfig {
    /// 初始化选项
    pub option: EmbeddingInitOption,
    /// OpenAI API Key (临时)
    pub openai_api_key: Option<String>,
    /// 模型路径 (如果下载了本地模型)
    pub model_path: Option<PathBuf>,
}

impl EmbeddingInitConfig {
    /// 本地模型配置
    pub fn local() -> Self {
        Self {
            option: EmbeddingInitOption::DownloadLocalModel,
            openai_api_key: None,
            model_path: Some(Paths::models_dir().join("nomic-embed-text-v1.5").join("model.onnx")),
        }
    }
    
    /// OpenAI 配置
    pub fn openai() -> Self {
        Self {
            option: EmbeddingInitOption::UseOpenAI,
            openai_api_key: None,
            model_path: None,
        }
    }
    
    /// OpenAI 临时配置
    pub fn openai_temp(api_key: String) -> Self {
        Self {
            option: EmbeddingInitOption::UseOpenAI,
            openai_api_key: Some(api_key),
            model_path: None,
        }
    }
    
    /// Claude CLI 配置
    pub fn claude_cli() -> Self {
        Self {
            option: EmbeddingInitOption::UseClaudeCli,
            openai_api_key: None,
            model_path: None,
        }
    }
    
    /// SQL 回退配置
    pub fn sql_fallback() -> Self {
        Self {
            option: EmbeddingInitOption::UseSqlFallback,
            openai_api_key: None,
            model_path: None,
        }
    }
    
    /// 跳过配置
    pub fn skip() -> Self {
        Self {
            option: EmbeddingInitOption::Skip,
            openai_api_key: None,
            model_path: None,
        }
    }
}

/// 检查是否需要初始化
pub fn needs_init() -> bool {
    let config = ModelDownloadConfig::default();
    
    // 1. 优先检查本地模型
    if config.exists() {
        return false;
    }
    
    // 2. 检查 Claude CLI
    if std::process::Command::new("claude").arg("--version").output().is_ok() {
        return false;
    }
    
    // 3. 如果配置了 OpenAI API Key，不需要初始化
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return false;
    }
    
    // 4. 如果已有配置文件
    let config_path = Paths::config_dir().join("embedding.toml");
    if config_path.exists() {
        return false;
    }
    
    true
}

/// 非交互式自动配置（用于 CI/自动化场景）
/// 
/// 优先级（从高到低）：
/// 1. 本地模型（Nomic Embed v1.5）- 优先使用本地模型
/// 2. Claude CLI（Agent 工具）
/// 3. OpenAI API（需要 API Key）
/// 4. SQL LIKE 回退
pub fn auto_init() -> Result<EmbeddingInitConfig> {
    let config = ModelDownloadConfig::default();
    
    // 1. 优先检查本地模型（Nomic Embed v1.5）
    if config.exists() {
        info!("Using local embedding model (Nomic Embed v1.5)");
        return Ok(EmbeddingInitConfig::local());
    }
    
    // 2. 检查 Claude CLI
    if std::process::Command::new("claude").arg("--version").output().is_ok() {
        info!("Using Claude CLI as embedding service");
        return Ok(EmbeddingInitConfig::claude_cli());
    }
    
    // 3. 检查 OpenAI API Key
    if std::env::var("OPENAI_API_KEY").is_ok() {
        info!("Using OpenAI embedding service (requires API key)");
        return Ok(EmbeddingInitConfig::openai());
    }
    
    // 4. 回退到 SQL
    warn!("No embedding service available, falling back to SQL LIKE search");
    Ok(EmbeddingInitConfig::sql_fallback())
}
