//! # Network Management Commands
//!
//! Commands for network access control:
//! - `cis network status` - Show network status
//! - `cis network mode <whitelist|solitary|open|quarantine>` - Set network mode
//! - `cis network allow <did>` - Add DID to whitelist
//! - `cis network deny <did>` - Add DID to blacklist
//! - `cis network quarantine <did>` - Quarantine a DID
//! - `cis network unallow <did>` - Remove from whitelist
//! - `cis network undeny <did>` - Remove from blacklist
//! - `cis network unquarantine <did>` - Remove from quarantine
//! - `cis network list [whitelist|blacklist|quarantine]` - List entries
//! - `cis network acl sync` - Sync ACL from peers
//! - `cis network rules` - Manage ACL rules
//!
//! ## Examples
//!
//! ```bash
//! # Set network mode
//! cis network mode solitary
//!
//! # Allow a DID with reason and expiration
//! cis network allow did:cis:peer:abc123 --reason "Trusted node" --expires 30d
//!
//! # Deny a DID temporarily
//! cis network deny did:cis:malicious:xyz789 --reason "Spam" --expires 7d
//!
//! # Quarantine a DID (audit only)
//! cis network quarantine did:cis:suspicious:def456 --reason "Under investigation"
//!
//! # List with format
//! cis network list --format json
//!
//! # Sync ACL
//! cis network acl sync --broadcast
//! ```

use clap::{Subcommand, ValueEnum};
use cis_core::network::{NetworkAcl, NetworkMode, AclEntry, AclAction};
use cis_core::network::acl_rules::{AclRule, AclRulesEngine, Condition, RuleContext};

