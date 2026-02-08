//! Benchmark for Skill Dispatch and Routing
//!
//! Tests the performance of skill-related operations including:
//! - Skill registry operations
//! - Chain orchestration
//! - Skill routing
//! - Manifest parsing

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_skill_manifest_parsing(c: &mut Criterion) {
    use cis_core::skill::{ManifestValidator, SkillManifest};

    let mut group = c.benchmark_group("skill_manifest");

    let simple_manifest = r#"
name = "test-skill"
version = "1.0.0"
description = "A test skill"

[skill]
type = "native"
entry = "main"

[permissions]
memory = "read-write"
network = false
"#;

    let complex_manifest = r#"
name = "complex-skill"
version = "2.1.0"
description = "A complex skill with many dependencies"
author = "Test Author"
license = "MIT"

[skill]
type = "wasm"
entry = "skill.wasm"
wasm_bytes = "skill.wasm"

[permissions]
memory = "read-write"
network = true
filesystem = ["/tmp", "/var/data"]

[dependencies]
serde = "1.0"
tokio = "1.0"
anyhow = "1.0"

[config]
required = ["api_key", "endpoint"]
optional = ["timeout", "retries"]

[matrix]
room_id = "!test:cis.local"
federate = false
"#;

    group.bench_function("parse_simple_manifest", |b| {
        b.iter(|| {
            let result: Result<SkillManifest, _> = toml::from_str(black_box(simple_manifest));
            black_box(result);
        });
    });

    group.bench_function("parse_complex_manifest", |b| {
        b.iter(|| {
            let result: Result<SkillManifest, _> = toml::from_str(black_box(complex_manifest));
            black_box(result);
        });
    });

    group.bench_function("validate_manifest", |b| {
        let manifest: SkillManifest = toml::from_str(simple_manifest).unwrap();
        b.iter(|| {
            let result = ManifestValidator::validate(black_box(&manifest));
            let _ = black_box(result);
        });
    });

    group.finish();
}

fn bench_skill_compatibility(c: &mut Criterion) {
    use cis_core::skill::SkillCompatibilityRecord;

    let mut group = c.benchmark_group("skill_compatibility");

    group.bench_function("create_compatibility_record", |b| {
        b.iter(|| {
            let record = SkillCompatibilityRecord {
                source_skill_id: black_box("skill-a".to_string()),
                target_skill_id: black_box("skill-b".to_string()),
                compatibility_score: black_box(0.85),
                data_flow_types: black_box("json".to_string()),
                discovered_at: black_box(chrono::Utc::now().timestamp()),
            };
            black_box(record);
        });
    });

    group.finish();
}

fn bench_skill_chain(c: &mut Criterion) {
    use cis_core::skill::{ChainStep, StepResult};
    use serde_json::Value;

    let mut group = c.benchmark_group("skill_chain");

    group.bench_function("create_chain_step", |b| {
        b.iter(|| {
            let step = ChainStep {
                skill_id: black_box("test-skill".to_string()),
                input_mapping: vec![(black_box("input1".to_string()), black_box("param1".to_string()))],
                condition: None,
                max_retries: 3,
                timeout_secs: 60,
            };
            black_box(step);
        });
    });

    group.bench_function("create_step_result", |b| {
        b.iter(|| {
            let result = StepResult {
                step_index: black_box(0),
                skill_id: black_box("test-skill".to_string()),
                output: Value::String(black_box("output".to_string())),
                success: true,
                execution_time_ms: 100,
            };
            black_box(result);
        });
    });

    group.finish();
}

fn bench_skill_semantics(c: &mut Criterion) {
    use cis_core::skill::{
        SkillIoSignature, SkillScope, SkillSemanticDescription, SkillSemanticMatcher,
        SkillSemanticsExt,
    };

    let mut group = c.benchmark_group("skill_semantics");

    group.bench_function("create_semantic_description", |b| {
        b.iter(|| {
            let desc = SkillSemanticDescription::new(
                black_box("test-skill"),
                black_box("Test Skill"),
                black_box("Process text data"),
                black_box("Returns processed text"),
            );
            black_box(desc);
        });
    });

    group.bench_function("create_semantics_ext", |b| {
        b.iter(|| {
            let desc = SkillSemanticsExt::new(
                black_box("test-skill"),
                black_box("Test Skill"),
            )
            .with_description(black_box("A test skill"))
            .with_scope(SkillScope::Project);
            black_box(desc);
        });
    });

    group.bench_function("create_io_signature", |b| {
        b.iter(|| {
            let sig = SkillIoSignature::new(
                vec![black_box("text".to_string())],
                vec![black_box("summary".to_string())],
            )
            .with_pipeable(true);
            black_box(sig);
        });
    });

    let desc1 = SkillSemanticDescription::new(
        "skill-a",
        "Skill A",
        "Processes text data and returns summary",
        "Text summarization",
    );

    let desc2 = SkillSemanticDescription::new(
        "skill-b",
        "Skill B",
        "Analyzes text content for sentiment",
        "Sentiment analysis",
    );

    group.bench_function("calculate_similarity", |b| {
        b.iter(|| {
            let similarity =
                SkillSemanticMatcher::calculate_similarity(black_box(&desc1), black_box(&desc2));
            black_box(similarity);
        });
    });

    group.finish();
}

fn bench_skill_dag_builder(c: &mut Criterion) {
    use cis_core::skill::{SkillDagBuilder, SkillDagContext, SkillDagStats};
    use serde_json::Value;

    let mut group = c.benchmark_group("skill_dag_builder");

    group.bench_function("create_dag_builder", |b| {
        b.iter(|| {
            let builder = SkillDagBuilder::new();
            black_box(builder);
        });
    });

    group.bench_function("create_dag_context", |b| {
        b.iter(|| {
            let inputs = Value::Object(serde_json::Map::new());
            let ctx = SkillDagContext::new(black_box(inputs));
            black_box(ctx);
        });
    });

    group.bench_function("create_dag_stats", |b| {
        b.iter(|| {
            let stats = SkillDagStats {
                total_tasks: black_box(10),
                completed_tasks: black_box(8),
                failed_tasks: black_box(1),
                skipped_tasks: black_box(1),
                debt_count: black_box(0),
                total_duration_ms: black_box(5000),
            };
            black_box(stats);
        });
    });

    group.finish();
}

criterion_group!(
    skill_benches,
    bench_skill_manifest_parsing,
    bench_skill_compatibility,
    bench_skill_chain,
    bench_skill_semantics,
    bench_skill_dag_builder
);
criterion_main!(skill_benches);
