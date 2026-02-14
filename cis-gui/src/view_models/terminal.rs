//! Terminal ViewModel
//!
//! Manages terminal history, command processing, and output

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::RwLock;
use tracing::{info, warn};

use cis_core::types::TaskLevel;
use cis_core::service::{NodeService, DagService, ListOptions};
use cis_core::service::node_service::{BindOptions, TrustLevel};

use crate::decision_panel::PendingDecision;
use super::{ViewModel, ViewModelState};
use super::{NodeViewModel, DecisionViewModel};

/// Terminal ViewModel
///
/// Responsibilities:
/// - Manage terminal command history
/// - Parse and execute commands
/// - Format output for display
/// - Handle command-specific logic
pub struct TerminalViewModel {
    /// Command history
    history: Arc<RwLock<Vec<String>>>,

    /// Dependencies
    node_vm: Arc<NodeViewModel>,
    decision_vm: Arc<DecisionViewModel>,

    /// Services
    node_service: Option<NodeService>,
    dag_service: Option<DagService>,

    /// Async runtime handle
    runtime_handle: tokio::runtime::Handle,

    /// View model state
    state: ViewModelState,
}

impl TerminalViewModel {
    /// Create a new TerminalViewModel
    pub fn new(
        node_vm: Arc<NodeViewModel>,
        decision_vm: Arc<DecisionViewModel>,
        node_service: Option<NodeService>,
        dag_service: Option<DagService>,
        runtime_handle: tokio::runtime::Handle,
    ) -> Self {
        info!("Initializing TerminalViewModel");

        let history = vec![
            "CIS Agent Terminal v0.1.0".to_string(),
            "Type 'help' for available commands".to_string(),
            "".to_string(),
        ];

        Self {
            history: Arc::new(RwLock::new(history)),
            node_vm,
            decision_vm,
            node_service,
            dag_service,
            runtime_handle,
            state: ViewModelState::new(),
        }
    }

    /// Get terminal history
    pub async fn get_history(&self) -> Vec<String> {
        self.history.read().await.clone()
    }

    /// Clear terminal history
    pub async fn clear(&self) {
        let mut history = self.history.write().await;
        history.clear();
        history.push("CIS Agent Terminal v0.1.0".to_string());
        history.push("Type 'help' for available commands".to_string());
        history.push("".to_string());
        self.state.mark_dirty();
    }

    /// Execute a command
    pub async fn execute_command(&self, cmd: &str) {
        let mut history = self.history.write().await;
        history.push(format!("$ {}", cmd));

        let cmd = cmd.trim();
        let parts: Vec<&str> = cmd.split_whitespace().collect();

        if parts.is_empty() {
            history.push(String::new());
            return;
        }

        match parts[0] {
            "help" => self.show_help(&mut history).await,
            "clear" => {
                // Clear is handled separately
                history.push("Terminal cleared.".to_string());
            }
            "node" => {
                if parts.len() < 2 {
                    history.push("Usage: node <ls|list|inspect|ping|stats|bind> [args]".to_string());
                } else {
                    match parts[1] {
                        "ls" | "list" => {
                            self.cmd_node_list(&mut history).await;
                        }
                        "inspect" => {
                            if parts.len() < 3 {
                                history.push("Usage: node inspect <node_id>".to_string());
                            } else {
                                self.cmd_node_inspect(&mut history, parts[2]).await;
                            }
                        }
                        "ping" => {
                            if parts.len() < 3 {
                                history.push("Usage: node ping <node_id>".to_string());
                            } else {
                                self.cmd_node_ping(&mut history, parts[2]).await;
                            }
                        }
                        "stats" => {
                            if parts.len() < 3 {
                                history.push("Usage: node stats <node_id>".to_string());
                            } else {
                                self.cmd_node_stats(&mut history, parts[2]).await;
                            }
                        }
                        "bind" => {
                            if parts.len() < 3 {
                                history.push("Usage: node bind <endpoint> [--did <did>]".to_string());
                            } else {
                                let endpoint = parts[2];
                                let did = parts.iter().position(|&p| p == "--did")
                                    .and_then(|pos| parts.get(pos + 1).copied());
                                self.cmd_node_bind(&mut history, endpoint, did).await;
                            }
                        }
                        _ => {
                            history.push(format!("Unknown node subcommand: {}", parts[1]));
                            history.push("Available: ls, inspect, ping, stats, bind".to_string());
                        }
                    }
                }
            }
            "dag" => {
                if parts.len() < 2 {
                    history.push("Usage: dag <ls|list|run|status|definitions> [args]".to_string());
                } else {
                    match parts[1] {
                        "ls" | "list" => self.cmd_dag_list(&mut history).await,
                        "run" => {
                            if parts.len() < 3 {
                                history.push("Usage: dag run <dag_id>".to_string());
                            } else {
                                self.cmd_dag_run(&mut history, parts[2]).await;
                            }
                        }
                        "status" => {
                            let run_id = parts.get(2).copied();
                            self.cmd_dag_status(&mut history, run_id).await;
                        }
                        "definitions" => self.cmd_dag_definitions(&mut history).await,
                        "runs" => {
                            if parts.len() < 3 {
                                history.push("Usage: dag runs <dag_id>".to_string());
                            } else {
                                self.cmd_dag_runs(&mut history, parts[2]).await;
                            }
                        }
                        _ => {
                            history.push(format!("Unknown dag subcommand: {}", parts[1]));
                            history.push("Available: ls, run, status, definitions, runs".to_string());
                        }
                    }
                }
            }
            "demo" => {
                if parts.len() < 2 {
                    history.push("Usage: demo <decision|confirm|arbitrate>".to_string());
                } else {
                    match parts[1] {
                        "decision" => self.cmd_demo_decision(&mut history).await,
                        "confirm" => self.cmd_demo_confirm(&mut history).await,
                        "arbitrate" => self.cmd_demo_arbitrate(&mut history).await,
                        _ => {
                            history.push(format!("Unknown demo: {}", parts[1]));
                        }
                    }
                }
            }
            "glm" => {
                history.push("Opening GLM API panel...".to_string());
                // This would trigger opening the GLM panel
            }
            "agent" => {
                history.push("Agent command: Use 'dag run <id>' or 'node ping <id>' for now.".to_string());
            }
            _ => {
                history.push(format!("Unknown command: {}", cmd));
                history.push("Type 'help' for available commands".to_string());
            }
        }

        history.push(String::new());
        self.state.mark_dirty();
    }

