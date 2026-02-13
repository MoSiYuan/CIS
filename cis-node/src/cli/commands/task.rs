//! # Task Command
//!
//! Task management commands for CIS.

use crate::cli::{CommandContext, CommandOutput, CommandError};
use colored::Colorize;

/// Task management command
pub struct TaskCommand;

impl TaskCommand {
    /// List all tasks
    pub async fn execute_list(
        &self,
        ctx: &CommandContext,
        status: Option<String>,
        task_type: Option<String>,
        priority: Option<String>,
        limit: Option<usize>,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose("Querying tasks from database...");

        // Build filter from arguments
        let filter = cis_core::task::models::TaskFilter {
            status: status.and_then(|s| Self::parse_status(&s).map(|v| vec![v])),
            task_types: task_type.and_then(|t| Self::parse_task_type(&t).map(|v| vec![v])),
            min_priority: priority.as_ref().and_then(|p| Self::parse_priority(p)),
            max_priority: priority.as_ref().and_then(|p| Self::parse_priority(p)),
            limit,
            ..Default::default()
        };

        // Get task manager from context
        let task_manager = ctx.get_task_manager()
            .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

        let tasks = task_manager.query_tasks(filter).await
            .map_err(|e| CommandError::custom(format!("Failed to query tasks: {}", e)))?;

        if tasks.is_empty() {
            return Ok(CommandOutput::Message("No tasks found".to_string()));
        }

        Ok(CommandOutput::Table {
            headers: vec![
                "ID".to_string(),
                "Task ID".to_string(),
                "Name".to_string(),
                "Type".to_string(),
                "Priority".to_string(),
                "Status".to_string(),
                "Team".to_string(),
            ],
            rows: tasks.iter().map(|t| {
                vec![
                    t.id.to_string(),
                    t.task_id.clone(),
                    t.name.clone(),
                    Self::format_task_type(&t.task_type),
                    Self::format_priority(&t.priority),
                    Self::format_status(&t.status),
                    t.assigned_team_id.clone().unwrap_or_else(|| "-".to_string()),
                ]
            }).collect(),
        })
    }

    /// Create a new task
    pub async fn execute_create(
        &self,
        ctx: &CommandContext,
        task_id: String,
        name: String,
        task_type: String,
        priority: String,
        dependencies: Vec<String>,
        prompt_template: Option<String>,
        description: Option<String>,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Creating task: {}", name));

        let task_type_parsed = Self::parse_task_type(&task_type)
            .ok_or_else(|| CommandError::invalid_argument("type", &format!("Invalid task type: {}", task_type)))?;

        let priority_parsed = Self::parse_priority(&priority)
            .ok_or_else(|| CommandError::invalid_argument("priority", &format!("Invalid priority: {}", priority)))?;

        let prompt = prompt_template.unwrap_or_else(|| {
            format!("Execute task: {}", name)
        });

        let task = cis_core::task::models::TaskEntity {
            id: 0, // Will be assigned by database
            task_id: task_id.clone(),
            name: name.clone(),
            task_type: task_type_parsed,
            priority: priority_parsed,
            prompt_template: prompt,
            context_variables: serde_json::json!({}),
            description,
            estimated_effort_days: None,
            dependencies,
            engine_type: None,
            engine_context_id: None,
            status: cis_core::task::models::TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            metadata: None,
            created_at_ts: chrono::Utc::now().timestamp(),
            updated_at_ts: chrono::Utc::now().timestamp(),
        };

        let task_manager = ctx.get_task_manager()
            .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

        let id = task_manager.create_task(task).await
            .map_err(|e| CommandError::custom(format!("Failed to create task: {}", e)))?;

        Ok(CommandOutput::Message(
            format!("{} Task created: {} (ID: {})", "✓".green(), name, id)
        ))
    }

    /// Update task status
    pub async fn execute_update(
        &self,
        ctx: &CommandContext,
        task_id: String,
        status: String,
        error_message: Option<String>,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Updating task {} status to {}", task_id, status));

        // Check if task_id is numeric ID or string task_id
        let id = if let Ok(numeric_id) = task_id.parse::<i64>() {
            numeric_id
        } else {
            // Lookup by task_id string
            let task_manager = ctx.get_task_manager()
                .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

            let task = task_manager.get_task_by_task_id(&task_id).await
                .map_err(|e| CommandError::custom(format!("Failed to find task: {}", e)))?
                .ok_or_else(|| CommandError::not_found("Task", &task_id))?;

            task.id
        };

        let status_parsed = Self::parse_status(&status)
            .ok_or_else(|| CommandError::invalid_argument("status", &format!("Invalid status: {}", status)))?;

        let task_manager = ctx.get_task_manager()
            .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

        task_manager.update_task_status(id, status_parsed, error_message).await
            .map_err(|e| CommandError::custom(format!("Failed to update task: {}", e)))?;

        Ok(CommandOutput::Message(
            format!("{} Task {} updated to {}", "✓".green(), task_id, status)
        ))
    }

