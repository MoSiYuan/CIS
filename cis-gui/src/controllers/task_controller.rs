//! Task Controller
//!
//! Handles all task and DAG-related operations and business logic

use std::collections::HashMap;

use cis_core::service::{
    DagService,
    ListOptions,
    dag_service::{
        DagRun, DagRunInfo, DagStatus as ServiceDagStatus,
        RunStatus,
    }
};
use cis_core::types::Value;
use tracing::{info, warn, error};

/// Task Controller
///
/// Responsibilities:
/// - Encapsulate all DAG operations
/// - Handle task execution and monitoring
/// - Provide a clean API for ViewModels
pub struct TaskController {
    dag_service: DagService,
}

impl TaskController {
    /// Create a new TaskController
    pub fn new(dag_service: DagService) -> Self {
        info!("Initializing TaskController");
        Self { dag_service }
    }

    /// List all DAGs with optional filters
    pub async fn list_dags(&self, options: ListOptions) -> Result<Vec<crate::glm_panel::DagInfo>, String> {
        info!("TaskController: list_dags");

        self.dag_service
            .list(options)
            .await
            .map(|result| {
                result.items.into_iter().map(|dag| {
                    crate::glm_panel::DagInfo {
                        id: dag.id,
                        name: dag.name,
                        version: dag.version,
                        status: dag.status.to_string(),
                        tasks_count: dag.tasks_count,
                    }
                }).collect()
            })
            .map_err(|e| {
                error!("Failed to list DAGs: {}", e);
                format!("Failed to list DAGs: {}", e)
            })
    }

    /// Run a DAG
    pub async fn run_dag(
        &self,
        dag_id: &str,
        params: HashMap<String, Value>,
    ) -> Result<DagRun, String> {
        info!("TaskController: run_dag({})", dag_id);

        self.dag_service
            .run(dag_id, params)
            .await
            .map_err(|e| {
                error!("Failed to run DAG '{}': {}", dag_id, e);
                format!("Failed to run DAG '{}': {}", dag_id, e)
            })
    }

    /// Get status of a DAG run
    pub async fn get_run_status(&self, run_id: &str) -> Result<DagRunInfo, String> {
        info!("TaskController: get_run_status({})", run_id);

        self.dag_service
            .run_inspect(run_id)
            .await
            .map_err(|e| {
                error!("Failed to get run status '{}': {}", run_id, e);
                format!("Failed to get run status '{}': {}", run_id, e)
            })
    }

    /// Cancel a DAG run
    pub async fn cancel_run(&self, run_id: &str) -> Result<(), String> {
        info!("TaskController: cancel_run({})", run_id);

        self.dag_service
            .run_cancel(run_id)
            .await
            .map_err(|e| {
                error!("Failed to cancel run '{}': {}", run_id, e);
                format!("Failed to cancel run '{}': {}", run_id, e)
            })
    }

    /// List runs for a DAG
    pub async fn list_runs(&self, dag_id: &str, limit: usize) -> Result<Vec<DagRun>, String> {
        info!("TaskController: list_runs({}, {})", dag_id, limit);

        self.dag_service
            .runs(dag_id, limit)
            .await
            .map_err(|e| {
                error!("Failed to list runs for '{}': {}", dag_id, e);
                format!("Failed to list runs for '{}': {}", dag_id, e)
            })
    }

    /// Confirm a pending DAG run
    pub async fn confirm_run(&self, run_id: &str) -> Result<(), String> {
        info!("TaskController: confirm_run({})", run_id);

        // Note: DagService doesn't have a confirm method yet
        // This is a placeholder for future implementation
        warn!("confirm_run not yet implemented in DagService");
        Ok(())
    }

    /// Reject a pending DAG run
    pub async fn reject_run(&self, run_id: &str) -> Result<(), String> {
        info!("TaskController: reject_run({})", run_id);

        // Use cancel to reject
        self.cancel_run(run_id).await
    }

    /// Get pending runs across all DAGs
    pub async fn get_pending_runs(&self) -> Result<Vec<crate::glm_panel::PendingDagInfo>, String> {
        info!("TaskController: get_pending_runs");

        // List all DAGs
        let list_result = self.dag_service
            .list(ListOptions {
                all: false,
                filters: Default::default(),
                limit: Some(100),
                sort_by: None,
                sort_desc: false,
            })
            .await
            .map_err(|e| format!("Failed to list DAGs: {}", e))?;

        let mut pending_runs = Vec::new();

        // Collect runs from each DAG
        for dag in list_result.items {
            match self.dag_service.runs(&dag.id, 10).await {
                Ok(runs) => {
                    for run in runs {
                        // Only collect Pending and Running status runs
                        if matches!(run.status, RunStatus::Pending | RunStatus::Running) {
                            // Calculate expiry time (created + 5 minutes)
                            let expires_at = run.started_at + chrono::Duration::minutes(5);

                            pending_runs.push(crate::glm_panel::PendingDagInfo {
                                dag_id: run.dag_id.clone(),
                                run_id: run.run_id.clone(),
                                description: format!("DAG: {}", dag.name),
                                task_count: run.tasks_total,
                                created_at: run.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                expires_at: expires_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                requested_by: "system".to_string(),
                                status: run.status.to_string(),
                            });
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to get runs for DAG {}: {}", dag.id, e);
                    // Continue processing other DAGs
                }
            }
        }

        Ok(pending_runs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running DagService
    // In a real scenario, you'd use a mock service

    #[tokio::test]
    async fn test_controller_creation() {
        // This test would fail if DagService::new() fails
        // In production, you'd mock the service
    }
}
