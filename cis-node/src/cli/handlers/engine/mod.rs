//! # Engine Command Handlers
//!
//! Handler functions for engine commands.

use crate::cli::{CommandContext, CommandOutput, CommandError};

/// Execute engine scan command
pub async fn execute_scan(
    ctx: &CommandContext,
    directory: String,
    engine_type: Option<String>,
    output_file: Option<String>,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::EngineCommand;
    let cmd = EngineCommand;
    cmd.execute_scan(ctx, directory, engine_type, output_file).await
}

/// Execute engine report command
pub async fn execute_report(
    ctx: &CommandContext,
    scan_result: String,
    output_format: String,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::EngineCommand;
    let cmd = EngineCommand;
    cmd.execute_report(ctx, scan_result, output_format).await
}

/// Execute engine list-engines command
pub async fn execute_list_engines(
    ctx: &CommandContext,
) -> Result<CommandOutput, CommandError> {
    use crate::cli::commands::EngineCommand;
    let cmd = EngineCommand;
    cmd.execute_list_engines().await
}
