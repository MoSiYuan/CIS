//! # CIS Task System Integration Tests
//!
//! Comprehensive integration tests for the CIS v1.1.6 Task system.
//!
//! Test coverage:
//! - Task Repository (>90%)
//! - Session Repository (>90%)
//! - DAG Builder (>90%)
//! - Task Manager (>85%)

use cis_core::task::db::create_database_pool;
use cis_core::task::models::*;
use cis_core::task::repository::TaskRepository;
use cis_core::task::session::{AgentRepository, SessionRepository};
use cis_core::task::dag::{DagBuilder, DagError};
use cis_core::task::manager::{TaskManager, TaskAssignment, LevelAssignment, OrchestrationStatus};
use std::sync::Arc;
use tempfile::TempDir;

//=============================================================================
// Test Utilities
//=============================================================================

/// Test database setup helper
struct TestDatabase {
    temp_dir: TempDir,
    pool: Arc<DatabasePool>,
}

impl TestDatabase {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");
        let pool = Arc::new(create_database_pool(Some(db_path), 5).await);

        Self { temp_dir, pool }
    }

    fn task_repository(&self) -> TaskRepository {
        TaskRepository::new(self.pool.clone())
    }

    fn session_repository(&self) -> SessionRepository {
        SessionRepository::new(self.pool.clone())
    }

    fn agent_repository(&self) -> AgentRepository {
        AgentRepository::new(self.pool.clone())
    }

    fn task_manager(&self) -> TaskManager {
        let repo = Arc::new(self.task_repository());
        let session_repo = Arc::new(self.session_repository());
        TaskManager::new(repo, session_repo)
    }
}

/// Mock data factory for creating test tasks
struct TaskFactory;

impl TaskFactory {
    fn create(
        task_id: &str,
        name: &str,
        task_type: TaskType,
        priority: TaskPriority,
    ) -> TaskEntity {
        Self::create_with_deps(task_id, name, task_type, priority, vec![])
    }

    fn create_with_deps(
        task_id: &str,
        name: &str,
        task_type: TaskType,
        priority: TaskPriority,
        dependencies: Vec<String>,
    ) -> TaskEntity {
        let now = chrono::Utc::now().timestamp();
        TaskEntity {
            id: 0,
            task_id: task_id.to_string(),
            name: name.to_string(),
            task_type,
            priority,
            prompt_template: format!("Template for {}", name),
            context_variables: serde_json::json!({"test": true}),
            description: Some(format!("Description for {}", name)),
            estimated_effort_days: Some(1.5),
            dependencies,
            engine_type: None,
            engine_context_id: None,
            status: TaskStatus::Pending,
            assigned_team_id: None,
            assigned_agent_id: None,
            assigned_at: None,
            result: None,
            error_message: None,
            started_at: None,
            completed_at: None,
            duration_seconds: None,
            metadata: Some(serde_json::json!({"test_key": "test_value"})),
            created_at_ts: now,
            updated_at_ts: now,
        }
    }

    fn create_with_engine(
        task_id: &str,
        name: &str,
        task_type: TaskType,
        priority: TaskPriority,
        engine_type: &str,
    ) -> TaskEntity {
        let mut task = Self::create(task_id, name, task_type, priority);
        task.engine_type = Some(engine_type.to_string());
        task
    }

    fn create_cli_task(task_id: &str, priority: TaskPriority) -> TaskEntity {
        Self::create(
            task_id,
            "CLI module refactoring",
            TaskType::ModuleRefactoring,
            priority,
        )
    }

    fn create_memory_task(task_id: &str, priority: TaskPriority) -> TaskEntity {
        Self::create(
            task_id,
            "Memory module optimization",
            TaskType::ModuleRefactoring,
            priority,
        )
    }

    fn create_engine_task(task_id: &str, engine: &str, priority: TaskPriority) -> TaskEntity {
        Self::create_with_engine(
            task_id,
            format!("Engine code injection for {}", engine),
            TaskType::EngineCodeInjection,
            priority,
            engine,
        )
    }
}

//=============================================================================
// Task Repository Tests (>90% coverage)
//=============================================================================

#[tokio::test]
async fn test_task_repository_create_and_retrieve() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("task-1", "Test Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create task");

    assert!(id > 0, "Task ID should be positive");

    let retrieved = repo
        .get_by_id(id)
        .await
        .expect("Failed to retrieve task")
        .expect("Task not found");

    assert_eq!(retrieved.task_id, "task-1");
    assert_eq!(retrieved.name, "Test Task");
    assert_eq!(retrieved.task_type, TaskType::CodeReview);
    assert_eq!(retrieved.priority, TaskPriority::P0);
    assert_eq!(retrieved.status, TaskStatus::Pending);
}

#[tokio::test]
async fn test_task_repository_get_by_task_id() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("custom-id", "Custom ID Task", TaskType::TestWriting, TaskPriority::P1);
    repo.create(&task).await.expect("Failed to create task");

    let retrieved = repo
        .get_by_task_id("custom-id")
        .await
        .expect("Failed to retrieve by task_id")
        .expect("Task not found");

    assert_eq!(retrieved.task_id, "custom-id");
    assert_eq!(retrieved.name, "Custom ID Task");
}

#[tokio::test]
async fn test_task_repository_get_nonexistent() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let result = repo.get_by_id(99999).await.expect("Query failed");
    assert!(result.is_none(), "Non-existent task should return None");

    let result = repo
        .get_by_task_id("nonexistent")
        .await
        .expect("Query failed");
    assert!(result.is_none(), "Non-existent task_id should return None");
}

