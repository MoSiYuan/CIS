//! # Node Command
//!
//! Docker-style node management commands for CIS federation.

use anyhow::{Context, Result};
use clap::{Subcommand, ValueEnum};
use cis_core::service::{
    node_service::{BindOptions, NodeService, TrustLevel as CoreTrustLevel, NodeStatus},
    ListOptions,
};
use std::collections::HashMap;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Wide,
}

/// Trust level for CLI
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum TrustLevel {
    #[default]
    Full,
    Limited,
    Untrusted,
}

impl From<TrustLevel> for CoreTrustLevel {
    fn from(t: TrustLevel) -> Self {
        match t {
            TrustLevel::Full => CoreTrustLevel::Full,
            TrustLevel::Limited => CoreTrustLevel::Limited,
            TrustLevel::Untrusted => CoreTrustLevel::Untrusted,
        }
    }
}

/// Node subcommands - Docker style
#[derive(Debug, Subcommand)]
pub enum NodeAction {
    /// List known nodes (like `docker node ls`)
    #[command(alias = "list")]
    Ls {
        /// Show all nodes (including offline)
        #[arg(long, short)]
        all: bool,
        
        /// Only display node IDs
        #[arg(long, short)]
        quiet: bool,
        
        /// Filter nodes by condition
        #[arg(long, value_parser = parse_filter)]
        filter: Vec<(String, String)>,
        
        /// Output format (table|json|wide)
        #[arg(long, short, default_value = "table")]
        format: OutputFormat,
    },
    
    /// Bind/connect to a new node
    Bind {
        /// Node endpoint URL
        endpoint: String,
        
        /// Node DID (optional, will be discovered)
        #[arg(long)]
        did: Option<String>,
        
        /// Trust level (full|limited|untrusted)
        #[arg(long, short, default_value = "limited")]
        trust: TrustLevel,
        
        /// Enable auto-sync
        #[arg(long)]
        auto_sync: bool,
    },
    
    /// Display detailed information about a node
    Inspect {
        /// Node ID (or prefix)
        node_id: String,
        
        /// Format output using template syntax
        #[arg(long)]
        format: Option<String>,
    },
    
    /// Disconnect from a node
    Disconnect {
        /// Node ID(s) to disconnect
        node_ids: Vec<String>,
        
        /// Force disconnect without confirmation
        #[arg(long, short)]
        force: bool,
    },
    
    /// Blacklist a node (block all communication)
    Blacklist {
        /// Node ID to blacklist
        node_id: String,
        
        /// Reason for blacklisting
        #[arg(long)]
        reason: Option<String>,
    },
    
    /// Remove a node from blacklist
    Unblacklist {
        /// Node ID to unblacklist
        node_id: String,
    },
    
    /// Ping a node to check connectivity
    Ping {
        /// Node ID to ping
        node_id: String,
        
        /// Number of ping attempts
        #[arg(long, short, default_value = "3")]
        count: u32,
    },
    
    /// Sync data with a node
    Sync {
        /// Node ID to sync with
        node_id: String,
        
        /// Full sync (not just incremental)
        #[arg(long)]
        full: bool,
    },
    
    /// Remove offline nodes
    Prune {
        /// Remove nodes offline for more than N days
        #[arg(long, default_value = "30")]
        max_offline_days: u32,
        
        /// Do not prompt for confirmation
        #[arg(long, short)]
        force: bool,
    },
    
    /// Show node resource usage
    Stats {
        /// Node ID(s) (if not specified, shows all)
        node_ids: Vec<String>,
    },
}

/// Parse filter argument in format "key=value"
fn parse_filter(s: &str) -> Result<(String, String), String> {
    s.find('=')
        .map(|i| (s[..i].to_string(), s[i+1..].to_string()))
        .ok_or_else(|| "Filter must be in format 'key=value'".to_string())
}

/// Handle node commands
pub async fn handle(cmd: NodeAction) -> Result<()> {
    match cmd {
        NodeAction::Ls { all, quiet, filter, format } => {
            list_nodes(all, quiet, filter, format).await
        }
        NodeAction::Bind { endpoint, did, trust, auto_sync } => {
            bind_node(endpoint, did, trust, auto_sync).await
        }
        NodeAction::Inspect { node_id, format } => {
            inspect_node(&node_id, format.as_deref()).await
        }
        NodeAction::Disconnect { node_ids, force } => {
            disconnect_nodes(&node_ids, force).await
        }
        NodeAction::Blacklist { node_id, reason } => {
            blacklist_node(&node_id, reason.as_deref()).await
        }
        NodeAction::Unblacklist { node_id } => {
            unblacklist_node(&node_id).await
        }
        NodeAction::Ping { node_id, count } => {
            ping_node(&node_id, count).await
        }
        NodeAction::Sync { node_id, full } => {
            sync_node(&node_id, full).await
        }
        NodeAction::Prune { max_offline_days, force } => {
            prune_nodes(max_offline_days, force).await
        }
        NodeAction::Stats { node_ids } => {
            show_node_stats(&node_ids).await
        }
    }
}

