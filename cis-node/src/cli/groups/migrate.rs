//! # Migrate Command Group
//!
//! Data migration commands.

use clap::{Parser, Subcommand};
use crate::cli::command::{Command, CommandCategory, CommandContext, CommandOutput, CommandError, Example};

/// Migrate commands - data migration
#[derive(Parser, Debug)]
pub struct MigrateGroup {
    #[command(subcommand)]
    pub action: MigrateAction,
}

impl MigrateGroup {
    /// Get all examples for migrate commands
    pub fn examples() -> Vec<Example> {
        vec![
            Example {
                command: "cis migrate run docs/plan/v1.1.6/TASKS_DEFINITIONS.toml".to_string(),
                description: "Migrate tasks from TOML file to SQLite".to_string(),
            },
            Example {
                command: "cis migrate run docs/plan/v1.1.6/ --verify".to_string(),
                description: "Migrate all TOML files in directory and verify".to_string(),
            },
            Example {
                command: "cis migrate rollback --before 1678886400".to_string(),
                description: "Rollback migrations before timestamp".to_string(),
            },
        ]
    }
}

/// Migrate command actions
#[derive(Subcommand, Debug)]
pub enum MigrateAction {
    /// Run migration from TOML to SQLite
    Run {
        /// Source TOML file or directory
        source: String,

        /// Database file path (default: ~/.cis/data/tasks.db)
        #[arg(long)]
        database: Option<String>,

        /// Verify migration after completion
        #[arg(long)]
        verify: bool,
    },

    /// Verify migration results
    Verify {
        /// Database file path (default: ~/.cis/data/tasks.db)
        #[arg(long)]
        database: Option<String>,
    },

    /// Rollback migration
    Rollback {
        /// Rollback migrations created before this timestamp (Unix timestamp)
        #[arg(long)]
        before: Option<i64>,

        /// Database file path (default: ~/.cis/data/tasks.db)
        #[arg(long)]
        database: Option<String>,
    },
}

impl Command for MigrateGroup {
    fn execute(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        use crate::cli::commands::migrate as migrate_cmd;

        match &self.action {
            MigrateAction::Run { source, database, verify } => {
                migrate_cmd::execute(
                    source.clone(),
                    database.clone(),
                    *verify,
                    false,
                    ctx,
                ).map_err(|e| CommandError::new(format!("迁移失败: {}", e))
                    .with_suggestion("检查 TOML 文件格式是否正确")
                    .with_suggestion("确保数据库路径有效且有写权限"))?;

                Ok(CommandOutput::Message("迁移完成".to_string()))
            }

            MigrateAction::Verify { database } => {
                // TODO: Implement verify-only command
                Ok(CommandOutput::Message("验证命令开发中...".to_string()))
            }

            MigrateAction::Rollback { before, database } => {
                if before.is_none() {
                    return Err(CommandError::new("缺少必需参数: --before")
                        .with_suggestion("提供 Unix 时间戳，例如: --before 1678886400"));
                }

                migrate_cmd::execute(
                    String::new(),
                    database.clone(),
                    false,
                    true,
                    ctx,
                ).map_err(|e| CommandError::new(format!("回滚失败: {}", e)))?;

                Ok(CommandOutput::Message("回滚完成".to_string()))
            }
        }
    }

    fn category() -> CommandCategory {
        CommandCategory::Data
    }

    fn name() -> &'static str {
        "migrate"
    }
}