    /// Show help information
    async fn show_help(&self, history: &mut Vec<String>) {
        history.push("Available commands:".to_string());
        history.push("".to_string());
        history.push("Node Management:".to_string());
        history.push("  node ls                   - List all nodes".to_string());
        history.push("  node inspect <id>         - Show node details".to_string());
        history.push("  node ping <id>            - Ping a node".to_string());
        history.push("  node stats <id>           - Show node statistics".to_string());
        history.push("  node bind <endpoint>      - Bind to a new node".to_string());
        history.push("".to_string());
        history.push("DAG Management:".to_string());
        history.push("  dag ls                    - List all DAGs".to_string());
        history.push("  dag run <id>              - Run a DAG".to_string());
        history.push("  dag status [run_id]       - Show DAG run status".to_string());
        history.push("  dag definitions           - List DAG definitions".to_string());
        history.push("  dag runs <dag_id>         - List runs for a DAG".to_string());
        history.push("".to_string());
        history.push("Demo Commands:".to_string());
        history.push("  demo decision             - Demo decision panel (Recommended)".to_string());
        history.push("  demo confirm              - Demo decision panel (Confirmed)".to_string());
        history.push("  demo arbitrate            - Demo decision panel (Arbitrated)".to_string());
        history.push("".to_string());
        history.push("Other:".to_string());
        history.push("  glm                       - Open GLM API panel".to_string());
        history.push("  clear                     - Clear terminal".to_string());
        history.push("  help                      - Show this help".to_string());
    }

