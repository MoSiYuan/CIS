//! # Engine Command Group
//!
//! Engine code scanning and injection commands.

use clap::{Parser, Subcommand};
use crate::cli::command::{Command, CommandCategory, CommandContext, CommandOutput, CommandError, Example};

/// Engine commands - scan and inject code
#[derive(Parser, Debug)]
pub struct EngineGroup {
    #[command(subcommand)]
    pub action: EngineAction,
}

impl EngineGroup {
    /// Get all examples for engine commands
    pub fn examples() -> Vec<Example> {
        vec![
            Example {
                command: "cis engine scan ./my-game".to_string(),
                description: "Scan directory for engine code".to_string(),
            },
            Example {
                command: "cis engine scan ./my-game --engine unreal --output scan-results.json".to_string(),
                description: "Scan with specific engine and output".to_string(),
            },
            Example {
                command: "cis engine report scan-results.json --format markdown".to_string(),
                description: "Generate injection report".to_string(),
            },
            Example {
                command: "cis engine list-engines".to_string(),
                description: "List supported engine types".to_string(),
            },
        ]
    }
}

/// Engine command actions
#[derive(Subcommand, Debug)]
pub enum EngineAction {
    /// Scan directory for engine code
    Scan {
        /// Directory to scan
        directory: String,

        /// Engine type (auto-detected if not specified)
        #[arg(long)]
        #[arg(value_parser = ["unreal", "unity", "godot", "unreal5.7", "unreal5.6", "unity2022", "unity2021", "godot4", "godot3"])]
        engine_type: Option<String>,

        /// Output file (JSON format)
        #[arg(long, short)]
        output: Option<String>,
    },

    /// Generate injection report
    Report {
        /// Scan result file (JSON)
        scan_result: String,

        /// Output format
        #[arg(long, default_value = "markdown")]
        #[arg(value_parser = ["markdown", "json", "csv"])]
        format: String,
    },

    /// List supported engine types
    ListEngines,
}

impl Command for EngineAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Scan { .. } => "scan",
            Self::Report { .. } => "report",
            Self::ListEngines => "list-engines",
        }
    }

    fn about(&self) -> &'static str {
        match self {
            Self::Scan { .. } => "Scan directory for engine code",
            Self::Report { .. } => "Generate injection report",
            Self::ListEngines => "List supported engine types",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        use crate::cli::commands::EngineCommand;

        // Create runtime for async operations
        let runtime = ctx.runtime();

        match self {
            Self::Scan { directory, engine_type, output } => {
                let cmd = EngineCommand;
                runtime.block_on(cmd.execute_scan(ctx, directory.clone(), engine_type.clone(), output.clone()))
            }
            Self::Report { scan_result, format } => {
                let cmd = EngineCommand;
                runtime.block_on(cmd.execute_report(ctx, scan_result.clone(), format.clone()))
            }
            Self::ListEngines => {
                let cmd = EngineCommand;
                runtime.block_on(cmd.execute_list_engines())
            }
        }
    }

    fn examples(&self) -> Vec<Example> {
        EngineGroup::examples()
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Advanced
    }

    fn requires_init(&self) -> bool {
        false
    }
}