/// Network management commands
#[derive(Debug, Subcommand)]
pub enum NetworkCommands {
    /// Show network status
    Status {
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    
    /// Set network mode
    Mode {
        /// Mode: whitelist, solitary, open, quarantine
        mode: NetworkModeArg,
    },
    
    /// Add DID to whitelist
    Allow {
        /// DID to allow
        did: String,
        /// Reason for adding
        #[arg(short, long)]
        reason: Option<String>,
        /// Expiration time (e.g., "7d", "24h", "30m")
        #[arg(short, long)]
        expires: Option<String>,
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
    
    /// Quarantine a DID (allow connection but restrict data)
    Quarantine {
        /// DID to quarantine
        did: String,
        /// Reason for quarantine
        #[arg(short, long)]
        reason: Option<String>,
        /// Expiration time
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
    
    /// Remove DID from quarantine
    Unquarantine {
        /// DID to remove
        did: String,
    },
    
    /// List whitelist, blacklist, or quarantine entries
    List {
        /// Type of list to show
        #[arg(value_enum)]
        list_type: Option<ListType>,
        /// Output format
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    
    /// Sync ACL from/to peers
    Sync {
        /// Peer ID to sync from
        #[arg(short, long)]
        from: Option<String>,
        /// Sync to all connected peers (broadcast)
        #[arg(short, long)]
        broadcast: bool,
    },
    
    /// Manage ACL rules
    Rules {
        #[command(subcommand)]
        action: RuleCommands,
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
    
    /// Clean up expired entries
    Cleanup,
}

/// Rule management commands
#[derive(Debug, Subcommand)]
pub enum RuleCommands {
    /// List all rules
    List {
        #[arg(short, long, value_enum, default_value = "table")]
        format: OutputFormat,
    },
    
    /// Add a new rule
    Add {
        /// Rule ID
        id: String,
        /// Rule name
        #[arg(short, long)]
        name: String,
        /// Target DID pattern (supports * wildcard)
        #[arg(short, long)]
        did: Option<String>,
        /// Action: allow, deny, quarantine
        #[arg(short, long, value_enum)]
        action: RuleAction,
        /// Priority (lower = higher priority)
        #[arg(short, long, default_value = "100")]
        priority: i32,
        /// IP CIDR condition (e.g., "192.168.0.0/16")
        #[arg(long)]
        ip_cidr: Option<String>,
        /// Time window (e.g., "09:00-17:00")
        #[arg(long)]
        time_window: Option<String>,
        /// Days of week for time window (0=Sun, 6=Sat, e.g., "1,2,3,4,5")
        #[arg(long)]
        days: Option<String>,
        /// Required capability
        #[arg(long)]
        capability: Option<String>,
    },
    
    /// Remove a rule
    Remove {
        /// Rule ID
        id: String,
    },
    
    /// Enable a rule
    Enable {
        /// Rule ID
        id: String,
    },
    
    /// Disable a rule
    Disable {
        /// Rule ID
        id: String,
    },
    
    /// Test a rule against a context
    Test {
        /// Rule ID
        #[arg(short, long)]
        rule: Option<String>,
        /// DID to test
        #[arg(short, long)]
        did: Option<String>,
        /// IP address to test
        #[arg(long)]
        ip: Option<String>,
        /// Capability to test
        #[arg(long)]
        capability: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NetworkModeArg {
    Whitelist,
    Solitary,
    Open,
    Quarantine,
}

impl From<NetworkModeArg> for NetworkMode {
    fn from(mode: NetworkModeArg) -> Self {
        match mode {
            NetworkModeArg::Whitelist => NetworkMode::Whitelist,
            NetworkModeArg::Solitary => NetworkMode::Solitary,
            NetworkModeArg::Open => NetworkMode::Open,
            NetworkModeArg::Quarantine => NetworkMode::Quarantine,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListType {
    Whitelist,
    Blacklist,
    Quarantine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Json,
    Table,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RuleAction {
    Allow,
    Deny,
    Quarantine,
}

impl From<RuleAction> for AclAction {
    fn from(action: RuleAction) -> Self {
        match action {
            RuleAction::Allow => AclAction::Allow,
            RuleAction::Deny => AclAction::Deny,
            RuleAction::Quarantine => AclAction::Quarantine,
        }
    }
}

/// Handle network commands
pub async fn handle(cmd: NetworkCommands) -> anyhow::Result<()> {
    let acl_path = cis_core::network::default_acl_path();
    let rules_path = get_rules_path().await?;
    
    match cmd {
        NetworkCommands::Status { format } => {
            show_status(&acl_path, format).await?;
        }
        NetworkCommands::Allow { did, reason, expires } => {
            add_to_whitelist(&acl_path, did, reason, expires).await?;
        }
        NetworkCommands::Deny { did, reason, expires } => {
            add_to_blacklist(&acl_path, did, reason, expires).await?;
        }
        NetworkCommands::Quarantine { did, reason, expires } => {
            quarantine_did(&acl_path, did, reason, expires).await?;
        }
        NetworkCommands::Unallow { did } => {
            remove_from_whitelist(&acl_path, did).await?;
        }
        NetworkCommands::Undeny { did } => {
            remove_from_blacklist(&acl_path, did).await?;
        }
        NetworkCommands::Unquarantine { did } => {
            unquarantine_did(&acl_path, did).await?;
        }
        NetworkCommands::Mode { mode } => {
            set_mode(&acl_path, mode).await?;
        }
        NetworkCommands::List { list_type, format } => {
            list_entries(&acl_path, list_type, format).await?;
        }
        NetworkCommands::Sync { from, broadcast } => {
            sync_acl(from, broadcast).await?;
        }
        NetworkCommands::Rules { action } => {
            handle_rules(&rules_path, action).await?;
        }
        NetworkCommands::Audit { limit, event_type } => {
            show_audit(limit, event_type).await?;
        }
        NetworkCommands::Cleanup => {
            cleanup_expired(&acl_path, &rules_path).await?;
        }
    }
    
    Ok(())
}

/// Show network status
async fn show_status(acl_path: &std::path::Path, format: OutputFormat) -> anyhow::Result<()> {
    let acl = if acl_path.exists() {
        NetworkAcl::load(acl_path)?
    } else {
        match format {
            OutputFormat::Json => {
                println!("{{}}");
            }
            OutputFormat::Table => {
                println!("Network ACL not initialized. Run 'cis init' first.");
            }
        }
        return Ok(());
    };
    
    let rules_path = get_rules_path().await?;
    let rules_engine = load_or_create_rules_engine(&rules_path).await?;
    
    match format {
        OutputFormat::Json => {
            let status = serde_json::json!({
                "local_did": acl.local_did,
                "mode": format!("{}", acl.mode),
                "whitelist_count": acl.whitelist.len(),
                "blacklist_count": acl.blacklist.len(),
                "version": acl.version,
                "updated_at": acl.updated_at,
                "rules_count": rules_engine.rules.len(),
            });
            println!("{}", serde_json::to_string_pretty(&status)?);
        }
        OutputFormat::Table => {
            println!("{}", "═".repeat(60));
            println!(" Network Status ");
            println!("{}", "═".repeat(60));
            
            println!("\nLocal Identity:");
            println!("  DID:    {}", acl.local_did);
            
            println!("\nNetwork Mode:");
            println!("  Mode:   {}", acl.mode);
            
            let mode_desc = match acl.mode {
                NetworkMode::Whitelist => "Only whitelisted DIDs can connect (recommended)",
                NetworkMode::Solitary => "Rejecting all new connections",
                NetworkMode::Open => "Accepting any verified DID (insecure)",
                NetworkMode::Quarantine => "Accepting connections but restricting data",
            };
            println!("  {}", mode_desc);
            
            println!("\nAccess Control:");
            println!("  Whitelist: {} entries", acl.whitelist.len());
            println!("  Blacklist: {} entries", acl.blacklist.len());
            println!("  ACL Rules: {} rules", rules_engine.rules.len());
            println!("  ACL Version: {}", acl.version);
            
            let updated = chrono::DateTime::from_timestamp(acl.updated_at, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown".into());
            println!("  Last Updated: {}", updated);
            
            // Show recent whitelist entries
            if !acl.whitelist.is_empty() {
                println!("\nRecent Whitelist Entries:");
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
        }
    }
    
    Ok(())
}

/// Add DID to whitelist
async fn add_to_whitelist(
    acl_path: &std::path::Path,
    did: String,
    reason: Option<String>,
    expires: Option<String>,
) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    // Validate DID format
    if !cis_core::identity::did::DIDManager::is_valid_did(&did) {
        println!("Error: Invalid DID format: {}", did);
        return Ok(());
    }
    
    let local_did = acl.local_did.clone();
    let mut entry = AclEntry::new(&did, &local_did);
    
    if let Some(r) = reason {
        entry.reason = Some(r);
    }
    
    // Parse expiration
    if let Some(exp) = expires {
        let duration = parse_duration(&exp)?;
        entry.expires_at = Some(chrono::Utc::now().timestamp() + duration);
    }
    
    acl.whitelist.retain(|e| e.did != did); // Remove if exists
    acl.whitelist.push(entry);
    acl.bump_version();
    acl.save(acl_path)?;
    
    println!("✓ Added {} to whitelist", did);
    
    // 广播 ACL 更新到 P2P 网络
    broadcast_acl_update(&acl).await;
    
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
        println!("Error: Invalid DID format: {}", did);
        return Ok(());
    }
    
    let local_did = acl.local_did.clone();
    let mut entry = AclEntry::new(&did, &local_did);
    
    if let Some(r) = reason {
        entry.reason = Some(r);
    }
    
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
    
    println!("✓ Added {} to blacklist", did);
    
    if let Some(desc) = exp_desc_str {
        println!("  Expires: {}", desc);
    }
    
    Ok(())
}

/// Quarantine a DID
async fn quarantine_did(
    acl_path: &std::path::Path,
    did: String,
    reason: Option<String>,
    expires: Option<String>,
) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    if !cis_core::identity::did::DIDManager::is_valid_did(&did) {
        println!("Error: Invalid DID format: {}", did);
        return Ok(());
    }
    
    // Remove from whitelist and blacklist first
    acl.whitelist.retain(|e| e.did != did);
    acl.blacklist.retain(|e| e.did != did);
    
    // Add to quarantine list (stored in metadata)
    let local_did = acl.local_did.clone();
    let mut entry = AclEntry::new(&did, &local_did);
    entry.reason = reason.or_else(|| Some("Quarantined".to_string()));
    
    if let Some(exp) = expires {
        let duration = parse_duration(&exp)?;
        entry.expires_at = Some(chrono::Utc::now().timestamp() + duration);
    }
    
    // Store quarantine entries in a separate section (we'll use a metadata field)
    // For now, add to blacklist with a special marker in reason
    let quarantine_reason = format!("[QUARANTINE] {}", entry.reason.as_ref().unwrap());
    entry.reason = Some(quarantine_reason);
    acl.blacklist.push(entry);
    
    acl.bump_version();
    acl.save(acl_path)?;
    
    println!("✓ Quarantined {}", did);
    println!("  Connection allowed but data forwarding restricted.");
    
    Ok(())
}

/// Remove DID from whitelist
async fn remove_from_whitelist(acl_path: &std::path::Path, did: String) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    let before = acl.whitelist.len();
    acl.whitelist.retain(|e| e.did != did);
    if acl.whitelist.len() < before {
        acl.bump_version();
        acl.save(acl_path)?;
        println!("✓ Removed {} from whitelist", did);
    } else {
        println!("! {} was not in whitelist", did);
    }
    
    Ok(())
}

/// Remove DID from blacklist
async fn remove_from_blacklist(acl_path: &std::path::Path, did: String) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    let before = acl.blacklist.len();
    acl.blacklist.retain(|e| e.did != did);
    
    if acl.blacklist.len() < before {
        acl.bump_version();
        acl.save(acl_path)?;
        println!("✓ Removed {} from blacklist", did);
    } else {
        println!("! {} was not in blacklist", did);
    }
    
    Ok(())
}

/// Remove DID from quarantine
async fn unquarantine_did(acl_path: &std::path::Path, did: String) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    // Remove entries with quarantine marker
    let before = acl.blacklist.len();
    acl.blacklist.retain(|e| {
        if e.did == did {
            !e.reason.as_ref().map(|r| r.starts_with("[QUARANTINE]")).unwrap_or(false)
        } else {
            true
        }
    });
    
    if acl.blacklist.len() < before {
        acl.bump_version();
        acl.save(acl_path)?;
        println!("✓ Removed {} from quarantine", did);
    } else {
        println!("! {} was not quarantined", did);
    }
    
    Ok(())
}

/// Set network mode
async fn set_mode(acl_path: &std::path::Path, mode: NetworkModeArg) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    
    let new_mode: NetworkMode = mode.into();
    let old_mode_str = format!("{}", acl.mode);
    acl.mode = new_mode;
    acl.bump_version();
    acl.save(acl_path)?;
    
    println!("✓ Changed network mode: {} → {}",
        old_mode_str,
        new_mode
    );
    
    match new_mode {
        NetworkMode::Solitary => {
            println!("\n⚠️  Solitary mode activated.");
            println!("  Only existing connections to whitelisted peers will be maintained.");
            println!("  New connections will be rejected.");
        }
        NetworkMode::Whitelist => {
            println!("\n✓ Whitelist mode activated.");
            println!("  Only DIDs in the whitelist can connect.");
        }
        NetworkMode::Open => {
            println!("\n⚠️  Open mode activated.");
            println!("  Warning: This mode is insecure and should only be used for testing.");
        }
        NetworkMode::Quarantine => {
            println!("\n⚠️  Quarantine mode activated.");
            println!("  Connections allowed but data forwarding is restricted.");
        }
    }
    
    Ok(())
}

/// List entries
async fn list_entries(
    acl_path: &std::path::Path,
    list_type: Option<ListType>,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let acl = if acl_path.exists() {
        NetworkAcl::load(acl_path)?
    } else {
        println!("Network ACL not initialized.");
        return Ok(());
    };
    
    // If no type specified, show all
    let types_to_show: Vec<ListType> = match list_type {
        Some(t) => vec![t],
        None => vec![ListType::Whitelist, ListType::Blacklist, ListType::Quarantine],
    };
    
    for list_type in types_to_show {
        let entries = match list_type {
            ListType::Whitelist => &acl.whitelist,
            ListType::Blacklist => &acl.blacklist,
            ListType::Quarantine => {
                // Filter blacklist for quarantine entries
                &acl.blacklist.iter()
                    .filter(|e| e.reason.as_ref().map(|r| r.starts_with("[QUARANTINE]")).unwrap_or(false))
                    .cloned()
                    .collect::<Vec<_>>()
            }
        };
        
        let title = match list_type {
            ListType::Whitelist => "Whitelist",
            ListType::Blacklist => "Blacklist",
            ListType::Quarantine => "Quarantine",
        };
        
        match format {
            OutputFormat::Json => {
                let json_entries: Vec<serde_json::Value> = entries.iter().map(|e| {
                    serde_json::json!({
                        "did": e.did,
                        "added_at": e.added_at,
                        "added_by": e.added_by,
                        "reason": e.reason,
                        "expires_at": e.expires_at,
                        "is_expired": e.is_expired(),
                    })
                }).collect();
                println!("{}", serde_json::to_string_pretty(&json_entries)?);
            }
            OutputFormat::Table => {
                println!("\n {} ", title);
                println!("Total: {} entries\n", entries.len());
                
                if entries.is_empty() {
                    println!("No entries.");
                    continue;
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
                        let clean_reason = if reason.starts_with("[QUARANTINE] ") {
                            &reason[13..]
                        } else {
                            reason
                        };
                        println!("     Reason: {}", clean_reason);
                    }
                    
                    if let Some(exp) = entry.expires_at {
                        let exp_str = chrono::DateTime::from_timestamp(exp, 0)
                            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| "Unknown".into());
                        if exp < chrono::Utc::now().timestamp() {
                            println!("     Expires: {} (EXPIRED)", exp_str);
                        } else {
                            println!("     Expires: {}", exp_str);
                        }
                    }
                    
                    println!();
                }
            }
        }
    }
    
    Ok(())
}

/// Handle rules commands
async fn handle_rules(
    rules_path: &std::path::Path,
    action: RuleCommands,
) -> anyhow::Result<()> {
    match action {
        RuleCommands::List { format } => {
            list_rules(rules_path, format).await?;
        }
        RuleCommands::Add { id, name, did, action, priority, ip_cidr, time_window, days, capability } => {
            add_rule(rules_path, id, name, did, action, priority, ip_cidr, time_window, days, capability).await?;
        }
        RuleCommands::Remove { id } => {
            remove_rule(rules_path, &id).await?;
        }
        RuleCommands::Enable { id } => {
            toggle_rule(rules_path, &id, true).await?;
        }
        RuleCommands::Disable { id } => {
            toggle_rule(rules_path, &id, false).await?;
        }
        RuleCommands::Test { rule, did, ip, capability } => {
            test_rule(rules_path, rule, did, ip, capability).await?;
        }
    }
    Ok(())
}

/// List all rules
async fn list_rules(rules_path: &std::path::Path, format: OutputFormat) -> anyhow::Result<()> {
    let engine = load_or_create_rules_engine(rules_path).await?;
    
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&engine.rules)?);
        }
        OutputFormat::Table => {
            println!(" ACL Rules ");
            println!("Total: {} rules\n", engine.rules.len());
            
            if engine.rules.is_empty() {
                println!("No rules defined.");
                println!("\nUse 'cis network rules add' to create rules.");
                return Ok(());
            }
            
            for rule in &engine.rules {
                let status = if !rule.enabled {
                    "[DISABLED]"
                } else if rule.is_expired() {
                    "[EXPIRED]"
                } else {
                    "[ACTIVE]"
                };
                
                println!("{} {} (priority: {})", status, rule.id, rule.priority);
                println!("  Name: {}", rule.name);
                println!("  Action: {}", rule.action);
                if let Some(ref did) = rule.did {
                    println!("  DID: {}", did);
                }
                println!("  Conditions: {}", rule.conditions.len());
                for (i, cond) in rule.conditions.iter().enumerate() {
                    println!("    {}. {:?}", i + 1, cond);
                }
                println!();
            }
        }
    }
    
