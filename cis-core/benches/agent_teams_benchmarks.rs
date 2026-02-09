//! Agent Teams Performance Benchmarks
//!
//! Measures key performance metrics for the Agent Teams system:
//! - Agent Pool operations (acquire, release, reuse)
//! - DAG execution (serial, parallel)
//! - Task execution latency
//! - Context injection performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;

use async_trait::async_trait;

use cis_core::agent::persistent::{
    AgentAcquireConfig, AgentConfig, AgentInfo, AgentPool, AgentRuntime, AgentStatus,
    PersistentAgent, PoolConfig, RuntimeType, TaskRequest, TaskResult,
};
use cis_core::agent::cluster::context::ContextStore;
use cis_core::scheduler::{DagScheduler, TaskDag};
use cis_core::scheduler::multi_agent_executor::{
    MultiAgentDagExecutor, MultiAgentExecutorConfig,
};
use cis_core::error::Result;

// ============================================================================
// Tokio Runtime Helper
// ============================================================================

/// Create Tokio Runtime for async benchmarks
fn rt() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

// ============================================================================
// Mock Implementations for Benchmarking
// ============================================================================

/// Mock PersistentAgent for lightweight benchmarking
pub struct BenchAgent {
    agent_id: String,
    runtime_type: RuntimeType,
}

impl BenchAgent {
    fn new(agent_id: impl Into<String>, runtime_type: RuntimeType) -> Self {
        Self {
            agent_id: agent_id.into(),
            runtime_type,
        }
    }
}

#[async_trait]
impl PersistentAgent for BenchAgent {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn runtime_type(&self) -> RuntimeType {
        self.runtime_type
    }

    async fn execute(&self, _task: TaskRequest) -> Result<TaskResult> {
        // Simulate 10ms processing time
        tokio::time::sleep(Duration::from_millis(10)).await;

        Ok(TaskResult::success(&self.agent_id, "Done"))
    }

    async fn status(&self) -> AgentStatus {
        AgentStatus::Idle
    }

    async fn attach(&self) -> Result<()> {
        Ok(())
    }

    async fn detach(&self) -> Result<()> {
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

/// Mock AgentRuntime for benchmarking
pub struct BenchRuntime {
    runtime_type: RuntimeType,
}

impl BenchRuntime {
    fn new(runtime_type: RuntimeType) -> Self {
        Self { runtime_type }
    }
}

#[async_trait]
impl AgentRuntime for BenchRuntime {
    fn runtime_type(&self) -> RuntimeType {
        self.runtime_type
    }

    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn PersistentAgent>> {
        let agent_id = format!("{}-{}", config.name, uuid::Uuid::new_v4());
        Ok(Box::new(BenchAgent::new(agent_id, self.runtime_type)))
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        vec![]
    }
}

// ============================================================================
// Agent Pool Benchmarks
// ============================================================================

/// Measure Agent acquire and release performance with varying agent counts
fn bench_agent_pool_acquire_release(c: &mut Criterion) {
    let mut group = c.benchmark_group("agent_pool_acquire_release");

    for agent_count in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(agent_count),
            agent_count,
            |b, &count| {
                b.to_async(rt()).iter(|| async move {
                    let pool = AgentPool::new(PoolConfig {
                        max_agents: count * 2,
                        default_timeout: Duration::from_secs(60),
                        health_check_interval: Duration::from_secs(30),
                        auto_cleanup: false,
                        idle_timeout: Duration::from_secs(600),
                    });

                    pool.register_runtime(Arc::new(BenchRuntime::new(RuntimeType::Claude)))
                        .await
                        .unwrap();

                    // Acquire multiple agents
                    let mut agents = vec![];
                    for _ in 0..count {
                        let config = AgentAcquireConfig::new(RuntimeType::Claude)
                            .with_timeout(Duration::from_secs(60));
                        let agent = pool.acquire(config).await.unwrap();
                        agents.push(agent);
                    }

                    // Release all agents
                    for agent in agents {
                        pool.release(agent, false).await.unwrap();
                    }
                });
            },
        );
    }

    group.finish();
}

