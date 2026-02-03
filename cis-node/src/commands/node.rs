//! # 节点管理命令
//!
//! 管理静态配置的联邦节点（长期在线节点发现）

use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use std::path::PathBuf;

/// 节点子命令
#[derive(Subcommand, Debug)]
pub enum NodeAction {
    /// 添加静态节点（长期在线节点）
    Add {
        /// 节点地址 (支持: host:port / ip:port / domain)
        /// 示例: seed1.cis.dev:6767, 192.168.1.100:6767
        address: String,
        
        /// 节点名称（可选，默认为地址）
        #[arg(short, long)]
        name: Option<String>,
        
        /// 是否标记为可信节点
        #[arg(long)]
        trusted: bool,
        
        /// 是否为种子节点（用于其他节点发现）
        #[arg(long)]
        seed: bool,
    },
    
    /// 批量添加节点
    AddBatch {
        /// 逗号分隔的节点地址列表
        /// 示例: "seed1:6767,seed2:6767,192.168.1.100"
        addresses: String,
    },
    
    /// 移除静态节点
    Remove {
        /// 节点名称或地址
        name_or_address: String,
    },
    
    /// 列出所有静态配置的节点
    List {
        /// 显示详细信息（包括连接状态）
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// 测试节点连通性
    Ping {
        /// 节点名称或地址
        name_or_address: String,
    },
    
    /// 从配置文件导入节点
    Import {
        /// 配置文件路径（默认: ./nodes.txt）
        #[arg(default_value = "nodes.txt")]
        file: PathBuf,
    },
    
    /// 导出节点配置
    Export {
        /// 输出文件路径
        #[arg(default_value = "nodes-export.txt")]
        output: PathBuf,
    },
    
    /// 编辑配置文件（打开默认编辑器）
    Edit,
}

/// 节点命令参数
#[derive(Args, Debug)]
pub struct NodeArgs {
    #[command(subcommand)]
    pub action: NodeAction,
}

/// 处理节点命令
pub async fn handle_node(args: NodeArgs) -> Result<()> {
    match args.action {
        NodeAction::Add { address, name, trusted, seed } => {
            add_node(&address, name.as_deref(), trusted, seed).await
        }
        NodeAction::AddBatch { addresses } => {
            add_batch_nodes(&addresses).await
        }
        NodeAction::Remove { name_or_address } => {
            remove_node(&name_or_address).await
        }
        NodeAction::List { verbose } => {
            list_nodes(verbose).await
        }
        NodeAction::Ping { name_or_address } => {
            ping_node(&name_or_address).await
        }
        NodeAction::Import { file } => {
            import_nodes(&file).await
        }
        NodeAction::Export { output } => {
            export_nodes(&output).await
        }
        NodeAction::Edit => {
            edit_config().await
        }
    }
}

/// 添加单个节点到配置文件
async fn add_node(
    address: &str,
    name: Option<&str>,
    trusted: bool,
    _seed: bool,
) -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    // 解析地址
    let (host, port) = parse_address(address)?;
    let node_name = name.unwrap_or(&host);
    
    // 读取现有配置
    let config_path = Paths::config_file();
    let config_content = if config_path.exists() {
        std::fs::read_to_string(&config_path)?
    } else {
        return Err(anyhow::anyhow!("CIS not initialized. Run 'cis init' first."));
    };
    
    // 添加节点到配置
    let new_entry = format!(r#"{} = {{ host = "{}", port = {}, trusted = {} }}"#,
        node_name, host, port, trusted
    );
    
    // 检查是否已存在
    if config_content.contains(&format!("host = \"{}\"", host)) {
        println!("⚠️  Node '{}' already exists", host);
        println!("   Use 'cis node remove {}' to remove it first", host);
        return Ok(());
    }
    
    // 追加到 federation.known_peers 部分
    let updated_config = add_to_known_peers(&config_content, &new_entry)?;
    
    // 写回文件
    std::fs::write(&config_path, updated_config)?;
    
    println!("✅ Added static node:");
    println!("   Name:     {}", node_name);
    println!("   Address:  {}:{}", host, port);
    println!("   Trusted:  {}", if trusted { "yes" } else { "no" });
    println!();
    println!("   Restart CIS to connect: cis node restart");
    
    Ok(())
}

/// 批量添加节点
async fn add_batch_nodes(addresses: &str) -> Result<()> {
    let addrs: Vec<&str> = addresses.split(',').map(|s| s.trim()).collect();
    
    println!("Adding {} nodes...\n", addrs.len());
    
    for (i, addr) in addrs.iter().enumerate() {
        if addr.is_empty() {
            continue;
        }
        
        println!("[{}/{}] Adding: {}", i + 1, addrs.len(), addr);
        
        if let Err(e) = add_node(addr, None, false, false).await {
            println!("   ❌ Failed: {}", e);
        }
    }
    
    println!("\n✅ Batch add complete");
    Ok(())
}

