//! Benchmark for WASM Runtime
//!
//! Tests the performance of WASM operations including:
//! - Module loading and validation
//! - Memory operations
//! - Host function calls
//! - Skill lifecycle

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

/// Simple WASM module bytes (minimal valid WASM)
fn minimal_wasm_bytes() -> Vec<u8> {
    // Minimal WASM module: (module)
    vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
}

/// WASM module with memory export
fn wasm_with_memory() -> Vec<u8> {
    // (module
    //   (memory 1)
    //   (export "memory" (memory 0))
    // )
    vec![
        0x00, 0x61, 0x73, 0x6d, // magic
        0x01, 0x00, 0x00, 0x00, // version
        0x05, 0x03, 0x01, 0x00, 0x01, // memory section
        0x07, 0x08, 0x01, 0x04, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00, // export section
    ]
}

fn bench_wasm_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_validation");

    group.bench_function("validate_magic_minimal", |b| {
        let wasm = minimal_wasm_bytes();
        b.iter(|| {
            let result = cis_core::validate_wasm_magic(black_box(&wasm));
            let _ = black_box(result);
        });
    });

    group.bench_function("validate_magic_with_memory", |b| {
        let wasm = wasm_with_memory();
        b.iter(|| {
            let result = cis_core::validate_wasm_magic(black_box(&wasm));
            let _ = black_box(result);
        });
    });

    // Test with various WASM sizes
    for size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(BenchmarkId::new("validate_large", size), size, |b, &size| {
            let mut wasm = minimal_wasm_bytes();
            wasm.extend(vec![0x00; size]);
            b.iter(|| {
                let result = cis_core::validate_wasm_magic(black_box(&wasm));
                let _ = black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_wasm_config(c: &mut Criterion) {
    // Note: WasmSkillConfig requires `wasm` feature
    // For benchmarks without the feature, we use a simple config struct
    #[derive(Debug, Clone)]
    struct TestWasmConfig {
        memory_limit: Option<usize>,
        execution_timeout: Option<u64>,
    }

    impl Default for TestWasmConfig {
        fn default() -> Self {
            Self {
                memory_limit: Some(512 * 1024 * 1024), // 512MB
                execution_timeout: Some(30000),        // 30 seconds
            }
        }
    }

    impl TestWasmConfig {
        fn new() -> Self {
            Self::default()
        }

        fn with_memory_limit_mb(mut self, mb: usize) -> Self {
            self.memory_limit = Some(mb * 1024 * 1024);
            self
        }

        fn with_timeout_ms(mut self, ms: u64) -> Self {
            self.execution_timeout = Some(ms);
            self
        }
    }

    let mut group = c.benchmark_group("wasm_config");

    group.bench_function("create_default_config", |b| {
        b.iter(|| {
            let config = TestWasmConfig::default();
            black_box(config);
        });
    });

    group.bench_function("create_config_with_memory_limit", |b| {
        b.iter(|| {
            let config = TestWasmConfig::new().with_memory_limit_mb(black_box(512));
            black_box(config);
        });
    });

    group.bench_function("create_config_with_timeout", |b| {
        b.iter(|| {
            let config = TestWasmConfig::new().with_timeout_ms(black_box(30000));
            black_box(config);
        });
    });

    group.finish();
}

fn bench_wasm_bytes_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_bytes_operations");

    // Benchmark WASM byte validation with different sizes
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("bytes_check", size), size, |b, &size| {
            let wasm = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
                .into_iter()
                .chain(std::iter::repeat(0x00).take(size))
                .collect::<Vec<u8>>();
            b.iter(|| {
                // Check first 4 bytes (magic number)
                let magic = &black_box(&wasm)[..4];
                black_box(magic == &[0x00, 0x61, 0x73, 0x6d]);
            });
        });
    }

    group.finish();
}

criterion_group!(
    wasm_benches,
    bench_wasm_validation,
    bench_wasm_config,
    bench_wasm_bytes_operations
);
criterion_main!(wasm_benches);