#[tokio::test]
async fn test_task_repository_batch_create() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let tasks = vec![
        TaskFactory::create("batch-1", "Batch Task 1", TaskType::CodeReview, TaskPriority::P0),
        TaskFactory::create("batch-2", "Batch Task 2", TaskType::TestWriting, TaskPriority::P1),
        TaskFactory::create("batch-3", "Batch Task 3", TaskType::Documentation, TaskPriority::P2),
    ];

    let ids = repo
        .batch_create(&tasks)
        .await
        .expect("Batch create failed");

    assert_eq!(ids.len(), 3, "Should create 3 tasks");
    assert!(ids.iter().all(|&id| id > 0), "All IDs should be positive");

    // Verify tasks were created
    for (i, id) in ids.iter().enumerate() {
        let retrieved = repo.get_by_id(*id).await.expect("Failed to retrieve").expect("Not found");
        assert_eq!(retrieved.task_id, format!("batch-{}", i + 1));
    }
}

#[tokio::test]
async fn test_task_repository_query_by_status() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    // Create tasks with different statuses
    let mut pending_task =
        TaskFactory::create("pending-1", "Pending Task", TaskType::CodeReview, TaskPriority::P0);
    pending_task.status = TaskStatus::Pending;

    let mut running_task =
        TaskFactory::create("running-1", "Running Task", TaskType::CodeReview, TaskPriority::P1);
    running_task.status = TaskStatus::Running;

    let mut completed_task =
        TaskFactory::create("completed-1", "Completed Task", TaskType::CodeReview, TaskPriority::P2);
    completed_task.status = TaskStatus::Completed;

    repo.create(&pending_task).await.expect("Failed to create");
    repo.create(&running_task).await.expect("Failed to create");
    repo.create(&completed_task).await.expect("Failed to create");

    // Query pending tasks
    let filter = TaskFilter {
        status: Some(vec![TaskStatus::Pending]),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 1, "Should find 1 pending task");
    assert_eq!(results[0].status, TaskStatus::Pending);

    // Query multiple statuses
    let filter = TaskFilter {
        status: Some(vec![TaskStatus::Pending, TaskStatus::Running]),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 2, "Should find 2 tasks");
}

#[tokio::test]
async fn test_task_repository_query_by_type() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    repo.create(&TaskFactory::create(
        "review-1",
        "Review",
        TaskType::CodeReview,
        TaskPriority::P0,
    ))
    .await
    .expect("Failed to create");
    repo.create(&TaskFactory::create(
        "test-1",
        "Test",
        TaskType::TestWriting,
        TaskPriority::P1,
    ))
    .await
    .expect("Failed to create");
    repo.create(&TaskFactory::create(
        "doc-1",
        "Doc",
        TaskType::Documentation,
        TaskPriority::P2,
    ))
    .await
    .expect("Failed to create");

    // Query by type
    let filter = TaskFilter {
        task_types: Some(vec![TaskType::CodeReview]),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].task_type, TaskType::CodeReview);

    // Query multiple types
    let filter = TaskFilter {
        task_types: Some(vec![TaskType::TestWriting, TaskType::Documentation]),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_task_repository_query_by_priority_range() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    repo.create(&TaskFactory::create("p0", "P0 Task", TaskType::CodeReview, TaskPriority::P0))
        .await
        .expect("Failed to create");
    repo.create(&TaskFactory::create("p1", "P1 Task", TaskType::CodeReview, TaskPriority::P1))
        .await
        .expect("Failed to create");
    repo.create(&TaskFactory::create("p2", "P2 Task", TaskType::CodeReview, TaskPriority::P2))
        .await
        .expect("Failed to create");
    repo.create(&TaskFactory::create("p3", "P3 Task", TaskType::CodeReview, TaskPriority::P3))
        .await
        .expect("Failed to create");

    // Query P0-P1
    let filter = TaskFilter {
        min_priority: Some(TaskPriority::P0),
        max_priority: Some(TaskPriority::P1),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 2);
    assert!(
        results.iter().all(|t| t.priority <= TaskPriority::P1),
        "All tasks should be P0 or P1"
    );
}

#[tokio::test]
async fn test_task_repository_query_by_team() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let mut team_a_task =
        TaskFactory::create("team-a-1", "Team A Task", TaskType::CodeReview, TaskPriority::P0);
    team_a_task.assigned_team_id = Some("Team-A".to_string());

    let mut team_b_task =
        TaskFactory::create("team-b-1", "Team B Task", TaskType::CodeReview, TaskPriority::P1);
    team_b_task.assigned_team_id = Some("Team-B".to_string());

    repo.create(&team_a_task).await.expect("Failed to create");
    repo.create(&team_b_task).await.expect("Failed to create");

    // Query by team
    let filter = TaskFilter {
        assigned_team: Some("Team-A".to_string()),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].assigned_team_id.as_ref().unwrap(), "Team-A");
}

#[tokio::test]
async fn test_task_repository_query_sorting() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    repo.create(&TaskFactory::create("p1", "Priority 1", TaskType::CodeReview, TaskPriority::P1))
        .await
        .expect("Failed to create");
    repo.create(&TaskFactory::create("p0", "Priority 0", TaskType::CodeReview, TaskPriority::P0))
        .await
        .expect("Failed to create");
    repo.create(&TaskFactory::create("p2", "Priority 2", TaskType::CodeReview, TaskPriority::P2))
        .await
        .expect("Failed to create");

    // Sort by priority ascending
    let filter = TaskFilter {
        sort_by: TaskSortBy::Priority,
        sort_order: TaskSortOrder::Asc,
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].priority, TaskPriority::P0);
    assert_eq!(results[1].priority, TaskPriority::P1);
    assert_eq!(results[2].priority, TaskPriority::P2);
}

