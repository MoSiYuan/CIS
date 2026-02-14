//! Task Manager Performance Benchmarks
//!
//! Measures key performance metrics for Task Manager operations:
//! - Team matching (finding appropriate agent/team for tasks)
//! - Task assignment and scheduling
//! - Execution plan generation
//! - Task status tracking and updates
//!
//! Uses criterion.rs for statistical analysis with warm-up runs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use cis_core::service::task_service::{TaskInfo, TaskPriority, TaskStatus, CreateTaskOptions};
use cis_core::agent::{AgentType, AgentConfig};
use cis_core::scheduler::{TaskDag, TaskLevel, DagNodeStatus};
use cis_core::agent::cluster::{AgentClusterConfig, AgentClusterExecutor, SessionManager};
use std::time::Duration;
use std::collections::HashMap;
use tokio::runtime::Runtime;

// ============================================================================
// Mock Team and Task Structures
// ============================================================================

/// Mock team with capabilities
#[derive(Debug, Clone)]
struct MockTeam {
    id: String,
    capabilities: Vec<String>,
    current_load: usize,
    max_capacity: usize,
}

impl MockTeam {
    fn new(id: String, capabilities: Vec<String>, max_capacity: usize) -> Self {
        Self {
            id,
            capabilities,
            current_load: 0,
            max_capacity,
        }
    }

    fn can_handle(&self, required_capabilities: &[String]) -> bool {
        required_capabilities.iter()
            .all(|cap| self.capabilities.contains(cap))
    }

    fn available_capacity(&self) -> usize {
        self.max_capacity.saturating_sub(self.current_load)
    }

    fn assign_task(&mut self) -> bool {
        if self.current_load < self.max_capacity {
            self.current_load += 1;
            true
        } else {
            false
        }
    }

    fn complete_task(&mut self) {
        if self.current_load > 0 {
            self.current_load -= 1;
        }
    }
}

/// Mock task requirements
#[derive(Debug, Clone)]
struct MockTaskRequirements {
    task_type: String,
    capabilities: Vec<String>,
    priority: TaskPriority,
    estimated_duration_secs: u64,
}

impl MockTaskRequirements {
    fn new(task_type: String, capabilities: Vec<String>, priority: TaskPriority) -> Self {
        Self {
            task_type,
            capabilities,
            priority,
            estimated_duration_secs: 60,
        }
    }
}

// ============================================================================
// Runtime Helpers
// ============================================================================

fn rt() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark team matching algorithm
fn bench_team_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_team_matching");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    // Create diverse team pool
    let teams = vec![
        MockTeam::new("rust-team".to_string(), vec!["rust".to_string(), "compilation".to_string()], 5),
        MockTeam::new("python-team".to_string(), vec!["python".to_string(), "ml".to_string()], 3),
        MockTeam::new("general-team".to_string(), vec!["general".to_string()], 10),
        MockTeam::new("deployment-team".to_string(), vec!["deploy".to_string(), "docker".to_string()], 4),
        MockTeam::new("test-team".to_string(), vec!["test".to_string(), "qa".to_string()], 8),
    ];

    group.bench_function("match_single_capability", |b| {
        b.iter(|| {
            let requirements = MockTaskRequirements::new(
                "compile".to_string(),
                vec!["rust".to_string()],
                TaskPriority::Normal,
            );

            black_box(
                teams.iter()
                    .filter(|team| team.can_handle(&requirements.capabilities))
                    .filter(|team| team.available_capacity() > 0)
                    .min_by_key(|team| team.current_load)
            );
        });
    });

    group.bench_function("match_multiple_capabilities", |b| {
        b.iter(|| {
            let requirements = MockTaskRequirements::new(
                "ml-task".to_string(),
                vec!["python".to_string(), "ml".to_string()],
                TaskPriority::High,
            );

            black_box(
                teams.iter()
                    .filter(|team| team.can_handle(&requirements.capabilities))
                    .filter(|team| team.available_capacity() > 0)
                    .min_by_key(|team| team.current_load)
            );
        });
    });

    group.bench_function("match_no_available_team", |b| {
        // All teams at capacity
        let mut full_teams = teams.clone();
        for team in &mut full_teams {
            team.current_load = team.max_capacity;
        }

        let requirements = MockTaskRequirements::new(
            "test".to_string(),
            vec!["test".to_string()],
            TaskPriority::Normal,
        );

        b.iter(|| {
            black_box(
                full_teams.iter()
                    .find(|team| team.can_handle(&requirements.capabilities))
            );
        });
    });

    group.finish();
}