    Ok(())
}

/// Add a new rule
async fn add_rule(
    rules_path: &std::path::Path,
    id: String,
    name: String,
    did: Option<String>,
    action: RuleAction,
    priority: i32,
    ip_cidr: Option<String>,
    time_window: Option<String>,
    days: Option<String>,
    capability: Option<String>,
) -> anyhow::Result<()> {
    let mut engine = load_or_create_rules_engine(rules_path).await?;
    
    // Check if rule ID already exists
    if engine.find_rule(&id).is_some() {
        println!("Error: Rule '{}' already exists", id);
        return Ok(());
    }
    
    let local_did = get_local_did().await?;
    
    let mut rule = AclRule::new(&id, &name, &local_did)
        .with_action(action.into())
        .with_priority(priority);
    
    if let Some(d) = did {
        rule = rule.with_did(d);
    }
    
    // Add conditions
    if let Some(cidr) = ip_cidr {
        rule = rule.with_condition(Condition::IpMatch { cidr });
    }
    
    if let Some(window) = time_window {
        let parts: Vec<&str> = window.split('-').collect();
        if parts.len() == 2 {
            let days_vec = days.map(|d| {
                d.split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect()
            }).unwrap_or_default();
            
            rule = rule.with_condition(Condition::TimeWindow {
                start: parts[0].to_string(),
                end: parts[1].to_string(),
                days: days_vec,
                timezone: "UTC".to_string(),
            });
        }
    }
    
    if let Some(cap) = capability {
        rule = rule.with_condition(Condition::Capability { required: cap });
    }
    
    engine.add_rule(rule);
    save_rules_engine(rules_path, &engine).await?;
    
    println!("✓ Added rule '{}'", id);
    
    Ok(())
}

