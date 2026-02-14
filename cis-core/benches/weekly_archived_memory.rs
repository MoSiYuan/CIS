//! WeeklyArchivedMemory Performance Benchmarks
//!
//! Measures key performance metrics for the weekly archived memory system:
//! - Write operations (single and batch)
//! - Vector search with embeddings
//! - Index lookup and retrieval
//! - Cross-week queries
//!
//! Uses criterion.rs for statistical analysis with warm-up runs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use cis_core::memory::weekly_archived::{WeeklyArchivedMemory, IndexStrategy};
use cis_core::types::{MemoryCategory, MemoryDomain};
use std::time::Duration;
use tempfile::TempDir;
use tokio::runtime::Runtime;

// ============================================================================
// Runtime and Setup Helpers
// ============================================================================

/// Create Tokio runtime for async benchmarks
fn rt() -> Runtime {
    tokio::runtime::Runtime::new().unwrap()
}

/// Setup a temporary WeeklyArchivedMemory instance
fn setup_memory(max_weeks: usize) -> (WeeklyArchivedMemory, TempDir) {
    let dir = TempDir::new().unwrap();
    let memory = WeeklyArchivedMemory::new(dir.path().to_path_buf(), max_weeks)
        .unwrap();
    (memory, dir)
}

/// Generate test memory value
fn generate_test_value(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

/// Generate memory key
fn generate_memory_key(prefix: &str, id: usize) -> String {
    format!("{}{}/memory-{}", prefix, id)
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark single write operation
fn bench_single_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_write_single");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("write_small", |b| {
        let rt = rt();
        let (memory, _dir) = setup_memory(54);

        b.iter(|| {
            rt.block_on(async {
                let key = generate_memory_key("test/", black_box(1));
                let value = generate_test_value(1024); // 1KB

                black_box(
                    memory.set(
                        &key,
                        &value,
                        MemoryDomain::Public,
                        MemoryCategory::Context,
                    ).await
                ).unwrap();
            });
        });
    });

    group.bench_function("write_large", |b| {
        let rt = rt();
        let (memory, _dir) = setup_memory(54);

        b.iter(|| {
            rt.block_on(async {
                let key = generate_memory_key("test/", black_box(1));
                let value = generate_test_value(10240); // 10KB

                black_box(
                    memory.set(
                        &key,
                        &value,
                        MemoryDomain::Public,
                        MemoryCategory::Context,
                    ).await
                ).unwrap();
            });
        });
    });

    group.finish();
}

/// Benchmark batch write operations
fn bench_batch_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_write_batch");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rt = rt();
            let (memory, _dir) = setup_memory(54);

            b.iter(|| {
                rt.block_on(async {
                    for i in 0..size {
                        let key = generate_memory_key("batch/", i);
                        let value = generate_test_value(1024);

                        black_box(
                            memory.set(
                                &key,
                                &value,
                                MemoryDomain::Public,
                                MemoryCategory::Context,
                            ).await
                        ).unwrap();
                    }
                });
            });
        });
    }

    group.finish();
}

/// Benchmark read operation (key-based lookup)
fn bench_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_read");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let rt = rt();
    let (memory, _dir) = setup_memory(54);

    // Pre-populate with test data
    rt.block_on(async {
        for i in 0..100 {
            let key = generate_memory_key("read_test/", i);
            let value = generate_test_value(1024);
            memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
        }
    });

    group.bench_function("read_by_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let key = generate_memory_key("read_test/", black_box(50));
                black_box(memory.get(&key).await).unwrap();
            });
        });
    });

    group.finish();
}

/// Benchmark vector search with embeddings
fn bench_vector_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_vector_search");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    for size in [100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let rt = rt();
            let (memory, _dir) = setup_memory(54);

            // Pre-populate with test data
            rt.block_on(async {
                for i in 0..size {
                    let key = generate_memory_key("search_test/", i);
                    let value = generate_test_value(512);
                    memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
                }
            });

            b.iter(|| {
                rt.block_on(async {
                    // Search with pattern matching
                    let results = memory.search(
                        black_box("search_test/"),
                        MemoryDomain::Public,
                        Some(MemoryCategory::Context),
                        black_box(10),
                    ).await.unwrap();

                    black_box(results);
                });
            });
        });
    }

    group.finish();
}