/// Benchmark task assignment with varying team pool sizes
fn bench_task_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_assignment");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));
    group.throughput(Throughput::Elements(1));

    for team_count in [5, 10, 20, 50].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(team_count), team_count, |b, &team_count| {
            let mut teams: Vec<MockTeam> = (0..team_count)
                .map(|i| {
                    MockTeam::new(
                        format!("team-{}", i),
                        vec![format!("skill-{}", i % 10)],
                        5,
                    )
                })
                .collect();

            b.iter(|| {
                let requirements = MockTaskRequirements::new(
                    format!("task-{}", black_box(1)),
                    vec![format!("skill-{}", black_box(3))],
                    TaskPriority::Normal,
                );

                if let Some(team) = teams.iter_mut()
                    .find(|team| team.can_handle(&requirements.capabilities) && team.available_capacity() > 0)
                {
                    black_box(team.assign_task());
                }
            });
        });
    }

    group.finish();
}

/// Benchmark execution plan generation from DAG
fn bench_execution_plan_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_execution_plan");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut dag = TaskDag::new();

            // Create complex DAG with dependencies
            for i in 0..size {
                let task_id = format!("task-{}", i);
                let deps = if i > 2 {
                    vec![
                        format!("task-{}", i - 1),
                        format!("task-{}", i - 2),
                    ]
                } else if i > 0 {
                    vec![format!("task-{}", i - 1)]
                } else {
                    vec![]
                };

                dag.add_node_with_level(
                    task_id,
                    deps,
                    TaskLevel::Mechanical { retry: 2 },
                ).unwrap();
            }

            b.iter(|| {
                black_box(dag.get_execution_order()).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark task status tracking
fn bench_status_tracking(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_status");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("create_task_info", |b| {
        b.iter(|| {
            let task = TaskInfo {
                summary: cis_core::service::task_service::TaskSummary {
                    id: format!("task-{}", black_box(1)),
                    name: "Test Task".to_string(),
                    status: TaskStatus::Pending,
                    priority: TaskPriority::Normal,
                    worker_id: None,
                    dag_id: None,
                    created_at: chrono::Utc::now(),
                    started_at: None,
                    finished_at: None,
                },
                task_type: "test".to_string(),
                input: serde_json::json!({"test": "data"}),
                output: None,
                error: None,
                retries: 0,
                max_retries: 3,
                timeout: 300,
                metadata: HashMap::new(),
            };

            black_box(task);
        });
    });

    group.bench_function("update_task_status", |b| {
        let mut task = TaskInfo {
            summary: cis_core::service::task_service::TaskSummary {
                id: "task-1".to_string(),
                name: "Test Task".to_string(),
                status: TaskStatus::Pending,
                priority: TaskPriority::Normal,
                worker_id: None,
                dag_id: None,
                created_at: chrono::Utc::now(),
                started_at: None,
                finished_at: None,
            },
            task_type: "test".to_string(),
            input: serde_json::json!({"test": "data"}),
            output: None,
            error: None,
            retries: 0,
            max_retries: 3,
            timeout: 300,
            metadata: HashMap::new(),
        };

        b.iter(|| {
            task.summary.status = black_box(TaskStatus::Running);
            task.summary.started_at = Some(chrono::Utc::now());
            task.summary.worker_id = Some("worker-1".to_string());
        });
    });

    group.finish();
}

/// Benchmark priority-based task scheduling
fn bench_priority_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_priority");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for task_count in [10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*task_count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(task_count), task_count, |b, &task_count| {
            let mut tasks: Vec<TaskInfo> = (0..task_count)
                .map(|i| {
                    let priority = match i % 4 {
                        0 => TaskPriority::Critical,
                        1 => TaskPriority::High,
                        2 => TaskPriority::Normal,
                        _ => TaskPriority::Low,
                    };

                    TaskInfo {
                        summary: cis_core::service::task_service::TaskSummary {
                            id: format!("task-{}", i),
                            name: format!("Task {}", i),
                            status: TaskStatus::Pending,
                            priority,
                            worker_id: None,
                            dag_id: None,
                            created_at: chrono::Utc::now(),
                            started_at: None,
                            finished_at: None,
                        },
                        task_type: "test".to_string(),
                        input: serde_json::json!({}),
                        output: None,
                        error: None,
                        retries: 0,
                        max_retries: 3,
                        timeout: 300,
                        metadata: HashMap::new(),
                    }
                })
                .collect();

            b.iter(|| {
                black_box(
                    tasks.sort_by(|a, b| {
                        // Sort by priority (Critical > High > Normal > Low)
                        let priority_order = |p: &TaskPriority| match p {
                            TaskPriority::Critical => 0,
                            TaskPriority::High => 1,
                            TaskPriority::Normal => 2,
                            TaskPriority::Low => 3,
                        };
                        priority_order(&a.summary.priority)
                            .cmp(&priority_order(&b.summary.priority))
                    })
                );
            });
        });
    }

    group.finish();
}