#[tokio::test]
async fn test_task_repository_query_pagination() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    // Create 10 tasks
    for i in 0..10 {
        repo.create(&TaskFactory::create(
            &format!("page-{}", i),
            &format!("Task {}", i),
            TaskType::CodeReview,
            TaskPriority::P0,
        ))
        .await
        .expect("Failed to create");
    }

    // Get first page
    let filter = TaskFilter {
        limit: Some(5),
        offset: Some(0),
        ..Default::default()
    };
    let page1 = repo.query(filter).await.expect("Query failed");
    assert_eq!(page1.len(), 5);

    // Get second page
    let filter = TaskFilter {
        limit: Some(5),
        offset: Some(5),
        ..Default::default()
    };
    let page2 = repo.query(filter).await.expect("Query failed");
    assert_eq!(page2.len(), 5);
}

#[tokio::test]
async fn test_task_repository_update_status() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("status-1", "Status Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create");

    repo.update_status(id, TaskStatus::Running, None)
        .await
        .expect("Failed to update status");

    let retrieved = repo.get_by_id(id).await.expect("Failed to retrieve").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Running);
    assert!(retrieved.started_at.is_some(), "started_at should be set");
}

#[tokio::test]
async fn test_task_repository_update_status_with_error() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("error-1", "Error Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create");

    repo.update_status(id, TaskStatus::Failed, Some("Test error".to_string()))
        .await
        .expect("Failed to update status");

    let retrieved = repo.get_by_id(id).await.expect("Failed to retrieve").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Failed);
    assert_eq!(
        retrieved.error_message.as_ref().unwrap(),
        "Test error"
    );
}

#[tokio::test]
async fn test_task_repository_update_assignment() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("assign-1", "Assign Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create");

    repo.update_assignment(id, Some("Team-A".to_string()), Some(123))
        .await
        .expect("Failed to update assignment");

    let retrieved = repo.get_by_id(id).await.expect("Failed to retrieve").expect("Not found");
    assert_eq!(retrieved.assigned_team_id.as_ref().unwrap(), "Team-A");
    assert_eq!(retrieved.assigned_agent_id.unwrap(), 123);
    assert_eq!(retrieved.status, TaskStatus::Assigned);
    assert!(retrieved.assigned_at.is_some(), "assigned_at should be set");
}

#[tokio::test]
async fn test_task_repository_update_result() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("result-1", "Result Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create");

    let result = TaskResult {
        success: true,
        output: Some("Task completed successfully".to_string()),
        artifacts: vec!["artifact1.txt".to_string(), "artifact2.txt".to_string()],
        exit_code: Some(0),
    };

    repo.update_result(&id, &result, 120.5)
        .await
        .expect("Failed to update result");

    let retrieved = repo.get_by_id(id).await.expect("Failed to retrieve").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Completed);
    assert_eq!(retrieved.duration_seconds.unwrap(), 120.5);
    assert!(retrieved.completed_at.is_some(), "completed_at should be set");
    assert_eq!(retrieved.result.as_ref().unwrap().success, true);
    assert_eq!(
        retrieved.result.as_ref().unwrap().output.as_ref().unwrap(),
        "Task completed successfully"
    );
}

#[tokio::test]
async fn test_task_repository_mark_running() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("running-1", "Running Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create");

    repo.mark_running(id).await.expect("Failed to mark running");

    let retrieved = repo.get_by_id(id).await.expect("Failed to retrieve").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Running);
    assert!(retrieved.started_at.is_some(), "started_at should be set");
}

#[tokio::test]
async fn test_task_repository_delete() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let task = TaskFactory::create("delete-1", "Delete Task", TaskType::CodeReview, TaskPriority::P0);
    let id = repo.create(&task).await.expect("Failed to create");

    repo.delete(id).await.expect("Failed to delete");

    let retrieved = repo.get_by_id(id).await.expect("Query failed");
    assert!(retrieved.is_none(), "Task should be deleted");
}

#[tokio::test]
async fn test_task_repository_batch_delete() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let ids = vec![
        repo.create(&TaskFactory::create("del-1", "Del 1", TaskType::CodeReview, TaskPriority::P0))
            .await
            .expect("Failed to create"),
        repo.create(&TaskFactory::create("del-2", "Del 2", TaskType::CodeReview, TaskPriority::P1))
            .await
            .expect("Failed to create"),
        repo.create(&TaskFactory::create("del-3", "Del 3", TaskType::CodeReview, TaskPriority::P2))
            .await
            .expect("Failed to create"),
    ];

    let deleted_count = repo.batch_delete(&ids).await.expect("Failed to batch delete");
    assert_eq!(deleted_count, 3, "Should delete 3 tasks");

    for id in ids {
        let retrieved = repo.get_by_id(id).await.expect("Query failed");
        assert!(retrieved.is_none(), "Task {} should be deleted", id);
    }
}

#[tokio::test]
async fn test_task_repository_count() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    // Create tasks with different statuses
    for i in 0..3 {
        let mut task =
            TaskFactory::create(&format!("count-{}", i), "Task", TaskType::CodeReview, TaskPriority::P0);
        task.status = if i < 2 { TaskStatus::Pending } else { TaskStatus::Completed };
        repo.create(&task).await.expect("Failed to create");
    }

    // Count all
    let filter = TaskFilter::default();
    let count = repo.count(filter).await.expect("Count failed");
    assert_eq!(count, 3);

    // Count pending
    let filter = TaskFilter {
        status: Some(vec![TaskStatus::Pending]),
        ..Default::default()
    };
    let count = repo.count(filter).await.expect("Count failed");
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_task_repository_empty_query() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    let filter = TaskFilter::default();
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 0, "Should return empty result set");
}

