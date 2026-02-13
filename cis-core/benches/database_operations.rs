//! Database Operations Performance Benchmarks
//!
//! Measures key performance metrics for database operations:
//! - Task insertion (single and batch)
//! - Query operations (simple and complex filters)
//! - Update operations
//! - Delete operations
//!
//! Uses criterion.rs for statistical analysis with warm-up runs.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rusqlite::{Connection, params};
use std::time::Duration;
use tempfile::TempDir;

// ============================================================================
// Database Setup Helpers
// ============================================================================

/// Create a temporary database with tasks table
fn setup_temp_db() -> (Connection, TempDir) {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("bench.db");
    let conn = Connection::open(&db_path).unwrap();

    // Configure for performance
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA temp_store = memory;
         PRAGMA locking_mode = EXCLUSIVE;"
    ).unwrap();

    // Create tasks table
    conn.execute(
        "CREATE TABLE tasks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            description TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            priority INTEGER DEFAULT 2,
            created_at INTEGER NOT NULL
        )",
        [],
    ).unwrap();

    (conn, dir)
}

/// Generate a task with given ID
fn generate_task(id: i32) -> (String, String, String, i32, i64) {
    (
        format!("task-{}", id),
        format!("Task Title {}", id),
        format!("Task Description {}", id),
        id % 5, // priority 0-4
        1704067200 + (id as i64 * 60), // timestamp
    )
}

// ============================================================================
// Benchmarks
// ============================================================================

/// Benchmark single task insertion
fn bench_single_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_insert_single");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("insert_one_task", |b| {
        b.iter(|| {
            let (conn, _dir) = setup_temp_db();
            let (id, title, desc, priority, created) = generate_task(1);

            black_box(
                conn.execute(
                    "INSERT INTO tasks (id, title, description, priority, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![id, title, desc, priority, created],
                )
            ).unwrap();
        });
    });

    group.finish();
}

/// Benchmark batch task insertion with varying sizes
fn bench_batch_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_insert_batch");
    group.warm_up_time(Duration::from_secs(2));
    group.measurement_time(Duration::from_secs(5));
    group.throughput(Throughput::Elements(1));

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let (conn, _dir) = setup_temp_db();

                // Begin transaction for batch insert
                let tx = conn.unchecked_transaction().unwrap();

                for i in 0..size {
                    let (id, title, desc, priority, created) = generate_task(i);
                    black_box(
                        tx.execute(
                            "INSERT INTO tasks (id, title, description, priority, created_at)
                             VALUES (?1, ?2, ?3, ?4, ?5)",
                            params![id, title, desc, priority, created],
                        )
                    ).unwrap();
                }

                tx.commit().unwrap();
            });
        });
    }

    group.finish();
}

/// Benchmark simple query by ID
fn bench_simple_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_query_simple");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    // Prepare database with test data
    let (conn, _dir) = setup_temp_db();
    let tx = conn.unchecked_transaction().unwrap();

    for i in 0..1000 {
        let (id, title, desc, priority, created) = generate_task(i);
        tx.execute(
            "INSERT INTO tasks (id, title, description, priority, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, desc, priority, created],
        ).unwrap();
    }
    tx.commit().unwrap();

    group.bench_function("query_by_id", |b| {
        b.iter(|| {
            let task_id = format!("task-{}", black_box(500));
            black_box(
                conn.query_row(
                    "SELECT * FROM tasks WHERE id = ?1",
                    params![task_id],
                    |_, _| Ok(()),
                )
            ).unwrap();
        });
    });

    group.finish();
}

/// Benchmark complex query with filters
fn bench_complex_query(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_query_complex");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    // Prepare database with test data
    let (conn, _dir) = setup_temp_db();
    let tx = conn.unchecked_transaction().unwrap();

    for i in 0..1000 {
        let (id, title, desc, priority, created) = generate_task(i);
        tx.execute(
            "INSERT INTO tasks (id, title, description, priority, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, desc, priority, created],
        ).unwrap();
    }
    tx.commit().unwrap();

    group.bench_function("query_with_filters", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare_cached(
                "SELECT * FROM tasks
                 WHERE priority >= ?1 AND created_at >= ?2
                 ORDER BY priority DESC, created_at ASC
                 LIMIT 100"
            ).unwrap();

            black_box(
                stmt.query_map(params![2, 1704067200], |_, _| Ok(()))
            ).unwrap();
        });
    });

    group.bench_function("query_with_like", |b| {
        b.iter(|| {
            let mut stmt = conn.prepare_cached(
                "SELECT * FROM tasks
                 WHERE title LIKE ?1 OR description LIKE ?1
                 LIMIT 50"
            ).unwrap();

            black_box(
                stmt.query_map(params!["%Title%"], |_, _| Ok(()))
            ).unwrap();
        });
    });

    group.finish();
}

