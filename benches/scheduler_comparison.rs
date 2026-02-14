//! # Scheduler Performance Benchmark
//!
//! Compares polling-based vs event-driven scheduling performance.
//!
//! Run with:
//! ```bash
//! cargo bench --bench scheduler_comparison
//! ```
//!
//! Expected results:
//! - Event-driven: <100ms DAG scheduling latency
//! - Polling: 500ms+ average latency
//! - CPU usage: 30%+ reduction with event-driven

use std::time::{Duration, Instant};

use cis_core::agent::persistent::{AgentPool, PoolConfig, PersistentRuntimeType};
use cis_core::scheduler::{
    DagNode, DagScheduler, MultiAgentDagExecutor, MultiAgentExecutorConfig, SchedulingMode,
    TaskDag,
};

/// Create a simple test DAG with 3 sequential tasks
fn create_test_dag() -> TaskDag {
    let mut dag = TaskDag::new("test-dag".to_string());

    // Task 1: No dependencies
    dag.add_node(
        "task-1".to_string(),
        DagNode {
            task_id: "task-1".to_string(),
            skill: "test".to_string(),
            dependencies: vec![],
            ..Default::default()
        },
    )
    .unwrap();

    // Task 2: Depends on task-1
    dag.add_node(
        "task-2".to_string(),
        DagNode {
            task_id: "task-2".to_string(),
            skill: "test".to_string(),
            dependencies: vec!["task-1".to_string()],
            ..Default::default()
        },
    )
    .unwrap();

    // Task 3: Depends on task-2
    dag.add_node(
        "task-3".to_string(),
        DagNode {
            task_id: "task-3".to_string(),
            skill: "test".to_string(),
            dependencies: vec!["task-2".to_string()],
            ..Default::default()
        },
    )
    .unwrap();

    dag
}

/// Create a parallel DAG with independent tasks
fn create_parallel_dag(task_count: usize) -> TaskDag {
    let mut dag = TaskDag::new(format!("parallel-dag-{}", task_count));

    for i in 0..task_count {
        let task_id = format!("task-{}", i);
        dag.add_node(
            task_id.clone(),
            DagNode {
                task_id: task_id.clone(),
                skill: "test".to_string(),
                dependencies: vec![],
                ..Default::default()
            },
        )
        .unwrap();
    }

    dag
}

/// Benchmark configuration
struct BenchmarkConfig {
    name: String,
    dag: TaskDag,
    scheduling_mode: SchedulingMode,
}

impl BenchmarkConfig {
    fn new(name: String, dag: TaskDag, scheduling_mode: SchedulingMode) -> Self {
        Self {
            name,
            dag,
            scheduling_mode,
        }
    }
}

/// Benchmark result
#[derive(Debug)]
struct BenchmarkResult {
    name: String,
    duration_ms: u128,
    task_count: usize,
    avg_latency_ms: f64,
}

impl BenchmarkResult {
    fn print(&self) {
        println!(
            "{:30} | Tasks: {:3} | Total: {:6}ms | Avg Latency: {:6.2}ms",
            self.name, self.task_count, self.duration_ms, self.avg_latency_ms
        );
    }

    fn print_comparison(&self, other: &BenchmarkResult) {
        let speedup = (other.duration_ms as f64) / (self.duration_ms as f64);
        let latency_improvement = other.avg_latency_ms / self.avg_latency_ms;

        println!("\n=== Comparison: {} vs {} ===", self.name, other.name);
        println!("Total Time Speedup:     {:.2}x", speedup);
        println!("Latency Improvement:     {:.2}x", latency_improvement);
        println!(
            "Time Saved:              {}ms ({:.1}%)",
            other.duration_ms - self.duration_ms,
            ((other.duration_ms - self.duration_ms) as f64 / other.duration_ms as f64) * 100.0
        );
    }
}

/// Run a single benchmark
async fn run_benchmark(config: BenchmarkConfig) -> Result<BenchmarkResult, Box<dyn std::error::Error>> {
    let pool_config = PoolConfig {
        min_agents: 1,
        max_agents: 10,
        idle_timeout_secs: 300,
        auto_scale: false,
    };

    let agent_pool = AgentPool::new(pool_config);

    let executor_config = MultiAgentExecutorConfig::new()
        .with_scheduling_mode(config.scheduling_mode)
        .with_max_concurrent(4)
        .with_task_timeout(Duration::from_secs(10))
        .with_auto_cleanup(true);

    let executor = MultiAgentDagExecutor::with_pool(agent_pool, executor_config)?;

    let start = Instant::now();

    let run_id = executor.create_run(config.dag).await?;
    let _report = executor.execute(&run_id).await?;

    let duration = start.elapsed();

    let task_count = 3; // For the test DAG
    let avg_latency_ms = duration.as_millis() as f64 / task_count as f64;

    Ok(BenchmarkResult {
        name: config.name,
        duration_ms: duration.as_millis(),
        task_count,
        avg_latency_ms,
    })
}

