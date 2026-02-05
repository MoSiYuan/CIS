//! # Task Level Commands (Four-Tier Decision Mechanism)
//!
//! Commands for managing task execution levels:
//! - `cis task-level mechanical <task-id>` - Set task to auto-execute
//! - `cis task-level recommended <task-id>` - Set task to countdown execution
//! - `cis task-level confirmed <task-id>` - Set task to modal confirmation
//! - `cis task-level arbitrated <task-id>` - Set task to pause for arbitration

use anyhow::Result;
use clap::Subcommand;
use cis_core::types::{Action, Task, TaskLevel};

use crate::commands::task::TaskStore;

/// Task level management commands (four-tier decision)
#[derive(Debug, Subcommand)]
pub enum TaskLevelCommands {
    /// Set task to Mechanical level (auto-execute)
    Mechanical {
        /// Task ID to set level for
        task_id: String,
        /// Number of retries on failure
        #[arg(short, long, default_value = "3")]
        retry: u8,
    },

    /// Set task to Recommended level (countdown execution)
    Recommended {
        /// Task ID to set level for
        task_id: String,
        /// Timeout in seconds before default action is taken
        #[arg(short, long, default_value = "30")]
        timeout: u16,
        /// Default action: execute, skip, or abort
        #[arg(short, long, default_value = "execute")]
        default_action: DefaultAction,
    },

    /// Set task to Confirmed level (modal confirmation required)
    Confirmed {
        /// Task ID to set level for
        task_id: String,
    },

    /// Set task to Arbitrated level (pause for human arbitration)
    Arbitrated {
        /// Task ID to set level for
        task_id: String,
        /// Stakeholders to notify for arbitration
        #[arg(short, long, value_delimiter = ',')]
        stakeholders: Vec<String>,
    },
}

/// Default action for Recommended level
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DefaultAction {
    Execute,
    Skip,
    Abort,
}

impl From<DefaultAction> for Action {
    fn from(action: DefaultAction) -> Self {
        match action {
            DefaultAction::Execute => Action::Execute,
            DefaultAction::Skip => Action::Skip,
            DefaultAction::Abort => Action::Abort,
        }
    }
}

/// Handle task level commands
pub async fn handle(cmd: TaskLevelCommands) -> Result<()> {
    match cmd {
        TaskLevelCommands::Mechanical { task_id, retry } => {
            set_mechanical(&task_id, retry).await?;
        }
        TaskLevelCommands::Recommended {
            task_id,
            timeout,
            default_action,
        } => {
            set_recommended(&task_id, timeout, default_action.into()).await?;
        }
        TaskLevelCommands::Confirmed { task_id } => {
            set_confirmed(&task_id).await?;
        }
        TaskLevelCommands::Arbitrated {
            task_id,
            stakeholders,
        } => {
            set_arbitrated(&task_id, stakeholders).await?;
        }
    }

    Ok(())
}

/// Set task to Mechanical level (auto-execute)
pub async fn set_mechanical(task_id: &str, retry: u8) -> Result<()> {
    let mut store = TaskStore::load()?;

    let task = match store.get(task_id) {
        Some(t) => t,
        None => {
            println!("Task not found: {}", task_id);
            return Ok(());
        }
    };

    let mut updated_task = task.clone();
    updated_task.level = TaskLevel::Mechanical { retry };

    // Update task in store
    update_task_in_store(&mut store, updated_task)?;

    println!("✓ Task {} set to Mechanical level", task_id);
    println!("  Auto-execute with {} retries on failure", retry);

    Ok(())
}

/// Set task to Recommended level (countdown execution)
pub async fn set_recommended(
    task_id: &str,
    timeout: u16,
    default_action: Action,
) -> Result<()> {
    let mut store = TaskStore::load()?;

    let task = match store.get(task_id) {
        Some(t) => t,
        None => {
            println!("Task not found: {}", task_id);
            return Ok(());
        }
    };

    let mut updated_task = task.clone();
    updated_task.level = TaskLevel::Recommended {
        default_action,
        timeout_secs: timeout,
    };

    // Update task in store
    update_task_in_store(&mut store, updated_task)?;

    println!("✓ Task {} set to Recommended level", task_id);
    println!("  Countdown: {} seconds", timeout);
    println!(
        "  Default action: {}",
        format_action(default_action).to_lowercase()
    );

    Ok(())
}