/// Benchmark update operations
fn bench_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_update");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    // Prepare database with test data
    let (conn, _dir) = setup_temp_db();
    let tx = conn.unchecked_transaction().unwrap();

    for i in 0..1000 {
        let (id, title, desc, priority, created) = generate_task(i);
        tx.execute(
            "INSERT INTO tasks (id, title, description, priority, created_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, 'pending')",
            params![id, title, desc, priority, created],
        ).unwrap();
    }
    tx.commit().unwrap();

    group.bench_function("update_single", |b| {
        b.iter(|| {
            let task_id = format!("task-{}", black_box(500));
            black_box(
                conn.execute(
                    "UPDATE tasks SET status = 'completed', priority = ?1 WHERE id = ?2",
                    params![5, task_id],
                )
            ).unwrap();
        });
    });

    group.bench_function("batch_update", |b| {
        b.iter(|| {
            let tx = conn.unchecked_transaction().unwrap();
            black_box(
                tx.execute(
                    "UPDATE tasks SET status = 'completed' WHERE priority >= 3",
                    [],
                )
            ).unwrap();
            tx.commit().unwrap();
        });
    });

    group.finish();
}

/// Benchmark delete operations
fn bench_delete(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_delete");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    // Prepare database with test data
    let (conn, _dir) = setup_temp_db();
    let tx = conn.unchecked_transaction().unwrap();

    for i in 0..1000 {
        let (id, title, desc, priority, created) = generate_task(i);
        tx.execute(
            "INSERT INTO tasks (id, title, description, priority, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, desc, priority, created],
        ).unwrap();
    }
    tx.commit().unwrap();

    group.bench_function("delete_single", |b| {
        b.iter_batched(
            || {
                // Setup: Insert a test record
                let task_id = format!("task-del-{}", black_box(1));
                conn.execute(
                    "INSERT INTO tasks (id, title, description, priority, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![task_id, "Delete Test", "Test", 1, 1704067200],
                ).unwrap();
                task_id
            },
            |task_id| {
                black_box(
                    conn.execute("DELETE FROM tasks WHERE id = ?1", params![task_id])
                ).unwrap();
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("batch_delete", |b| {
        b.iter_batched(
            || {
                // Setup: Insert batch records
                let tx = conn.unchecked_transaction().unwrap();
                for i in 0..100 {
                    let task_id = format!("task-batch-del-{}", i);
                    tx.execute(
                        "INSERT INTO tasks (id, title, description, priority, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![task_id, "Batch Delete", "Test", 1, 1704067200],
                    ).unwrap();
                }
                tx.commit().unwrap();
            },
            |_| {
                let tx = conn.unchecked_transaction().unwrap();
                black_box(
                    tx.execute(
                        "DELETE FROM tasks WHERE title = 'Batch Delete'",
                        [],
                    )
                ).unwrap();
                tx.commit().unwrap();
            },
            criterion::BatchSize::LargeInput,
        );
    });

    group.finish();
}

/// Benchmark transaction overhead
fn bench_transaction(c: &mut Criterion) {
    let mut group = c.benchmark_group("db_transaction");
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(3));

    group.bench_function("with_transaction", |b| {
        b.iter(|| {
            let (conn, _dir) = setup_temp_db();
            let tx = conn.unchecked_transaction().unwrap();

            for i in 0..10 {
                let (id, title, desc, priority, created) = generate_task(i);
                black_box(
                    tx.execute(
                        "INSERT INTO tasks (id, title, description, priority, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![id, title, desc, priority, created],
                    )
                ).unwrap();
            }

            tx.commit().unwrap();
        });
    });

    group.bench_function("without_transaction", |b| {
        b.iter(|| {
            let (conn, _dir) = setup_temp_db();

            for i in 0..10 {
                let (id, title, desc, priority, created) = generate_task(i);
                black_box(
                    conn.execute(
                        "INSERT INTO tasks (id, title, description, priority, created_at)
                         VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![id, title, desc, priority, created],
                    )
                ).unwrap();
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = db_benches;
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(5))
        .sample_size(100);
    targets =
        bench_single_insert,
        bench_batch_insert,
        bench_simple_query,
        bench_complex_query,
        bench_update,
        bench_delete,
        bench_transaction
);

criterion_main!(db_benches);
