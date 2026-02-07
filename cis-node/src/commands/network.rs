//! # Network Management Commands
//!
//! Commands for network access control:
//! - `cis network status` - Show network status
//! - `cis network allow <did>` - Add DID to whitelist
//! - `cis network deny <did>` - Add DID to blacklist
//! - `cis network unallow <did>` - Remove from whitelist
//! - `cis network undeny <did>` - Remove from blacklist
//! - `cis network mode <mode>` - Set network mode
//! - `cis network list <whitelist|blacklist>` - List entries
//! - `cis network sync` - Sync ACL from peers

use clap::Subcommand;
use cis_core::network::{NetworkAcl, NetworkMode};
// use colored::Colorize;

/// Network management commands
#[derive(Debug, Subcommand)]
pub enum NetworkCommands {
    /// Show network status
    Status,
    
    /// Add DID to whitelist
    Allow {
        /// DID to allow
        did: String,
        /// Reason for adding
        #[arg(short, long)]
        reason: Option<String>,
    },
    
    /// Add DID to blacklist
    Deny {
        /// DID to deny
        did: String,
        /// Reason for denying
        #[arg(short, long)]
        reason: Option<String>,
        /// Expiration time (e.g., "7d", "24h")
        #[arg(short, long)]
        expires: Option<String>,
    },
    
    /// Remove DID from whitelist
    Unallow {
        /// DID to remove
        did: String,
    },
    
    /// Remove DID from blacklist
    Undeny {
        /// DID to remove
        did: String,
    },
    
    /// Set network mode
    Mode {
        /// Mode: whitelist, solitary, open, quarantine
        mode: String,
    },
    
    /// List whitelist or blacklist entries
    List {
        /// List to show: whitelist or blacklist
        #[arg(value_enum)]
        list_type: ListType,
    },
    
    /// Sync ACL from a peer
    Sync {
        /// Peer ID to sync from
        #[arg(short, long)]
        from: Option<String>,
        /// Sync to all connected peers
        #[arg(short, long)]
        broadcast: bool,
    },
    