/// Measure Agent reuse performance
fn bench_agent_pool_reuse(c: &mut Criterion) {
    c.bench_function("agent_pool_reuse", |b| {
        b.to_async(rt()).iter(|| async {
            let pool = AgentPool::new(PoolConfig::default());
            pool.register_runtime(Arc::new(BenchRuntime::new(RuntimeType::Claude)))
                .await
                .unwrap();

            // Acquire agent
            let config = AgentAcquireConfig::new(RuntimeType::Claude)
                .with_timeout(Duration::from_secs(60));
            let agent1 = pool.acquire(config.clone()).await.unwrap();
            let agent_id = agent1.agent_id().to_string();
            pool.release(agent1, true).await.unwrap();

            // Reuse same agent
            let config2 =
                AgentAcquireConfig::new(RuntimeType::Claude).with_reuse_agent_id(&agent_id);
            let agent2 = pool.acquire(config2).await.unwrap();
            pool.release(agent2, false).await.unwrap();
        });
    });
}

// ============================================================================
// DAG Execution Benchmarks
// ============================================================================

/// Measure DAG execution performance with varying task counts
fn bench_dag_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_execution");
    group.sample_size(10); // Reduce sample size due to longer execution time

    for task_count in [1, 5, 10, 20].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(task_count),
            task_count,
            |b, &count| {
                b.to_async(rt()).iter(|| async move {
                    let scheduler = DagScheduler::new();
                    let pool = AgentPool::new(PoolConfig {
                        max_agents: count,
                        default_timeout: Duration::from_secs(300),
                        health_check_interval: Duration::from_secs(30),
                        auto_cleanup: false,
                        idle_timeout: Duration::from_secs(600),
                    });
                    pool.register_runtime(Arc::new(BenchRuntime::new(RuntimeType::Claude)))
                        .await
                        .unwrap();

                    let executor = MultiAgentDagExecutor::new(
                        scheduler,
                        pool,
                        MultiAgentExecutorConfig::default(),
                    )
                    .unwrap();

                    // Create linear DAG
                    let mut dag = TaskDag::new();
                    let mut prev_task = None;

                    for i in 0..count {
                        let task_id = format!("task-{}", i);
                        let deps = prev_task.map(|p| vec![p]).unwrap_or_default();
                        dag.add_node(task_id.clone(), deps).unwrap();
                        prev_task = Some(task_id);
                    }

                    let run_id = executor.create_run(dag).await.unwrap();
                    let _report = executor.execute(&run_id).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

/// Measure parallel task execution performance
fn bench_parallel_tasks(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_tasks");
    group.sample_size(10);

    for concurrency in [1, 2, 4, 8].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &count| {
                b.to_async(rt()).iter(|| async move {
                    let scheduler = DagScheduler::new();
                    let pool = AgentPool::new(PoolConfig {
                        max_agents: count,
                        default_timeout: Duration::from_secs(300),
                        health_check_interval: Duration::from_secs(30),
                        auto_cleanup: false,
                        idle_timeout: Duration::from_secs(600),
                    });
                    pool.register_runtime(Arc::new(BenchRuntime::new(RuntimeType::Claude)))
                        .await
                        .unwrap();

                    let executor = MultiAgentDagExecutor::new(
                        scheduler,
                        pool,
                        MultiAgentExecutorConfig::new().with_max_concurrent(count),
                    )
                    .unwrap();

                    // Create parallel DAG (no dependencies)
                    let mut dag = TaskDag::new();
                    for i in 0..count {
                        dag.add_node(format!("task-{}", i), vec![]).unwrap();
                    }

                    let run_id = executor.create_run(dag).await.unwrap();
                    let _report = executor.execute(&run_id).await.unwrap();
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Task Execution Benchmarks
// ============================================================================

/// Measure task execution latency
fn bench_task_execution_latency(c: &mut Criterion) {
    c.bench_function("task_execution_latency", |b| {
        b.to_async(rt()).iter(|| async {
            let pool = AgentPool::new(PoolConfig::default());
            pool.register_runtime(Arc::new(BenchRuntime::new(RuntimeType::Claude)))
                .await
                .unwrap();

            let config = AgentAcquireConfig::new(RuntimeType::Claude)
                .with_timeout(Duration::from_secs(60));

            let agent = pool.acquire(config).await.unwrap();

            let task = TaskRequest::new("bench-task", "Benchmark task execution");

            let _result = agent.execute(task).await.unwrap();

            pool.release(agent, false).await.unwrap();
        });
    });
}

/// Measure context injection performance
fn bench_context_injection(c: &mut Criterion) {
    c.bench_function("context_injection", |b| {
        b.to_async(rt()).iter(|| async {
            // Use a temporary directory for the context store
            let temp_dir = std::env::temp_dir().join(format!("cis-bench-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir).unwrap();
            let context_store = ContextStore::new(temp_dir.join("context.db")).unwrap();

            let run_id = "test-run";

            // Simulate saving multiple upstream task outputs
            for i in 0..10 {
                context_store
                    .save(run_id, &format!("task-{}", i), &"A".repeat(1000), Some(0))
                    .await
                    .unwrap();
            }

            // Build context (simulating what happens in multi-agent executor)
            let mut context = String::new();
            for i in 0..10 {
                let output = context_store
                    .load(run_id, &format!("task-{}", i))
                    .await
                    .unwrap();
                context.push_str(&output);
            }

            black_box(context);

            // Cleanup
            let _ = std::fs::remove_dir_all(&temp_dir);
        });
    });
}

// ============================================================================
// Performance Report Generation
// ============================================================================

/// Performance measurement data structure
#[derive(Debug, Default)]
pub struct Measurements {
    pub timestamp: String,
    pub cpu_info: String,
    pub memory_info: String,
    pub agent_acquire_time: Duration,
    pub agent_release_time: Duration,
    pub reuse_hit_rate: f64,
    pub serial_10_time: Duration,
    pub parallel_10_time: Duration,
    pub speedup_10: f64,
    pub serial_20_time: Duration,
    pub parallel_20_time: Duration,
    pub speedup_20: f64,
    pub avg_latency: Duration,
    pub p99_latency: Duration,
    pub throughput: f64,
}

/// Generate performance test report
pub fn generate_report(measurements: &Measurements) -> String {
    format!(
        r#"
# Agent Teams Performance Benchmark Report

## Environment Information
- Test Time: {}
- CPU: {}
- Memory: {}

## Key Metrics

### Agent Pool
| Metric | Value |
|--------|-------|
| Agent Acquire Time | {:?} |
| Agent Release Time | {:?} |
| Reuse Hit Rate | {:.1}% |

### DAG Execution
| Task Count | Serial Time | Parallel (4) Time | Speedup |
|------------|-------------|-------------------|---------|
| 10 | {:?} | {:?} | {:.2}x |
| 20 | {:?} | {:?} | {:.2}x |

### Task Execution
| Metric | Value |
|--------|-------|
| Average Latency | {:?} |
| P99 Latency | {:?} |
| Throughput | {:.1} tasks/sec |

## Notes
- All times are in milliseconds unless otherwise noted
- Benchmarks use mock agents with 10ms simulated processing time
- Parallel execution uses concurrent task processing
- Context injection measures upstream output retrieval
"#,
        measurements.timestamp,
        measurements.cpu_info,
        measurements.memory_info,
        measurements.agent_acquire_time,
        measurements.agent_release_time,
        measurements.reuse_hit_rate,
        measurements.serial_10_time,
        measurements.parallel_10_time,
        measurements.speedup_10,
        measurements.serial_20_time,
        measurements.parallel_20_time,
        measurements.speedup_20,
        measurements.avg_latency,
        measurements.p99_latency,
        measurements.throughput,
    )
}

// ============================================================================
// Criterion Benchmark Groups
// ============================================================================

criterion_group!(
    agent_pool_benches,
    bench_agent_pool_acquire_release,
    bench_agent_pool_reuse,
);

criterion_group!(
    dag_execution_benches,
    bench_dag_execution,
    bench_parallel_tasks,
);

criterion_group!(
    task_execution_benches,
    bench_task_execution_latency,
    bench_context_injection,
);

criterion_main!(
    agent_pool_benches,
    dag_execution_benches,
    task_execution_benches,
);