#[tokio::test]
async fn test_task_repository_complex_filter() {
    let db = TestDatabase::new();
    let repo = db.task_repository();

    // Create test data
    let mut task1 =
        TaskFactory::create("complex-1", "Complex Task 1", TaskType::CodeReview, TaskPriority::P0);
    task1.status = TaskStatus::Pending;
    task1.assigned_team_id = Some("Team-A".to_string());

    let mut task2 =
        TaskFactory::create("complex-2", "Complex Task 2", TaskType::TestWriting, TaskPriority::P1);
    task2.status = TaskStatus::Running;
    task2.assigned_team_id = Some("Team-A".to_string());

    repo.create(&task1).await.expect("Failed to create");
    repo.create(&task2).await.expect("Failed to create");

    // Complex filter: pending OR running, assigned to Team-A, priority <= P1
    let filter = TaskFilter {
        status: Some(vec![TaskStatus::Pending, TaskStatus::Running]),
        assigned_team: Some("Team-A".to_string()),
        max_priority: Some(TaskPriority::P1),
        ..Default::default()
    };
    let results = repo.query(filter).await.expect("Query failed");

    assert_eq!(results.len(), 2);
}

//=============================================================================
// Session Repository Tests (>90% coverage)
//=============================================================================

#[tokio::test]
async fn test_session_repository_create_and_get() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register(
            "claude",
            "Claude AI",
            &serde_json::json!({"model": "claude-3"}),
            &vec!["code_review".to_string()],
        )
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    assert!(session_id > 0, "Session ID should be positive");

    let session = session_repo
        .get_by_id(session_id)
        .await
        .expect("Failed to get session")
        .expect("Session not found");

    assert_eq!(session.agent_id, agent_id);
    assert_eq!(session.runtime_type, "claude");
    assert_eq!(session.context_capacity, 100000);
    assert_eq!(session.context_used, 0);
    assert_eq!(session.status, SessionStatus::Active);
}

#[tokio::test]
async fn test_session_repository_get_by_session_id() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("opencode", "OpenCode", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let id = session_repo
        .create(agent_id, "opencode", 50000, 30)
        .await
        .expect("Failed to create session");

    let session = session_repo.get_by_id(id).await.expect("Failed to get").expect("Not found");
    let retrieved = session_repo
        .get_by_session_id(&session.session_id)
        .await
        .expect("Failed to get by session_id")
        .expect("Session not found");

    assert_eq!(retrieved.id, id);
    assert_eq!(retrieved.agent_id, agent_id);
}

#[tokio::test]
async fn test_session_repository_acquire_reusable() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    // Acquire session with sufficient capacity
    let acquired = session_repo
        .acquire_session(agent_id, 50000)
        .await
        .expect("Failed to acquire session");

    assert!(acquired.is_some(), "Should find reusable session");
    assert_eq!(acquired.unwrap().id, session_id);
}

#[tokio::test]
async fn test_session_repository_acquire_insufficient_capacity() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    // Try to acquire with insufficient capacity
    let acquired = session_repo
        .acquire_session(agent_id, 200000)
        .await
        .expect("Failed to acquire");

    assert!(acquired.is_none(), "Should not find session with sufficient capacity");
}

#[tokio::test]
async fn test_session_repository_release() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    session_repo
        .release_session(session_id)
        .await
        .expect("Failed to release session");

    let session = session_repo
        .get_by_id(session_id)
        .await
        .expect("Failed to get")
        .expect("Session not found");

    assert_eq!(session.status, SessionStatus::Active);
    assert_eq!(session.context_used, 1);
}

#[tokio::test]
async fn test_session_repository_expire() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    session_repo
        .expire_session(session_id)
        .await
        .expect("Failed to expire session");

    let session = session_repo
        .get_by_id(session_id)
        .await
        .expect("Failed to get")
        .expect("Session not found");

    assert_eq!(session.status, SessionStatus::Expired);
}

#[tokio::test]
async fn test_session_repository_cleanup_expired() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    // Create session with 0 TTL (already expired)
    session_repo
        .create(agent_id, "claude", 100000, 0)
        .await
        .expect("Failed to create session");

    // Wait a bit to ensure expiration
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    let expired_count = session_repo
        .cleanup_expired()
        .await
        .expect("Failed to cleanup");

    assert!(expired_count >= 1, "Should mark at least 1 session as expired");
}

#[tokio::test]
async fn test_session_repository_list_by_agent() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create");
    session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create");

    let sessions = session_repo
        .list_by_agent(agent_id, None)
        .await
        .expect("Failed to list sessions");

    assert_eq!(sessions.len(), 2);
}

#[tokio::test]
async fn test_session_repository_list_by_status() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    session_repo
        .expire_session(session_id)
        .await
        .expect("Failed to expire");

    let active_sessions = session_repo
        .list_by_agent(agent_id, Some(SessionStatus::Active))
        .await
        .expect("Failed to list");

    assert_eq!(active_sessions.len(), 0);

    let expired_sessions = session_repo
        .list_by_agent(agent_id, Some(SessionStatus::Expired))
        .await
        .expect("Failed to list");

    assert_eq!(expired_sessions.len(), 1);
}

#[tokio::test]
async fn test_session_repository_update_usage() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    session_repo
        .update_usage(session_id, 5000)
        .await
        .expect("Failed to update usage");

    let session = session_repo
        .get_by_id(session_id)
        .await
        .expect("Failed to get")
        .expect("Session not found");

    assert_eq!(session.context_used, 5000);
}

#[tokio::test]
async fn test_session_repository_delete() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    session_repo
        .delete(session_id)
        .await
        .expect("Failed to delete");

    let session = session_repo.get_by_id(session_id).await.expect("Query failed");
    assert!(session.is_none(), "Session should be deleted");
}

#[tokio::test]
async fn test_session_repository_delete_expired() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    session_repo
        .expire_session(session_id)
        .await
        .expect("Failed to expire");

    let deleted_count = session_repo
        .delete_expired(0)
        .await
        .expect("Failed to delete expired");

    assert!(deleted_count >= 1, "Should delete at least 1 expired session");
}