/// Remove a rule
async fn remove_rule(rules_path: &std::path::Path, id: &str) -> anyhow::Result<()> {
    let mut engine = load_or_create_rules_engine(rules_path).await?;
    
    if engine.remove_rule(id) {
        save_rules_engine(rules_path, &engine).await?;
        println!("✓ Removed rule '{}'", id);
    } else {
        println!("! Rule '{}' not found", id);
    }
    
    Ok(())
}

/// Enable/disable a rule
async fn toggle_rule(rules_path: &std::path::Path, id: &str, enabled: bool) -> anyhow::Result<()> {
    let mut engine = load_or_create_rules_engine(rules_path).await?;
    
    if engine.set_rule_enabled(id, enabled) {
        save_rules_engine(rules_path, &engine).await?;
        let status = if enabled { "enabled" } else { "disabled" };
        println!("✓ Rule '{}' {}", id, status);
    } else {
        println!("! Rule '{}' not found", id);
    }
    
    Ok(())
}

/// Test a rule
async fn test_rule(
    rules_path: &std::path::Path,
    rule_id: Option<String>,
    did: Option<String>,
    ip: Option<String>,
    capability: Option<String>,
) -> anyhow::Result<()> {
    let engine = load_or_create_rules_engine(rules_path).await?;
    
    let mut ctx = RuleContext::new();
    
    if let Some(d) = did {
        ctx = ctx.with_did(d);
    }
    
    if let Some(i) = ip {
        ctx = ctx.with_ip(i.parse()?);
    }
    
    if let Some(c) = capability {
        ctx = ctx.with_capability(c);
    }
    
    if let Some(id) = rule_id {
        // Test specific rule
        if let Some(rule) = engine.find_rule(&id) {
            match rule.evaluate(&ctx) {
                Some(action) => {
                    println!("Rule '{}' matched with action: {}", id, action);
                }
                None => {
                    println!("Rule '{}' did not match", id);
                }
            }
        } else {
            println!("Rule '{}' not found", id);
        }
    } else {
        // Test against all rules
        let action = engine.evaluate(&ctx);
        println!("Evaluation result: {}", action);
        println!("  (No specific rule matched, using default action: {})", engine.default_action);
    }
    
    Ok(())
}