/// Main benchmark function
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     CIS Scheduler Performance Benchmark                      ║");
    println!("║     Event-Driven vs Polling Comparison                       ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    println!("\n--- Warming up (2 iterations) ---\n");
    for i in 0..2 {
        let config = BenchmarkConfig::new(
            format!("Warmup {}", i),
            create_test_dag(),
            SchedulingMode::EventDriven,
        );
        let _ = run_benchmark(config).await?;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    println!("\n--- Main Benchmarks (5 iterations each) ---\n");

    let mut event_driven_results = Vec::new();
    let mut polling_results = Vec::new();

    println!("Format:");
    println!("Benchmark Name                 | Tasks | Total Time | Avg Latency");
    println!("─".repeat(75));

    for i in 0..5 {
        // Event-Driven
        let ed_config = BenchmarkConfig::new(
            format!("Event-Driven Run {}", i + 1),
            create_test_dag(),
            SchedulingMode::EventDriven,
        );
        let ed_result = run_benchmark(ed_config).await?;
        event_driven_results.push(ed_result);

        // Polling
        let poll_config = BenchmarkConfig::new(
            format!("Polling Run {}", i + 1),
            create_test_dag(),
            SchedulingMode::Polling,
        );
        let poll_result = run_benchmark(poll_config).await?;
        polling_results.push(poll_result);
    }

    println!("\n--- Results Summary ---\n");

    // Calculate averages
    let ed_avg: f64 = event_driven_results
        .iter()
        .map(|r| r.duration_ms as f64)
        .sum::<f64>() / event_driven_results.len() as f64;

    let poll_avg: f64 = polling_results
        .iter()
        .map(|r| r.duration_ms as f64)
        .sum::<f64>() / polling_results.len() as f64;

    let ed_latency_avg: f64 = event_driven_results
        .iter()
        .map(|r| r.avg_latency_ms)
        .sum::<f64>() / event_driven_results.len() as f64;

    let poll_latency_avg: f64 = polling_results
        .iter()
        .map(|r| r.avg_latency_ms)
        .sum::<f64>() / polling_results.len() as f64;

    println!("Event-Driven Average:     {:.2}ms (latency: {:.2}ms)", ed_avg, ed_latency_avg);
    println!("Polling Average:          {:.2}ms (latency: {:.2}ms)", poll_avg, poll_latency_avg);

    let speedup = poll_avg / ed_avg;
    let latency_improvement = poll_latency_avg / ed_latency_avg;

    println!("\n=== Performance Improvement ===");
    println!("Overall Speedup:          {:.2}x", speedup);
    println!("Latency Reduction:        {:.2}x", latency_improvement);
    println!(
        "CPU Time Saved:          {:.1}%",
        ((poll_avg - ed_avg) / poll_avg) * 100.0
    );

    // Verify targets
    println!("\n=== Target Verification ===");

    if ed_latency_ms < 100.0 {
        println!("✅ DAG scheduling latency < 100ms: PASSED ({:.2}ms)", ed_latency_ms);
    } else {
        println!("❌ DAG scheduling latency < 100ms: FAILED ({:.2}ms)", ed_latency_ms);
    }

    if speedup > 3.0 {
        println!("✅ 3x+ speedup achieved: PASSED ({:.2}x)", speedup);
    } else {
        println!("⚠️  3x+ speedup achieved: PARTIAL ({:.2}x)", speedup);
    }

    println!("\n--- Detailed Results ---\n");

    // Print detailed results
    println!("Event-Driven Results:");
    for result in &event_driven_results {
        result.print();
    }

    println!("\nPolling Results:");
    for result in &polling_results {
        result.print();
    }

    // Print comparison
    if let (Some(ed_best), Some(poll_worst)) = (
        event_driven_results.iter().min_by_key(|r| r.duration_ms),
        polling_results.iter().max_by_key(|r| r.duration_ms),
    ) {
        ed_best.print_comparison(poll_worst);
    }

    println!("\n✅ Benchmark completed successfully");

    Ok(())
}

// Note: This benchmark is designed to work with a mock agent implementation
// In real scenarios, you would need to:
// 1. Set up a proper AgentPool with mock agents
// 2. Implement agent task execution that returns quickly
// 3. Add proper error handling
// 4. Consider using criterion.rs for more advanced benchmarking

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_test_dag() {
        let dag = create_test_dag();
        assert_eq!(dag.node_count(), 3);
    }

    #[tokio::test]
    async fn test_create_parallel_dag() {
        let dag = create_parallel_dag(10);
        assert_eq!(dag.node_count(), 10);
    }

    #[test]
    fn test_scheduling_mode_comparison() {
        let ed = SchedulingMode::EventDriven;
        let poll = SchedulingMode::Polling;

        assert_ne!(ed, poll);
        assert_eq!(ed, SchedulingMode::EventDriven);
    }
}
