//! # Session Command
//!
//! Session management commands for CIS.

use crate::cli::{CommandContext, CommandOutput, CommandError};
use colored::Colorize;

/// Session management command
pub struct SessionCommand;

impl SessionCommand {
    /// List all active sessions
    pub async fn execute_list(
        &self,
        ctx: &CommandContext,
        agent_id: Option<i64>,
        status: Option<String>,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose("Querying sessions from database...");

        let status_filter = status.and_then(|s| Self::parse_session_status(&s));

        let session_repo = ctx.get_session_repository()
            .ok_or_else(|| CommandError::custom("Session repository not available".to_string()))?;

        let sessions = if let Some(agent_id) = agent_id {
            session_repo.list_by_agent(agent_id, status_filter).await
                .map_err(|e| CommandError::custom(format!("Failed to list sessions: {}", e)))?
        } else {
            // For now, just return empty if no agent_id specified
            // In production, we'd have a list_all method
            return Ok(CommandOutput::Message("Specify --agent-id to list sessions".to_string()));
        };

        if sessions.is_empty() {
            return Ok(CommandOutput::Message("No sessions found".to_string()));
        }

        Ok(CommandOutput::Table {
            headers: vec![
                "ID".to_string(),
                "Session ID".to_string(),
                "Agent ID".to_string(),
                "Runtime".to_string(),
                "Status".to_string(),
                "Capacity".to_string(),
                "Used".to_string(),
                "Created".to_string(),
                "Expires".to_string(),
            ],
            rows: sessions.iter().map(|s| {
                vec![
                    s.id.to_string(),
                    s.session_id.clone(),
                    s.agent_id.to_string(),
                    s.runtime_type.clone(),
                    Self::format_session_status(&s.status),
                    format!("{}", s.context_capacity),
                    format!("{}", s.context_used),
                    Self::format_timestamp(s.created_at),
                    Self::format_timestamp(s.expires_at),
                ]
            }).collect(),
        })
    }

    /// Create a new session
    pub async fn execute_create(
        &self,
        ctx: &CommandContext,
        agent_type: String,
        runtime: String,
        context_capacity: i64,
        ttl_minutes: i64,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Creating session: {}@{}", agent_type, runtime));

        // First, get or create the agent
        let agent_repo = ctx.get_agent_repository()
            .ok_or_else(|| CommandError::custom("Agent repository not available".to_string()))?;

        let agent = agent_repo.get_by_type(&agent_type).await
            .map_err(|e| CommandError::custom(format!("Failed to get agent: {}", e)))?
            .ok_or_else(|| CommandError::not_found("Agent", &agent_type))?;

        let session_repo = ctx.get_session_repository()
            .ok_or_else(|| CommandError::custom("Session repository not available".to_string()))?;

        let id = session_repo.create(
            agent.id,
            &runtime,
            context_capacity,
            ttl_minutes,
        ).await.map_err(|e| CommandError::custom(format!("Failed to create session: {}", e)))?;

        Ok(CommandOutput::Message(
            format!("{} Session created: {}@{} (ID: {})", "✓".green(), agent_type, runtime, id)
        ))
    }

    /// Release a session
    pub async fn execute_release(
        &self,
        ctx: &CommandContext,
        session_id: i64,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Releasing session: {}", session_id));

        let session_repo = ctx.get_session_repository()
            .ok_or_else(|| CommandError::custom("Session repository not available".to_string()))?;

        session_repo.release_session(session_id).await
            .map_err(|e| CommandError::custom(format!("Failed to release session: {}", e)))?;

        Ok(CommandOutput::Message(
            format!("{} Session {} released", "✓".green(), session_id)
        ))
    }

    /// Show session details
    pub async fn execute_show(
        &self,
        ctx: &CommandContext,
        session_id: i64,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Fetching session details: {}", session_id));

        let session_repo = ctx.get_session_repository()
            .ok_or_else(|| CommandError::custom("Session repository not available".to_string()))?;

        let session = session_repo.get_by_id(session_id).await
            .map_err(|e| CommandError::custom(format!("Failed to get session: {}", e)))?
            .ok_or_else(|| CommandError::not_found("Session", &session_id.to_string()))?;

        let usage_percent = if session.context_capacity > 0 {
            (session.context_used as f64 / session.context_capacity as f64) * 100.0
        } else {
            0.0
        };

        let now = chrono::Utc::now().timestamp();
        let ttl_seconds = session.expires_at - now;
        let ttl_minutes = ttl_seconds / 60;

        let output = format!(
            "{} Session Details\n\
             {}\n\
             \n\
             Session ID: {}\n\
             ID: {}\n\
             Agent ID: {}\n\
             Runtime: {}\n\
             Status: {}\n\
             \n\
             Capacity:\n\
             • Total: {} tokens\n\
             • Used: {} tokens ({:.1}%)\n\
             • Available: {} tokens\n\
             \n\
             Timing:\n\
             • Created: {}\n\
             • Last Used: {}\n\
             • Expires: {} (in {} minutes)\n\
             • TTL: {} minutes",
            "═".repeat(40),
            "═".repeat(40),
            session.session_id.bold(),
            session.id,
            session.agent_id,
            session.runtime_type,
            Self::format_session_status(&session.status),
            session.context_capacity,
            session.context_used,
            usage_percent,
            session.context_capacity - session.context_used,
            Self::format_timestamp(session.created_at),
            session.last_used_at.map_or("-".to_string(), |ts| Self::format_timestamp(ts)),
            Self::format_timestamp(session.expires_at),
            ttl_minutes,
            (session.expires_at - session.created_at) / 60,
        );

        Ok(CommandOutput::Message(output))
    }

