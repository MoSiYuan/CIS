//! Benchmark for DAG Execution Scheduler
//!
//! Tests the performance of DAG operations including:
//! - Node addition
//! - Cycle detection
//! - Topological sorting
//! - Status transitions

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use cis_core::scheduler::TaskDag;

/// Create a linear chain DAG with n nodes
fn create_linear_dag(n: usize) -> TaskDag {
    let mut dag = TaskDag::new();
    for i in 0..n {
        let prev = if i == 0 {
            vec![]
        } else {
            vec![format!("task_{}", i - 1)]
        };
        dag.add_node(format!("task_{}", i), prev).unwrap();
    }
    dag
}

/// Create a fan-out DAG (one root, n children)
fn create_fan_out_dag(n: usize) -> TaskDag {
    let mut dag = TaskDag::new();
    dag.add_node("root".to_string(), vec![]).unwrap();
    for i in 0..n {
        dag.add_node(format!("task_{}", i), vec!["root".to_string()]).unwrap();
    }
    dag
}

/// Create a diamond DAG (one root, n parallel, one sink)
fn create_diamond_dag(n: usize) -> TaskDag {
    let mut dag = TaskDag::new();
    dag.add_node("root".to_string(), vec![]).unwrap();
    for i in 0..n {
        dag.add_node(format!("mid_{}", i), vec!["root".to_string()]).unwrap();
    }
    let mids: Vec<String> = (0..n).map(|i| format!("mid_{}", i)).collect();
    dag.add_node("sink".to_string(), mids).unwrap();
    dag
}

/// Create a complex DAG with multiple levels and dependencies
fn create_complex_dag(levels: usize, width: usize) -> TaskDag {
    let mut dag = TaskDag::new();
    let mut prev_level: Vec<String> = vec![];

    for level in 0..levels {
        let mut current_level = vec![];
        for i in 0..width {
            let task_id = format!("L{}_T{}", level, i);
            let deps = if prev_level.is_empty() {
                vec![]
            } else {
                // Each task depends on 1-2 tasks from previous level
                let mut deps = vec![prev_level[i % prev_level.len()].clone()];
                if i > 0 && i < prev_level.len() {
                    deps.push(prev_level[i - 1].clone());
                }
                deps
            };
            dag.add_node(task_id.clone(), deps).unwrap();
            current_level.push(task_id);
        }
        prev_level = current_level;
    }
    dag
}

fn bench_dag_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_creation");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("linear", size), size, |b, &size| {
            b.iter(|| {
                let dag = create_linear_dag(black_box(size));
                black_box(dag);
            });
        });

        group.bench_with_input(BenchmarkId::new("fan_out", size), size, |b, &size| {
            b.iter(|| {
                let dag = create_fan_out_dag(black_box(size));
                black_box(dag);
            });
        });

        group.bench_with_input(BenchmarkId::new("diamond", size), size, |b, &size| {
            b.iter(|| {
                let dag = create_diamond_dag(black_box(size));
                black_box(dag);
            });
        });
    }

    group.finish();
}

fn bench_cycle_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("cycle_detection");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("validate", size), size, |b, &size| {
            let dag = create_complex_dag(5, size / 5);
            b.iter(|| {
                let result = dag.validate();
                let _ = black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("topological_sort");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("get_execution_order", size), size, |b, &size| {
            let dag = create_complex_dag(5, size / 5);
            b.iter(|| {
                let result = dag.get_execution_order();
                let _ = black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_status_transitions(c: &mut Criterion) {
    let mut group = c.benchmark_group("status_transitions");

    group.bench_function("mark_completed", |b| {
        let mut dag = create_linear_dag(100);
        dag.initialize();
        let mut task_iter = (0..100).map(|i| format!("task_{}", i)).cycle();
        b.iter(|| {
            let task_id = task_iter.next().unwrap();
            let result = dag.mark_completed(black_box(task_id));
            let _ = black_box(result);
        });
    });

    group.bench_function("mark_failed", |b| {
        let mut dag = create_fan_out_dag(100);
        dag.initialize();
        let mut task_iter = (0..100).map(|i| format!("task_{}", i)).cycle();
        b.iter(|| {
            let task_id = task_iter.next().unwrap();
            let result = dag.mark_failed(black_box(task_id));
            let _ = black_box(result);
        });
    });

    group.bench_function("mark_running", |b| {
        let mut dag = create_fan_out_dag(100);
        dag.initialize();
        let mut task_iter = (0..100).map(|i| format!("task_{}", i)).cycle();
        b.iter(|| {
            let task_id = task_iter.next().unwrap();
            let result = dag.mark_running(black_box(task_id));
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn bench_get_ready_tasks(c: &mut Criterion) {
    let mut group = c.benchmark_group("get_ready_tasks");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::new("ready_tasks", size), size, |b, &size| {
            let mut dag = create_fan_out_dag(size);
            dag.initialize();
            b.iter(|| {
                let result = dag.get_ready_tasks();
                black_box(result);
            });
        });
    }

    group.finish();
}

criterion_group!(
    dag_benches,
    bench_dag_creation,
    bench_cycle_detection,
    bench_topological_sort,
    bench_status_transitions,
    bench_get_ready_tasks
);
criterion_main!(dag_benches);