    /// Show audit log
    Audit {
        /// Number of entries to show
        #[arg(short, long, default_value = "50")]
        limit: usize,
        /// Filter by event type
        #[arg(short, long)]
        event_type: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ListType {
    Whitelist,
    Blacklist,
}

/// Handle network commands
pub async fn handle(cmd: NetworkCommands) -> anyhow::Result<()> {
    let acl_path = cis_core::network::default_acl_path();
    
    match cmd {
        NetworkCommands::Status => {
            show_status(&acl_path).await?;
        }
        NetworkCommands::Allow { did, reason } => {
            add_to_whitelist(&acl_path, did, reason).await?;
        }
        NetworkCommands::Deny { did, reason, expires } => {
            add_to_blacklist(&acl_path, did, reason, expires).await?;
        }
        NetworkCommands::Unallow { did } => {
            remove_from_whitelist(&acl_path, did).await?;
        }
        NetworkCommands::Undeny { did } => {
            remove_from_blacklist(&acl_path, did).await?;
        }
        NetworkCommands::Mode { mode } => {
            set_mode(&acl_path, mode).await?;
        }
        NetworkCommands::List { list_type } => {
            list_entries(&acl_path, list_type).await?;
        }
        NetworkCommands::Sync { from, broadcast } => {
            sync_acl(from, broadcast).await?;
        }
        NetworkCommands::Audit { limit, event_type } => {
            show_audit(limit, event_type).await?;
        }
    }
    
    Ok(())
}

/// Show network status
async fn show_status(acl_path: &std::path::Path) -> anyhow::Result<()> {
    let acl = if acl_path.exists() {
        NetworkAcl::load(acl_path)?
    } else {
        println!("{}", "Network ACL not initialized. Run 'cis init' first.");
        return Ok(());
    };
    
    println!("{}", "═".repeat(60));
    println!("{}", " Network Status ");
    println!("{}", "═".repeat(60));
    
    println!("\n{}", "Local Identity:");
    println!("  DID:    {}", acl.local_did);
    
    println!("\n{}", "Network Mode:");
    let mode_str = match acl.mode {
        NetworkMode::Whitelist => "whitelist",
        NetworkMode::Solitary => "solitary",
        NetworkMode::Open => "open",
        NetworkMode::Quarantine => "quarantine",
    };
    println!("  Mode:   {}", mode_str);
    
    let mode_desc = match acl.mode {
        NetworkMode::Whitelist => "Only whitelisted DIDs can connect (recommended)",
        NetworkMode::Solitary => "Rejecting all new connections",
        NetworkMode::Open => "Accepting any verified DID (insecure)",
        NetworkMode::Quarantine => "Accepting connections but restricting data",
    };
    println!("  {}", mode_desc);
    
    println!("\n{}", "Access Control:");
    println!("  Whitelist: {} entries", acl.whitelist.len().to_string());
    println!("  Blacklist: {} entries", acl.blacklist.len().to_string());
    println!("  ACL Version: {}", acl.version.to_string());
    
    let updated = chrono::DateTime::from_timestamp(acl.updated_at, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|| "Unknown".into());
    println!("  Last Updated: {}", updated);
    
    // Show recent whitelist entries
    if !acl.whitelist.is_empty() {
        println!("\n{}", "Recent Whitelist Entries:");
        for entry in acl.whitelist.iter().rev().take(5) {
            let added = chrono::DateTime::from_timestamp(entry.added_at, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "Unknown".into());
            println!("  • {} (added {})", entry.did, added);
            if let Some(ref reason) = entry.reason {
                println!("    Reason: {}", reason);
            }
        }
        if acl.whitelist.len() > 5 {
            println!("    ... and {} more", acl.whitelist.len() - 5);
        }
    }
    
    println!("\n{}", "═".repeat(60));
    
    Ok(())
}

/// Add DID to whitelist
async fn add_to_whitelist(
    acl_path: &std::path::Path,
    did: String,
    reason: Option<String>,
) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    // Validate DID format
    if !cis_core::identity::did::DIDManager::is_valid_did(&did) {
        println!("{} Invalid DID format: {}", "Error:", did);
        return Ok(());
    }
    
    let local_did = acl.local_did.clone();
    let mut entry = cis_core::network::AclEntry::new(&did, &local_did);
    
    if let Some(r) = reason {
        entry.reason = Some(r);
    }
    
    acl.whitelist.retain(|e| e.did != did); // Remove if exists
    acl.whitelist.push(entry);
    acl.bump_version();
    acl.save(acl_path)?;
    
    println!("{} Added {} to whitelist", "✓", did);
    
    // 广播 ACL 更新到 P2P 网络
    println!("  Broadcasting ACL update to peers...");
    
    // 创建 P2P 配置并尝试广播
    let p2p_config = cis_core::p2p::P2PConfig {
        enable_dht: true,
        bootstrap_nodes: vec![],
        enable_nat_traversal: false,
        external_address: None,
    };
    
    // 尝试创建 P2P 网络并广播 ACL 变更
    match cis_core::p2p::P2PNetwork::new(
        acl.local_did.clone(),
        acl.local_did.clone(),
        "0.0.0.0:7677",
        p2p_config,
    ).await {
        Ok(p2p) => {
            match serde_json::to_vec(&acl) {
                Ok(acl_data) => {
                    match p2p.broadcast("acl/update", acl_data).await {
                        Ok(()) => {
                            println!("  ✓ ACL update broadcasted to peers");
                        }
                        Err(e) => {
                            println!("  ⚠️  Failed to broadcast ACL update: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("  ⚠️  Failed to serialize ACL: {}", e);
                }
            }
        }
        Err(e) => {
            println!("  ⚠️  P2P network not available: {}", e);
            println!("     ACL update will be synced on next network startup");
        }
    }
    
    Ok(())
}

/// Add DID to blacklist
async fn add_to_blacklist(
    acl_path: &std::path::Path,
    did: String,
    reason: Option<String>,
    expires: Option<String>,
) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    if !cis_core::identity::did::DIDManager::is_valid_did(&did) {
        println!("{} Invalid DID format: {}", "Error:", did);
        return Ok(());
    }
    
    let local_did = acl.local_did.clone();
    let mut entry = cis_core::network::AclEntry::new(&did, &local_did);
    
    if let Some(r) = reason {
        entry.reason = Some(r);
    }
    
    // Parse expiration
    if let Some(exp) = expires {
        let duration = parse_duration(&exp)?;
        entry.expires_at = Some(chrono::Utc::now().timestamp() + duration);
    }
    
    let has_expiration = entry.expires_at.is_some();
    let exp_desc_str = if has_expiration { Some(exp_desc(&entry)) } else { None };
    
    acl.blacklist.retain(|e| e.did != did); // Remove if exists
    acl.blacklist.push(entry);
    acl.bump_version();
    acl.save(acl_path)?;
    
    println!("{} Added {} to blacklist", "✓", did);
    
    if let Some(desc) = exp_desc_str {
        println!("  Expires: {}", desc);
    }
    
    Ok(())
}

/// Remove from whitelist
async fn remove_from_whitelist(acl_path: &std::path::Path, did: String) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    let before = acl.whitelist.len();
    acl.whitelist.retain(|e| e.did != did);
    if acl.whitelist.len() < before {
        acl.bump_version();
        acl.save(acl_path)?;
        println!("{} Removed {} from whitelist", "✓", did);
    } else {
        println!("{} {} was not in whitelist", "!", did);
    }
    
    Ok(())
}

/// Remove from blacklist
async fn remove_from_blacklist(acl_path: &std::path::Path, did: String) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    let before = acl.blacklist.len();
    acl.blacklist.retain(|e| e.did != did);
    
    if acl.blacklist.len() < before {
        acl.bump_version();
        acl.save(acl_path)?;
        println!("{} Removed {} from blacklist", "✓", did);
    } else {
        println!("{} {} was not in blacklist", "!", did);
    }
    
    Ok(())
}

/// Set network mode
async fn set_mode(acl_path: &std::path::Path, mode: String) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    let new_mode = match mode.as_str() {
        "whitelist" => NetworkMode::Whitelist,
        "solitary" => NetworkMode::Solitary,
        "open" => NetworkMode::Open,
        "quarantine" => NetworkMode::Quarantine,
        _ => {
            println!("{} Invalid mode: {}", "Error:", mode);
            println!("Valid modes: whitelist, solitary, open, quarantine");
            return Ok(());
        }
    };
    
