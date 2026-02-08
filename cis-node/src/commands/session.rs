//! # Session Management CLI Commands
//!
//! Provides commands for managing remote agent sessions:
//! - List sessions
//! - Create new sessions
//! - Attach to existing sessions
//! - Switch between sessions
//! - Kill sessions
//! - Switch agents within sessions
//!
//! ## Usage
//!
//! ```bash
//! cis session ls                    # List sessions
//! cis session new <node> [--agent <type>]  # Create new session
//! cis session attach <id>           # Attach to session
//! cis session switch <id>           # Switch to session
//! cis session kill <id>             # Kill session
//! cis session agent <type>          # Switch agent in current session
//! ```

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::{Subcommand, ValueEnum};
use colored::Colorize;
use tracing::{debug, error, info, warn};

use cis_core::agent::AgentType;
use cis_core::network::agent_session::{SessionManager as AgentSessionManager, SessionState};
use cis_core::storage::paths::Paths;

/// Session commands
#[derive(Subcommand, Debug)]
pub enum SessionCommands {
    /// List all sessions
    Ls {
        /// Show all sessions including closed ones
        #[arg(long)]
        all: bool,
        /// Output format
        #[arg(long, value_enum, default_value = "table")]
        format: OutputFormat,
    },

    /// Create a new session
    New {
        /// Target node DID or alias
        node: String,
        /// Agent type to use
        #[arg(long, value_enum, default_value = "claude")]
        agent: AgentTypeArg,
        /// Project path
        #[arg(long, short)]
        project: Option<PathBuf>,
        /// Terminal columns
        #[arg(long, default_value = "80")]
        cols: u16,
        /// Terminal rows
        #[arg(long, default_value = "24")]
        rows: u16,
    },

    /// Attach to an existing session
    Attach {
        /// Session ID
        id: String,
    },

    /// Switch to a different session
    Switch {
        /// Session ID
        id: String,
    },

    /// Kill (force close) a session
    Kill {
        /// Session ID
        id: String,
        /// Kill without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Switch agent in the current or specified session
    Agent {
        /// Agent type to switch to
        agent: AgentTypeArg,
        /// Session ID (uses active session if not specified)
        #[arg(long, short)]
        session: Option<String>,
    },

    /// Show session info
    Info {
        /// Session ID (uses active session if not specified)
        id: Option<String>,
    },

    /// Resume a disconnected session
    Resume {
        /// Session ID
        id: String,
    },

    /// Clean up old sessions
    Cleanup {
        /// Remove sessions older than N days
        #[arg(long, default_value = "7")]
        days: u32,
        /// Dry run (show what would be removed)
        #[arg(long)]
        dry_run: bool,
    },
}

/// Output format for list command
#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum OutputFormat {
    /// Table format (default)
    Table,
    /// JSON format
    Json,
    /// Plain text
    Plain,
}

/// Agent type argument
#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum AgentTypeArg {
    Claude,
    Kimi,
    Aider,
    Opencode,
}

impl From<AgentTypeArg> for AgentType {
    fn from(arg: AgentTypeArg) -> Self {
        match arg {
            AgentTypeArg::Claude => AgentType::Claude,
            AgentTypeArg::Kimi => AgentType::Kimi,
            AgentTypeArg::Aider => AgentType::Aider,
            AgentTypeArg::Opencode => AgentType::OpenCode,
        }
    }
}

/// Session command arguments
pub struct SessionArgs {
    pub command: SessionCommands,
}

/// Handle session commands
pub async fn handle(args: SessionArgs) -> Result<()> {
    match args.command {
        SessionCommands::Ls { all, format } => list_sessions(all, format).await,
        SessionCommands::New {
            node,
            agent,
            project,
            cols,
            rows,
        } => create_session(node, agent.into(), project, cols, rows).await,
        SessionCommands::Attach { id } => attach_session(&id).await,
        SessionCommands::Switch { id } => switch_session(&id).await,
        SessionCommands::Kill { id, force } => kill_session(&id, force).await,
        SessionCommands::Agent { agent, session } => switch_agent(agent.into(), session).await,
        SessionCommands::Info { id } => show_session_info(id).await,
        SessionCommands::Resume { id } => resume_session(&id).await,
        SessionCommands::Cleanup { days, dry_run } => cleanup_sessions(days, dry_run).await,
    }
}

