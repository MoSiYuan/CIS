//! # Task Command Group
//!
//! Task management commands.

use clap::{Parser, Subcommand};
use crate::cli::command::{Command, CommandCategory, CommandContext, CommandOutput, CommandError, Example};

/// Task commands - manage tasks
#[derive(Parser, Debug)]
pub struct TaskGroup {
    #[command(subcommand)]
    pub action: TaskAction,
}

impl TaskGroup {
    /// Get all examples for task commands
    pub fn examples() -> Vec<Example> {
        vec![
            Example {
                command: "cis task list".to_string(),
                description: "List all tasks".to_string(),
            },
            Example {
                command: "cis task list --status pending --priority P0".to_string(),
                description: "List pending P0 tasks".to_string(),
            },
            Example {
                command: "cis task create TASK-001 \"Refactor CLI\" --type refactoring --priority P0".to_string(),
                description: "Create a new task".to_string(),
            },
            Example {
                command: "cis task update TASK-001 --status running".to_string(),
                description: "Update task status".to_string(),
            },
            Example {
                command: "cis task show TASK-001".to_string(),
                description: "Show task details".to_string(),
            },
            Example {
                command: "cis task delete TASK-001".to_string(),
                description: "Delete a task".to_string(),
            },
        ]
    }
}

/// Task command actions
#[derive(Subcommand, Debug)]
pub enum TaskAction {
    /// List all tasks
    List {
        /// Filter by status (pending, running, completed, failed)
        #[arg(long, short)]
        status: Option<String>,

        /// Filter by task type
        #[arg(long, short)]
        #[arg(value_parser = ["refactoring", "injection", "optimization", "review", "test", "documentation"])]
        task_type: Option<String>,

        /// Filter by priority (P0, P1, P2, P3)
        #[arg(long, short)]
        priority: Option<String>,

        /// Limit number of results
        #[arg(long, short)]
        limit: Option<usize>,
    },

    /// Create a new task
    Create {
        /// Task ID (unique identifier)
        task_id: String,

        /// Task name
        name: String,

        /// Task type
        #[arg(long)]
        #[arg(value_parser = ["refactoring", "injection", "optimization", "review", "test", "documentation"])]
        task_type: String,

        /// Task priority
        #[arg(long)]
        #[arg(value_parser = ["P0", "P1", "P2", "P3", "critical", "high", "medium", "low"])]
        priority: String,

        /// Task dependencies (task IDs)
        #[arg(long, value_delimiter = ',')]
        dependencies: Vec<String>,

        /// Prompt template
        #[arg(long)]
        prompt_template: Option<String>,

        /// Task description
        #[arg(long, short)]
        description: Option<String>,
    },

    /// Update task status
    Update {
        /// Task ID or numeric ID
        task_id: String,

        /// New status
        #[arg(long)]
        #[arg(value_parser = ["pending", "assigned", "running", "completed", "failed"])]
        status: String,

        /// Error message (if status is failed)
        #[arg(long)]
        error_message: Option<String>,
    },

    /// Show task details
    Show {
        /// Task ID or numeric ID
        task_id: String,
    },

    /// Delete a task
    Delete {
        /// Task ID or numeric ID
        task_id: String,
    },
}

impl Command for TaskAction {
    fn name(&self) -> &'static str {
        match self {
            Self::List { .. } => "list",
            Self::Create { .. } => "create",
            Self::Update { .. } => "update",
            Self::Show { .. } => "show",
            Self::Delete { .. } => "delete",
        }
    }

    fn about(&self) -> &'static str {
        match self {
            Self::List { .. } => "List all tasks",
            Self::Create { .. } => "Create a new task",
            Self::Update { .. } => "Update task status",
            Self::Show { .. } => "Show task details",
            Self::Delete { .. } => "Delete a task",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        use crate::cli::commands::TaskCommand;

        // Create runtime for async operations
        let runtime = ctx.runtime();

        match self {
            Self::List { status, task_type, priority, limit } => {
                let cmd = TaskCommand;
                runtime.block_on(cmd.execute_list(ctx, status.clone(), task_type.clone(), priority.clone(), *limit))
            }
            Self::Create { task_id, name, task_type, priority, dependencies, prompt_template, description } => {
                let cmd = TaskCommand;
                runtime.block_on(cmd.execute_create(
                    ctx,
                    task_id.clone(),
                    name.clone(),
                    task_type.clone(),
                    priority.clone(),
                    dependencies.clone(),
                    prompt_template.clone(),
                    description.clone(),
                ))
            }
            Self::Update { task_id, status, error_message } => {
                let cmd = TaskCommand;
                runtime.block_on(cmd.execute_update(ctx, task_id.clone(), status.clone(), error_message.clone()))
            }
            Self::Show { task_id } => {
                let cmd = TaskCommand;
                runtime.block_on(cmd.execute_show(ctx, task_id.clone()))
            }
            Self::Delete { task_id } => {
                let cmd = TaskCommand;
                runtime.block_on(cmd.execute_delete(ctx, task_id.clone()))
            }
        }
    }

    fn examples(&self) -> Vec<Example> {
        TaskGroup::examples()
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Workflow
    }

    fn requires_init(&self) -> bool {
        true
    }
}
