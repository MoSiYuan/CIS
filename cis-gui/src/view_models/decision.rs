//! Decision ViewModel
//!
//! Manages the four-tier decision panel

use std::sync::{Arc, Mutex};

use tracing::info;

use crate::decision_panel::{DecisionPanel, PendingDecision, DecisionAction};
use super::{ViewModel, ViewModelState};

/// Decision ViewModel
///
/// Responsibilities:
/// - Manage pending decisions
/// - Handle decision actions from UI
/// - Track decision panel state
pub struct DecisionViewModel {
    /// Decision panel component
    panel: DecisionPanel,

    /// Current pending decision
    pending_decision: Arc<Mutex<Option<PendingDecision>>>,

    /// View model state
    state: ViewModelState,
}

impl DecisionViewModel {
    /// Create a new DecisionViewModel
    pub fn new() -> Self {
        info!("Initializing DecisionViewModel");

        Self {
            panel: DecisionPanel::new(),
            pending_decision: Arc::new(Mutex::new(None)),
            state: ViewModelState::new(),
        }
    }

    /// Set a pending decision
    pub fn set_pending_decision(&self, decision: PendingDecision) {
        let mut pending = self.pending_decision.lock().unwrap();
        *pending = Some(decision);
        self.state.mark_dirty();
    }

    /// Get the current pending decision
    pub fn get_pending_decision(&self) -> Option<PendingDecision> {
        let pending = self.pending_decision.lock().unwrap();
        pending.clone()
    }

    /// Clear the pending decision
    pub fn clear_pending_decision(&self) {
        let mut pending = self.pending_decision.lock().unwrap();
        *pending = None;
        self.state.mark_dirty();
    }

    /// Handle a decision action and return a message
    pub fn handle_action(&self, action: DecisionAction) -> String {
        use DecisionAction::*;

        let message = match action {
            AutoProceed => {
                "[Decision] Auto-proceeding with task...".to_string()
            }
            Proceed => {
                "[Decision] User confirmed: Proceed".to_string()
            }
            Skip => {
                "[Decision] User chose: Skip task".to_string()
            }
            Abort => {
                "[Decision] User chose: Abort DAG".to_string()
            }
            Modify { .. } => {
                "[Decision] User modified task parameters".to_string()
            }
            MarkResolved => {
                "[Decision] Arbitration: Marked as resolved".to_string()
            }
            RequestAssistance => {
                "[Decision] Arbitration: Requested assistance".to_string()
            }
            Rollback => {
                "[Decision] Arbitration: Rollback initiated".to_string()
            }
            ViewDetails => {
                "[Decision] Arbitration: Viewing details...".to_string()
            }
        };

        info!("{}", message);

        // Clear pending decision after action
        self.clear_pending_decision();

        message
    }

    /// Get a reference to the decision panel for UI rendering
    pub fn panel(&self) -> &DecisionPanel {
        &self.panel
    }

    /// Get a mutable reference to the decision panel
    pub fn panel_mut(&mut self) -> &mut DecisionPanel {
        &mut self.panel
    }
}

impl ViewModel for DecisionViewModel {
    fn name(&self) -> &str {
        "DecisionViewModel"
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
    use cis_core::types::TaskLevel;

    #[test]
    fn test_decision_vm_creation() {
        let vm = DecisionViewModel::new();
        assert_eq!(vm.name(), "DecisionViewModel");
        assert!(vm.get_pending_decision().is_none());
    }

    #[test]
    fn test_set_pending_decision() {
        let vm = DecisionViewModel::new();

        let decision = PendingDecision::new(
            "test-task".to_string(),
            "Test Decision".to_string(),
            TaskLevel::Confirmed,
        );

        vm.set_pending_decision(decision);

        let pending = vm.get_pending_decision();
        assert!(pending.is_some());
        assert_eq!(pending.unwrap().task_id, "test-task");
    }

    #[test]
    fn test_handle_action() {
        let vm = DecisionViewModel::new();

        let decision = PendingDecision::new(
            "test-task".to_string(),
            "Test Decision".to_string(),
            TaskLevel::Confirmed,
        );

        vm.set_pending_decision(decision);

        let message = vm.handle_action(DecisionAction::Proceed);
        assert!(message.contains("User confirmed"));
        assert!(vm.get_pending_decision().is_none());
    }

    #[test]
    fn test_clear_pending_decision() {
        let vm = DecisionViewModel::new();

        let decision = PendingDecision::new(
            "test-task".to_string(),
            "Test Decision".to_string(),
            TaskLevel::Confirmed,
        );

        vm.set_pending_decision(decision);
        assert!(vm.get_pending_decision().is_some());

        vm.clear_pending_decision();
        assert!(vm.get_pending_decision().is_none());
    }
}
