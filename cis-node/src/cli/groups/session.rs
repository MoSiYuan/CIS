//! # Session Command Group
//!
//! Session management commands.

use clap::{Parser, Subcommand};
use crate::cli::command::{Command, CommandCategory, CommandContext, CommandOutput, CommandError, Example};

/// Session commands - manage agent sessions
#[derive(Parser, Debug)]
pub struct SessionGroup {
    #[command(subcommand)]
    pub action: SessionAction,
}

impl SessionGroup {
    /// Get all examples for session commands
    pub fn examples() -> Vec<Example> {
        vec![
            Example {
                command: "cis session list".to_string(),
                description: "List all sessions".to_string(),
            },
            Example {
                command: "cis session list --agent-id 1 --status active".to_string(),
                description: "List active sessions for agent".to_string(),
            },
            Example {
                command: "cis session create claude claude --capacity 100000 --ttl 60".to_string(),
                description: "Create a new session".to_string(),
            },
            Example {
                command: "cis session show 1".to_string(),
                description: "Show session details".to_string(),
            },
            Example {
                command: "cis session acquire 1 --min-capacity 50000".to_string(),
                description: "Acquire an existing session".to_string(),
            },
            Example {
                command: "cis session release 1".to_string(),
                description: "Release a session".to_string(),
            },
            Example {
                command: "cis session cleanup --older-than 7".to_string(),
                description: "Cleanup expired sessions".to_string(),
            },
        ]
    }
}

/// Session command actions
#[derive(Subcommand, Debug)]
pub enum SessionAction {
    /// List all sessions
    List {
        /// Filter by agent ID
        #[arg(long, short)]
        agent_id: Option<i64>,

        /// Filter by status (active, idle, expired, released)
        #[arg(long, short)]
        #[arg(value_parser = ["active", "idle", "expired", "released"])]
        status: Option<String>,
    },

    /// Create a new session
    Create {
        /// Agent type
        agent_type: String,

        /// Runtime type
        runtime: String,

        /// Context capacity (in tokens)
        #[arg(long, short)]
        capacity: i64,

        /// Time-to-live (in minutes)
        #[arg(long, short)]
        ttl_minutes: i64,
    },

    /// Show session details
    Show {
        /// Session ID
        session_id: i64,
    },

    /// Acquire an existing session
    Acquire {
        /// Agent ID
        #[arg(long, short)]
        agent_id: i64,

        /// Minimum capacity required
        #[arg(long)]
        min_capacity: i64,
    },

    /// Release a session
    Release {
        /// Session ID
        session_id: i64,
    },

    /// Cleanup expired sessions
    Cleanup {
        /// Delete sessions older than N days
        #[arg(long, short)]
        older_than_days: Option<i64>,
    },
}

impl Command for SessionAction {
    fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "list",
            Self::Create { .. } => "create",
            Self::Show { .. } => "show",
            Self::Acquire { .. } => "acquire",
            Self::Release { .. } => "release",
            Self::Cleanup { .. } => "cleanup",
        }
    }

    fn about(&self) -> &'static str {
        match self {
            Self::List { .. } => "List all sessions",
            Self::Create { .. } => "Create a new session",
            Self::Show { .. } => "Show session details",
            Self::Acquire { .. } => "Acquire an existing session",
            Self::Release { .. } => "Release a session",
            Self::Cleanup { .. } => "Cleanup expired sessions",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        use crate::cli::commands::SessionCommand;

        // Create runtime for async operations
        let runtime = ctx.runtime();

        match self {
            Self::List { agent_id, status } => {
                let cmd = SessionCommand;
                runtime.block_on(cmd.execute_list(ctx, *agent_id, status.clone()))
            }
            Self::Create { agent_type, runtime, capacity, ttl_minutes } => {
                let cmd = SessionCommand;
                runtime.block_on(cmd.execute_create(
                    ctx,
                    agent_type.clone(),
                    runtime.clone(),
                    *capacity,
                    *ttl_minutes,
                ))
            }
            Self::Show { session_id } => {
                let cmd = SessionCommand;
                runtime.block_on(cmd.execute_show(ctx, *session_id))
            }
            Self::Acquire { agent_id, min_capacity } => {
                let cmd = SessionCommand;
                runtime.block_on(cmd.execute_acquire(ctx, *agent_id, *min_capacity))
            }
            Self::Release { session_id } => {
                let cmd = SessionCommand;
                runtime.block_on(cmd.execute_release(ctx, *session_id))
            }
            Self::Cleanup { older_than_days } => {
                let cmd = SessionCommand;
                runtime.block_on(cmd.execute_cleanup(ctx, *older_than_days))
            }
        }
    }

    fn examples(&self) -> Vec<Example> {
        SessionGroup::examples()
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::System
    }

    fn requires_init(&self) -> bool {
        true
    }
}