/// List all sessions
async fn list_sessions(all: bool, format: OutputFormat) -> Result<()> {
    debug!("Listing sessions (all={}, format={:?})", all, format);

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Get sessions
    let sessions = manager.list_sessions().await;

    // Filter if needed
    let sessions: Vec<_> = if all {
        sessions
    } else {
        sessions
            .into_iter()
            .filter(|s| {
                !matches!(s.state, SessionState::Closed)
                    && s.last_activity > chrono::Utc::now() - chrono::Duration::hours(24)
            })
            .collect()
    };

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&sessions)?;
            println!("{}", json);
        }
        OutputFormat::Plain => {
            for session in sessions {
                let state = format_state(session.state);
                let agent = format_agent(session.agent_type);
                println!(
                    "{} {} {} {} {}",
                    session.id[..8].to_string().cyan(),
                    state,
                    agent,
                    session.target_did.dimmed(),
                    session.last_activity.format("%Y-%m-%d %H:%M").to_string().dimmed()
                );
            }
        }
        OutputFormat::Table => {
            print_session_table(&sessions);
        }
    }

    Ok(())
}

/// Print sessions as a table
fn print_session_table(sessions: &[cis_core::network::PersistentSession]) {
    if sessions.is_empty() {
        println!("{}", "No sessions found.".dimmed());
        return;
    }

    // Header
    println!(
        "{:<12} {:<10} {:<12} {:<30} {}",
        "ID".bold(),
        "State".bold(),
        "Agent".bold(),
        "Target".bold(),
        "Last Activity".bold()
    );
    println!("{}", "-".repeat(90).dimmed());

    // Rows
    for session in sessions {
        let id = &session.id[..8.min(session.id.len())];
        let state = format_state(session.state);
        let agent = format_agent(session.agent_type);
        let target = if session.target_did.len() > 28 {
            format!("{}...", &session.target_did[..25])
        } else {
            session.target_did.clone()
        };
        let activity = session.last_activity.format("%Y-%m-%d %H:%M").to_string();

        println!(
            "{:<12} {:<10} {:<12} {:<30} {}",
            id.cyan(),
            state,
            agent,
            target.dimmed(),
            activity.dimmed()
        );
    }

    println!("\n{} session(s) total", sessions.len());
}

/// Format session state with colors
fn format_state(state: SessionState) -> String {
    match state {
        SessionState::Initial => "initial".dimmed().to_string(),
        SessionState::Connecting => "connecting".yellow().to_string(),
        SessionState::Active => "active".green().to_string(),
        SessionState::Closing => "closing".yellow().to_string(),
        SessionState::Closed => "closed".dimmed().to_string(),
    }
}

/// Format agent type with colors
fn format_agent(agent: AgentType) -> String {
    match agent {
        AgentType::Claude => "Claude".blue().to_string(),
        AgentType::Kimi => "Kimi".magenta().to_string(),
        AgentType::Aider => "Aider".cyan().to_string(),
        AgentType::OpenCode => "OpenCode".green().to_string(),
        AgentType::Custom => "Custom".dimmed().to_string(),
    }
}

/// Create a new session
async fn create_session(
    node: String,
    agent: AgentType,
    project: Option<PathBuf>,
    cols: u16,
    rows: u16,
) -> Result<()> {
    info!(
        "Creating new session for node {} with agent {:?}",
        node, agent
    );

    // Validate terminal size
    if cols == 0 || cols > 512 || rows == 0 || rows > 256 {
        return Err(anyhow!("Invalid terminal size: {}x{} (max 512x256)", cols, rows));
    }

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Create session
    let session_id = manager.create_session(&node, agent, project.clone()).await?;

    println!("✓ Created session {}", session_id[..8].to_string().cyan());
    println!("  Target: {}", node.dimmed());
    println!("  Agent: {}", format_agent(agent));
    if let Some(ref path) = project {
        println!("  Project: {}", path.display().to_string().dimmed());
    }
    println!("\nUse {} to attach to this session.",
        format!("cis session attach {}", session_id).yellow()
    );

    Ok(())
}

