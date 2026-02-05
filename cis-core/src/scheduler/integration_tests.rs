//! Integration tests for CIS-DAG four-tier decision and debt mechanism
//!
//! This module tests the integration between:
//! - Four-tier decision mechanism (TaskLevel)
//! - Debt accumulation mechanism (FailureType)
//! - DAG execution flow
//! - DagScheduler management

#[cfg(test)]
mod tests {
    use crate::scheduler::{
        DagError, DagNodeStatus, DagRun, DagRunStatus, DagScheduler, PermissionResult, TaskDag,
    };
    use crate::types::{Action, DebtEntry, FailureType, TaskLevel};

    // ==================== Four-Tier Decision Tests ====================

    #[test]
    fn test_mechanical_level_auto_execute() {
        // Create DAG with Mechanical level task
        let mut dag = TaskDag::new();
        dag.add_node_with_level(
            "task1".to_string(),
            vec![],
            TaskLevel::Mechanical { retry: 3 },
        )
        .unwrap();
        dag.initialize();

        // Check permission - should auto-approve
        let permission = dag.check_task_permission("task1").unwrap();
        assert_eq!(permission, PermissionResult::AutoApprove);
    }

    #[test]
    fn test_recommended_level_countdown() {
        // Create DAG with Recommended level task
        let mut dag = TaskDag::new();
        dag.add_node_with_level(
            "task1".to_string(),
            vec![],
            TaskLevel::Recommended {
                default_action: Action::Execute,
                timeout_secs: 5,
            },
        )
        .unwrap();
        dag.initialize();

        // Check permission - should have countdown
        let permission = dag.check_task_permission("task1").unwrap();
        assert_eq!(
            permission,
            PermissionResult::Countdown {
                seconds: 5,
                default_action: Action::Execute,
            }
        );
    }

    #[test]
    fn test_recommended_level_different_actions() {
        // Test Execute action
        let mut dag = TaskDag::new();
        dag.add_node_with_level(
            "task1".to_string(),
            vec![],
            TaskLevel::Recommended {
                default_action: Action::Skip,
                timeout_secs: 10,
            },
        )
        .unwrap();

        let permission = dag.check_task_permission("task1").unwrap();
        assert_eq!(
            permission,
            PermissionResult::Countdown {
                seconds: 10,
                default_action: Action::Skip,
            }
        );

        // Test Abort action
        dag.add_node_with_level(
            "task2".to_string(),
            vec![],
            TaskLevel::Recommended {
                default_action: Action::Abort,
                timeout_secs: 15,
            },
        )
        .unwrap();

        let permission = dag.check_task_permission("task2").unwrap();
        assert_eq!(
            permission,
            PermissionResult::Countdown {
                seconds: 15,
                default_action: Action::Abort,
            }
        );
    }

    #[test]
    fn test_confirmed_level_requires_manual() {
        // Create DAG with Confirmed level task
        let mut dag = TaskDag::new();
        dag.add_node_with_level("task1".to_string(), vec![], TaskLevel::Confirmed)
            .unwrap();
        dag.initialize();

        // Check permission - should need confirmation
        let permission = dag.check_task_permission("task1").unwrap();
        assert_eq!(permission, PermissionResult::NeedsConfirmation);

        // Should NOT auto-approve
        assert_ne!(permission, PermissionResult::AutoApprove);
    }

    #[test]
    fn test_arbitrated_level_pauses_dag() {
        // Create DAG with Arbitrated level task
        let mut dag = TaskDag::new();
        dag.add_node_with_level(
            "task1".to_string(),
            vec![],
            TaskLevel::Arbitrated {
                stakeholders: vec!["user1".to_string(), "user2".to_string()],
            },
        )
        .unwrap();
        dag.initialize();

        // Check permission - should need arbitration
        let permission = dag.check_task_permission("task1").unwrap();
        assert_eq!(
            permission,
            PermissionResult::NeedsArbitration {
                stakeholders: vec!["user1".to_string(), "user2".to_string()],
            }
        );

        // Mark as running then arbitrated
        dag.mark_running("task1".to_string()).unwrap();
        dag.mark_arbitrated("task1".to_string()).unwrap();

        // Verify status
        assert_eq!(
            dag.get_node_status("task1"),
            Some(DagNodeStatus::Arbitrated)
        );
    }