/// Set task to Confirmed level (modal confirmation)
pub async fn set_confirmed(task_id: &str) -> Result<()> {
    let mut store = TaskStore::load()?;

    let task = match store.get(task_id) {
        Some(t) => t,
        None => {
            println!("Task not found: {}", task_id);
            return Ok(());
        }
    };

    let mut updated_task = task.clone();
    updated_task.level = TaskLevel::Confirmed;

    // Update task in store
    update_task_in_store(&mut store, updated_task)?;

    println!("✓ Task {} set to Confirmed level", task_id);
    println!("  Modal confirmation required before execution");

    Ok(())
}

/// Set task to Arbitrated level (pause for arbitration)
pub async fn set_arbitrated(task_id: &str, stakeholders: Vec<String>) -> Result<()> {
    let mut store = TaskStore::load()?;

    let task = match store.get(task_id) {
        Some(t) => t,
        None => {
            println!("Task not found: {}", task_id);
            return Ok(());
        }
    };

    let stakeholders = if stakeholders.is_empty() {
        vec!["default".to_string()]
    } else {
        stakeholders
    };

    let mut updated_task = task.clone();
    updated_task.level = TaskLevel::Arbitrated {
        stakeholders: stakeholders.clone(),
    };

    // Update task in store
    update_task_in_store(&mut store, updated_task)?;

    println!("✓ Task {} set to Arbitrated level", task_id);
    println!("  DAG will pause for human arbitration");
    if !stakeholders.is_empty() {
        println!("  Stakeholders: {}", stakeholders.join(", "));
    }

    Ok(())
}

/// Helper: Update task in store
fn update_task_in_store(store: &mut TaskStore, updated_task: Task) -> Result<()> {
    // Remove old task and add updated one
    store.remove(&updated_task.id);
    store.add(updated_task);
    store.save()?;
    Ok(())
}

/// Helper: Format action for display
fn format_action(action: Action) -> &'static str {
    match action {
        Action::Execute => "Execute",
        Action::Skip => "Skip",
        Action::Abort => "Abort",
    }
}

/// Show current task level information
pub fn show_task_level(task: &Task) {
    println!("Task Level: {}", format_task_level(&task.level));
    match &task.level {
        TaskLevel::Mechanical { retry } => {
            println!("  Type: Mechanical (auto-execute)");
            println!("  Retries: {}", retry);
        }
        TaskLevel::Recommended {
            default_action,
            timeout_secs,
        } => {
            println!("  Type: Recommended (countdown)");
            println!("  Timeout: {} seconds", timeout_secs);
            println!("  Default: {:?}", default_action);
        }
        TaskLevel::Confirmed => {
            println!("  Type: Confirmed (modal confirmation)");
        }
        TaskLevel::Arbitrated { stakeholders } => {
            println!("  Type: Arbitrated (human arbitration)");
            println!("  Stakeholders: {}", stakeholders.join(", "));
        }
    }
}

/// Helper: Format task level for display
fn format_task_level(level: &TaskLevel) -> String {
    match level {
        TaskLevel::Mechanical { .. } => "Mechanical".to_string(),
        TaskLevel::Recommended { .. } => "Recommended".to_string(),
        TaskLevel::Confirmed => "Confirmed".to_string(),
        TaskLevel::Arbitrated { .. } => "Arbitrated".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_action_conversion() {
        assert_eq!(
            Action::from(DefaultAction::Execute),
            Action::Execute
        );
        assert_eq!(Action::from(DefaultAction::Skip), Action::Skip);
        assert_eq!(Action::from(DefaultAction::Abort), Action::Abort);
    }
}