    let old_mode_str = format!("{}", acl.mode);
    acl.mode = new_mode;
    acl.bump_version();
    acl.save(acl_path)?;
    
    println!("{} Changed network mode: {} → {}", 
        "✓",
        old_mode_str,
        format!("{}", new_mode)
    );
    
    match new_mode {
        NetworkMode::Solitary => {
            println!("\n{} Solitary mode activated.", "⚠");
            println!("  Only existing connections to whitelisted peers will be maintained.");
            println!("  New connections will be rejected.");
        }
        NetworkMode::Whitelist => {
            println!("\n{} Whitelist mode activated.", "✓");
            println!("  Only DIDs in the whitelist can connect.");
        }
        NetworkMode::Open => {
            println!("\n{} Open mode activated.", "⚠");
            println!("  {} This mode is insecure and should only be used for testing.", "Warning:");
        }
        _ => {}
    }
    
    Ok(())
}

/// List entries
async fn list_entries(acl_path: &std::path::Path, list_type: ListType) -> anyhow::Result<()> {
    let acl = if acl_path.exists() {
        NetworkAcl::load(acl_path)?
    } else {
        println!("{}", "Network ACL not initialized.");
        return Ok(());
    };
    
    let entries = match list_type {
        ListType::Whitelist => &acl.whitelist,
        ListType::Blacklist => &acl.blacklist,
    };
    
    let title = match list_type {
        ListType::Whitelist => "Whitelist",
        ListType::Blacklist => "Blacklist",
    };
    
    println!("{}", format!(" {} ", title));
    println!("Total: {} entries\n", entries.len());
    
    if entries.is_empty() {
        println!("{}", "No entries.");
        return Ok(());
    }
    
    for (i, entry) in entries.iter().enumerate() {
        let num = format!("{:3}.", i + 1);
        let did = &entry.did;
        
        println!("{} {}", num, did);
        
        let added = chrono::DateTime::from_timestamp(entry.added_at, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "Unknown".into());
        println!("     Added: {} by {}", added, entry.added_by);
        
        if let Some(ref reason) = entry.reason {
            println!("     Reason: {}", reason);
        }
        
        if let Some(exp) = entry.expires_at {
            let exp_str = chrono::DateTime::from_timestamp(exp, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "Unknown".into());
            if exp < chrono::Utc::now().timestamp() {
                println!("     Expires: {} {}", exp_str, "(EXPIRED)");
            } else {
                println!("     Expires: {}", exp_str);
            }
        }
        
        println!();
    }
    
    Ok(())
}