    /// Execute node list command
    async fn cmd_node_list(&self, history: &mut Vec<String>) {
        if let Some(ref service) = self.node_service {
            let options = ListOptions::default();
            match service.list(options).await {
                Ok(result) => {
                    if result.items.is_empty() {
                        history.push("No nodes found.".to_string());
                        history.push("Use 'node bind <endpoint>' to add a new node.".to_string());
                    } else {
                        history.push(format!("Nodes ({} total):", result.total));
                        history.push("".to_string());
                        history.push(
                            format!("{:<20} {:<12} {:<20} {:<30} {}", "NODE ID", "STATUS", "NAME", "ENDPOINT", "DID")
                        );
                        history.push("-".repeat(100));
                        for node in &result.items {
                            use cis_core::service::node_service::NodeStatus as CoreNodeStatus;
                            let status_icon = match node.status {
                                CoreNodeStatus::Online => "● online",
                                CoreNodeStatus::Offline => "○ offline",
                                CoreNodeStatus::Blacklisted => "✗ blacklisted",
                                CoreNodeStatus::Suspicious => "⚠ suspicious",
                                CoreNodeStatus::Unknown => "? unknown",
                            };
                            let name = if node.name.len() > 20 {
                                format!("{}...", &node.name[..17])
                            } else {
                                node.name.clone()
                            };
                            history.push(
                                format!("{:<20} {:<12} {:<20} {:<30} {}",
                                    node.id,
                                    status_icon,
                                    name,
                                    node.endpoint,
                                    node.did
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    history.push(format!("Error listing nodes: {}", e));
                    self.fallback_to_demo_nodes(history).await;
                }
            }
        } else {
            history.push("NodeService not available. Showing demo data...".to_string());
            self.fallback_to_demo_nodes(history).await;
        }
    }

    /// Fallback to demo nodes when service is unavailable
    async fn fallback_to_demo_nodes(&self, history: &mut Vec<String>) {
        let nodes = self.node_vm.get_nodes().await;
        history.push("Demo Nodes:".to_string());
        for node in nodes {
            use crate::node_manager::{NodeStatus, TrustState};
            let status = match node.status {
                NodeStatus::Online => "● online",
                NodeStatus::Offline => "○ offline",
                NodeStatus::Connecting => "◐ connecting",
            };
            let trust = match node.trust_state {
                TrustState::Verified => "verified",
                TrustState::Pending => "pending",
                TrustState::Blocked => "blocked",
                TrustState::Unknown => "unknown",
            };
            history.push(
                format!("  {} {} @ {} [{}]", status, node.name, node.address, trust)
            );
        }
    }

    /// Execute node inspect command
    async fn cmd_node_inspect(&self, history: &mut Vec<String>, node_id: &str) {
        match self.node_vm.inspect_node(node_id).await {
            Ok(info) => {
                history.push(format!("Node: {}", info.summary.id));
                history.push(format!("  DID:        {}", info.summary.did));
                history.push(format!("  Name:       {}", info.summary.name));
                history.push(format!("  Status:     {:?}", info.summary.status));
                history.push(format!("  Endpoint:   {}", info.summary.endpoint));
                history.push(format!("  Version:    {}", info.summary.version));
                let pk_short = if info.public_key.len() > 40 {
                    format!("{}...", &info.public_key[..40])
                } else {
                    info.public_key.clone()
                };
                history.push(format!("  Public Key: {}", pk_short));
                history.push(format!("  Trust Score: {:.2}", info.trust_score));
                history.push(format!("  Blacklisted: {}", info.is_blacklisted));
            }
            Err(e) => {
                history.push(format!("Error inspecting node '{}': {}", node_id, e));
            }
        }
    }

    /// Execute node ping command
    async fn cmd_node_ping(&self, history: &mut Vec<String>, node_id: &str) {
        history.push(format!("Pinging node: {}", node_id));
        match self.node_vm.ping_node(node_id).await {
            Ok(true) => {
                history.push(format!("✓ Node '{}' is online", node_id));
            }
            Ok(false) => {
                history.push(format!("✗ Node '{}' is offline or blacklisted", node_id));
            }
            Err(e) => {
                history.push(format!("✗ Error pinging node '{}': {}", node_id, e));
            }
        }
    }

    /// Execute node stats command
    async fn cmd_node_stats(&self, history: &mut Vec<String>, node_id: &str) {
        if let Some(ref service) = self.node_service {
            match service.stats(node_id).await {
                Ok(stats) => {
                    history.push(format!("Node Statistics: {}", node_id));
                    history.push(format!("  CPU Usage:      {:.1}%", stats.cpu_percent));
                    history.push(format!("  Memory Usage:   {:.1}%", stats.memory_percent));
                    history.push(format!("  Memory:         {} MB / {} MB",
                        stats.memory_usage / (1024 * 1024),
                        stats.memory_limit / (1024 * 1024)));
                    history.push(format!("  Network RX:     {} bytes", stats.net_rx_bytes));
                    history.push(format!("  Network TX:     {} bytes", stats.net_tx_bytes));
                    history.push(format!("  Processes:      {}", stats.pids));
                }
                Err(e) => {
                    history.push(format!("Error getting stats for '{}': {}", node_id, e));
                }
            }
        } else {
            history.push("NodeService not available.".to_string());
        }
    }

    /// Execute node bind command
    async fn cmd_node_bind(&self, history: &mut Vec<String>, endpoint: &str, did: Option<&str>) {
        match self.node_vm.bind_node(endpoint, did.map(|s| s.to_string())).await {
            Ok(info) => {
                history.push("✓ Node bound successfully".to_string());
                history.push(format!("  Node ID:   {}", info.summary.id));
                history.push(format!("  DID:       {}", info.summary.did));
                history.push(format!("  Endpoint:  {}", info.summary.endpoint));
                history.push(format!("  Status:    {:?}", info.summary.status));
            }
            Err(e) => {
                history.push(format!("✗ Error binding node: {}", e));
            }
        }
    }

    /// Execute dag list command
    async fn cmd_dag_list(&self, history: &mut Vec<String>) {
        if let Some(ref service) = self.dag_service {
            let options = ListOptions::default();
            match service.list(options).await {
                Ok(result) => {
                    if result.items.is_empty() {
                        history.push("No DAGs found.".to_string());
                    } else {
                        history.push(format!("DAGs ({} total):", result.total));
                        history.push("".to_string());
                        history.push(
                            format!("{:<20} {:<12} {:<10} {:<12} {}", "ID", "NAME", "VERSION", "STATUS", "TASKS")
                        );
                        history.push("-".repeat(70));
                        for dag in &result.items {
                            use cis_core::service::dag_service::DagStatus;
                            let status_icon = match dag.status {
                                DagStatus::Active => "● active",
                                DagStatus::Draft => "○ draft",
                                DagStatus::Paused => "⏸ paused",
                                DagStatus::Deprecated => "⚠ deprecated",
                            };
                            history.push(
                                format!("{:<20} {:<12} {:<10} {:<12} {}",
                                    dag.id,
                                    dag.name,
                                    dag.version,
                                    status_icon,
                                    dag.tasks_count
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    history.push(format!("Error listing DAGs: {}", e));
                }
            }
        } else {
            history.push("DagService not available.".to_string());
        }
    }

    /// Execute dag run command
    async fn cmd_dag_run(&self, history: &mut Vec<String>, dag_id: &str) {
        if let Some(ref service) = self.dag_service {
            let params = HashMap::new();
            match service.run(dag_id, params).await {
                Ok(run) => {
                    history.push("✓ DAG run started".to_string());
                    history.push(format!("  Run ID:      {}", run.run_id));
                    history.push(format!("  DAG ID:      {}", run.dag_id));
                    history.push(format!("  Status:      {:?}", run.status));
                    history.push(format!("  Tasks:       {}/{}", run.tasks_completed, run.tasks_total));
                }
                Err(e) => {
                    history.push(format!("✗ Error running DAG '{}': {}", dag_id, e));
                }
            }
        } else {
            history.push("DagService not available.".to_string());
        }
    }

    /// Execute dag status command
    async fn cmd_dag_status(&self, history: &mut Vec<String>, run_id: Option<&str>) {
        if let Some(id) = run_id {
            if let Some(ref service) = self.dag_service {
                match service.run_inspect(id).await {
                    Ok(run) => {
                        history.push(format!("DAG Run Status: {}", run.run_id));
                        history.push(format!("  DAG ID:       {}", run.dag_id));
                        history.push(format!("  Status:       {:?}", run.status));
                        history.push(format!("  Started:      {}", run.started_at.format("%Y-%m-%d %H:%M:%S")));
                        if let Some(finished) = run.finished_at {
                            history.push(format!("  Finished:     {}", finished.format("%Y-%m-%d %H:%M:%S")));
                        }
                        history.push(format!("  Tasks:        {}/{} completed, {} failed",
                            run.tasks_completed, run.tasks_total, run.tasks_failed));
                    }
                    Err(e) => {
                        history.push(format!("Error getting run status: {}", e));
                    }
                }
            } else {
                history.push("DagService not available.".to_string());
            }
        } else {
            history.push("Usage: dag status <run_id>".to_string());
            history.push("Note: Active run tracking not yet implemented.".to_string());
        }
    }

    /// Execute dag definitions command
    async fn cmd_dag_definitions(&self, history: &mut Vec<String>) {
        history.push("DAG definitions from database:".to_string());
        history.push("Note: This would query dag_specs table.".to_string());
        history.push("Use 'dag ls' for high-level DAG listing.".to_string());
    }

    /// Execute dag runs command
    async fn cmd_dag_runs(&self, history: &mut Vec<String>, dag_id: &str) {
        if let Some(ref service) = self.dag_service {
            match service.runs(dag_id, 10).await {
                Ok(runs) => {
                    if runs.is_empty() {
                        history.push(format!("No runs found for DAG '{}'", dag_id));
                    } else {
                        history.push(format!("Runs for DAG '{}' (last {}):", dag_id, runs.len()));
                        history.push("".to_string());
                        history.push(
                            format!("{:<36} {:<12} {:<20} {}", "RUN ID", "STATUS", "STARTED", "TASKS")
                        );
                        history.push("-".repeat(90));
                        for run in &runs {
                            let started = run.started_at.format("%Y-%m-%d %H:%M");
                            history.push(
                                format!("{:<36} {:<12} {:<20} {}/{}",
                                    run.run_id,
                                    format!("{:?}", run.status),
                                    started,
                                    run.tasks_completed,
                                    run.tasks_total
                                )
                            );
                        }
                    }
                }
                Err(e) => {
                    history.push(format!("Error listing runs for '{}': {}", dag_id, e));
                }
            }
        } else {
            history.push("DagService not available.".to_string());
        }
    }

    /// Demo decision panel command
    async fn cmd_demo_decision(&self, history: &mut Vec<String>) {
        history.push("Demo: Triggering Recommended decision level...".to_string());
        let decision = PendingDecision::new(
            "task-demo-1".to_string(),
            "编译测试".to_string(),
            TaskLevel::Recommended {
                default_action: cis_core::types::Action::Execute,
                timeout_secs: 30,
            },
        )
        .with_description("执行 cargo test 进行测试");
        self.decision_vm.set_pending_decision(decision);
    }

    /// Demo confirm command
    async fn cmd_demo_confirm(&self, history: &mut Vec<String>) {
        history.push("Demo: Triggering Confirmed decision level...".to_string());
        let decision = PendingDecision::new(
            "task-demo-2".to_string(),
            "部署到生产环境".to_string(),
            TaskLevel::Confirmed,
        )
        .with_description("此操作将影响线上服务")
        .with_risk("可能导致服务中断");
        self.decision_vm.set_pending_decision(decision);
    }

    /// Demo arbitrate command
    async fn cmd_demo_arbitrate(&self, history: &mut Vec<String>) {
        history.push("Demo: Triggering Arbitrated decision level...".to_string());
        let decision = PendingDecision::new(
            "task-demo-3".to_string(),
            "解决合并冲突".to_string(),
            TaskLevel::Arbitrated {
                stakeholders: vec!["user1".to_string(), "user2".to_string()],
            },
        )
        .with_description("Git merge 产生冲突，需要手动解决")
        .with_conflicts(vec![
            "src/main.rs".to_string(),
            "config.toml".to_string(),
        ]);
        self.decision_vm.set_pending_decision(decision);
    }
}

impl ViewModel for TerminalViewModel {
    fn name(&self) -> &str {
        "TerminalViewModel"
    }

    fn needs_refresh(&self) -> bool {
        self.state.is_dirty()
    }

    fn mark_dirty(&self) {
        self.state.mark_dirty();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn test_terminal_vm_creation() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();

        let node_vm = Arc::new(NodeViewModel::new(None, handle.clone()));
        let decision_vm = Arc::new(DecisionViewModel::new());

        let terminal_vm = TerminalViewModel::new(
            node_vm,
            decision_vm,
            None,
            None,
            handle,
        );

        assert_eq!(terminal_vm.name(), "TerminalViewModel");

        let history = terminal_vm.get_history().await;
        assert_eq!(history.len(), 3);
        assert!(history[0].contains("CIS Agent Terminal"));
    }

    #[tokio::test]
    async fn test_execute_help() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();

        let node_vm = Arc::new(NodeViewModel::new(None, handle.clone()));
        let decision_vm = Arc::new(DecisionViewModel::new());

        let terminal_vm = TerminalViewModel::new(
            node_vm,
            decision_vm,
            None,
            None,
            handle,
        );

        terminal_vm.execute_command("help").await;

        let history = terminal_vm.get_history().await;
        assert!(history.iter().any(|line| line.contains("Available commands")));
    }

    #[tokio::test]
    async fn test_clear() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();

        let node_vm = Arc::new(NodeViewModel::new(None, handle.clone()));
        let decision_vm = Arc::new(DecisionViewModel::new());

        let terminal_vm = TerminalViewModel::new(
            node_vm,
            decision_vm,
            None,
            None,
            handle,
        );

        terminal_vm.execute_command("test command").await;
        terminal_vm.clear().await;

        let history = terminal_vm.get_history().await;
        assert_eq!(history.len(), 3);
    }
}
