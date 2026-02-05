//! GLM API 服务管理命令
//!
//! 管理云端节点的 GLM API 服务（启动/停止/状态/配置）
//!
//! 认证方式: Bearer Token (助记词或 DID)

use clap::{Args, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;

use cis_core::glm::{GlmApiConfig, start_glm_api_service};

/// 默认的示例 DID
const DEFAULT_DID: &str = "did:cis:glm-cloud:abc123";

/// GLM 命令
#[derive(Subcommand, Debug)]
pub enum GlmCommands {
    /// 启动 GLM API 服务
    Start(GlmStartArgs),
    /// 停止 GLM API 服务
    Stop,
    /// 查看服务状态
    Status,
    /// 查看待确认任务
    Pending,
    /// 确认一个 DAG
    Confirm {
        /// DAG ID
        dag_id: String,
    },
    /// 配置 GLM API
    Config(GlmConfigArgs),
}

#[derive(Args, Debug)]
pub struct GlmStartArgs {
    /// 监听地址
    #[arg(short, long, default_value = "127.0.0.1:6767")]
    bind: String,
    /// 允许的 DID（可多次指定，格式: did:cis:{node_id}:{pub_key_short}）
    #[arg(short, long)]
    did: Vec<String>,
    /// 默认 Room ID
    #[arg(short, long)]
    room_id: Option<String>,
    /// 后台运行
    #[arg(long)]
    daemon: bool,
}

#[derive(Args, Debug)]
pub struct GlmConfigArgs {
    /// 设置监听地址
    #[arg(long)]
    bind: Option<String>,
    /// 添加允许的 DID（可多次指定）
    #[arg(long)]
    did: Vec<String>,
    /// 设置默认 Room ID
    #[arg(long)]
    room_id: Option<String>,
    /// 查看当前配置
    #[arg(long)]
    show: bool,
}

/// 执行 GLM 命令
pub async fn execute(cmd: GlmCommands) -> anyhow::Result<()> {
    match cmd {
        GlmCommands::Start(args) => start_service(args).await,
        GlmCommands::Stop => stop_service().await,
        GlmCommands::Status => show_status().await,
        GlmCommands::Pending => list_pending().await,
        GlmCommands::Confirm { dag_id } => confirm_dag(dag_id).await,
        GlmCommands::Config(args) => configure(args).await,
    }
}

async fn start_service(args: GlmStartArgs) -> anyhow::Result<()> {
    println!("🚀 Starting GLM API service...");

    let bind_addr: SocketAddr = args
        .bind
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind address: {}", e))?;

    // 获取允许的 DID 列表
    let allowed_dids = if !args.did.is_empty() {
        args.did
    } else {
        let config = load_config().await?;
        config.allowed_dids
    };

    let config = GlmApiConfig {
        bind_addr,
        allowed_dids,
        default_room_id: args.room_id.unwrap_or_else(|| "!default:matrix.org".to_string()),
        task_timeout_secs: 300,
    };

    println!("  📡 Bind: {}", config.bind_addr);
    println!("  👥 Allowed DIDs:");
    for did in &config.allowed_dids {
        println!("     - {}", did);
    }
    println!("  💬 Room ID: {}", config.default_room_id);

    if args.daemon {
        println!("  👻 Running in daemon mode (not implemented, running foreground)");
    }

    println!("\n✅ GLM API service is ready!");
    println!("   Health check: http://{}/health", config.bind_addr);
    println!("   API endpoint: http://{}/api/v1/", config.bind_addr);
    println!("\n💡 使用 Bearer DID 认证（与 CIS 节点间认证一致）:");
    println!("   Authorization: Bearer did:cis:{{node_id}}:{{pub_key_short}}");

    // 启动服务（会阻塞）
    start_glm_api_service(config).await?;

    Ok(())
}

async fn stop_service() -> anyhow::Result<()> {
    println!("🛑 Stopping GLM API service...");
    println!("   (Implementation: read PID from ~/.cis/glm-api.pid and kill process)");
    Ok(())
}

async fn show_status() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let config = load_config().await?;
    
    // 使用第一个允许的 DID 进行认证
    let auth_did = config.allowed_dids.first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_DID.to_string());
    
    match client
        .get(format!("http://{}/health", config.bind_addr))
        .header("Authorization", format!("Bearer {}", auth_did))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            println!("✅ GLM API service is running");
            println!("   URL: http://{}", config.bind_addr);
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                println!("   Response: {}", body);
            }
        }
        _ => {
            println!("❌ GLM API service is not running");
            println!("   Configured bind: {}", config.bind_addr);
            println!("   Start with: cis glm start");
        }
    }

    Ok(())
}