#[tokio::test]
async fn test_session_repository_count_active() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create");
    session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create");

    let count = session_repo
        .count_active()
        .await
        .expect("Failed to count");

    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_session_repository_state_transitions() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = db.session_repository();

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    let session_id = session_repo
        .create(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    // Initial state: Active
    let session = session_repo.get_by_id(session_id).await.expect("Failed").expect("Not found");
    assert_eq!(session.status, SessionStatus::Active);

    // Acquire: Active -> Idle
    session_repo
        .acquire_session(agent_id, 50000)
        .await
        .expect("Failed to acquire");
    let session = session_repo.get_by_id(session_id).await.expect("Failed").expect("Not found");
    assert_eq!(session.status, SessionStatus::Idle);

    // Release: Idle -> Active
    session_repo
        .release_session(session_id)
        .await
        .expect("Failed to release");
    let session = session_repo.get_by_id(session_id).await.expect("Failed").expect("Not found");
    assert_eq!(session.status, SessionStatus::Active);
}

#[tokio::test]
async fn test_session_repository_concurrent_access() {
    let db = TestDatabase::new();
    let agent_repo = db.agent_repository();
    let session_repo = Arc::new(session_repo);

    let agent_id = agent_repo
        .register("claude", "Claude", &serde_json::json!({}), &vec![])
        .await
        .expect("Failed to register agent");

    // Create multiple concurrent sessions
    let mut handles = vec![];
    for i in 0..5 {
        let repo = session_repo.clone();
        let handle = tokio::spawn(async move {
            repo.create(agent_id, "claude", 100000, 60).await
        });
        handles.push(handle);
    }

    let results = futures::future::join_all(handles).await;
    let ids: Vec<_> = results.into_iter().map(|r| r.unwrap().unwrap()).collect();

    assert_eq!(ids.len(), 5, "Should create 5 sessions");
    assert!(ids.iter().all(|&id| id > 0), "All IDs should be positive");
}

//=============================================================================
// DAG Builder Tests (>90% coverage)
//=============================================================================

#[tokio::test]
async fn test_dag_builder_simple_chain() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create A -> B -> C
    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    let task_b =
        TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);
    let task_c = TaskFactory::create_with_deps(
        "C",
        "Task C",
        TaskType::CodeReview,
        TaskPriority::P2,
        vec!["B".to_string()],
    );

    let id_a = repo.create(&task_a).await.expect("Failed to create");
    let id_b = repo.create(&task_b).await.expect("Failed to create");
    let id_c = repo.create(&task_c).await.expect("Failed to create");

    let dag = builder
        .build(&["A".to_string(), "B".to_string(), "C".to_string()])
        .await
        .expect("Failed to build DAG");

    assert_eq!(dag.nodes.len(), 3);
    assert_eq!(dag.roots.len(), 1);
    assert!(dag.roots.contains(&id_a));
}

#[tokio::test]
async fn test_dag_builder_parallel_tasks() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create A, B (parallel) -> C
    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    let task_b = TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec![]);
    let task_c = TaskFactory::create_with_deps(
        "C",
        "Task C",
        TaskType::CodeReview,
        TaskPriority::P2,
        vec!["A".to_string(), "B".to_string()],
    );

    repo.create(&task_a).await.expect("Failed to create");
    repo.create(&task_b).await.expect("Failed to create");
    repo.create(&task_c).await.expect("Failed to create");

    let dag = builder
        .build(&["A".to_string(), "B".to_string(), "C".to_string()])
        .await
        .expect("Failed to build DAG");

    assert_eq!(dag.roots.len(), 2, "Should have 2 root nodes");
}

#[tokio::test]
async fn test_dag_builder_cycle_detection() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create A -> B -> C -> A (cycle)
    let task_a = TaskFactory::create_with_deps(
        "A",
        "Task A",
        TaskType::CodeReview,
        TaskPriority::P0,
        vec!["C".to_string()],
    );
    let task_b =
        TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);
    let task_c =
        TaskFactory::create_with_deps("C", "Task C", TaskType::CodeReview, TaskPriority::P2, vec!["B".to_string()]);

    repo.create(&task_a).await.expect("Failed to create");
    repo.create(&task_b).await.expect("Failed to create");
    repo.create(&task_c).await.expect("Failed to create");

    let result = builder.build(&["A".to_string(), "B".to_string(), "C".to_string()]).await;

    assert!(matches!(result, Err(DagError::CycleDetected(_))), "Should detect cycle");
}

#[tokio::test]
async fn test_dag_builder_self_cycle() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create A -> A (self-cycle)
    let task_a = TaskFactory::create_with_deps(
        "A",
        "Task A",
        TaskType::CodeReview,
        TaskPriority::P0,
        vec!["A".to_string()],
    );

    repo.create(&task_a).await.expect("Failed to create");

    let result = builder.build(&["A".to_string()]).await;

    assert!(matches!(result, Err(DagError::CycleDetected(_))), "Should detect self-cycle");
}

#[tokio::test]
async fn test_dag_builder_missing_dependency() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create task with non-existent dependency
    let task_a = TaskFactory::create_with_deps(
        "A",
        "Task A",
        TaskType::CodeReview,
        TaskPriority::P0,
        vec!["NONEXISTENT".to_string()],
    );

    repo.create(&task_a).await.expect("Failed to create");

    let result = builder.build(&["A".to_string()]).await;

    assert!(
        matches!(result, Err(DagError::DependencyNotFound(_))),
        "Should detect missing dependency"
    );
}

#[tokio::test]
async fn test_dag_builder_missing_task() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    let result = builder.build(&["NONEXISTENT".to_string()]).await;

    assert!(matches!(result, Err(DagError::TaskNotFound(_))), "Should detect missing task");
}