    /// Cleanup expired sessions
    pub async fn execute_cleanup(
        &self,
        ctx: &CommandContext,
        older_than_days: Option<i64>,
    ) -> Result<CommandOutput, CommandError> {
        let days = older_than_days.unwrap_or(7);
        ctx.verbose(&format!("Cleaning up sessions older than {} days", days));

        let session_repo = ctx.get_session_repository()
            .ok_or_else(|| CommandError::custom("Session repository not available".to_string()))?;

        // First mark expired sessions
        let marked_count = session_repo.cleanup_expired().await
            .map_err(|e| CommandError::custom(format!("Failed to cleanup sessions: {}", e)))?;

        // Then delete old expired sessions
        let deleted_count = session_repo.delete_expired(days).await
            .map_err(|e| CommandError::custom(format!("Failed to delete sessions: {}", e)))?;

        Ok(CommandOutput::Message(
            format!("{} Cleanup complete: {} marked as expired, {} deleted", "✓".green(), marked_count, deleted_count)
        ))
    }

    /// Acquire an existing session
    pub async fn execute_acquire(
        &self,
        ctx: &CommandContext,
        agent_id: i64,
        min_capacity: i64,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Acquiring session for agent {} with min capacity {}", agent_id, min_capacity));

        let session_repo = ctx.get_session_repository()
            .ok_or_else(|| CommandError::custom("Session repository not available".to_string()))?;

        let session = session_repo.acquire_session(agent_id, min_capacity).await
            .map_err(|e| CommandError::custom(format!("Failed to acquire session: {}", e)))?;

        match session {
            Some(s) => {
                Ok(CommandOutput::Message(
                    format!("{} Session acquired: {} (ID: {})", "✓".green(), s.session_id, s.id)
                ))
            }
            None => {
                Ok(CommandOutput::Message(
                    format!("No suitable session found. Create a new session with `cis session create`")
                ))
            }
        }
    }

    /// Parse session status from string
    fn parse_session_status(s: &str) -> Option<cis_core::task::models::SessionStatus> {
        match s.to_lowercase().as_str() {
            "active" => Some(cis_core::task::models::SessionStatus::Active),
            "idle" => Some(cis_core::task::models::SessionStatus::Idle),
            "expired" => Some(cis_core::task::models::SessionStatus::Expired),
            "released" => Some(cis_core::task::models::SessionStatus::Released),
            _ => None,
        }
    }

    /// Format session status for display
    fn format_session_status(status: &cis_core::task::models::SessionStatus) -> String {
        match status {
            cis_core::task::models::SessionStatus::Active => "Active".green().to_string(),
            cis_core::task::models::SessionStatus::Idle => "Idle".yellow().to_string(),
            cis_core::task::models::SessionStatus::Expired => "Expired".red().to_string(),
            cis_core::task::models::SessionStatus::Released => "Released".dim().to_string(),
        }
    }

    /// Format timestamp for display
    fn format_timestamp(ts: i64) -> String {
        chrono::DateTime::from_timestamp(ts, 0)
            .unwrap_or_default()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_session_status() {
        assert!(matches!(SessionCommand::parse_session_status("active"), Some(cis_core::task::models::SessionStatus::Active)));
        assert!(matches!(SessionCommand::parse_session_status("idle"), Some(cis_core::task::models::SessionStatus::Idle)));
        assert!(matches!(SessionCommand::parse_session_status("expired"), Some(cis_core::task::models::SessionStatus::Expired)));
        assert!(matches!(SessionCommand::parse_session_status("released"), Some(cis_core::task::models::SessionStatus::Released)));
        assert!(SessionCommand::parse_session_status("invalid").is_none());
    }
}
