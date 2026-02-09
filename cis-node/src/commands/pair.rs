//! # 组网配对命令
//!
//! 通过6位数字组网码快速配对节点。

use cis_core::network::pairing::{PairingManager, PairingService, PairingNodeInfo};
use std::sync::Arc;
use anyhow::{Result, Context};
use clap::Subcommand;

/// 处理配对命令
pub async fn handle(command: PairCommands) -> Result<()> {
    match command {
        PairCommands::Generate { timeout, auto_accept, alias } => {
            handle_generate(timeout, auto_accept, alias).await
        }
        PairCommands::Join { code, alias, address } => {
            handle_join(&code, alias, address).await
        }
        PairCommands::Cancel => {
            handle_cancel().await
        }
        PairCommands::Status => {
            handle_status().await
        }
    }
}

/// 组网配对命令
#[derive(Debug, Subcommand)]
pub enum PairCommands {
    /// 生成组网码并等待连接
    #[command(name = "generate")]
    Generate {
        /// 超时时间（秒）
        #[arg(short, long, default_value = "300")]
        timeout: u64,
        /// 自动接受请求（不询问）
        #[arg(long)]
        auto_accept: bool,
        /// 设置别名
        #[arg(short, long)]
        alias: Option<String>,
    },
    
    /// 使用组网码连接节点
    #[command(name = "join")]
    Join {
        /// 6位数字组网码
        code: String,
        /// 设置别名
        #[arg(short, long)]
        alias: Option<String>,
        /// 指定目标地址（可选，用于已知地址）
        #[arg(short, long)]
        address: Option<String>,
    },
    
    /// 取消当前组网会话
    #[command(name = "cancel")]
    Cancel,
    
    /// 查看当前组网状态
    #[command(name = "status")]
    Status,
}

/// 解析配置文件中的值
fn parse_config_value(config_str: &str, key: &str) -> Option<String> {
    for line in config_str.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("{} =", key)) || trimmed.starts_with(&format!("{}=", key)) {
            let value = trimmed
                .splitn(2, '=')
                .nth(1)?
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            return Some(value);
        }
    }
    None
}

/// 处理 generate 命令
async fn handle_generate(
    timeout: u64,
    auto_accept: bool,
    alias: Option<String>,
) -> Result<()> {
    // 读取配置文件
    use cis_core::storage::paths::Paths;
    let config_path = Paths::config_dir().join("config.toml");
    let config_str = tokio::fs::read_to_string(&config_path).await
        .context("Failed to read config. Please run `cis init` first")?;
    
    let node_id = parse_config_value(&config_str, "node_id")
        .unwrap_or_else(|| "unknown".to_string());
    let did = parse_config_value(&config_str, "did")
        .unwrap_or_else(|| "unknown".to_string());
    let hostname = parse_config_value(&config_str, "hostname")
        .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().to_string());
    
    let node = PairingNodeInfo {
        node_id,
        did,
        hostname: hostname.clone(),
    };
    
    // 创建组网管理器
    let manager = Arc::new(PairingManager::new());
    manager.start_cleanup_task();
    
    // 生成组网码
    let code = manager.generate_code(node.clone())
        .context("Failed to generate pairing code")?;
    
    // 显示组网码
    println!();
    println!("╔══════════════════════════════════════════╗");
    println!("║           🔢 组网配对码                   ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║                                          ║");
    println!("║       {:>6}                            ║", code);
    println!("║                                          ║");
    println!("╠══════════════════════════════════════════╣");
    println!("║  ⏱️  有效期: {}分钟                        ║", timeout / 60);
    println!("║  📌 节点: {}                    ║", hostname);
    println!("╚══════════════════════════════════════════╝");
    println!();
    
    if auto_accept {
        println!("⚠️  自动接受模式已开启");
    } else {
        println!("🔔 等待组网请求，按 Ctrl+C 取消");
    }
    println!();
    
    // 启动组网服务监听
    let service = PairingService::new(manager.clone());
    
    match service.listen(code.clone()).await {
        Ok(result) => {
            if result.success {
                println!("✅ 组网成功!");
                println!("   节点ID: {}", result.node_id);
                println!("   端点: {}", result.endpoint);
                
                // 添加到邻居节点
                println!();
                println!("💡 使用以下命令添加为邻居:");
                println!("   cis neighbor add {} --yes", result.node_id);
            }
        }
        Err(e) => {
            eprintln!("❌ 组网失败: {}", e);
        }
    }
    
    // 清理
    let _ = manager.reject_pairing(&code);
    
    Ok(())
}

/// 处理 join 命令
async fn handle_join(
    code: &str,
    alias: Option<String>,
    address: Option<String>,
) -> Result<()> {
    // 验证组网码格式
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        anyhow::bail!("无效的组网码，必须是6位数字");
    }
    
    // 读取配置文件
    use cis_core::storage::paths::Paths;
    let config_path = Paths::config_dir().join("config.toml");
    let config_str = tokio::fs::read_to_string(&config_path).await
        .context("Failed to read config. Please run `cis init` first")?;
    
    let node_id = parse_config_value(&config_str, "node_id")
        .unwrap_or_else(|| "unknown".to_string());
    let did = parse_config_value(&config_str, "did")
        .unwrap_or_else(|| "unknown".to_string());
    let hostname = parse_config_value(&config_str, "hostname")
        .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().to_string());
    
    let node = PairingNodeInfo {
        node_id,
        did,
        hostname,
    };
    
    println!();
    println!("🔍 正在使用组网码 {} 查找节点...", code);
    println!();
    
    let manager = Arc::new(PairingManager::new());
    let service = PairingService::new(manager);
    
    // 确定目标地址
    let target_addr = if let Some(addr_str) = address {
        addr_str.parse()
            .context("Invalid address format")?
    } else {
        // 使用广播发现，或尝试默认端口
        println!("🌐 搜索网络中的节点...");
        
        // 简化实现：直接发送到广播地址
        "255.255.255.255:6768".parse()
            .context("Failed to parse broadcast address")?
    };
    
    // 发送组网请求
    match service.request_pairing(code, target_addr, node).await {
        Ok(result) => {
            println!("✅ 发现目标节点!");
            println!("   节点ID: {}", result.node_id);
            println!("   端点: {}", result.endpoint);
            
            if let Some(did) = result.did {
                println!("   DID: {}", did);
            }
            
            println!();
            println!("💡 组网请求已发送，等待目标节点确认...");
            println!();
            println!("使用以下命令查看状态:");
            println!("   cis neighbor list");
        }
        Err(e) => {
            eprintln!("❌ 组网失败: {}", e);
            eprintln!();
            eprintln!("可能的原因:");
            eprintln!("   - 组网码已过期");
            eprintln!("   - 目标节点不在同一网络");
            eprintln!("   - 目标节点未开启组网监听");
            eprintln!();
            eprintln!("建议:");
            eprintln!("   - 请确认组网码正确且未过期（有效期5分钟）");
            eprintln!("   - 如果跨网段，请指定目标地址: --address <IP>:6768");
        }
    }
    
    Ok(())
}

/// 处理 cancel 命令
async fn handle_cancel() -> Result<()> {
    println!("组网会话已取消（如要取消，请按 Ctrl+C 结束当前命令）");
    Ok(())
}

/// 处理 status 命令
async fn handle_status() -> Result<()> {
    println!("当前无活动的组网会话");
    println!();
    println!("要生成组网码，请运行:");
    println!("   cis pair generate");
    Ok(())
}