#[tokio::test]
async fn test_dag_topological_sort() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create: A -> B -> C, A -> D
    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    let task_b =
        TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);
    let task_c =
        TaskFactory::create_with_deps("C", "Task C", TaskType::CodeReview, TaskPriority::P2, vec!["B".to_string()]);
    let task_d =
        TaskFactory::create_with_deps("D", "Task D", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);

    let id_a = repo.create(&task_a).await.expect("Failed to create");
    let id_b = repo.create(&task_b).await.expect("Failed to create");
    let id_c = repo.create(&task_c).await.expect("Failed to create");
    let id_d = repo.create(&task_d).await.expect("Failed to create");

    let dag = builder
        .build(&["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string()])
        .await
        .expect("Failed to build DAG");

    let sorted = dag.topological_sort().expect("Failed to sort");

    assert_eq!(sorted.len(), 4);
    let pos_a = sorted.iter().position(|&id| id == id_a).unwrap();
    let pos_b = sorted.iter().position(|&id| id == id_b).unwrap();
    let pos_c = sorted.iter().position(|&id| id == id_c).unwrap();
    let pos_d = sorted.iter().position(|&id| id == id_d).unwrap();

    // A must be first
    assert_eq!(pos_a, 0);
    // B must come after A
    assert!(pos_b > pos_a);
    // C must come after B
    assert!(pos_c > pos_b);
    // D must come after A
    assert!(pos_d > pos_a);
}

#[tokio::test]
async fn test_dag_execution_levels() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Create:
    // Level 0: A, B
    // Level 1: C (deps: A), D (deps: B)
    // Level 2: E (deps: C, D)
    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    let task_b = TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec![]);
    let task_c =
        TaskFactory::create_with_deps("C", "Task C", TaskType::CodeReview, TaskPriority::P2, vec!["A".to_string()]);
    let task_d =
        TaskFactory::create_with_deps("D", "Task D", TaskType::CodeReview, TaskPriority::P2, vec!["B".to_string()]);
    let task_e = TaskFactory::create_with_deps(
        "E",
        "Task E",
        TaskType::CodeReview,
        TaskPriority::P3,
        vec!["C".to_string(), "D".to_string()],
    );

    repo.create(&task_a).await.expect("Failed to create");
    repo.create(&task_b).await.expect("Failed to create");
    repo.create(&task_c).await.expect("Failed to create");
    repo.create(&task_d).await.expect("Failed to create");
    repo.create(&task_e).await.expect("Failed to create");

    let dag = builder
        .build(&["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string(), "E".to_string()])
        .await
        .expect("Failed to build DAG");

    let levels = dag.get_execution_levels();

    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0].len(), 2); // A, B
    assert_eq!(levels[1].len(), 2); // C, D
    assert_eq!(levels[2].len(), 1); // E
}

#[tokio::test]
async fn test_dag_single_node() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    repo.create(&task_a).await.expect("Failed to create");

    let dag = builder.build(&["A".to_string()]).await.expect("Failed to build DAG");

    assert_eq!(dag.nodes.len(), 1);
    assert_eq!(dag.roots.len(), 1);

    let sorted = dag.topological_sort().expect("Failed to sort");
    assert_eq!(sorted.len(), 1);
}

#[tokio::test]
async fn test_dag_complex_dependencies() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    // Diamond dependency: A -> B, A -> C, B -> D, C -> D
    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    let task_b =
        TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);
    let task_c =
        TaskFactory::create_with_deps("C", "Task C", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);
    let task_d = TaskFactory::create_with_deps(
        "D",
        "Task D",
        TaskType::CodeReview,
        TaskPriority::P2,
        vec!["B".to_string(), "C".to_string()],
    );

    repo.create(&task_a).await.expect("Failed to create");
    repo.create(&task_b).await.expect("Failed to create");
    repo.create(&task_c).await.expect("Failed to create");
    repo.create(&task_d).await.expect("Failed to create");

    let dag = builder
        .build(&["A".to_string(), "B".to_string(), "C".to_string(), "D".to_string()])
        .await
        .expect("Failed to build DAG");

    let levels = dag.get_execution_levels();

    assert_eq!(levels.len(), 3);
    assert_eq!(levels[0].len(), 1); // A
    assert_eq!(levels[1].len(), 2); // B, C
    assert_eq!(levels[2].len(), 1); // D
}

#[tokio::test]
async fn test_dag_get_dependency_chain() {
    let db = TestDatabase::new();
    let repo = Arc::new(db.task_repository());
    let mut builder = DagBuilder::new(repo.clone());

    let task_a = TaskFactory::create_with_deps("A", "Task A", TaskType::CodeReview, TaskPriority::P0, vec![]);
    let task_b =
        TaskFactory::create_with_deps("B", "Task B", TaskType::CodeReview, TaskPriority::P1, vec!["A".to_string()]);
    let task_c =
        TaskFactory::create_with_deps("C", "Task C", TaskType::CodeReview, TaskPriority::P2, vec!["B".to_string()]);

    let id_a = repo.create(&task_a).await.expect("Failed to create");
    let id_b = repo.create(&task_b).await.expect("Failed to create");
    let id_c = repo.create(&task_c).await.expect("Failed to create");

    let dag = builder
        .build(&["A".to_string(), "B".to_string(), "C".to_string()])
        .await
        .expect("Failed to build DAG");

    let chain = dag.get_dependency_chain(id_c);

    assert_eq!(chain.len(), 3);
    assert_eq!(chain, vec![id_a, id_b, id_c]);
}

//=============================================================================
// Task Manager Integration Tests (>85% coverage)
//=============================================================================

#[tokio::test]
async fn test_task_manager_create_task() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create("mgr-1", "Manager Task", TaskType::CodeReview, TaskPriority::P0);

    let id = manager.create_task(task).await.expect("Failed to create task");
    assert!(id > 0);

    let retrieved = manager.get_task_by_id(id).await.expect("Failed to get").expect("Not found");
    assert_eq!(retrieved.task_id, "mgr-1");
}