/// Benchmark index lookup performance
fn bench_index_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_index_lookup");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let rt = rt();
    let (memory, _dir) = setup_memory(54);

    // Pre-populate with indexed data
    rt.block_on(async {
        // User preferences (should be indexed)
        for i in 0..50 {
            let key = format!("user/preference/setting-{}", i);
            let value = generate_test_value(256);
            memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
        }

        // Project configs (should be indexed)
        for i in 0..50 {
            let key = format!("project/proj-{}/config", i);
            let value = generate_test_value(512);
            memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
        }
    });

    group.bench_function("lookup_user_preference", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = memory.search(
                    "user/preference/",
                    MemoryDomain::Public,
                    Some(MemoryCategory::Context),
                    10,
                ).await.unwrap();

                black_box(results);
            });
        });
    });

    group.bench_function("lookup_project_config", |b| {
        b.iter(|| {
            rt.block_on(async {
                let results = memory.search(
                    "project/",
                    MemoryDomain::Public,
                    Some(MemoryCategory::Context),
                    10,
                ).await.unwrap();

                black_box(results);
            });
        });
    });

    group.finish();
}

/// Benchmark cross-week queries
fn bench_cross_week_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_cross_week_query");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    let rt = rt();
    let (memory, _dir) = setup_memory(54);

    // This benchmark simulates querying across different weeks
    // Note: In real scenario, this would require time manipulation or multiple weeks of data

    group.bench_function("query_current_week", |b| {
        rt.block_on(async {
            // Insert data in current week
            for i in 0..100 {
                let key = generate_memory_key("week_current/", i);
                let value = generate_test_value(512);
                memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
            }
        });

        b.iter(|| {
            rt.block_on(async {
                let results = memory.search(
                    "week_current/",
                    MemoryDomain::Public,
                    Some(MemoryCategory::Context),
                    20,
                ).await.unwrap();

                black_box(results);
            });
        });
    });

    group.finish();
}

/// Benchmark memory classification (indexing decision)
fn bench_classification(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_classification");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let rt = rt();
    let (memory, _dir) = setup_memory(54);

    group.bench_function("classify_user_preference", |b| {
        b.iter(|| {
            let key = black_box("user/preference/theme");
            let _ = memory.classify_entry(key, MemoryDomain::Public, MemoryCategory::Context);
        });
    });

    group.bench_function("classify_sensitive", |b| {
        b.iter(|| {
            let key = black_box("secret/api_key");
            let _ = memory.classify_entry(key, MemoryDomain::Private, MemoryCategory::Credential);
        });
    });

    group.bench_function("classify_project_config", |b| {
        b.iter(|| {
            let key = black_box("project/my-app/database-config");
            let _ = memory.classify_entry(key, MemoryDomain::Public, MemoryCategory::Context);
        });
    });

    group.bench_function("classify_temporary", |b| {
        b.iter(|| {
            let key = black_box("temp/cache/data");
            let _ = memory.classify_entry(key, MemoryDomain::Private, MemoryCategory::Context);
        });
    });

    group.finish();
}

/// Benchmark mixed operations (read/write/search pattern)
fn bench_mixed_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_mixed_ops");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));

    let rt = rt();
    let (memory, _dir) = setup_memory(54);

    group.bench_function("write_read_search_cycle", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Write
                let key = generate_memory_key("mixed/", black_box(1));
                let value = generate_test_value(1024);
                memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await.unwrap();

                // Read
                let _ = memory.get(&key).await.unwrap();

                // Search
                let _ = memory.search("mixed/", MemoryDomain::Public, Some(MemoryCategory::Context), 5).await.unwrap();
            });
        });
    });

    group.finish();
}

/// Benchmark domain-specific operations
fn bench_domain_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_domain_ops");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let rt = rt();
    let (memory, _dir) = setup_memory(54);

    group.bench_function("public_domain_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                let key = generate_memory_key("public/", black_box(1));
                let value = generate_test_value(512);
                black_box(
                    memory.set(&key, &value, MemoryDomain::Public, MemoryCategory::Context).await
                ).unwrap();
            });
        });
    });

    group.bench_function("private_domain_write", |b| {
        b.iter(|| {
            rt.block_on(async {
                let key = generate_memory_key("private/", black_box(1));
                let value = generate_test_value(512);
                black_box(
                    memory.set(&key, &value, MemoryDomain::Private, MemoryCategory::Context).await
                ).unwrap();
            });
        });
    });

    group.finish();
}

/// Benchmark indexing strategy performance
fn bench_index_strategy(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_index_strategy");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    let strategy = IndexStrategy::default();

    group.bench_function("should_index_user_preference", |b| {
        use cis_core::memory::weekly_archived::IndexType;
        b.iter(|| {
            black_box(strategy.allowed_types.contains(&IndexType::UserPreference));
        });
    });

    group.bench_function("importance_calculation", |b| {
        b.iter(|| {
            let score = black_box(0.8);
            black_box(score >= strategy.min_importance);
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = memory_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(50);
    targets =
        bench_single_write,
        bench_batch_write,
        bench_read,
        bench_vector_search,
        bench_index_lookup,
        bench_cross_week_query,
        bench_classification,
        bench_mixed_operations,
        bench_domain_operations,
        bench_index_strategy
);

criterion_main!(memory_benches);
