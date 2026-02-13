//! DAG Operations Performance Benchmarks
//!
//! Measures key performance metrics for DAG operations:
//! - DAG construction (10, 100, 1000 nodes)
//! - Topological sort
//! - Dependency validation
//! - Cycle detection
//!
//! Uses criterion.rs for statistical analysis with warm-up runs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use cis_core::scheduler::{TaskDag, DagError, TaskLevel};
use std::time::Duration;

// ============================================================================
// DAG Generation Helpers
// ============================================================================

/// Generate a linear chain DAG: 1 -> 2 -> 3 -> ... -> n
fn generate_linear_dag(size: usize) -> TaskDag {
    let mut dag = TaskDag::new();

    for i in 0..size {
        let task_id = format!("task-{}", i);
        let deps = if i > 0 {
            vec![format!("task-{}", i - 1)]
        } else {
            vec![]
        };

        dag.add_node_with_level(
            task_id,
            deps,
            TaskLevel::Mechanical { retry: 1 },
        ).unwrap();
    }

    dag
}

/// Generate a tree DAG: root -> [child1, child2, ...] -> grandchildren
fn generate_tree_dag(fan_out: usize, depth: usize) -> TaskDag {
    let mut dag = TaskDag::new();
    let mut node_count = 0;

    fn add_tree_nodes(
        dag: &mut TaskDag,
        parent: Option<String>,
        fan_out: usize,
        depth: usize,
        node_count: &mut usize,
    ) {
        if depth == 0 {
            return;
        }

        let deps = parent.into_iter().collect();
        let start_id = *node_count;

        for i in 0..fan_out {
            let task_id = format!("node-{}-{}", start_id, i);
            dag.add_node_with_level(
                task_id.clone(),
                deps.clone(),
                TaskLevel::Mechanical { retry: 1 },
            ).unwrap();

            let current_id = task_id.clone();
            *node_count += 1;

            add_tree_nodes(dag, Some(current_id), fan_out, depth - 1, node_count);
        }
    }

    add_tree_nodes(&mut dag, None, fan_out, depth, &mut node_count);
    dag
}

/// Generate a diamond DAG: multiple parallel paths merging
fn generate_diamond_dag(width: usize, depth: usize) -> TaskDag {
    let mut dag = TaskDag::new();
    let mut nodes = Vec::new();

    // Generate layers
    for layer in 0..=depth {
        let layer_nodes: Vec<String> = (0..width)
            .map(|i| format!("layer-{}-node-{}", layer, i))
            .collect();

        // Add nodes
        for node in &layer_nodes {
            let deps = if layer == 0 {
                vec![]
            } else {
                // Connect to all nodes in previous layer
                nodes.last().unwrap().clone()
            };

            dag.add_node_with_level(
                node.clone(),
                deps,
                TaskLevel::Mechanical { retry: 1 },
            ).unwrap();
        }

        nodes.push(layer_nodes);
    }

    dag
}

/// Generate a DAG with random dependencies (ensuring no cycles)
fn generate_random_dag(size: usize) -> TaskDag {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let mut dag = TaskDag::new();

    for i in 0..size {
        let task_id = format!("task-{}", i);

        // Random dependencies only from lower-numbered tasks (prevents cycles)
        let num_deps = rng.gen_range(0..=std::cmp::min(3, i));
        let deps: Vec<String> = if i > 0 && num_deps > 0 {
            (0..num_deps)
                .map(|_| format!("task-{}", rng.gen_range(0..i)))
                .collect()
        } else {
            vec![]
        };

        dag.add_node_with_level(
            task_id,
            deps,
            TaskLevel::Mechanical { retry: 1 },
        ).unwrap();
    }

    dag
}

/// Count total nodes in DAG
fn count_dag_nodes(dag: &TaskDag) -> usize {
    // Since TaskDag doesn't expose a public count method, we validate
    // and infer from successful operations
    dag.validate().unwrap();
    // We'll return the size we used to create it
    0 // Placeholder - actual count tracked in benchmarks
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark DAG construction - linear chain
fn bench_linear_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_construct_linear");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(1));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut dag = TaskDag::new();
                for i in 0..size {
                    let task_id = format!("task-{}", i);
                    let deps = if i > 0 {
                        vec![format!("task-{}", i - 1)]
                    } else {
                        vec![]
                    };

                    black_box(
                        dag.add_node_with_level(
                            task_id,
                            deps,
                            TaskLevel::Mechanical { retry: 1 },
                        )
                    ).unwrap();
                }
                black_box(dag);
            });
        });
    }

    group.finish();
}