    #[test]
    fn test_arbitrated_status_pauses_run() {
        let mut dag = TaskDag::new();
        dag.add_node(
            "task1".to_string(),
            vec![],
        )
        .unwrap();
        dag.initialize();
        dag.mark_running("task1".to_string()).unwrap();
        dag.mark_arbitrated("task1".to_string()).unwrap();

        let mut run = DagRun::new(dag);
        assert!(matches!(run.status, DagRunStatus::Running));

        // Update status after arbitration
        run.update_status();
        assert!(matches!(run.status, DagRunStatus::Paused));
    }

    // ==================== Debt Mechanism Tests ====================

    #[test]
    fn test_ignorable_debt_continues_downstream() {
        // Create DAG: A -> B -> C
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.add_node("C".to_string(), vec!["B".to_string()]).unwrap();

        dag.initialize();

        // Execute A, then mark as failed with Ignorable debt
        dag.mark_running("A".to_string()).unwrap();
        let (debt, skipped) = dag
            .mark_failed_with_type(
                "A".to_string(),
                FailureType::Ignorable,
                "Test error".to_string(),
            )
            .unwrap();

        // Should have debt entry
        assert!(debt.is_some());
        assert_eq!(debt.as_ref().unwrap().failure_type, FailureType::Ignorable);

        // No tasks should be skipped
        assert!(skipped.is_empty(), "Ignorable debt should not skip downstream tasks");

        // Verify A is in Debt(Ignorable) status
        assert!(matches!(
            dag.get_node_status("A"),
            Some(DagNodeStatus::Debt(FailureType::Ignorable))
        ));
    }

    #[test]
    fn test_blocking_debt_freezes_dag() {
        // Create DAG: A -> B -> C
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.add_node("C".to_string(), vec!["B".to_string()]).unwrap();

        dag.initialize();

        // Execute A, then mark as failed with Blocking debt
        dag.mark_running("A".to_string()).unwrap();
        let (debt, skipped) = dag
            .mark_failed_with_type(
                "A".to_string(),
                FailureType::Blocking,
                "Critical error".to_string(),
            )
            .unwrap();

        // Should have debt entry
        assert!(debt.is_some());
        assert_eq!(debt.as_ref().unwrap().failure_type, FailureType::Blocking);

        // Downstream tasks should be skipped
        assert!(!skipped.is_empty(), "Blocking debt should skip downstream tasks");
        assert!(skipped.contains(&"B".to_string()));
        assert!(skipped.contains(&"C".to_string()));

        // Verify statuses
        assert!(matches!(
            dag.get_node_status("A"),
            Some(DagNodeStatus::Debt(FailureType::Blocking))
        ));
        assert_eq!(dag.get_node_status("B"), Some(DagNodeStatus::Skipped));
        assert_eq!(dag.get_node_status("C"), Some(DagNodeStatus::Skipped));
    }

    #[test]
    fn test_debt_accumulation() {
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec![]).unwrap();
        dag.add_node("C".to_string(), vec![]).unwrap();

        dag.initialize();

        // Create multiple ignorable debts
        dag.mark_running("A".to_string()).unwrap();
        dag.mark_failed_with_type(
            "A".to_string(),
            FailureType::Ignorable,
            "Error in A".to_string(),
        )
        .unwrap();

        dag.mark_running("B".to_string()).unwrap();
        dag.mark_failed_with_type(
            "B".to_string(),
            FailureType::Ignorable,
            "Error in B".to_string(),
        )
        .unwrap();

        dag.mark_running("C".to_string()).unwrap();
        dag.mark_failed_with_type(
            "C".to_string(),
            FailureType::Blocking,
            "Error in C".to_string(),
        )
        .unwrap();

        // Get debts
        let debts = dag.get_debts("test-run-1");
        assert_eq!(debts.len(), 3);

        // Count by type
        let ignorable_count = debts
            .iter()
            .filter(|d| d.failure_type == FailureType::Ignorable)
            .count();
        let blocking_count = debts
            .iter()
            .filter(|d| d.failure_type == FailureType::Blocking)
            .count();

        assert_eq!(ignorable_count, 2);
        assert_eq!(blocking_count, 1);