/// Sync ACL from peers
async fn sync_acl(from: Option<String>, broadcast: bool) -> anyhow::Result<()> {
    #[cfg(feature = "p2p")]
    if broadcast {
        println!("→ Broadcasting local ACL to all connected peers...");
        
        let acl_path = get_acl_path().await?;
        let acl = load_or_create_acl(&acl_path).await?;
        
        let acl_data = serde_json::to_vec(&acl)?;
        
        println!("  Broadcasting {} bytes to topic 'acl/update'", acl_data.len());
        
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
                        println!("✓ ACL broadcast complete");
                    }
                    Err(e) => {
                        println!("✗ Failed to broadcast ACL: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️  P2P network not available: {}", e);
                println!("  ACL will be synced when P2P is available");
            }
        }
    }
    
    #[cfg(not(feature = "p2p"))]
    if broadcast {
        println!("⚠️  P2P feature not enabled, cannot broadcast ACL");
        println!("  Build with --features p2p to enable P2P networking");
    }
    
    #[cfg(feature = "p2p")]
    let from_peer = from.clone();
    if let Some(peer) = from_peer {
        println!("→ Syncing ACL from {}...", peer);
        
        println!("  Requesting ACL sync from peer: {}", peer);
        
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
                        println!("✓ ACL sync from {} complete", peer);
                    }
                    Err(e) => {
                        println!("✗ Failed to sync ACL from {}: {}", peer, e);
                    }
                }
            }
            Err(e) => {
                println!("⚠️  P2P network not available: {}", e);
            }
        }
    }
    
    #[cfg(not(feature = "p2p"))]
    if from.is_some() {
        println!("⚠️  P2P feature not enabled, cannot sync ACL from peer");
        println!("  Build with --features p2p to enable P2P networking");
    }
    
    if from.is_none() && !broadcast {
        println!("Error: Either --from or --broadcast must be specified");
        println!("  cis network sync --from <peer-id>    # Sync from specific peer");
        println!("  cis network sync --broadcast         # Broadcast to all peers");
    }
    
    Ok(())
}