/// Sync ACL from peers
async fn sync_acl(from: Option<String>, broadcast: bool) -> anyhow::Result<()> {
    if broadcast {
        println!("{} Broadcasting local ACL to all connected peers...", "→");
        
        // 加载本地 ACL
        let acl_path = get_acl_path().await?;
        let acl = load_or_create_acl(&acl_path).await?;
        
        // 序列化 ACL 数据
        let acl_data = serde_json::to_vec(&acl)?;
        
        // 通过 P2P 广播
        println!("  Broadcasting {} bytes to topic 'acl/update'", acl_data.len());
        
        // 创建 P2P 网络并广播 ACL 变更
        let p2p_config = cis_core::p2p::P2PConfig {
            enable_dht: true,
            bootstrap_nodes: vec![],
            enable_nat_traversal: false,
            external_address: None,
        };
        
        match cis_core::p2p::P2PNetwork::new(
            acl.local_did.clone(),
            acl.local_did.clone(),
            "0.0.0.0:7677",
            p2p_config,
        ).await {
            Ok(p2p) => {
                match p2p.broadcast("acl/update", acl_data).await {
                    Ok(()) => {
                        println!("{} ACL broadcast complete", "✓");
                    }
                    Err(e) => {
                        println!("{} Failed to broadcast ACL: {}", "✗", e);
                    }
                }
            }
            Err(e) => {
                println!("{} P2P network not available: {}", "⚠️", e);
                println!("  ACL will be synced when P2P is available");
            }
        }
    } else if let Some(peer) = from {
        println!("{} Syncing ACL from {}...", "→", peer);
        
        // 通过 P2P 同步特定节点的公域记忆
        println!("  Requesting ACL sync from peer: {}", peer);
        
        // 创建 P2P 网络并同步 ACL
        let acl_path = get_acl_path().await?;
        let acl = load_or_create_acl(&acl_path).await?;
        
        let p2p_config = cis_core::p2p::P2PConfig {
            enable_dht: true,
            bootstrap_nodes: vec![peer.clone()],
            enable_nat_traversal: false,
            external_address: None,
        };
        
        match cis_core::p2p::P2PNetwork::new(
            acl.local_did.clone(),
            acl.local_did.clone(),
            "0.0.0.0:7677",
            p2p_config,
        ).await {
            Ok(p2p) => {
                match p2p.sync_public_memory(&peer).await {
                    Ok(()) => {
                        println!("{} ACL sync from {} complete", "✓", peer);
                    }
                    Err(e) => {
                        println!("{} Failed to sync ACL from {}: {}", "✗", peer, e);
                    }
                }
            }
            Err(e) => {
                println!("{} P2P network not available: {}", "⚠️", e);
            }
        }
    } else {
        println!("{}", "Error: Either --from or --broadcast must be specified");
        println!("  cis network sync --from <peer-id>    # Sync from specific peer");
        println!("  cis network sync --broadcast         # Broadcast to all peers");
    }
    
    Ok(())
}

/// Helper: Get ACL file path
async fn get_acl_path() -> anyhow::Result<std::path::PathBuf> {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("cis");
    Ok(data_dir.join("network.acl"))
}