/// 移除节点
async fn remove_node(name_or_address: &str) -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    let config_path = Paths::config_file();
    if !config_path.exists() {
        return Err(anyhow::anyhow!("CIS not initialized"));
    }
    
    let config_content = std::fs::read_to_string(&config_path)?;
    
    // 移除匹配的节点配置
    let updated_config = remove_from_known_peers(&config_content, name_or_address)?;
    
    std::fs::write(&config_path, updated_config)?;
    
    println!("✅ Removed node: {}", name_or_address);
    
    Ok(())
}

/// 列出所有静态节点
async fn list_nodes(verbose: bool) -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    let config_path = Paths::config_file();
    if !config_path.exists() {
        println!("❌ CIS not initialized");
        return Ok(());
    }
    
    let config_content = std::fs::read_to_string(&config_path)?;
    
    // 解析 known_peers
    let peers = parse_known_peers(&config_content)?;
    
    if peers.is_empty() {
        println!("No static nodes configured.");
        println!();
        println!("💡 Add nodes with:");
        println!("   cis node add seed1.example.com:6767 --trusted");
        println!("   cis node add 192.168.1.100 --name home-server");
        return Ok(());
    }
    
    println!("\n📡 Static Nodes ({} configured)\n", peers.len());
    
    if verbose {
        println!("{:<15} {:<25} {:<10} {:<15}", "NAME", "ADDRESS", "TRUSTED", "STATUS");
        println!("{}", "-".repeat(70));
        
        for peer in &peers {
            // 简单的连通性检查（仅显示，非阻塞）
            let status = check_connectivity(&peer.host, peer.port).await;
            
            println!("{:<15} {:<25} {:<10} {:<15}",
                peer.name,
                format!("{}:{}", peer.host, peer.port),
                if peer.trusted { "✓" } else { "-" },
                status
            );
        }
    } else {
        println!("{:<15} {:<25} {}", "NAME", "ADDRESS", "TRUSTED");
        println!("{}", "-".repeat(55));
        
        for peer in &peers {
            println!("{:<15} {:<25} {}",
                peer.name,
                format!("{}:{}", peer.host, peer.port),
                if peer.trusted { "✓" } else { "-" }
            );
        }
    }
    
    println!();
    Ok(())
}

/// Ping 节点测试连通性
async fn ping_node(name_or_address: &str) -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    let config_path = Paths::config_file();
    if !config_path.exists() {
        return Err(anyhow::anyhow!("CIS not initialized"));
    }
    
    let config_content = std::fs::read_to_string(&config_path)?;
    let peers = parse_known_peers(&config_content)?;
    
    // 查找节点
    let peer = peers.iter()
        .find(|p| p.name == name_or_address || p.host == name_or_address)
        .ok_or_else(|| anyhow::anyhow!("Node '{}' not found", name_or_address))?;
    
    println!("Pinging {} ({}:{})...", peer.name, peer.host, peer.port);
    
    // 尝试 TCP 连接
    let start = std::time::Instant::now();
    match tokio::net::TcpStream::connect((peer.host.as_str(), peer.port)).await {
        Ok(_) => {
            let rtt = start.elapsed();
            println!("✅ Online (RTT: {:?})", rtt);
        }
        Err(e) => {
            println!("❌ Offline: {}", e);
        }
    }
    
    Ok(())
}

/// 从文件导入节点
async fn import_nodes(file: &PathBuf) -> Result<()> {
    if !file.exists() {
        println!("Creating example file: {}", file.display());
        let example = r#"# CIS Static Nodes Configuration
# Format: host:port or host (default port 6767)
# Lines starting with # are ignored

# Seed nodes
seed1.cis-network.org:6767
seed2.cis-network.org:6767

# Private nodes
192.168.1.100:6767
10.0.0.5
"#;
        std::fs::write(file, example)?;
        println!("✅ Created example file: {}", file.display());
        println!("   Edit this file and run 'cis node import' again");
        return Ok(());
    }
    
    let content = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = content.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    
    println!("Importing {} nodes from {}...\n", lines.len(), file.display());
    
    let addresses = lines.join(",");
    add_batch_nodes(&addresses).await
}

/// 导出节点配置
async fn export_nodes(output: &PathBuf) -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    let config_path = Paths::config_file();
    if !config_path.exists() {
        return Err(anyhow::anyhow!("CIS not initialized"));
    }
    
    let config_content = std::fs::read_to_string(&config_path)?;
    let peers = parse_known_peers(&config_content)?;
    
    let mut output_content = String::from("# CIS Static Nodes Export\n\n");
    
    for peer in &peers {
        output_content.push_str(&format!("{}\n", peer.host));
        if peer.port != 6767 {
            output_content.push_str(&format!("# Port: {}\n", peer.port));
        }
        if peer.trusted {
            output_content.push_str("# Trusted: yes\n");
        }
        output_content.push('\n');
    }
    
    std::fs::write(output, output_content)?;
    
    println!("✅ Exported {} nodes to {}", peers.len(), output.display());
    
    Ok(())
}