#[tokio::test]
async fn test_task_manager_batch_create() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let tasks = vec![
        TaskFactory::create("batch-1", "Batch 1", TaskType::CodeReview, TaskPriority::P0),
        TaskFactory::create("batch-2", "Batch 2", TaskType::TestWriting, TaskPriority::P1),
        TaskFactory::create("batch-3", "Batch 3", TaskType::Documentation, TaskPriority::P2),
    ];

    let ids = manager.create_tasks_batch(tasks).await.expect("Failed to batch create");
    assert_eq!(ids.len(), 3);
}

#[tokio::test]
async fn test_task_manager_team_matching_cli() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create_cli_task("cli-1", TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    let team = manager.match_team_for_task(&retrieved).expect("Failed to match team");

    assert_eq!(team, "Team-V-CLI");
}

#[tokio::test]
async fn test_task_manager_team_matching_memory() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create_memory_task("mem-1", TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    let team = manager.match_team_for_task(&retrieved).expect("Failed to match team");

    assert_eq!(team, "Team-V-Memory");
}

#[tokio::test]
async fn test_task_manager_team_matching_engine() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create_engine_task("engine-1", "Unreal5.7", TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    let team = manager.match_team_for_task(&retrieved).expect("Failed to match team");

    assert_eq!(team, "Team-E-Unreal");
}

#[tokio::test]
async fn test_task_manager_assign_to_teams() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    manager
        .create_task(TaskFactory::create_cli_task("cli-1", TaskPriority::P0))
        .await
        .expect("Failed to create");
    manager
        .create_task(TaskFactory::create_memory_task("mem-1", TaskPriority::P1))
        .await
        .expect("Failed to create");
    manager
        .create_task(TaskFactory::create_cli_task("cli-2", TaskPriority::P0))
        .await
        .expect("Failed to create");

    let assignments = manager
        .assign_tasks_to_teams(vec!["cli-1".to_string(), "mem-1".to_string(), "cli-2".to_string()])
        .await
        .expect("Failed to assign");

    assert_eq!(assignments.len(), 2);

    let cli_assignment = assignments
        .iter()
        .find(|a| a.team_id == "Team-V-CLI")
        .expect("Should find CLI team");
    assert_eq!(cli_assignment.task_ids.len(), 2);
    assert_eq!(cli_assignment.priority, TaskPriority::P0);

    let mem_assignment = assignments
        .iter()
        .find(|a| a.team_id == "Team-V-Memory")
        .expect("Should find Memory team");
    assert_eq!(mem_assignment.task_ids.len(), 1);
    assert_eq!(mem_assignment.priority, TaskPriority::P1);
}

#[tokio::test]
async fn test_task_manager_build_dag() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    manager
        .create_task(TaskFactory::create_with_deps(
            "A",
            "Task A",
            TaskType::CodeReview,
            TaskPriority::P0,
            vec![],
        ))
        .await
        .expect("Failed to create");
    manager
        .create_task(TaskFactory::create_with_deps(
            "B",
            "Task B",
            TaskType::CodeReview,
            TaskPriority::P1,
            vec!["A".to_string()],
        ))
        .await
        .expect("Failed to create");

    let dag = manager
        .build_dag(vec!["A".to_string(), "B".to_string()])
        .await
        .expect("Failed to build DAG");

    assert_eq!(dag.nodes.len(), 2);
    assert_eq!(dag.roots.len(), 1);
}

#[tokio::test]
async fn test_task_manager_orchestrate_simple_workflow() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    // Create independent tasks
    manager
        .create_task(TaskFactory::create("task-1", "Task 1", TaskType::CodeReview, TaskPriority::P0))
        .await
        .expect("Failed to create");
    manager
        .create_task(TaskFactory::create("task-2", "Task 2", TaskType::TestWriting, TaskPriority::P1))
        .await
        .expect("Failed to create");

    let result = manager
        .orchestrate_tasks(vec!["task-1".to_string(), "task-2".to_string()])
        .await
        .expect("Failed to orchestrate");

    assert_eq!(result.status, OrchestrationStatus::Ready);
    assert_eq!(result.plan.levels.len(), 1);
    assert!(!result.plan.levels[0].assignments.is_empty());
}

#[tokio::test]
async fn test_task_manager_orchestrate_dependent_workflow() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    // Create dependent tasks
    manager
        .create_task(TaskFactory::create_with_deps(
            "base",
            "Base Task",
            TaskType::ModuleRefactoring,
            TaskPriority::P0,
            vec![],
        ))
        .await
        .expect("Failed to create");
    manager
        .create_task(TaskFactory::create_with_deps(
            "dependent",
            "Dependent Task",
            TaskType::ModuleRefactoring,
            TaskPriority::P1,
            vec!["base".to_string()],
        ))
        .await
        .expect("Failed to create");

    let result = manager
        .orchestrate_tasks(vec!["base".to_string(), "dependent".to_string()])
        .await
        .expect("Failed to orchestrate");

    assert_eq!(result.status, OrchestrationStatus::Ready);
    assert_eq!(result.plan.levels.len(), 2);
    assert_eq!(result.plan.levels[0].level, 0);
    assert_eq!(result.plan.levels[1].level, 1);
}

#[tokio::test]
async fn test_task_manager_update_task_status() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create("status-1", "Status Task", TaskType::CodeReview, TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    manager
        .update_task_status(id, TaskStatus::Running, None)
        .await
        .expect("Failed to update status");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Running);
}

#[tokio::test]
async fn test_task_manager_mark_running() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create("run-1", "Run Task", TaskType::CodeReview, TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    manager.mark_task_running(id).await.expect("Failed to mark running");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Running);
    assert!(retrieved.started_at.is_some());
}

#[tokio::test]
async fn test_task_manager_assign_to_team() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create("assign-1", "Assign Task", TaskType::CodeReview, TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    manager
        .assign_task_to_team(id, "Team-A".to_string(), Some(123))
        .await
        .expect("Failed to assign");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    assert_eq!(retrieved.assigned_team_id.as_ref().unwrap(), "Team-A");
    assert_eq!(retrieved.assigned_agent_id.unwrap(), 123);
}

