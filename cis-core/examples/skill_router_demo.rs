//! Skill Vector Router 和 Chain Orchestrator 演示
//!
//! 演示 CVI-009 和 CVI-010 的功能：
//! - 自然语言意图路由
//! - 技能链自动发现
//! - 技能兼容性自动发现

use std::sync::Arc;

use cis_core::skill::{
    ChainOrchestrator, ChainTemplates, SkillIoSignature, SkillSemanticsExt,
    SkillVectorRouter, SkillManager,
};
use cis_core::vector::storage::VectorStorage;
use cis_core::storage::db::DbManager;
use cis_core::error::Result;
use cis_core::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};

/// 模拟的 Embedding 服务
struct MockEmbeddingService;

#[async_trait::async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 简单的确定性模拟
        let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
        let hash = text.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        for i in 0..DEFAULT_EMBEDDING_DIM {
            let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
            vec[i] = val;
        }
        // 归一化
        let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in &mut vec {
                *x /= norm;
            }
        }
        Ok(vec)
    }

    async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

fn create_test_skill(
    id: &str,
    name: &str,
    input_types: Vec<&str>,
    output_types: Vec<&str>,
    sink: bool,
) -> SkillSemanticsExt {
    SkillSemanticsExt::new(id, name)
        .with_description(format!("{} description", name))
        .with_examples(vec![format!("Use {} to do something", name)])
        .with_io_signature(
            SkillIoSignature::new(
                input_types.into_iter().map(|s| s.to_string()).collect(),
                output_types.into_iter().map(|s| s.to_string()).collect(),
            )
            .with_sink(sink)
            .with_pipeable(true),
        )
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== CIS Vector Intelligence 演示 ===\n");

    // 初始化向量存储
    let temp_dir = tempfile::tempdir().map_err(|e| cis_core::error::CisError::storage(e.to_string()))?;
    let db_path = temp_dir.path().join("vector.db");
    let embedding: Arc<dyn EmbeddingService> = Arc::new(MockEmbeddingService);
    let storage = Arc::new(VectorStorage::open_with_service(&db_path, embedding.clone())?);

    // 创建 SkillManager 和 DbManager
    let db_manager = Arc::new(DbManager::new()?);
    let skill_manager = Arc::new(SkillManager::new(db_manager.clone())?);

    // 创建路由器
    let mut router = SkillVectorRouter::new(storage.clone(), embedding.clone(), skill_manager, db_manager);

    // 注册测试技能
    println!("1. 注册技能...");
    
    let data_analyzer = create_test_skill(
        "data-analyzer",
        "Data Analyzer",
        vec!["json", "csv"],
        vec!["analysis_result"],
        false,
    );
    router.register_global_skill(data_analyzer.clone());
    println!("   ✓ 注册技能: data-analyzer");

    let report_gen = create_test_skill(
        "report-gen",
        "Report Generator",
        vec!["analysis_result"],
        vec!["report"],
        true,
    );
    router.register_global_skill(report_gen.clone());
    println!("   ✓ 注册技能: report-gen");

    let file_reader = create_test_skill(
        "file-reader",
        "File Reader",
        vec!["path"],
        vec!["content"],
        false,
    );
    router.register_global_skill(file_reader);
    println!("   ✓ 注册技能: file-reader");

    // CVI-009: 自然语言路由
    println!("\n2. 测试自然语言路由 (CVI-009)...");
    
    let test_inputs = vec![
        "分析今天的销售数据",
        "生成一份报告",
        "读取文件并分析",
    ];

    for input in test_inputs {
        println!("\n   输入: '{}'", input);
        match router.route_by_intent(input).await {
            Ok(result) => {
                println!("   ✓ 主技能: {} (置信度: {:.2})", 
                    result.primary_skill.skill_id, 
                    result.primary_skill.confidence
                );
                if let Some(chain) = result.skill_chain {
                    println!("   ✓ 技能链: {} 个步骤", chain.steps().len());
                    for (i, step) in chain.steps().iter().enumerate() {
                        println!("     步骤 {}: {}", i + 1, step.skill_id);
                    }
                }
                println!("   ✓ 总体置信度: {:.2}", result.overall_confidence);
            }
            Err(e) => {
                println!("   ✗ 路由失败: {}", e);
            }
        }
    }

    // CVI-010: 自动发现技能兼容性
    println!("\n3. 测试自动发现技能兼容性 (CVI-010)...");
    
    match router.auto_discover_compatibility().await {
        Ok(_) => {
            println!("   ✓ 兼容性发现完成");
        }
        Err(e) => {
            println!("   ✗ 兼容性发现失败: {}", e);
        }
    }

    // 使用链编排器
    println!("\n4. 测试 Chain Orchestrator...");
    
    let orchestrator = ChainOrchestrator::new();
    let skills = vec![data_analyzer.clone(), report_gen.clone()];
    
    let discovered_chains = orchestrator.auto_discover_chains(&skills, 3).await;
    println!("   ✓ 发现 {} 个技能链", discovered_chains.len());
    
    for (i, result) in discovered_chains.iter().enumerate() {
        println!("   链 {}: 置信度 {:.2}, 理由: {}", 
            i + 1, 
            result.confidence,
            result.reason
        );
    }

    // 使用预定义模板
    println!("\n5. 使用预定义链模板...");
    
    let input = serde_json::json!({
        "files": ["data.csv", "report.txt"],
        "query": "分析并生成报告"
    });
    
    let chain = ChainTemplates::analyze_and_report(input);
    println!("   ✓ 使用 analyze_and_report 模板");
    println!("   ✓ 链包含 {} 个步骤", chain.steps().len());

    println!("\n=== 演示完成 ===");
    Ok(())
}