async fn list_pending() -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let config = load_config().await?;

    // 使用第一个允许的 DID 进行认证
    let auth_did = config.allowed_dids.first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_DID.to_string());

    match client
        .get(format!("http://{}/api/v1/pending", config.bind_addr))
        .header("Authorization", format!("Bearer {}", auth_did))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                let count = body["count"].as_u64().unwrap_or(0);
                if count == 0 {
                    println!("📋 No pending DAG confirmations");
                } else {
                    println!("📋 Pending DAG confirmations ({}):\n", count);
                    if let Some(pending) = body["pending"].as_array() {
                        for dag in pending {
                            println!("  🔒 {}", dag["dag_id"].as_str().unwrap_or("unknown"));
                            println!("     Description: {}", dag["description"].as_str().unwrap_or(""));
                            println!("     Tasks: {}", dag["tasks"].as_array().map(|t| t.len()).unwrap_or(0));
                            println!("     Expires: {}", dag["expires_at"].as_str().unwrap_or(""));
                            println!("     Confirm: cis glm confirm {}", dag["dag_id"].as_str().unwrap_or(""));
                            println!();
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to connect: {}", e);
            println!("   Make sure the service is running: cis glm start");
        }
        Ok(resp) => {
            println!("❌ Error: {}", resp.status());
        }
    }

    Ok(())
}

async fn confirm_dag(dag_id: String) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let config = load_config().await?;

    // 使用第一个允许的 DID 进行认证
    let auth_did = config.allowed_dids.first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_DID.to_string());

    println!("🔓 Confirming DAG: {}...", dag_id);

    match client
        .post(format!("http://{}/api/v1/dag/{}/confirm", config.bind_addr, dag_id))
        .header("Authorization", format!("Bearer {}", auth_did))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                println!("✅ {}", body["message"].as_str().unwrap_or("DAG confirmed"));
                println!("   Tasks: {}", body["tasks_count"].as_u64().unwrap_or(0));
            }
        }
        Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
            println!("❌ DAG not found or expired: {}", dag_id);
        }
        Err(e) => {
            println!("❌ Failed to connect: {}", e);
        }
        Ok(resp) => {
            println!("❌ Error: {}", resp.status());
        }
    }

    Ok(())
}

async fn configure(args: GlmConfigArgs) -> anyhow::Result<()> {
    if args.show {
        let config = load_config().await?;
        println!("📋 GLM API Configuration:");
        println!("   Bind: {}", config.bind_addr);
        println!("   Allowed DIDs:");
        for did in &config.allowed_dids {
            println!("      - {}", did);
        }
        println!("   Room ID: {}", config.default_room_id);
        return Ok(());
    }

    // 更新配置
    let mut config = load_config().await?;
    
    if let Some(bind) = args.bind {
        config.bind_addr = bind.parse()?;
        println!("✅ Bind address updated: {}", config.bind_addr);
    }
    
    if !args.did.is_empty() {
        config.allowed_dids = args.did;
        println!("✅ Allowed DIDs updated:");
        for did in &config.allowed_dids {
            println!("   - {}", did);
        }
    }
    
    if let Some(room_id) = args.room_id {
        config.default_room_id = room_id;
        println!("✅ Room ID updated: {}", config.default_room_id);
    }

    // 保存配置
    save_config(&config).await?;
    println!("\n💾 Configuration saved to ~/.cis/glm-api.toml");

    Ok(())
}

/// 从配置文件加载配置
async fn load_config() -> anyhow::Result<GlmApiConfig> {
    let config_path = get_config_path()?;

    if config_path.exists() {
        let content = tokio::fs::read_to_string(&config_path).await?;
        let config: GlmApiConfigFile = toml::from_str(&content)?;
        Ok(config.into())
    } else {
        Ok(GlmApiConfig::default())
    }
}

/// 保存配置到文件
async fn save_config(config: &GlmApiConfig) -> anyhow::Result<()> {
    let config_path = get_config_path()?;
    let parent = config_path.parent().unwrap();
    
    tokio::fs::create_dir_all(&parent).await?;
    
    let config_file: GlmApiConfigFile = config.clone().into();
    let content = toml::to_string_pretty(&config_file)?;
    
    tokio::fs::write(&config_path, content).await?;
    Ok(())
}

/// 获取配置文件路径
fn get_config_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| anyhow::anyhow!("Cannot determine home directory"))?;
    
    Ok(PathBuf::from(home).join(".cis").join("glm-api.toml"))
}

/// 配置文件结构
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct GlmApiConfigFile {
    pub bind: String,
    #[serde(default)]
    pub allowed_dids: Vec<String>,
    pub room_id: String,
    pub timeout_secs: u64,
}

impl From<GlmApiConfigFile> for GlmApiConfig {
    fn from(f: GlmApiConfigFile) -> Self {
        let allowed_dids = if f.allowed_dids.is_empty() {
            vec![DEFAULT_DID.to_string()]
        } else {
            f.allowed_dids
        };
        
        Self {
            bind_addr: f.bind.parse().unwrap_or_else(|_| "127.0.0.1:6767".parse().unwrap()),
            allowed_dids,
            default_room_id: f.room_id,
            task_timeout_secs: f.timeout_secs,
        }
    }
}

impl From<GlmApiConfig> for GlmApiConfigFile {
    fn from(c: GlmApiConfig) -> Self {
        Self {
            bind: c.bind_addr.to_string(),
            allowed_dids: c.allowed_dids,
            room_id: c.default_room_id,
            timeout_secs: c.task_timeout_secs,
        }
    }
}