/// Attach to an existing session
async fn attach_session(id: &str) -> Result<()> {
    info!("Attaching to session {}", id);

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Resolve session ID (allow short IDs)
    let session_id = resolve_session_id(&manager, id).await?;

    // Check if session exists
    let session = manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| anyhow!("Session {} not found", id))?;

    if session.metadata.state == SessionState::Closed {
        return Err(anyhow!(
            "Session {} is closed. Use 'cis session resume {}' to resume it.",
            id, id
        ));
    }

    // Set as active session
    manager.switch_session(&session_id).await?;

    println!("✓ Attached to session {}", session_id[..8].to_string().cyan());
    println!("  Target: {}", session.metadata.target_did.dimmed());
    println!("  Agent: {}", format_agent(session.metadata.agent_type));
    println!("  Duration: {}s", session.duration().as_secs());

    // TODO: Start interactive PTY session here
    println!("\n{}", "Interactive mode not yet implemented.".yellow());

    Ok(())
}

/// Switch to a different session
async fn switch_session(id: &str) -> Result<()> {
    info!("Switching to session {}", id);

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Resolve session ID
    let session_id = resolve_session_id(&manager, id).await?;

    // Switch to session
    manager.switch_session(&session_id).await?;

    let session = manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| anyhow!("Session {} not found", id))?;

    println!("✓ Switched to session {}", session_id[..8].to_string().cyan());
    println!("  Agent: {}", format_agent(session.metadata.agent_type));

    Ok(())
}

/// Kill a session
async fn kill_session(id: &str, force: bool) -> Result<()> {
    info!("Killing session {}", id);

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Resolve session ID
    let session_id = resolve_session_id(&manager, id).await?;

    // Get session info
    let session = manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| anyhow!("Session {} not found", id))?;

    // Confirm unless force
    if !force {
        println!("You are about to kill session:",);
        println!("  ID: {}", session_id[..8].to_string().cyan());
        println!("  Target: {}", session.metadata.target_did);
        println!("  Agent: {}", format_agent(session.metadata.agent_type));
        println!("\nThis will terminate the session and any running processes.");
        print!("Are you sure? [y/N] ");
        use std::io::Write;
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Kill session
    manager.kill_session(&session_id).await?;

    println!("✓ Session {} killed", session_id[..8].to_string().cyan());

    Ok(())
}

/// Switch agent in a session
async fn switch_agent(agent: AgentType, session_id: Option<String>) -> Result<()> {
    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Get session ID
    let session_id = if let Some(id) = session_id {
        resolve_session_id(&manager, &id).await?
    } else {
        manager
            .get_active_session()
            .await
            .ok_or_else(|| anyhow!("No active session. Use --session to specify a session."))?
    };

    // Get current agent for display
    let session = manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

    let old_agent = session.metadata.agent_type;

    // Switch agent
    manager.switch_agent(&session_id, agent).await?;

    println!(
        "✓ Switched agent in session {} from {} to {}",
        session_id[..8].to_string().cyan(),
        format_agent(old_agent),
        format_agent(agent)
    );

    Ok(())
}

/// Show session info
async fn show_session_info(id: Option<String>) -> Result<()> {
    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Get session ID
    let session_id = if let Some(id) = id {
        resolve_session_id(&manager, &id).await?
    } else {
        manager
            .get_active_session()
            .await
            .ok_or_else(|| anyhow!("No active session. Specify a session ID or attach to one."))?
    };

    // Get session
    let session = manager
        .get_session(&session_id)
        .await
        .ok_or_else(|| anyhow!("Session {} not found", session_id))?;

    // Get stats
    let stats = manager.get_session_stats(&session_id).await?;

    println!("{}", "Session Information".bold().underline());
    println!();
    println!("  ID:          {}", session.metadata.id.cyan());
    println!("  Target DID:  {}", session.metadata.target_did);
    println!("  Agent:       {}", format_agent(session.metadata.agent_type));
    println!("  State:       {}", format_state(session.metadata.state));
    println!(
        "  Project:     {}",
        session
            .metadata
            .project_path
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "None".to_string())
    );
    println!(
        "  Terminal:    {}x{}",
        session.metadata.terminal_size.0, session.metadata.terminal_size.1
    );
    println!(
        "  Created:     {}",
        session.metadata.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!(
        "  Last Active: {}",
        session.metadata.last_activity.format("%Y-%m-%d %H:%M:%S")
    );
    println!();
    println!("{}", "Statistics".bold().underline());
    println!();
    println!("  Duration:      {}s", stats.duration_secs);
    println!("  Idle:          {}s", stats.idle_secs);
    println!("  Agent switches: {}", stats.agent_switches);
    let is_active = stats.state == cis_core::network::agent_session::SessionState::Active;
    println!("  Active:        {}", if is_active { "Yes".green() } else { "No".dimmed() });

    if !session.agent_history.is_empty() {
        println!();
        println!("{}", "Agent History".bold().underline());
        println!();
        for (time, agent) in &session.agent_history {
            println!(
                "  {} - {}",
                time.format("%H:%M:%S").to_string().dimmed(),
                format_agent(*agent)
            );
        }
    }

    Ok(())
}