/// List nodes with Docker-style formatting
async fn list_nodes(
    all: bool,
    quiet: bool,
    filters: Vec<(String, String)>,
    format: OutputFormat,
) -> Result<()> {
    let service = NodeService::new()?;
    
    let options = ListOptions {
        all,
        filters: filters.into_iter().collect(),
        limit: None,
        sort_by: Some("last_seen".to_string()),
        sort_desc: true,
    };
    
    let result = service.list(options).await?;
    
    if quiet {
        for node in &result.items {
            println!("{}", node.id);
        }
        return Ok(());
    }
    
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&result.items)?);
        }
        OutputFormat::Wide => {
            println!("{:<20} {:<12} {:<20} {:<15} {:<30}", 
                "NODE ID", "STATUS", "NAME", "VERSION", "ENDPOINT");
            println!("{}", "-".repeat(100));
            
            for node in &result.items {
                let status_str = format!("{:?}", node.status).to_lowercase();
                
                println!("{:<20} {:<12} {:<20} {:<15} {:<30}",
                    truncate(&node.id, 20),
                    status_str,
                    truncate(&node.name, 20),
                    truncate(&node.version, 15),
                    truncate(&node.endpoint, 30)
                );
            }
            println!("\nTotal: {} nodes", result.total);
        }
        OutputFormat::Table => {
            println!("{:<20} {:<12} {:<20} {:<15} {}", 
                "NODE ID", "STATUS", "NAME", "VERSION", "LAST SEEN");
            println!("{}", "-".repeat(85));
            
            for node in &result.items {
                let status_str = format!("{:?}", node.status).to_lowercase();
                
                let last_seen = chrono::Utc::now().signed_duration_since(node.last_seen);
                let last_seen_str = format_duration_short(last_seen);
                
                println!("{:<20} {:<12} {:<20} {:<15} {}",
                    truncate(&node.id, 20),
                    status_str,
                    truncate(&node.name, 20),
                    truncate(&node.version, 15),
                    last_seen_str
                );
            }
            
            if result.items.is_empty() {
                println!("No nodes found.");
            }
            println!();
            println!("Use 'cis node bind <endpoint>' to add a new node.");
        }
    }
    
    Ok(())
}

/// Bind to a new node
async fn bind_node(
    endpoint: String,
    did: Option<String>,
    trust: TrustLevel,
    auto_sync: bool,
) -> Result<()> {
    println!("Binding to node: {}", endpoint);
    
    let service = NodeService::new()?;
    
    let options = BindOptions {
        endpoint,
        did,
        trust_level: trust.into(),
        auto_sync,
    };
    
    let node = service.bind(options).await?;
    
    println!("Node bound successfully");
    println!();
    println!("Node ID:   {}", node.summary.id);
    println!("DID:       {}", node.summary.did);
    println!("Name:      {}", node.summary.name);
    println!("Status:    {:?}", node.summary.status);
    println!("Trust:     {:?}", trust);
    
    Ok(())
}

/// Inspect node details
async fn inspect_node(node_id: &str, format: Option<&str>) -> Result<()> {
    let service = NodeService::new()?;
    
    let node = service.inspect(node_id).await
        .map_err(|e| anyhow::anyhow!("No such node: {} ({})", node_id, e))?;
    
    // Custom format
    if let Some(fmt) = format {
        match fmt {
            "{{.ID}}" | "{{.Id}}" => println!("{}", node.summary.id),
            "{{.DID}}" => println!("{}", node.summary.did),
            "{{.Status}}" => println!("{:?}", node.summary.status),
            "{{.Endpoint}}" => println!("{}", node.summary.endpoint),
            "{{.Name}}" => println!("{}", node.summary.name),
            "{{.TrustScore}}" => println!("{}", node.trust_score),
            "{{.Blacklisted}}" => println!("{}", node.is_blacklisted),
            _ => println!("Unknown format: {}", fmt),
        }
        return Ok(());
    }
    
    // Full JSON output
    let inspect_data = serde_json::json!({
        "ID": node.summary.id,
        "DID": node.summary.did,
        "Name": node.summary.name,
        "Status": format!("{:?}", node.summary.status),
        "Endpoint": node.summary.endpoint,
        "Version": node.summary.version,
        "PublicKey": node.public_key,
        "TrustScore": node.trust_score,
        "Blacklisted": node.is_blacklisted,
        "Capabilities": node.summary.capabilities,
        "Metadata": node.metadata,
        "LastSeen": node.summary.last_seen,
        "CreatedAt": node.created_at,
    });
    
    println!("{}", serde_json::to_string_pretty(&inspect_data)?);
    Ok(())
}