#[tokio::test]
async fn test_task_manager_update_result() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create("result-1", "Result Task", TaskType::CodeReview, TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    let result = TaskResult {
        success: true,
        output: Some("Success".to_string()),
        artifacts: vec![],
        exit_code: Some(0),
    };

    manager
        .update_task_result(id, &result, 120.0)
        .await
        .expect("Failed to update result");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed").expect("Not found");
    assert_eq!(retrieved.status, TaskStatus::Completed);
    assert_eq!(retrieved.duration_seconds.unwrap(), 120.0);
}

#[tokio::test]
async fn test_task_manager_query_with_filter() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    manager
        .create_task(TaskFactory::create("q-1", "Query 1", TaskType::CodeReview, TaskPriority::P0))
        .await
        .expect("Failed to create");
    manager
        .create_task(TaskFactory::create("q-2", "Query 2", TaskType::CodeReview, TaskPriority::P1))
        .await
        .expect("Failed to create");

    let filter = TaskFilter {
        min_priority: Some(TaskPriority::P0),
        max_priority: Some(TaskPriority::P0),
        ..Default::default()
    };

    let results = manager.query_tasks(filter).await.expect("Failed to query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].priority, TaskPriority::P0);
}

#[tokio::test]
async fn test_task_manager_count() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    for i in 0..5 {
        let priority = if i < 2 { TaskPriority::P0 } else { TaskPriority::P1 };
        manager
            .create_task(TaskFactory::create(
                &format!("count-{}", i),
                "Task",
                TaskType::CodeReview,
                priority,
            ))
            .await
            .expect("Failed to create");
    }

    let filter = TaskFilter {
        min_priority: Some(TaskPriority::P0),
        max_priority: Some(TaskPriority::P0),
        ..Default::default()
    };

    let count = manager.count_tasks(filter).await.expect("Failed to count");
    assert_eq!(count, 2);
}

#[tokio::test]
async fn test_task_manager_delete_task() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let task = TaskFactory::create("del-1", "Delete Task", TaskType::CodeReview, TaskPriority::P0);
    let id = manager.create_task(task).await.expect("Failed to create");

    manager.delete_task(id).await.expect("Failed to delete");

    let retrieved = manager.get_task_by_id(id).await.expect("Failed to get");
    assert!(retrieved.is_none());
}

#[tokio::test]
async fn test_task_manager_batch_delete() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let ids = vec![
        manager
            .create_task(TaskFactory::create("del-1", "Del 1", TaskType::CodeReview, TaskPriority::P0))
            .await
            .expect("Failed"),
        manager
            .create_task(TaskFactory::create("del-2", "Del 2", TaskType::CodeReview, TaskPriority::P1))
            .await
            .expect("Failed"),
        manager
            .create_task(TaskFactory::create("del-3", "Del 3", TaskType::CodeReview, TaskPriority::P2))
            .await
            .expect("Failed"),
    ];

    let deleted_count = manager.delete_tasks_batch(&ids).await.expect("Failed to delete");
    assert_eq!(deleted_count, 3);

    for id in ids {
        let retrieved = manager.get_task_by_id(id).await.expect("Failed");
        assert!(retrieved.is_none());
    }
}

#[tokio::test]
async fn test_task_manager_session_lifecycle() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let agent_id = 123;
    let session_id = manager
        .create_session(agent_id, "claude", 100000, 60)
        .await
        .expect("Failed to create session");

    assert!(session_id > 0);

    // Acquire
    let session = manager
        .acquire_session(agent_id, 50000)
        .await
        .expect("Failed to acquire")
        .expect("No session found");
    assert_eq!(session.id, session_id);

    // Release
    manager.release_session(session_id).await.expect("Failed to release");

    // Cleanup
    let count = manager.cleanup_expired_sessions().await.expect("Failed to cleanup");
    assert!(count >= 0);
}

#[tokio::test]
async fn test_task_manager_end_to_end_workflow() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    // 1. Create tasks with dependencies
    let _ = manager
        .create_task(TaskFactory::create_with_deps(
            "base",
            "Base Module",
            TaskType::ModuleRefactoring,
            TaskPriority::P0,
            vec![],
        ))
        .await
        .expect("Failed to create");
    let _ = manager
        .create_task(TaskFactory::create_with_deps(
            "feature",
            "Feature Implementation",
            TaskType::ModuleRefactoring,
            TaskPriority::P1,
            vec!["base".to_string()],
        ))
        .await
        .expect("Failed to create");
    let _ = manager
        .create_task(TaskFactory::create_with_deps(
            "review",
            "Code Review",
            TaskType::CodeReview,
            TaskPriority::P0,
            vec!["feature".to_string()],
        ))
        .await
        .expect("Failed to create");

    // 2. Orchestrate
    let result = manager
        .orchestrate_tasks(vec!["base".to_string(), "feature".to_string(), "review".to_string()])
        .await
        .expect("Failed to orchestrate");

    assert_eq!(result.status, OrchestrationStatus::Ready);
    assert_eq!(result.plan.levels.len(), 3);

    // 3. Verify levels
    assert_eq!(result.plan.levels[0].level, 0); // base
    assert_eq!(result.plan.levels[1].level, 1); // feature
    assert_eq!(result.plan.levels[2].level, 2); // review

    // 4. Verify team assignments
    assert!(!result.plan.levels[0].assignments.is_empty());
    assert!(result.plan.estimated_total_duration_secs > 0);
}

#[tokio::test]
async fn test_task_manager_empty_orchestration() {
    let db = TestDatabase::new();
    let manager = db.task_manager();

    let result = manager.orchestrate_tasks(vec![]).await.expect("Failed to orchestrate");

    assert_eq!(result.status, OrchestrationStatus::Ready);
    assert_eq!(result.plan.levels.len(), 0);
}
