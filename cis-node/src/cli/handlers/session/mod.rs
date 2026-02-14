//! # Session Command Handlers
//!
//! Handler functions for session commands.

use crate::cli::{CommandContext, CommandOutput, CommandError};

/// Execute session list command
pub async fn execute_list(
    ctx: &CommandContext,
    agent_id: Option<i64>,
    status: Option<String>,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::SessionCommand;
    let cmd = SessionCommand;
    cmd.execute_list(ctx, agent_id, status).await
}

/// Execute session create command
pub async fn execute_create(
    ctx: &CommandContext,
    agent_type: String,
    runtime: String,
    context_capacity: i64,
    ttl_minutes: i64,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::SessionCommand;
    let cmd = SessionCommand;
    cmd.execute_create(ctx, agent_type, runtime, context_capacity, ttl_minutes).await
}

/// Execute session show command
pub async fn execute_show(
    ctx: &CommandContext,
    session_id: i64,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::SessionCommand;
    let cmd = SessionCommand;
    cmd.execute_show(ctx, session_id).await
}

/// Execute session acquire command
pub async fn execute_acquire(
    ctx: &CommandContext,
    agent_id: i64,
    min_capacity: i64,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::SessionCommand;
    let cmd = SessionCommand;
    cmd.execute_acquire(ctx, agent_id, min_capacity).await
}

/// Execute session release command
pub async fn execute_release(
    ctx: &CommandContext,
    session_id: i64,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::SessionCommand;
    let cmd = SessionCommand;
    cmd.execute_release(ctx, session_id).await
}

/// Execute session cleanup command
pub async fn execute_cleanup(
    ctx: &CommandContext,
    older_than_days: Option<i64>,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::SessionCommand;
    let cmd = SessionCommand;
    cmd.execute_cleanup(ctx, older_than_days).await
}