/// Benchmark DAG construction - tree structure
fn bench_tree_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_construct_tree");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    // Test different fan-out and depth combinations
    let configs = [
        (2, 5),   // Binary tree, depth 5
        (3, 4),   // 3-ary tree, depth 4
        (5, 3),   // 5-ary tree, depth 3
        (10, 2),  // 10-ary tree, depth 2
    ];

    for (fan_out, depth) in configs.iter() {
        let expected_nodes = (fan_out.pow(depth as u32 + 1) - 1) / (fan_out - 1);
        group.throughput(Throughput::Elements(expected_nodes as u64));
        group.bench_with_input(
            BenchmarkId::new("fan_out", fan_out),
            &(*fan_out, *depth),
            |b, &(fan_out, depth)| {
                b.iter(|| {
                    let mut dag = TaskDag::new();
                    let mut node_count = 0;

                    fn add_tree_nodes(
                        dag: &mut TaskDag,
                        parent: Option<String>,
                        fan_out: usize,
                        depth: usize,
                        node_count: &mut usize,
                    ) {
                        if depth == 0 {
                            return;
                        }

                        let deps = parent.into_iter().collect();
                        let start_id = *node_count;

                        for i in 0..fan_out {
                            let task_id = format!("node-{}-{}", start_id, i);
                            dag.add_node_with_level(
                                task_id.clone(),
                                deps.clone(),
                                TaskLevel::Mechanical { retry: 1 },
                            ).unwrap();

                            let current_id = task_id.clone();
                            *node_count += 1;

                            add_tree_nodes(dag, Some(current_id), fan_out, depth - 1, node_count);
                        }
                    }

                    add_tree_nodes(&mut dag, None, fan_out, depth, &mut node_count);
                    black_box(dag);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark DAG construction - random dependencies
fn bench_random_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_construct_random");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(1));

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                use rand::Rng;
                let mut rng = rand::thread_rng();
                let mut dag = TaskDag::new();

                for i in 0..size {
                    let task_id = format!("task-{}", i);
                    let num_deps = rng.gen_range(0..=std::cmp::min(3, i));
                    let deps: Vec<String> = if i > 0 && num_deps > 0 {
                        (0..num_deps)
                            .map(|_| format!("task-{}", rng.gen_range(0..i)))
                            .collect()
                    } else {
                        vec![]
                    };

                    black_box(
                        dag.add_node_with_level(
                            task_id,
                            deps,
                            TaskLevel::Mechanical { retry: 1 },
                        )
                    ).unwrap();
                }

                black_box(dag);
            });
        });
    }

    group.finish();
}

/// Benchmark topological sort performance
fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_topological_sort");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let dag = generate_linear_dag(size);
            b.iter(|| {
                black_box(dag.get_execution_order()).unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark dependency validation (cycle detection)
fn bench_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_validation");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let dag = generate_random_dag(size);
            b.iter(|| {
                black_box(dag.validate()).unwrap();
            });
        });
    }

    // Benchmark cycle detection (should fail fast)
    group.bench_function("cycle_detection", |b| {
        b.iter(|| {
            let mut dag = TaskDag::new();

            // Create a cycle: A -> B -> C -> A
            dag.add_node("task-a".to_string(), vec!["task-c".to_string()]).unwrap();
            dag.add_node("task-b".to_string(), vec!["task-a".to_string()]).unwrap();
            dag.add_node("task-c".to_string(), vec!["task-b".to_string()]).unwrap();

            black_box(dag.validate());
        });
    });

    group.finish();
}

/// Benchmark complex DAG with diamond dependencies
fn bench_diamond_dag(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_diamond");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    let configs = [
        (5, 3),   // 5 nodes per layer, 3 layers
        (10, 3),  // 10 nodes per layer, 3 layers
        (20, 2),  // 20 nodes per layer, 2 layers
    ];

    for (width, depth) in configs.iter() {
        let expected_nodes = width * (depth + 1);
        group.throughput(Throughput::Elements(expected_nodes as u64));
        group.bench_with_input(
            BenchmarkId::new("construct", width),
            &(*width, *depth),
            |b, &(width, depth)| {
                b.iter(|| {
                    let mut dag = TaskDag::new();
                    let mut nodes = Vec::new();

                    for layer in 0..=depth {
                        let layer_nodes: Vec<String> = (0..width)
                            .map(|i| format!("layer-{}-node-{}", layer, i))
                            .collect();

                        for node in &layer_nodes {
                            let deps = if layer == 0 {
                                vec![]
                            } else {
                                nodes.last().unwrap().clone()
                            };

                            dag.add_node_with_level(
                                node.clone(),
                                deps,
                                TaskLevel::Mechanical { retry: 1 },
                            ).unwrap();
                        }

                        nodes.push(layer_nodes);
                    }

                    black_box(dag);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark batch node addition
fn bench_batch_addition(c: &mut Criterion) {
    let mut group = c.benchmark_group("dag_batch_add");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let mut dag = TaskDag::new();
                let tasks: Vec<(String, Vec<String>)> = (0..size)
                    .map(|i| {
                        (
                            format!("task-{}", i),
                            if i > 0 { vec![format!("task-{}", i - 1)] } else { vec![] },
                        )
                    })
                    .collect();

                black_box(dag.add_nodes(tasks)).unwrap();
            });
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = dag_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(100);
    targets =
        bench_linear_construction,
        bench_tree_construction,
        bench_random_construction,
        bench_topological_sort,
        bench_validation,
        bench_diamond_dag,
        bench_batch_addition
);

criterion_main!(dag_benches);
