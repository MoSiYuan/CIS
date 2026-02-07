//! # Agent Cluster Executor
//!
//! Integrates Agent Cluster with DAG scheduler for concurrent task execution.
//! Manages multiple Agent sessions with max_workers limit and upstream context injection.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::agent::AgentType;

use crate::agent::cluster::{
    context::{build_task_prompt, ContextStore},
    events::{SessionEvent, SessionState},
    manager::SessionManager,
    SessionId,
};
use crate::error::Result;
use crate::scheduler::{DagNodeStatus, DagRun, DagRunStatus};

/// Execution report for a DAG run
#[derive(Debug, Clone)]
pub struct ExecutionReport {
    /// Run ID
    pub run_id: String,
    /// Final status
    pub status: DagRunStatus,
    /// Completed tasks count
    pub completed: usize,
    /// Failed tasks count
    pub failed: usize,
    /// Skipped tasks count
    pub skipped: usize,
    /// Execution duration
    pub duration_secs: u64,
    /// Task outputs
    pub outputs: HashMap<String, TaskOutput>,
}

/// Task output summary
#[derive(Debug, Clone)]
pub struct TaskOutput {
    pub task_id: String,
    pub output: String,
    pub exit_code: i32,
    pub success: bool,
}

/// Agent Cluster Executor configuration
#[derive(Debug, Clone)]
pub struct AgentClusterConfig {
    /// Maximum concurrent workers
    pub max_workers: usize,
    /// Default agent type
    pub default_agent: AgentType,
    /// Base work directory for sessions
    pub base_work_dir: std::path::PathBuf,
    /// Enable upstream context injection
    pub enable_context_injection: bool,
    /// Auto-attach on blockage
    pub auto_attach_on_block: bool,
    /// Task timeout (seconds)
    pub task_timeout_secs: u64,
}

impl Default for AgentClusterConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            default_agent: AgentType::OpenCode,  // 使用 OpenCode 作为默认 Agent
            base_work_dir: std::env::temp_dir().join("cis").join("dag-sessions"),
            enable_context_injection: true,
            auto_attach_on_block: false,
            task_timeout_secs: 3600,
        }
    }
}

/// Agent Cluster Executor
#[derive(Clone)]
pub struct AgentClusterExecutor {
    /// Session manager reference
    session_manager: &'static SessionManager,
    /// Context store for task outputs
    context_store: ContextStore,
    /// Configuration
    config: AgentClusterConfig,
    /// Active monitor tasks
    monitor_handles: Arc<RwLock<HashMap<SessionId, JoinHandle<()>>>>,
}