/// 编辑配置文件
async fn edit_config() -> Result<()> {
    use cis_core::storage::paths::Paths;
    
    let config_path = Paths::config_file();
    if !config_path.exists() {
        return Err(anyhow::anyhow!("CIS not initialized"));
    }
    
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    
    println!("Opening config with: {}", editor);
    println!("   {}", config_path.display());
    
    std::process::Command::new(&editor)
        .arg(&config_path)
        .status()
        .context("Failed to open editor")?;
    
    println!("✅ Config updated");
    
    Ok(())
}

// ============================================================================
// Helper functions
// ============================================================================

/// 解析地址字符串为 (host, port)
fn parse_address(address: &str) -> Result<(String, u16)> {
    if address.contains(':') {
        let parts: Vec<&str> = address.split(':').collect();
        if parts.len() == 2 {
            let host = parts[0].to_string();
            let port: u16 = parts[1].parse()
                .context("Invalid port number")?;
            return Ok((host, port));
        }
    }
    
    // 默认端口
    Ok((address.to_string(), 6767))
}

/// 解析配置文件中的 known_peers
fn parse_known_peers(config: &str) -> Result<Vec<KnownPeer>> {
    let mut peers = Vec::new();
    
    // 简单解析 TOML 格式的 known_peers
    if let Some(start) = config.find("[federation]") {
        let section = &config[start..];
        
        for line in section.lines() {
            if line.starts_with("[") && !line.starts_with("[federation]") {
                break; // 下一个 section
            }
            
            // 查找类似: node1 = { host = "...", port = ... }
            if line.contains("=") && line.contains("host") {
                if let Some(name) = line.split('=').next() {
                    let name = name.trim();
                    if let Some(host) = extract_value(line, "host") {
                        let port = extract_value(line, "port")
                            .and_then(|p| p.parse().ok())
                            .unwrap_or(6767u16);
                        let trusted = line.contains("trusted = true");
                        
                        peers.push(KnownPeer {
                            name: name.to_string(),
                            host,
                            port,
                            trusted,
                        });
                    }
                }
            }
        }
    }
    
    Ok(peers)
}

/// 从配置行提取值（简单解析，不使用 regex）
fn extract_value(line: &str, key: &str) -> Option<String> {
    let key_pattern = format!(r#"{} = ""#, key);
    if let Some(pos) = line.find(&key_pattern) {
        let after_key = &line[pos + key_pattern.len()..];
        if let Some(end_quote) = after_key.find('"') {
            return Some(after_key[..end_quote].to_string());
        }
    }
    None
}

/// 添加条目到 known_peers 配置
fn add_to_known_peers(config: &str, entry: &str) -> Result<String> {
    // 查找 [federation] section
    if let Some(pos) = config.find("[federation]") {
        let section_start = pos + "[federation]".len();
        let before = &config[..section_start];
        let after = &config[section_start..];
        
        // 找到 known_peers 数组或创建新数组
        if after.contains("known_peers") {
            // 在现有数组中添加
            let updated = format!("{}\n{}\n{}", before, entry, after);
            Ok(updated)
        } else {
            // 创建新数组
            let updated = format!("{}\n\nknown_peers = [\n    {}\n]\n{}",
                before, entry, after);
            Ok(updated)
        }
    } else {
        // 添加 federation section
        let updated = format!("{}\n\n[federation]\nknown_peers = [\n    {}\n]\n",
            config, entry);
        Ok(updated)
    }
}

/// 从 known_peers 中移除条目
fn remove_from_known_peers(config: &str, name_or_address: &str) -> Result<String> {
    // 简单的行移除
    let lines: Vec<&str> = config.lines().collect();
    let mut result = Vec::new();
    let mut removed = false;
    
    for line in lines {
        if line.contains(name_or_address) {
            removed = true;
            continue; // 跳过这行
        }
        result.push(line);
    }
    
    if !removed {
        return Err(anyhow::anyhow!("Node '{}' not found in config", name_or_address));
    }
    
    Ok(result.join("\n"))
}

/// 检查节点连通性
async fn check_connectivity(host: &str, port: u16) -> String {
    match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        tokio::net::TcpStream::connect((host, port))
    ).await {
        Ok(Ok(_)) => "🟢 online".to_string(),
        _ => "⚪ offline".to_string(),
    }
}

/// 已配置节点信息
struct KnownPeer {
    name: String,
    host: String,
    port: u16,
    trusted: bool,
}