/// Show audit log
async fn show_audit(limit: usize, event_type: Option<String>) -> anyhow::Result<()> {
    println!(" Recent Audit Events (last {}) ", limit);
    
    let audit_logger = cis_core::network::AuditLogger::default();
    
    let entries = if let Some(ref evt_type) = event_type {
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

/// Clean up expired entries
async fn cleanup_expired(
    acl_path: &std::path::Path,
    rules_path: &std::path::Path,
) -> anyhow::Result<()> {
    let mut acl = load_or_create_acl(acl_path).await?;
    let before_w = acl.whitelist.len();
    let before_b = acl.blacklist.len();
    
    acl.cleanup_expired();
    acl.save(acl_path)?;
    
    let removed_w = before_w - acl.whitelist.len();
    let removed_b = before_b - acl.blacklist.len();
    
    let mut engine = load_or_create_rules_engine(rules_path).await?;
    let before_r = engine.rules.len();
    engine.cleanup_expired();
    save_rules_engine(rules_path, &engine).await?;
    let removed_r = before_r - engine.rules.len();
    
    println!("✓ Cleanup complete:");
    println!("  Removed {} expired whitelist entries", removed_w);
    println!("  Removed {} expired blacklist entries", removed_b);
    println!("  Removed {} expired rules", removed_r);
    
    Ok(())
}

// ============================================================================
// Helper functions
// ============================================================================

/// Helper: Load or create ACL
async fn load_or_create_acl(acl_path: &std::path::Path) -> anyhow::Result<NetworkAcl> {
    if acl_path.exists() {
        Ok(NetworkAcl::load(acl_path)?)
    } else {
        let local_did = get_local_did().await?;
        let acl = NetworkAcl::new(local_did);
        acl.save(acl_path)?;
        Ok(acl)
    }
}

/// Helper: Load or create rules engine
async fn load_or_create_rules_engine(path: &std::path::Path) -> anyhow::Result<AclRulesEngine> {
    if path.exists() {
        let content = tokio::fs::read_to_string(path).await?;
        let engine: AclRulesEngine = toml::from_str(&content)?;
        Ok(engine)
    } else {
        Ok(AclRulesEngine::new())
    }
}

/// Helper: Save rules engine
async fn save_rules_engine(path: &std::path::Path, engine: &AclRulesEngine) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let content = toml::to_string_pretty(engine)?;
    tokio::fs::write(path, content).await?;
    Ok(())
}

/// Helper: Get local DID
async fn get_local_did() -> anyhow::Result<String> {
    let did_path = cis_core::storage::paths::Paths::config_dir().join("node.did");
    
    if did_path.exists() {
        let content = tokio::fs::read_to_string(&did_path).await?;
        let did = content.trim();
        if cis_core::identity::did::DIDManager::is_valid_did(did) {
            return Ok(did.to_string());
        }
    }
    
    let manager = cis_core::identity::did::DIDManager::generate("local-node")?;
    let did = manager.did().to_string();
    
    tokio::fs::create_dir_all(did_path.parent().unwrap()).await?;
    tokio::fs::write(&did_path, &did).await?;
    
    Ok(did)
}

/// Helper: Get ACL file path
async fn get_acl_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(cis_core::network::default_acl_path())
}

/// Helper: Get rules file path
async fn get_rules_path() -> anyhow::Result<std::path::PathBuf> {
    let config_dir = cis_core::storage::paths::Paths::config_dir();
    Ok(config_dir.join("acl_rules.toml"))
}

/// Helper: Parse duration string
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
fn exp_desc(entry: &AclEntry) -> String {
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

/// Helper: Broadcast ACL update to P2P network
#[cfg(feature = "p2p")]
async fn broadcast_acl_update(acl: &NetworkAcl) {
    println!("  Broadcasting ACL update to peers...");
    
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
            match serde_json::to_vec(acl) {
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
}

/// Helper: Broadcast ACL update to P2P network (stub when p2p disabled)
#[cfg(not(feature = "p2p"))]
async fn broadcast_acl_update(_acl: &NetworkAcl) {
    println!("  ⚠️  P2P feature not enabled, skipping ACL broadcast");
}