impl AgentClusterExecutor {
    /// Create new executor
    pub fn new(config: AgentClusterConfig) -> Result<Self> {
        let context_store = ContextStore::default_store()?;
        
        // Ensure base work directory exists
        std::fs::create_dir_all(&config.base_work_dir)?;
        
        Ok(Self {
            session_manager: SessionManager::global(),
            context_store,
            config,
            monitor_handles: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// Create executor with default config
    pub fn default_executor() -> Result<Self> {
        Self::new(AgentClusterConfig::default())
    }
    
    /// Execute a DAG run with concurrent task spawning and event-driven completion
    pub async fn execute_run(&self, run: &mut DagRun) -> Result<ExecutionReport> {
        let start_time = std::time::Instant::now();
        let run_id = run.run_id.clone();
        
        info!("Starting AgentCluster execution for run {}", run_id);
        info!("Max workers: {}", self.config.max_workers);
        
        // Initialize DAG if not already done
        if run.dag.get_ready_tasks().is_empty() {
            run.dag.initialize();
        }
        
        // Subscribe to session events
        let mut event_rx = self.session_manager.subscribe_events();
        
        // Channel for coordinating between event handler and main loop
        let (completion_tx, mut completion_rx) = tokio::sync::mpsc::unbounded_channel::<SessionEvent>();
        
        // Spawn background event forwarder for concurrent event processing
        let event_forwarder = self.spawn_event_forwarder(event_rx.resubscribe(), completion_tx, run_id.clone());
        
        // Main execution loop
        loop {
            // Check if run should stop
            if run.status == DagRunStatus::Failed {
                warn!("Run {} has failed, stopping execution", run_id);
                break;
            }
            
            if run.status == DagRunStatus::Paused {
                info!("Run {} is paused, waiting for resume", run_id);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            
            // Process any completed events first (non-blocking)
            while let Ok(event) = completion_rx.try_recv() {
                self.handle_session_event(run, &event).await;
            }
            
            // Count active sessions for this run
            let active_count = self.count_active_sessions(&run_id).await;
            let available_slots = self.config.max_workers.saturating_sub(active_count);
            
            debug!(
                "Run {}: active={}, slots={}, ready_tasks={}",
                run_id,
                active_count,
                available_slots,
                run.dag.get_ready_tasks().len()
            );
            
            // Start new tasks in parallel if slots available
            if available_slots > 0 {
                let ready_tasks = self.get_ready_tasks(run);
                let tasks_to_start: Vec<_> = ready_tasks.into_iter().take(available_slots).collect();
                
                if !tasks_to_start.is_empty() {
                    self.spawn_tasks_concurrently(run, tasks_to_start).await;
                }
            }
            
            // Check if run is complete
            if self.is_run_complete(run) {
                info!("Run {} completed", run_id);
                break;
            }
            
            // Wait for events or timeout (event-driven instead of polling)
            tokio::select! {
                Some(event) = completion_rx.recv() => {
                    self.handle_session_event(run, &event).await;
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {
                    // Timeout to periodically check for new ready tasks
                }
            }
        }
        
        // Clean up
        drop(event_forwarder);
        self.cleanup_run(&run_id).await;
        
        // Build execution report
        let report = self.build_report(run, start_time.elapsed()).await;
        
        info!(
            "Run {} execution finished: {} completed, {} failed, {} skipped",
            run_id,
            report.completed,
            report.failed,
            report.skipped
        );
        
        Ok(report)
    }
    
    /// Start a single task (internal, uses DagRun)
    async fn start_task(&self, run: &DagRun, task_id: &str, command: &str) -> Result<()> {
        // Get dependencies for this task
        let upstream_deps = run.dag.get_task_dependencies(task_id);
        self.start_task_by_id(&run.run_id, task_id, command, &upstream_deps).await
    }
    
    /// Start a single task by run_id (for concurrent spawning)
    async fn start_task_by_id(
        &self,
        run_id: &str,
        task_id: &str,
        command: &str,
        upstream_deps: &[String],
    ) -> Result<()> {
        info!("Starting task {} for run {}", task_id, run_id);
        
        // Prepare work directory
        let work_dir = self.config.base_work_dir.join(run_id).join(task_id);
        std::fs::create_dir_all(&work_dir)?;
        
        // Prepare upstream context
        let upstream_context = if self.config.enable_context_injection && !upstream_deps.is_empty() {
            self.context_store
                .prepare_upstream_context(run_id, task_id, upstream_deps)
                .await
        } else {
            String::new()
        };
        
        // Build full prompt
        let full_prompt = build_task_prompt(command, &upstream_context);
        
        // Determine agent type (could be extracted from task config)
        let agent_type = self.config.default_agent;
        
        // Create session
        let session_id = self.session_manager.create_session(
            run_id,
            task_id,
            agent_type,
            &full_prompt,
            &work_dir,
            &upstream_context,
        ).await?;
        
        // Start monitor task
        self.spawn_monitor_task(session_id.clone(), run_id.to_string());
        
        info!("Task {} started with session {}", task_id, session_id.short());
        Ok(())
    }
    
    /// Spawn monitor task for a session
    fn spawn_monitor_task(&self, session_id: SessionId, _run_id: String) {
        let manager = self.session_manager;
        let context_store = self.context_store.clone();
        let handles_for_monitor = self.monitor_handles.clone();
        let handles_for_store = self.monitor_handles.clone();
        let session_id_clone = session_id.clone();
        
        let handle = tokio::spawn(async move {
            info!("Monitor task started for session {}", session_id.short());
            
            loop {
                // Check session state
                match manager.get_state(&session_id).await {
                    Ok(SessionState::Completed { output, exit_code }) => {
                        info!("Session {} completed (exit: {})", session_id.short(), exit_code);
                        
                        // Save output to context store
                        if let Err(e) = context_store.save(
                            &session_id.dag_run_id,
                            &session_id.task_id,
                            &output,
                            Some(exit_code),
                        ).await {
                            warn!("Failed to save context: {}", e);
                        }
                        
                        break;
                    }
                    Ok(SessionState::Failed { error }) => {
                        warn!("Session {} failed: {}", session_id.short(), error);
                        
                        // Save error as output
                        let _ = context_store.save(
                            &session_id.dag_run_id,
                            &session_id.task_id,
                            &error,
                            Some(1),
                        ).await;
                        
                        break;
                    }
                    Ok(SessionState::Blocked { reason }) => {
                        debug!("Session {} blocked: {}", session_id.short(), reason);
                        // Wait for recovery
                    }
                    Ok(_) => {
                        // Still running
                    }
                    Err(e) => {
                        error!("Failed to get session state: {}", e);
                        break;
                    }
                }
                
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            
            // Remove from handles
            let mut handles_guard = handles_for_monitor.write().await;
            handles_guard.remove(&session_id);
            
            info!("Monitor task stopped for session {}", session_id.short());
        });
        
        // Store handle
        tokio::spawn(async move {
            let mut handles_guard = handles_for_store.write().await;
            handles_guard.insert(session_id_clone, handle);
        });
    }
    
    /// Spawn event forwarder for concurrent event processing
    fn spawn_event_forwarder(
        &self,
        mut event_rx: tokio::sync::broadcast::Receiver<SessionEvent>,
        completion_tx: tokio::sync::mpsc::UnboundedSender<SessionEvent>,
        run_id: String,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(event) => {
                        let should_forward = match &event {
                            SessionEvent::Completed { session_id, .. } => session_id.dag_run_id == run_id,
                            SessionEvent::Failed { session_id, .. } => session_id.dag_run_id == run_id,
                            SessionEvent::Blocked { session_id, .. } => session_id.dag_run_id == run_id,
                            _ => false,
                        };
                        
                        if should_forward {
                            if completion_tx.send(event).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        })
    }
    
    /// Handle a single session event
    async fn handle_session_event(&self, run: &mut DagRun, event: &SessionEvent) {
        match event {
            SessionEvent::Completed { session_id, exit_code, .. } => {
                if session_id.dag_run_id == run.run_id {
                    if *exit_code == 0 {
                        let _ = run.dag.mark_completed(session_id.task_id.clone());
                    } else {
                        let _ = run.dag.mark_failed(session_id.task_id.clone());
                    }
                    run.update_status();
                    info!("Task {} completed (exit: {})", session_id.task_id, exit_code);
                }
            }
            SessionEvent::Failed { session_id, .. } => {
                if session_id.dag_run_id == run.run_id {
                    let _ = run.dag.mark_failed(session_id.task_id.clone());
                    run.update_status();
                    warn!("Task {} failed", session_id.task_id);
                }
            }
            _ => {}
        }
    }
    
    /// Spawn tasks concurrently
    async fn spawn_tasks_concurrently(
        &self,
        run: &mut DagRun,
        tasks_to_start: Vec<(String, String)>,
    ) {
        let mut pending_tasks = Vec::new();
        for (task_id, command) in tasks_to_start {
            if let Err(e) = run.dag.mark_running(task_id.clone()) {
                warn!("Failed to mark task {} running: {}", task_id, e);
                continue;
            }
            pending_tasks.push((task_id, command));
        }
        
        if pending_tasks.is_empty() {
            return;
        }
        
        info!("Spawning {} tasks concurrently", pending_tasks.len());
        
        let mut spawn_handles = Vec::new();
        for (task_id, command) in pending_tasks {
            let run_id = run.run_id.clone();
            let upstream_deps = run.dag.get_task_dependencies(&task_id);
            
            let handle = tokio::spawn({
                let executor = self.clone();
                async move {
                    let result = executor.start_task_by_id(&run_id, &task_id, &command, &upstream_deps).await;
                    (task_id, result)
                }
            });
            spawn_handles.push(handle);
        }
        
        for handle in spawn_handles {
            match handle.await {
                Ok((task_id, Ok(_))) => debug!("Task {} spawned", task_id),
                Ok((task_id, Err(e))) => {
                    warn!("Failed to start task {}: {}", task_id, e);
                    let _ = run.dag.mark_failed(task_id);
                }
                Err(e) => warn!("Task spawn panicked: {}", e),
            }
        }
    }
    
    /// Get ready tasks from DAG
    fn get_ready_tasks(&self, run: &DagRun) -> Vec<(String, String)> {
        let ready_ids = run.dag.get_ready_tasks();
        let mut result = Vec::new();
        
        for task_id in ready_ids {
            if let Some(command) = run.task_commands.get(&task_id) {
                result.push((task_id, command.clone()));
            }
        }
        
        result
    }
    
    /// Count active sessions for a run
    async fn count_active_sessions(&self, run_id: &str) -> usize {
        let sessions = self.session_manager.list_sessions_by_dag(run_id).await;
        sessions
            .into_iter()
            .filter(|s| {
                !s.state.contains("completed") 
                    && !s.state.contains("failed")
                    && !s.state.contains("killed")
            })
            .count()
    }
    
    /// Prepare upstream context for a task
    async fn prepare_upstream_context(&self, run: &DagRun, task_id: &str) -> String {
        // Get dependencies
        if let Some(node) = run.dag.get_node(task_id) {
            if !node.dependencies.is_empty() {
                return self.context_store
                    .prepare_upstream_context(&run.run_id, task_id, &node.dependencies)
                    .await;
            }
        }
        String::new()
    }
    
    /// Check if run is complete
    fn is_run_complete(&self, run: &DagRun) -> bool {
        run.dag.nodes().values().all(|n| {
            matches!(
                n.status,
                DagNodeStatus::Completed
                    | DagNodeStatus::Failed
                    | DagNodeStatus::Skipped
            )
        })
    }
    
    /// Clean up all sessions for a run
    async fn cleanup_run(&self, run_id: &str) {
        info!("Cleaning up sessions for run {}", run_id);
        
        // Kill all sessions
        let _ = self.session_manager
            .kill_all_by_dag(run_id, "Run completed")
            .await;
        
        // Wait for monitor tasks to complete
        let mut handles = self.monitor_handles.write().await;
        let to_remove: Vec<SessionId> = handles
            .keys()
            .filter(|id| id.dag_run_id == run_id)
            .cloned()
            .collect();
        
        for session_id in to_remove {
            if let Some(handle) = handles.remove(&session_id) {
                let _ = tokio::time::timeout(Duration::from_secs(5), handle).await;
            }
        }
    }
    
    /// Build execution report
    async fn build_report(&self, run: &DagRun, duration: Duration) -> ExecutionReport {
        let mut completed = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut outputs = HashMap::new();
        
        for (task_id, node) in run.dag.nodes() {
            match node.status {
                DagNodeStatus::Completed => {
                    completed += 1;
                    // Load output from context store
                    if let Ok(output) = self.context_store.load(&run.run_id, task_id).await {
                        outputs.insert(
                            task_id.clone(),
                            TaskOutput {
                                task_id: task_id.clone(),
                                output,
                                exit_code: 0,
                                success: true,
                            },
                        );
                    }
                }
                DagNodeStatus::Failed => {
                    failed += 1;
                    if let Ok(output) = self.context_store.load(&run.run_id, task_id).await {
                        outputs.insert(
                            task_id.clone(),
                            TaskOutput {
                                task_id: task_id.clone(),
                                output,
                                exit_code: 1,
                                success: false,
                            },
                        );
                    }
                }
                DagNodeStatus::Skipped => skipped += 1,
                _ => {}
            }
        }
        
        ExecutionReport {
            run_id: run.run_id.clone(),
            status: run.status,
            completed,
            failed,
            skipped,
            duration_secs: duration.as_secs(),
            outputs,
        }
    }
    
    /// Get current execution stats
    pub async fn get_stats(&self, run_id: &str) -> ExecutionStats {
        let sessions = self.session_manager.list_sessions_by_dag(run_id).await;
        
        let active = sessions.iter().filter(|s| s.state == "running").count();
        let blocked = sessions.iter().filter(|s| s.state.contains("blocked")).count();
        let completed = sessions.iter().filter(|s| s.state.contains("completed")).count();
        let failed = sessions.iter().filter(|s| s.state.contains("failed")).count();
        
        ExecutionStats {
            total_sessions: sessions.len(),
            active,
            blocked,
            completed,
            failed,
        }
    }
}

/// Execution statistics
#[derive(Debug, Clone)]
pub struct ExecutionStats {
    pub total_sessions: usize,
    pub active: usize,
    pub blocked: usize,
    pub completed: usize,
    pub failed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_cluster_config_default() {
        let config = AgentClusterConfig::default();
        assert_eq!(config.max_workers, 4);
        assert!(config.enable_context_injection);
    }
}