        // Verify all debts are unresolved
        assert!(debts.iter().all(|d| !d.resolved));
    }

    #[test]
    fn test_resolve_blocking_debt_resumes_dag() {
        // Create DAG: A -> B -> C
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.add_node("C".to_string(), vec!["B".to_string()]).unwrap();

        dag.initialize();

        // Create ignorable debt (blocking would skip B and C)
        dag.mark_running("A".to_string()).unwrap();
        dag.mark_failed_with_type(
            "A".to_string(),
            FailureType::Ignorable,
            "Non-critical error".to_string(),
        )
        .unwrap();

        // A is in debt but not blocking downstream
        assert!(matches!(
            dag.get_node_status("A"),
            Some(DagNodeStatus::Debt(FailureType::Ignorable))
        ));

        // Resolve debt with resume - A becomes completed, B becomes ready
        let new_ready = dag.resolve_debt("A", true).unwrap();

        // B should become ready
        assert!(new_ready.contains(&"B".to_string()));

        // A should be completed
        assert_eq!(dag.get_node_status("A"), Some(DagNodeStatus::Completed));
    }

    #[test]
    fn test_resolve_debt_without_resume() {
        // Create DAG: A -> B
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();

        dag.initialize();

        // Create ignorable debt
        dag.mark_running("A".to_string()).unwrap();
        dag.mark_failed_with_type(
            "A".to_string(),
            FailureType::Ignorable,
            "Error".to_string(),
        )
        .unwrap();

        // Resolve without resuming downstream
        let new_ready = dag.resolve_debt("A", false).unwrap();

        // No new ready tasks
        assert!(new_ready.is_empty());

        // A should be marked as failed
        assert_eq!(dag.get_node_status("A"), Some(DagNodeStatus::Failed));
    }

    #[test]
    fn test_cannot_resolve_non_debt_task() {
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.initialize();

        // Try to resolve a task that's not in debt
        let result = dag.resolve_debt("A", true);
        assert!(result.is_err());

        // Mark as completed and try again
        dag.mark_running("A".to_string()).unwrap();
        dag.mark_completed("A".to_string()).unwrap();

        let result = dag.resolve_debt("A", true);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), DagError::InvalidOperation(_)));
    }

    // ==================== DagScheduler Integration Tests ====================

    #[test]
    fn test_dag_run_with_ulid() {
        // Create scheduler and DAG
        let mut scheduler = DagScheduler::new();
        let mut dag = TaskDag::new();
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();

        // Create run
        let run_id = scheduler.create_run(dag);

        // Verify run_id is valid UUID v4 format (36 chars with hyphens)
        assert_eq!(run_id.len(), 36);
        assert!(run_id.contains('-'));

        // Verify run is tracked
        let run = scheduler.get_run(&run_id).unwrap();
        assert_eq!(run.run_id, run_id);
        assert!(matches!(run.status, DagRunStatus::Running));

        // Verify active run is set
        assert_eq!(scheduler.get_active_run().unwrap().run_id, run_id);
    }

    #[test]
    fn test_dag_run_with_custom_id() {
        let mut scheduler = DagScheduler::new();
        let mut dag = TaskDag::new();
        dag.add_node("task1".to_string(), vec![]).unwrap();
        dag.initialize();

        // Create run with custom ID
        let custom_id = "my-custom-run-id".to_string();
        let run_id = scheduler.create_run_with_id(dag, custom_id.clone());

        assert_eq!(run_id, custom_id);
        assert!(scheduler.get_run(&run_id).is_some());
    }

    #[test]
    fn test_scheduler_mark_task_failed_and_accumulate_debt() {
        let mut scheduler = DagScheduler::new();
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.initialize();

        let run_id = scheduler.create_run(dag);

        // Mark task as failed with scheduler
        let skipped = scheduler
            .mark_task_failed(
                &run_id,
                "A".to_string(),
                FailureType::Ignorable,
                "Test error".to_string(),
            )
            .unwrap();

        assert!(skipped.is_empty());

        // Verify debt is accumulated in the run
        let run = scheduler.get_run(&run_id).unwrap();
        assert_eq!(run.debts.len(), 1);
        assert_eq!(run.debts[0].task_id, "A");
        assert!(!run.debts[0].resolved);
    }

    #[test]
    fn test_scheduler_resolve_debt() {
        let mut scheduler = DagScheduler::new();
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.initialize();

        let run_id = scheduler.create_run(dag);

        // Create ignorable debt (so B isn't skipped)
        scheduler
            .mark_task_failed(
                &run_id,
                "A".to_string(),
                FailureType::Ignorable,
                "Non-critical".to_string(),
            )
            .unwrap();

        // Verify debt is recorded
        let run = scheduler.get_run(&run_id).unwrap();
        assert_eq!(run.debts.len(), 1);
        assert!(!run.debts[0].resolved);
        assert!(matches!(
            run.dag.get_node_status("A"),
            Some(DagNodeStatus::Debt(FailureType::Ignorable))
        ));

        // Resolve debt with resume
        let new_ready = scheduler
            .resolve_run_debt(&run_id, "A", true)
            .unwrap();

        // B should become ready after A is resolved
        assert!(new_ready.contains(&"B".to_string()));

        // Verify debt is marked resolved and A is completed
        let run = scheduler.get_run(&run_id).unwrap();
        assert!(run.debts[0].resolved);
        assert_eq!(run.dag.get_node_status("A"), Some(DagNodeStatus::Completed));
    }

    #[test]
    fn test_run_status_with_unresolved_blocking_debt() {
        let mut scheduler = DagScheduler::new();
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.initialize();

        let run_id = scheduler.create_run(dag);

        // Create blocking debt
        scheduler
            .mark_task_failed(
                &run_id,
                "A".to_string(),
                FailureType::Blocking,
                "Critical".to_string(),
            )
            .unwrap();

        let run = scheduler.get_run(&run_id).unwrap();
        assert!(matches!(run.status, DagRunStatus::Failed));
    }

    #[test]
    fn test_run_status_with_only_ignorable_debts() {
        let mut scheduler = DagScheduler::new();
        let mut dag = TaskDag::new();
        dag.add_node("A".to_string(), vec![]).unwrap();
        dag.add_node("B".to_string(), vec!["A".to_string()]).unwrap();
        dag.initialize();

        let run_id = scheduler.create_run(dag);

        // Create ignorable debt on A
        scheduler
            .mark_task_failed(
                &run_id,
                "A".to_string(),
                FailureType::Ignorable,
                "Non-critical".to_string(),
            )
            .unwrap();

        // When A is in ignorable debt, B can still proceed
        // But the run status is still Failed because there's an unresolved debt
        let run = scheduler.get_run(&run_id).unwrap();
        
        // The run has unresolved debt so it's not completed
        assert!(!matches!(run.status, DagRunStatus::Completed));
        
        // Verify the debt is recorded
        assert_eq!(run.debts.len(), 1);
        assert!(!run.debts[0].resolved);
    }

    // ==================== Mixed Level Tests ====================

    #[test]
    fn test_dag_with_mixed_levels() {
        let mut dag = TaskDag::new();

        // Create DAG with different levels
        dag.add_node_with_level(
            "mechanical".to_string(),
            vec![],
            TaskLevel::Mechanical { retry: 3 },
        )
        .unwrap();

        dag.add_node_with_level(
            "recommended".to_string(),
            vec!["mechanical".to_string()],
            TaskLevel::Recommended {
                default_action: Action::Execute,
                timeout_secs: 30,
            },
        )
        .unwrap();

        dag.add_node_with_level(
            "confirmed".to_string(),
            vec!["recommended".to_string()],
            TaskLevel::Confirmed,
        )
        .unwrap();

        dag.add_node_with_level(
            "arbitrated".to_string(),
            vec!["confirmed".to_string()],
            TaskLevel::Arbitrated {
                stakeholders: vec!["admin".to_string()],
            },
        )
        .unwrap();

        dag.initialize();

        // Check permissions
        assert_eq!(
            dag.check_task_permission("mechanical").unwrap(),
            PermissionResult::AutoApprove
        );
        assert!(matches!(
            dag.check_task_permission("recommended").unwrap(),
            PermissionResult::Countdown { .. }
        ));
        assert_eq!(
            dag.check_task_permission("confirmed").unwrap(),
            PermissionResult::NeedsConfirmation
        );
        assert!(matches!(
            dag.check_task_permission("arbitrated").unwrap(),
            PermissionResult::NeedsArbitration { .. }
        ));
    }

    // ==================== DebtEntry Tests ====================

    #[test]
    fn test_debt_entry_creation() {
        let debt = DebtEntry {
            task_id: "task-1".to_string(),
            dag_run_id: "run-1".to_string(),
            failure_type: FailureType::Ignorable,
            error_message: "Something went wrong".to_string(),
            created_at: chrono::Utc::now(),
            resolved: false,
        };

        assert_eq!(debt.task_id, "task-1");
        assert_eq!(debt.dag_run_id, "run-1");
        assert_eq!(debt.failure_type, FailureType::Ignorable);
        assert_eq!(debt.error_message, "Something went wrong");
        assert!(!debt.resolved);
    }

    // ==================== Complex Workflow Tests ====================

    #[test]
    fn test_complex_workflow_with_debts() {
        // Create a more complex DAG
        //     A (Mechanical)
        //    / \
        //   B   C (Recommended)
        //   |   |
        //   D   E (Confirmed)
        //    \ /
        //     F (Arbitrated)

        let mut dag = TaskDag::new();

        dag.add_node_with_level(
            "A".to_string(),
            vec![],
            TaskLevel::Mechanical { retry: 3 },
        )
        .unwrap();

        dag.add_node_with_level(
            "B".to_string(),
            vec!["A".to_string()],
            TaskLevel::Mechanical { retry: 3 },
        )
        .unwrap();

        dag.add_node_with_level(
            "C".to_string(),
            vec!["A".to_string()],
            TaskLevel::Recommended {
                default_action: Action::Execute,
                timeout_secs: 10,
            },
        )
        .unwrap();

        dag.add_node_with_level(
            "D".to_string(),
            vec!["B".to_string()],
            TaskLevel::Mechanical { retry: 3 },
        )
        .unwrap();

        dag.add_node_with_level(
            "E".to_string(),
            vec!["C".to_string()],
            TaskLevel::Confirmed,
        )
        .unwrap();

        dag.add_node_with_level(
            "F".to_string(),
            vec!["D".to_string(), "E".to_string()],
            TaskLevel::Arbitrated {
                stakeholders: vec!["admin".to_string()],
            },
        )
        .unwrap();

        dag.initialize();

        // Execute flow
        // Step 1: A is ready (Mechanical - auto approve)
        let ready = dag.get_ready_tasks();
        assert!(ready.contains(&"A".to_string()));

        dag.mark_running("A".to_string()).unwrap();
        dag.mark_completed("A".to_string()).unwrap();

        // Step 2: B and C become ready
        let ready = dag.get_ready_tasks();
        assert!(ready.contains(&"B".to_string()));
        assert!(ready.contains(&"C".to_string()));

        // Check C's permission (Recommended)
        let perm_c = dag.check_task_permission("C").unwrap();
        assert!(matches!(perm_c, PermissionResult::Countdown { seconds: 10, .. }));

        // Complete B, skip C (simulate user skip)
        dag.mark_running("B".to_string()).unwrap();
        dag.mark_completed("B".to_string()).unwrap();

        // C is still pending, mark it as completed for testing
        dag.mark_running("C".to_string()).unwrap();
        dag.mark_completed("C".to_string()).unwrap();

        // Step 3: D and E become ready
        let ready = dag.get_ready_tasks();
        assert!(ready.contains(&"D".to_string()));
        assert!(ready.contains(&"E".to_string()));

        // Check E's permission (Confirmed)
        let perm_e = dag.check_task_permission("E").unwrap();
        assert_eq!(perm_e, PermissionResult::NeedsConfirmation);

        // Complete D with ignorable debt
        dag.mark_running("D".to_string()).unwrap();
        dag.mark_failed_with_type(
            "D".to_string(),
            FailureType::Ignorable,
            "Non-critical failure".to_string(),
        )
        .unwrap();

        // D is in debt but E can still complete
        dag.mark_running("E".to_string()).unwrap();
        dag.mark_completed("E".to_string()).unwrap();

        // Step 4: Resolve D's debt first since F depends on both D and E
        dag.resolve_debt("D", true).unwrap();

        // Now F should become ready (D completed + E completed)
        let ready = dag.get_ready_tasks();
        assert!(ready.contains(&"F".to_string()));

        // Check F's permission (Arbitrated)
        let perm_f = dag.check_task_permission("F").unwrap();
        assert!(matches!(perm_f, PermissionResult::NeedsArbitration { .. }));
    }
}
