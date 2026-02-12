//! Main ViewModel
//!
//! Manages application-level state and services

use std::sync::Arc;
use tracing::{info, warn};

use cis_core::service::{NodeService, DagService};

use super::{ViewModel, ViewModelState};
use super::{NodeViewModel, TerminalViewModel, DecisionViewModel};

/// Main application ViewModel
///
/// Responsibilities:
/// - Initialize and manage core services (NodeService, DagService)
/// - Create and manage child ViewModels
/// - Handle application-level state
/// - Provide async runtime for blocking operations
pub struct MainViewModel {
    /// Core services
    node_service: Option<NodeService>,
    dag_service: Option<DagService>,

    /// Async runtime for blocking operations
    runtime: tokio::runtime::Runtime,

    /// Child ViewModels
    node_vm: Arc<NodeViewModel>,
    terminal_vm: Arc<TerminalViewModel>,
    decision_vm: Arc<DecisionViewModel>,

    /// View model state
    state: ViewModelState,

    /// Initialization status
    is_initialized: bool,
    last_error: Option<String>,
}

impl MainViewModel {
    /// Create a new MainViewModel
    pub fn new() -> Self {
        info!("Initializing MainViewModel");

        // Create async runtime
        let runtime = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime");

        // Initialize services
        let node_service = match NodeService::new() {
            Ok(service) => {
                info!("NodeService initialized successfully");
                Some(service)
            }
            Err(e) => {
                warn!("Failed to initialize NodeService: {}", e);
                None
            }
        };

        let dag_service = match DagService::new() {
            Ok(service) => {
                info!("DagService initialized successfully");
                Some(service)
            }
            Err(e) => {
                warn!("Failed to initialize DagService: {}", e);
                None
            }
        };

        // Create child ViewModels
        let node_vm = Arc::new(NodeViewModel::new(
            node_service.clone(),
            runtime.handle().clone(),
        ));

        let decision_vm = Arc::new(DecisionViewModel::new());

        let terminal_vm = Arc::new(TerminalViewModel::new(
            Arc::clone(&node_vm),
            Arc::clone(&decision_vm),
            node_service.clone(),
            dag_service.clone(),
            runtime.handle().clone(),
        ));

        Self {
            node_service,
            dag_service,
            runtime,
            node_vm,
            terminal_vm,
            decision_vm,
            state: ViewModelState::new(),
            is_initialized: false,
            last_error: None,
        }
    }

    /// Initialize the main view model
    pub fn initialize(&mut self) -> Result<(), String> {
        info!("MainViewModel::initialize");

        // Trigger initial node refresh
        self.node_vm.refresh_nodes();

        self.is_initialized = true;
        self.state.update_timestamp();

        Ok(())
    }

    /// Get the node ViewModel
    pub fn get_node_vm(&self) -> Arc<NodeViewModel> {
        Arc::clone(&self.node_vm)
    }

    /// Get the terminal ViewModel
    pub fn get_terminal_vm(&self) -> Arc<TerminalViewModel> {
        Arc::clone(&self.terminal_vm)
    }

    /// Get the decision ViewModel
    pub fn get_decision_vm(&self) -> Arc<DecisionViewModel> {
        Arc::clone(&self.decision_vm)
    }

    /// Get the NodeService (if available)
    pub fn node_service(&self) -> Option<&NodeService> {
        self.node_service.as_ref()
    }

    /// Get the DagService (if available)
    pub fn dag_service(&self) -> Option<&DagService> {
        self.dag_service.as_ref()
    }

    /// Get the async runtime handle
    pub fn runtime(&self) -> &tokio::runtime::Runtime {
        &self.runtime
    }

    /// Check if initialized
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Get the last error (if any)
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Update all child ViewModels
    pub fn update(&mut self) {
        // Update node VM if needed
        if self.node_vm.needs_refresh() {
            self.state.mark_dirty();
        }

        // Update terminal VM if needed
        if self.terminal_vm.needs_refresh() {
            self.state.mark_dirty();
        }

        // Update decision VM if needed
        if self.decision_vm.needs_refresh() {
            self.state.mark_dirty();
        }
    }
}

impl ViewModel for MainViewModel {
    fn name(&self) -> &str {
        "MainViewModel"
    }

    fn needs_refresh(&self) -> bool {
        self.state.is_dirty() ||
            self.node_vm.needs_refresh() ||
            self.terminal_vm.needs_refresh() ||
            self.decision_vm.needs_refresh()
    }

    fn mark_dirty(&self) {
        self.state.mark_dirty();
    }

    fn last_update(&self) -> Option<Instant> {
        self.state.last_update
    }
}

impl Default for MainViewModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_vm_creation() {
        let vm = MainViewModel::new();
        assert_eq!(vm.name(), "MainViewModel");
        assert!(!vm.is_initialized());
    }

    #[test]
    fn test_main_vm_initialize() {
        let mut vm = MainViewModel::new();
        let result = vm.initialize();
        assert!(result.is_ok());
        assert!(vm.is_initialized());
    }

    #[test]
    fn test_get_child_vms() {
        let vm = MainViewModel::new();
        let node_vm = vm.get_node_vm();
        let terminal_vm = vm.get_terminal_vm();
        let decision_vm = vm.get_decision_vm();

        assert_eq!(node_vm.name(), "NodeViewModel");
        assert_eq!(terminal_vm.name(), "TerminalViewModel");
        assert_eq!(decision_vm.name(), "DecisionViewModel");
    }
}