    /// Delete a task
    pub async fn execute_delete(
        &self,
        ctx: &CommandContext,
        task_id: String,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Deleting task: {}", task_id));

        let id = if let Ok(numeric_id) = task_id.parse::<i64>() {
            numeric_id
        } else {
            let task_manager = ctx.get_task_manager()
                .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

            let task = task_manager.get_task_by_task_id(&task_id).await
                .map_err(|e| CommandError::custom(format!("Failed to find task: {}", e)))?
                .ok_or_else(|| CommandError::not_found("Task", &task_id))?;

            task.id
        };

        let task_manager = ctx.get_task_manager()
            .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

        task_manager.delete_task(id).await
            .map_err(|e| CommandError::custom(format!("Failed to delete task: {}", e)))?;

        Ok(CommandOutput::Message(
            format!("{} Task {} deleted", "✓".green(), task_id)
        ))
    }

    /// Show task details
    pub async fn execute_show(
        &self,
        ctx: &CommandContext,
        task_id: String,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Fetching task details: {}", task_id));

        let task_manager = ctx.get_task_manager()
            .ok_or_else(|| CommandError::custom("Task manager not available".to_string()))?;

        let task = if let Ok(numeric_id) = task_id.parse::<i64>() {
            task_manager.get_task_by_id(numeric_id).await
                .map_err(|e| CommandError::custom(format!("Failed to get task: {}", e)))?
                .ok_or_else(|| CommandError::not_found("Task", &task_id))?
        } else {
            task_manager.get_task_by_task_id(&task_id).await
                .map_err(|e| CommandError::custom(format!("Failed to get task: {}", e)))?
                .ok_or_else(|| CommandError::not_found("Task", &task_id))?
        };

        let output = format!(
            "{} Task Details\n\
             {}\n\
             \n\
             Name: {}\n\
             Task ID: {}\n\
             Type: {}\n\
             Priority: {}\n\
             Status: {}\n\
             Team: {}\n\
             Agent: {}\n\
             Description: {}\n\
             Dependencies: {}\n\
             Estimated Effort: {} days\n\
             Created: {}\n\
             Started: {}\n\
             Completed: {}\n\
             Duration: {} seconds",
            "═".repeat(40),
            "═".repeat(40),
            task.name.bold(),
            task.task_id.dim(),
            Self::format_task_type(&task.task_type),
            Self::format_priority(&task.priority),
            Self::format_status(&task.status),
            task.assigned_team_id.unwrap_or_else(|| "-".to_string()),
            task.assigned_agent_id.map(|id| id.to_string()).unwrap_or_else(|| "-".to_string()),
            task.description.unwrap_or_else(|| "-".to_string()),
            if task.dependencies.is_empty() {
                "-".to_string()
            } else {
                task.dependencies.join(", ")
            },
            task.estimated_effort_days.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string()),
            task.created_at().format("%Y-%m-%d %H:%M:%S"),
            task.started_at.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            }).unwrap_or_else(|| "-".to_string()),
            task.completed_at.map(|ts| {
                chrono::DateTime::from_timestamp(ts, 0)
                    .unwrap_or_default()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
            }).unwrap_or_else(|| "-".to_string()),
            task.duration_seconds.map(|d| d.to_string()).unwrap_or_else(|| "-".to_string()),
        );

        Ok(CommandOutput::Message(output))
    }

    /// Parse task type from string
    fn parse_task_type(s: &str) -> Option<cis_core::task::models::TaskType> {
        match s.to_lowercase().as_str() {
            "module_refactoring" | "refactoring" => Some(cis_core::task::models::TaskType::ModuleRefactoring),
            "engine_code_injection" | "injection" => Some(cis_core::task::models::TaskType::EngineCodeInjection),
            "performance_optimization" | "optimization" => Some(cis_core::task::models::TaskType::PerformanceOptimization),
            "code_review" | "review" => Some(cis_core::task::models::TaskType::CodeReview),
            "test_writing" | "test" => Some(cis_core::task::models::TaskType::TestWriting),
            "documentation" | "docs" => Some(cis_core::task::models::TaskType::Documentation),
            _ => None,
        }
    }

    /// Parse priority from string
    fn parse_priority(s: &str) -> Option<cis_core::task::models::TaskPriority> {
        match s.to_uppercase().as_str() {
            "P0" | "0" | "critical" => Some(cis_core::task::models::TaskPriority::P0),
            "P1" | "1" | "high" => Some(cis_core::task::models::TaskPriority::P1),
            "P2" | "2" | "medium" => Some(cis_core::task::models::TaskPriority::P2),
            "P3" | "3" | "low" => Some(cis_core::task::models::TaskPriority::P3),
            _ => None,
        }
    }

    /// Parse status from string
    fn parse_status(s: &str) -> Option<cis_core::task::models::TaskStatus> {
        match s.to_lowercase().as_str() {
            "pending" => Some(cis_core::task::models::TaskStatus::Pending),
            "assigned" => Some(cis_core::task::models::TaskStatus::Assigned),
            "running" => Some(cis_core::task::models::TaskStatus::Running),
            "completed" | "done" => Some(cis_core::task::models::TaskStatus::Completed),
            "failed" | "error" => Some(cis_core::task::models::TaskStatus::Failed),
            _ => None,
        }
    }

    /// Format task type for display
    fn format_task_type(task_type: &cis_core::task::models::TaskType) -> String {
        match task_type {
            cis_core::task::models::TaskType::ModuleRefactoring => "Module Refactoring".to_string(),
            cis_core::task::models::TaskType::EngineCodeInjection => "Engine Injection".to_string(),
            cis_core::task::models::TaskType::PerformanceOptimization => "Optimization".to_string(),
            cis_core::task::models::TaskType::CodeReview => "Code Review".to_string(),
            cis_core::task::models::TaskType::TestWriting => "Test Writing".to_string(),
            cis_core::task::models::TaskType::Documentation => "Documentation".to_string(),
        }
    }

    /// Format priority for display
    fn format_priority(priority: &cis_core::task::models::TaskPriority) -> String {
        match priority {
            cis_core::task::models::TaskPriority::P0 => "P0".red().to_string(),
            cis_core::task::models::TaskPriority::P1 => "P1".yellow().to_string(),
            cis_core::task::models::TaskPriority::P2 => "P2".to_string(),
            cis_core::task::models::TaskPriority::P3 => "P3".dim().to_string(),
        }
    }

    /// Format status for display
    fn format_status(status: &cis_core::task::models::TaskStatus) -> String {
        match status {
            cis_core::task::models::TaskStatus::Pending => "Pending".dim().to_string(),
            cis_core::task::models::TaskStatus::Assigned => "Assigned".yellow().to_string(),
            cis_core::task::models::TaskStatus::Running => "Running".blue().to_string(),
            cis_core::task::models::TaskStatus::Completed => "Completed".green().to_string(),
            cis_core::task::models::TaskStatus::Failed => "Failed".red().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_task_type() {
        assert!(matches!(TaskCommand::parse_task_type("refactoring"), Some(cis_core::task::models::TaskType::ModuleRefactoring)));
        assert!(matches!(TaskCommand::parse_task_type("injection"), Some(cis_core::task::models::TaskType::EngineCodeInjection)));
        assert!(matches!(TaskCommand::parse_task_type("optimization"), Some(cis_core::task::models::TaskType::PerformanceOptimization)));
        assert!(matches!(TaskCommand::parse_task_type("review"), Some(cis_core::task::models::TaskType::CodeReview)));
        assert!(matches!(TaskCommand::parse_task_type("test"), Some(cis_core::task::models::TaskType::TestWriting)));
        assert!(matches!(TaskCommand::parse_task_type("docs"), Some(cis_core::task::models::TaskType::Documentation)));
        assert!(TaskCommand::parse_task_type("invalid").is_none());
    }

    #[test]
    fn test_parse_priority() {
        assert!(matches!(TaskCommand::parse_priority("P0"), Some(cis_core::task::models::TaskPriority::P0)));
        assert!(matches!(TaskCommand::parse_priority("critical"), Some(cis_core::task::models::TaskPriority::P0)));
        assert!(matches!(TaskCommand::parse_priority("P1"), Some(cis_core::task::models::TaskPriority::P1)));
        assert!(matches!(TaskCommand::parse_priority("high"), Some(cis_core::task::models::TaskPriority::P1)));
        assert!(matches!(TaskCommand::parse_priority("medium"), Some(cis_core::task::models::TaskPriority::P2)));
        assert!(matches!(TaskCommand::parse_priority("low"), Some(cis_core::task::models::TaskPriority::P3)));
        assert!(TaskCommand::parse_priority("invalid").is_none());
    }

    #[test]
    fn test_parse_status() {
        assert!(matches!(TaskCommand::parse_status("pending"), Some(cis_core::task::models::TaskStatus::Pending)));
        assert!(matches!(TaskCommand::parse_status("assigned"), Some(cis_core::task::models::TaskStatus::Assigned)));
        assert!(matches!(TaskCommand::parse_status("running"), Some(cis_core::task::models::TaskStatus::Running)));
        assert!(matches!(TaskCommand::parse_status("completed"), Some(cis_core::task::models::TaskStatus::Completed)));
        assert!(matches!(TaskCommand::parse_status("done"), Some(cis_core::task::models::TaskStatus::Completed)));
        assert!(matches!(TaskCommand::parse_status("failed"), Some(cis_core::task::models::TaskStatus::Failed)));
        assert!(TaskCommand::parse_status("invalid").is_none());
    }
}