/// Resume a disconnected session
async fn resume_session(id: &str) -> Result<()> {
    info!("Resuming session {}", id);

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Resolve session ID
    let session_id = resolve_session_id(&manager, id).await?;

    // Resume session
    let session = manager.resume_session(&session_id).await?;

    println!("✓ Resumed session {}", session_id[..8].to_string().cyan());
    println!("  Target: {}", session.metadata.target_did.dimmed());
    println!("  Agent: {}", format_agent(session.metadata.agent_type));
    println!("\nUse {} to attach to this session.",
        format!("cis session attach {}", session_id).yellow()
    );

    Ok(())
}

/// Clean up old sessions
async fn cleanup_sessions(days: u32, dry_run: bool) -> Result<()> {
    info!("Cleaning up sessions older than {} days", days);

    // Get database path
    let db_path = Paths::data_dir().join("sessions.db");

    // Create session manager
    let manager = cis_core::network::EnhancedSessionManager::new(&db_path)?;

    // Get all sessions
    let sessions = manager.list_sessions().await;

    let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);

    let to_cleanup: Vec<_> = sessions
        .into_iter()
        .filter(|s| s.last_activity < cutoff && matches!(s.state, SessionState::Closed))
        .collect();

    if to_cleanup.is_empty() {
        println!("No old sessions to clean up.");
        return Ok(());
    }

    println!(
        "Found {} session(s) older than {} days",
        to_cleanup.len().to_string().yellow(),
        days
    );

    if dry_run {
        println!("\n{} (no changes made)", "Dry run".yellow().bold());
        for session in &to_cleanup {
            println!(
                "  Would remove: {} (last active: {})",
                session.id[..8].to_string().cyan(),
                session.last_activity.format("%Y-%m-%d").to_string().dimmed()
            );
        }
    } else {
        for session in &to_cleanup {
            // Delete from database
            let store = manager.store.read().await;
            if let Err(e) = store.delete_session(&session.id) {
                warn!("Failed to delete session {}: {}", session.id, e);
            } else {
                println!(
                    "  Removed: {}",
                    session.id[..8].to_string().cyan()
                );
            }
        }
        println!("\n✓ Cleaned up {} session(s)", to_cleanup.len());
    }

    Ok(())
}

/// Resolve a session ID (allows partial matches)
async fn resolve_session_id(
    manager: &cis_core::network::EnhancedSessionManager,
    id: &str,
) -> Result<String> {
    // If it's a full UUID, return as-is
    if id.len() == 36 {
        return Ok(id.to_string());
    }

    // Try to find matching session
    let sessions = manager.list_sessions().await;

    let matches: Vec<_> = sessions
        .into_iter()
        .filter(|s| s.id.starts_with(id))
        .collect();

    match matches.len() {
        0 => Err(anyhow!("No session found matching '{}'", id)),
        1 => Ok(matches[0].id.clone()),
        _ => Err(anyhow!(
            "Multiple sessions match '{}'. Please provide more characters.",
            id
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_conversion() {
        assert_eq!(AgentType::from(AgentTypeArg::Claude), AgentType::Claude);
        assert_eq!(AgentType::from(AgentTypeArg::Kimi), AgentType::Kimi);
        assert_eq!(AgentType::from(AgentTypeArg::Aider), AgentType::Aider);
        assert_eq!(AgentType::from(AgentTypeArg::Opencode), AgentType::OpenCode);
    }

    #[test]
    fn test_format_state() {
        use colored::Colorize;
        
        let s = format_state(SessionState::Active);
        assert!(s.contains("active"));
        
        let s = format_state(SessionState::Closed);
        assert!(s.contains("closed"));
    }

    #[test]
    fn test_format_agent() {
        let s = format_agent(AgentType::Claude);
        assert!(s.contains("Claude"));
        
        let s = format_agent(AgentType::Kimi);
        assert!(s.contains("Kimi"));
    }
}