/// Show audit log
async fn show_audit(limit: usize, event_type: Option<String>) -> anyhow::Result<()> {
    println!("{}", format!(" Recent Audit Events (last {}) ", limit));
    
    // 加载并显示审计日志
    let audit_logger = cis_core::network::AuditLogger::default();
    
    let entries = if let Some(ref evt_type) = event_type {
        // 根据事件类型过滤
        let evt_type_parsed = match evt_type.as_str() {
            "did_verification_success" => cis_core::network::AuditEventType::DidVerificationSuccess,
            "did_verification_failure" => cis_core::network::AuditEventType::DidVerificationFailure,
            "connection_blocked" => cis_core::network::AuditEventType::ConnectionBlocked,
            "communication_blocked" => cis_core::network::AuditEventType::CommunicationBlocked,
            "whitelist_add" => cis_core::network::AuditEventType::WhitelistAdd,
            "whitelist_remove" => cis_core::network::AuditEventType::WhitelistRemove,
            "blacklist_add" => cis_core::network::AuditEventType::BlacklistAdd,
            "blacklist_remove" => cis_core::network::AuditEventType::BlacklistRemove,
            "mode_change" => cis_core::network::AuditEventType::ModeChange,
            "auth_attempt" => cis_core::network::AuditEventType::AuthAttempt,
            "auth_success" => cis_core::network::AuditEventType::AuthSuccess,
            "auth_failure" => cis_core::network::AuditEventType::AuthFailure,
            "acl_sync" => cis_core::network::AuditEventType::AclSync,
            "solitary_triggered" => cis_core::network::AuditEventType::SolitaryTriggered,
            _ => {
                println!("Unknown event type: {}", evt_type);
                return Ok(());
            }
        };
        audit_logger.get_by_type(evt_type_parsed, limit).await
    } else {
        // 获取最近的日志
        audit_logger.get_recent(limit).await
    };
    
    if entries.is_empty() {
        println!("No audit events found.");
    } else {
        println!("Found {} events:\n", entries.len());
        for (i, entry) in entries.iter().enumerate() {
            println!("{}. {}", i + 1, entry.format());
            if let Some(ref details) = entry.details {
                println!("   Details: {}", serde_json::to_string_pretty(details)?);
            }
            println!();
        }
    }
    
    Ok(())
}

/// Helper: Load or create ACL
async fn load_or_create_acl(acl_path: &std::path::Path) -> anyhow::Result<NetworkAcl> {
    if acl_path.exists() {
        Ok(NetworkAcl::load(acl_path)?)
    } else {
        // Create default
        let local_did = get_local_did().await?;
        let acl = NetworkAcl::new(local_did);
        acl.save(acl_path)?;
        Ok(acl)
    }
}

/// Helper: Get local DID
async fn get_local_did() -> anyhow::Result<String> {
    // Try to load from identity file
    let did_path = cis_core::storage::paths::Paths::config_dir().join("node.did");
    
    if did_path.exists() {
        let content = tokio::fs::read_to_string(&did_path).await?;
        let did = content.trim();
        if cis_core::identity::did::DIDManager::is_valid_did(did) {
            return Ok(did.to_string());
        }
    }
    
    // Generate new DID
    let manager = cis_core::identity::did::DIDManager::generate("local-node")?;
    let did = manager.did().to_string();
    
    // Save
    tokio::fs::create_dir_all(did_path.parent().unwrap()).await?;
    tokio::fs::write(&did_path, &did).await?;
    
    Ok(did)
}

/// Helper: Parse duration string (e.g., "7d", "24h")
fn parse_duration(s: &str) -> anyhow::Result<i64> {
    let s = s.trim();
    
    if s.is_empty() {
        return Err(anyhow::anyhow!("Empty duration"));
    }
    
    let (num_str, unit) = s.split_at(s.len() - 1);
    let num: i64 = num_str.parse()?;
    
    let seconds = match unit {
        "s" => num,
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        "w" => num * 604800,
        _ => return Err(anyhow::anyhow!("Invalid duration unit: {}", unit)),
    };
    
    Ok(seconds)
}

/// Helper: Format expiration description
fn exp_desc(entry: &cis_core::network::AclEntry) -> String {
    entry.expires_at
        .map(|exp| {
            let now = chrono::Utc::now().timestamp();
            let diff = exp - now;
            
            if diff < 0 {
                "expired".to_string()
            } else if diff < 3600 {
                format!("in {} minutes", diff / 60)
            } else if diff < 86400 {
                format!("in {} hours", diff / 3600)
            } else {
                format!("in {} days", diff / 86400)
            }
        })
        .unwrap_or_else(|| "never".to_string())
}

// Note: NetworkAcl::bump_version() is now public in cis-core, no extension trait needed
