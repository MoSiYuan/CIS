//! # Task Command Handlers
//!
//! Handler functions for task commands.

use crate::cli::{CommandContext, CommandOutput, CommandError};

/// Execute task list command
pub async fn execute_list(
    ctx: &CommandContext,
    status: Option<String>,
    task_type: Option<String>,
    priority: Option<String>,
    limit: Option<usize>,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::TaskCommand;
    let cmd = TaskCommand;
    cmd.execute_list(ctx, status, task_type, priority, limit).await
}

/// Execute task create command
pub async fn execute_create(
    ctx: &CommandContext,
    task_id: String,
    name: String,
    task_type: String,
    priority: String,
    dependencies: Vec<String>,
    prompt_template: Option<String>,
    description: Option<String>,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::TaskCommand;
    let cmd = TaskCommand;
    cmd.execute_create(ctx, task_id, name, task_type, priority, dependencies, prompt_template, description).await
}

/// Execute task update command
pub async fn execute_update(
    ctx: &CommandContext,
    task_id: String,
    status: String,
    error_message: Option<String>,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::TaskCommand;
    let cmd = TaskCommand;
    cmd.execute_update(ctx, task_id, status, error_message).await
}

/// Execute task show command
pub async fn execute_show(
    ctx: &CommandContext,
    task_id: String,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::TaskCommand;
    let cmd = TaskCommand;
    cmd.execute_show(ctx, task_id).await
}

/// Execute task delete command
pub async fn execute_delete(
    ctx: &CommandContext,
    task_id: String,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::TaskCommand;
    let cmd = TaskCommand;
    cmd.execute_delete(ctx, task_id).await
}