/// Benchmark concurrent task assignment simulation
fn bench_concurrent_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_concurrent");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    let rt = rt();

    group.bench_function("parallel_team_assignment", |b| {
        let teams: Vec<MockTeam> = (0..10)
            .map(|i| {
                MockTeam::new(
                    format!("team-{}", i),
                    vec![format!("skill-{}", i % 5)],
                    5,
                )
            })
            .collect();

        b.to_async(&rt).iter(|| async {
            let mut teams = teams.clone();
            let task_count = 50;

            // Simulate concurrent assignment
            let handles: Vec<_> = (0..task_count)
                .map(|i| {
                    let mut team_ref = teams.clone();
                    tokio::spawn(async move {
                        let requirements = MockTaskRequirements::new(
                            format!("task-{}", i),
                            vec![format!("skill-{}", i % 5)],
                            TaskPriority::Normal,
                        );

                        if let Some(team) = team_ref.iter_mut()
                            .find(|t| t.can_handle(&requirements.capabilities) && t.available_capacity() > 0)
                        {
                            team.assign_task();
                        }
                    })
                })
                .collect();

            for handle in handles {
                handle.await.unwrap();
            }

            black_box(teams);
        });
    });

    group.finish();
}

/// Benchmark task retry logic
fn bench_task_retry(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_retry");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("check_retry_eligibility", |b| {
        let task = TaskInfo {
            summary: cis_core::service::task_service::TaskSummary {
                id: "task-1".to_string(),
                name: "Failing Task".to_string(),
                status: TaskStatus::Failed,
                priority: TaskPriority::High,
                worker_id: Some("worker-1".to_string()),
                dag_id: None,
                created_at: chrono::Utc::now(),
                started_at: Some(chrono::Utc::now()),
                finished_at: Some(chrono::Utc::now()),
            },
            task_type: "test".to_string(),
            input: serde_json::json!({}),
            output: None,
            error: Some("Connection timeout".to_string()),
            retries: 2,
            max_retries: 3,
            timeout: 300,
            metadata: HashMap::new(),
        };

        b.iter(|| {
            let can_retry = task.retries < task.max_retries;
            let should_retry = can_retry && matches!(
                task.summary.status,
                TaskStatus::Failed | TaskStatus::Retrying
            );

            black_box(should_retry);
        });
    });

    group.finish();
}

/// Benchmark DAG dependency resolution
fn bench_dependency_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_dependencies");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let mut dag = TaskDag::new();
            let mut statuses = HashMap::new();

            // Create DAG with dependencies
            for i in 0..size {
                let task_id = format!("task-{}", i);
                let deps = if i > 0 {
                    vec![format!("task-{}", (i - 1) / 2)]
                } else {
                    vec![]
                };

                dag.add_node_with_level(
                    task_id.clone(),
                    deps,
                    TaskLevel::Mechanical { retry: 1 },
                ).unwrap();

                statuses.insert(task_id, DagNodeStatus::Pending);
            }

            b.iter(|| {
                // Check which tasks are ready to run
                for task_id in dag.get_execution_order().unwrap() {
                    let status = statuses.get(&task_id);
                    black_box(status);
                }
            });
        });
    }

    group.finish();
}

/// Benchmark task metadata handling
fn bench_metadata_handling(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_manager_metadata");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    for metadata_size in [5, 20, 50].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(metadata_size), metadata_size, |b, &metadata_size| {
            let mut metadata = HashMap::new();
            for i in 0..metadata_size {
                metadata.insert(format!("key-{}", i), serde_json::json!(format!("value-{}", i)));
            }

            b.iter(|| {
                let task = TaskInfo {
                    summary: cis_core::service::task_service::TaskSummary {
                        id: "task-1".to_string(),
                        name: "Test Task".to_string(),
                        status: TaskStatus::Pending,
                        priority: TaskPriority::Normal,
                        worker_id: None,
                        dag_id: None,
                        created_at: chrono::Utc::now(),
                        started_at: None,
                        finished_at: None,
                    },
                    task_type: "test".to_string(),
                    input: serde_json::json!({}),
                    output: None,
                    error: None,
                    retries: 0,
                    max_retries: 3,
                    timeout: 300,
                    metadata: metadata.clone(),
                };

                // Simulate metadata query
                black_box(task.metadata.get("key-0"));
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = task_manager_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(50);
    targets =
        bench_team_matching,
        bench_task_assignment,
        bench_execution_plan_generation,
        bench_status_tracking,
        bench_priority_scheduling,
        bench_concurrent_assignment,
        bench_task_retry,
        bench_dependency_resolution,
        bench_metadata_handling
);

criterion_main!(task_manager_benches);