/// Disconnect from nodes
async fn disconnect_nodes(node_ids: &[String], force: bool) -> Result<()> {
    let service = NodeService::new()?;
    
    for id in node_ids {
        if !force {
            print!("Disconnect from node {}? [y/N] ", id);
            std::io::Write::flush(&mut std::io::stdout())?;
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            
            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Skipped.");
                continue;
            }
        }
        
        match service.disconnect(id).await {
            Ok(()) => println!("Disconnected: {}", id),
            Err(e) => eprintln!("Error disconnecting {}: {}", id, e),
        }
    }
    
    Ok(())
}

/// Blacklist a node
async fn blacklist_node(node_id: &str, reason: Option<&str>) -> Result<()> {
    let service = NodeService::new()?;
    
    println!("Blacklisting node: {}", node_id);
    if let Some(r) = reason {
        println!("   Reason: {}", r);
    }
    
    service.blacklist(node_id, reason).await?;
    
    println!("Node blacklisted successfully");
    Ok(())
}

/// Unblacklist a node
async fn unblacklist_node(node_id: &str) -> Result<()> {
    let service = NodeService::new()?;
    
    println!("Removing {} from blacklist...", node_id);
    
    service.unblacklist(node_id).await?;
    
    println!("Node unblacklisted successfully");
    Ok(())
}

/// Ping a node
async fn ping_node(node_id: &str, count: u32) -> Result<()> {
    let service = NodeService::new()?;
    
    println!("Pinging node: {}", node_id);
    
    let mut success_count = 0;
    for i in 1..=count {
        match service.ping(node_id).await {
            Ok(true) => {
                println!("Attempt {}: OK - Node is online", i);
                success_count += 1;
            }
            Ok(false) => {
                println!("Attempt {}: FAIL - Node is offline", i);
            }
            Err(e) => {
                println!("Attempt {}: ERROR - {}", i, e);
            }
        }
        
        if i < count {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
    
    println!();
    println!("Result: {}/{} successful", success_count, count);
    
    Ok(())
}

/// Sync with a node
async fn sync_node(node_id: &str, full: bool) -> Result<()> {
    let service = NodeService::new()?;
    
    if full {
        println!("Performing full sync with node: {}", node_id);
    } else {
        println!("Performing incremental sync with node: {}", node_id);
    }
    
    service.sync(node_id).await?;
    
    println!("Sync completed");
    Ok(())
}

/// Prune offline nodes
async fn prune_nodes(max_offline_days: u32, force: bool) -> Result<()> {
    let service = NodeService::new()?;
    
    let to_prune = service.prune(max_offline_days).await?;
    
    if to_prune.is_empty() {
        println!("No offline nodes to prune.");
        return Ok(());
    }
    
    println!("The following nodes will be removed:");
    for id in &to_prune {
        println!("  - {}", id);
    }
    println!();
    println!("Total: {} nodes (offline for > {} days)", to_prune.len(), max_offline_days);
    
    if !force {
        print!("\nAre you sure? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout())?;
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }
    
    println!("\nPruned {} nodes.", to_prune.len());
    Ok(())
}

/// Show node stats
async fn show_node_stats(node_ids: &[String]) -> Result<()> {
    let service = NodeService::new()?;
    
    let ids = if node_ids.is_empty() {
        // Get all nodes
        let nodes = service.list(ListOptions::default()).await?;
        nodes.items.iter().map(|n| n.id.clone()).collect::<Vec<_>>()
    } else {
        node_ids.to_vec()
    };
    
    if ids.is_empty() {
        println!("No nodes found.");
        return Ok(());
    }
    
    println!("{:<20} {:<10} {:<12} {:<12} {:<12}",
        "NODE ID", "STATUS", "CPU %", "MEM %", "LATENCY");
    println!("{}", "-".repeat(70));
    
    for id in ids {
        match service.stats(&id).await {
            Ok(stats) => {
                println!("{:<20} {:<10} {:<12.1} {:<12.1} {:<12}",
                    truncate(&id, 20),
                    "online",
                    stats.cpu_percent,
                    stats.memory_percent,
                    "-"
                );
            }
            Err(e) => {
                println!("{:<20} Error: {}", truncate(&id, 20), e);
            }
        }
    }
    
    Ok(())
}

// Helper functions

/// Format duration in short form
fn format_duration_short(duration: chrono::Duration) -> String {
    let seconds = duration.num_seconds();
    
    if seconds < 60 {
        format!("{}s ago", seconds)
    } else if seconds < 3600 {
        format!("{}m ago", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h ago", seconds / 3600)
    } else {
        format!("{}d ago", seconds / 86400)
    }
}

/// Truncate string with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}...", &s[..max_len - 3])
    } else {
        s.to_string()
    }
}
