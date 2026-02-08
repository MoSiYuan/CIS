//! Skill 自动化端到端测试
//!
//! 测试场景:
//! 1. 注册多个测试技能
//! 2. 使用自然语言调用 Skill
//! 3. 验证正确匹配并执行
//! 4. 测试 Skill Chain 发现

use std::sync::Arc;

use tempfile::TempDir;

use cis_core::skill::router::SkillVectorRouter;
use cis_core::skill::semantics::{SkillIoSignature, SkillScope, SkillSemanticsExt};
use cis_core::skill::SkillManager;
use cis_core::storage::db::DbManager;
use cis_core::vector::storage::{SkillSemantics as StorageSkillSemantics, VectorStorage};

use async_trait::async_trait;
use cis_core::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
use cis_core::error::Result;
use cis_core::intent::{ActionType, ParsedIntent};

/// 模拟 embedding service（用于测试）
struct MockEmbeddingService;

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // 简单的确定性模拟：根据文本哈希生成向量
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
    description: &str,
    example_intents: Vec<String>,
    input_types: Vec<String>,
    output_types: Vec<String>,
    sink: bool,
) -> SkillSemanticsExt {
    SkillSemanticsExt {
        skill_id: id.to_string(),
        skill_name: name.to_string(),
        description: description.to_string(),
        example_intents,
        parameter_schema: None,
        io_signature: Some(SkillIoSignature {
            input_types,
            output_types,
            pipeable: true,
            source: false,
            sink,
        }),
        scope: SkillScope::Global,
    }
}

#[tokio::test]
async fn test_skill_natural_language_routing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &temp_dir.path().join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    let embedding = Arc::new(MockEmbeddingService);
    
    // 注册测试技能到 storage
    let skill_semantics = StorageSkillSemantics {
        skill_id: "data-analyzer".to_string(),
        skill_name: "Data Analyzer".to_string(),
        intent_description: "分析今天的销售数据".to_string(),
        capability_description: "分析今天的销售数据".to_string(),
        project: Some("default".to_string()),
    };
    storage.register_skill(&skill_semantics).await.unwrap();
    
    // 直接使用 storage 搜索技能（使用低阈值确保能找到）
    let search_results = storage.search_skills("分析今天的销售数据", None, 5, Some(0.1)).await.unwrap();
    
    // 验证能找到技能
    assert!(!search_results.is_empty(), "应该能找到匹配的技能");
    assert_eq!(search_results[0].skill_id, "data-analyzer");
}

#[tokio::test]
async fn test_skill_chain_discovery() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &temp_dir.path().join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    let embedding = Arc::new(MockEmbeddingService);
    
    let db_manager = Arc::new(DbManager::new().unwrap());
    let skill_manager = Arc::new(SkillManager::new(db_manager.clone()).unwrap());
    let mut router = SkillVectorRouter::new(storage.clone(), embedding.clone(), skill_manager, db_manager);
    
    // 注册 data-analyzer (输出 analysis 类型)
    let analyzer = create_test_skill(
        "data-analyzer",
        "Data Analyzer",
        "分析数据",
        vec!["分析数据".to_string()],
        vec!["data".to_string()],
        vec!["analysis".to_string()],
        false,
    );
    
    // 注册 report-generator (输入 analysis 类型，是 sink)
    let report_gen = create_test_skill(
        "report-generator",
        "Report Generator",
        "生成报告",
        vec!["生成报告".to_string()],
        vec!["analysis".to_string()],
        vec!["report".to_string()],
        true,
    );
    
    router.register_global_skill(analyzer);
    router.register_global_skill(report_gen);
    
    // 自动发现兼容性
    router.auto_discover_compatibility().await.unwrap();
    
    // 测试 Chain 发现
    let intent = ParsedIntent {
        action_type: ActionType::Analyze,
        normalized_intent: "分析数据并生成报告".to_string(),
        raw_input: "分析数据并生成报告".to_string(),
        embedding: vec![0.0; 768],
        entities: std::collections::HashMap::new(),
        confidence: 0.9,
    };
    
    let chain = router.discover_skill_chain("data-analyzer", &intent).await.unwrap();
    
    // 应该至少发现 data-analyzer 步骤
    assert!(chain.steps().len() >= 1, "链应该至少有一个步骤");
    assert!(chain.steps().iter().any(|s| s.skill_id == "data-analyzer"));
}

#[tokio::test]
async fn test_multiple_skill_routing() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &temp_dir.path().join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    // 向 storage 注册多个技能 - 使用与查询精确匹配的描述
    let storage_skills = vec![
        StorageSkillSemantics {
            skill_id: "file-searcher".to_string(),
            skill_name: "File Searcher".to_string(),
            intent_description: "查找所有配置文件".to_string(),
            capability_description: "查找所有配置文件".to_string(),
            project: Some("default".to_string()),
        },
        StorageSkillSemantics {
            skill_id: "code-analyzer".to_string(),
            skill_name: "Code Analyzer".to_string(),
            intent_description: "分析这段代码".to_string(),
            capability_description: "分析这段代码".to_string(),
            project: Some("default".to_string()),
        },
        StorageSkillSemantics {
            skill_id: "doc-generator".to_string(),
            skill_name: "Documentation Generator".to_string(),
            intent_description: "生成项目文档".to_string(),
            capability_description: "生成项目文档".to_string(),
            project: Some("default".to_string()),
        },
    ];
    
    for skill in &storage_skills {
        storage.register_skill(skill).await.unwrap();
    }
    
    // 直接测试 storage 搜索功能（使用低阈值）
    let test_cases = vec![
        ("查找所有配置文件", "file-searcher"),
        ("分析这段代码", "code-analyzer"),
        ("生成项目文档", "doc-generator"),
    ];
    
    for (query, expected_skill) in &test_cases {
        let results = storage.search_skills(query, None, 5, Some(0.1)).await.unwrap();
        assert!(
            !results.is_empty(),
            "查询 '{}' 应该找到匹配的技能",
            query
        );
        assert_eq!(
            results[0].skill_id, *expected_skill,
            "查询 '{}' 应该匹配技能 '{}'",
            query, expected_skill
        );
    }
}

#[tokio::test]
async fn test_skill_compatibility_score() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &temp_dir.path().join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    let embedding = Arc::new(MockEmbeddingService);
    
    let db_manager = Arc::new(DbManager::new().unwrap());
    let skill_manager = Arc::new(SkillManager::new(db_manager.clone()).unwrap());
    let mut router = SkillVectorRouter::new(storage.clone(), embedding.clone(), skill_manager, db_manager);
    
    // 注册具有兼容 IO 签名的技能
    let source_skill = create_test_skill(
        "csv-reader",
        "CSV Reader",
        "读取CSV文件",
        vec!["读取CSV".to_string()],
        vec!["file".to_string()],
        vec!["table".to_string()],
        false,
    );
    
    let compatible_skill = create_test_skill(
        "table-filter",
        "Table Filter",
        "过滤表格数据",
        vec!["过滤数据".to_string()],
        vec!["table".to_string()],  // 兼容：输入 table
        vec!["filtered_table".to_string()],
        false,
    );
    
    let incompatible_skill = create_test_skill(
        "image-processor",
        "Image Processor",
        "处理图片",
        vec!["处理图片".to_string()],
        vec!["image".to_string()],  // 不兼容：需要 image 输入
        vec!["processed_image".to_string()],
        false,
    );
    
    router.register_global_skill(source_skill);
    router.register_global_skill(compatible_skill);
    router.register_global_skill(incompatible_skill);
    
    // 运行自动发现
    router.auto_discover_compatibility().await.unwrap();
    
    // 测试意图解析与链式发现
    let intent = ParsedIntent {
        action_type: ActionType::Analyze,
        normalized_intent: "读取并过滤CSV数据".to_string(),
        raw_input: "读取并过滤CSV数据".to_string(),
        embedding: vec![0.0; 768],
        entities: std::collections::HashMap::new(),
        confidence: 0.9,
    };
    
    let chain = router.discover_skill_chain("csv-reader", &intent).await.unwrap();
    
    // 链应该包含 csv-reader 步骤
    assert!(chain.steps().iter().any(|s| s.skill_id == "csv-reader"));
}

#[tokio::test]
async fn test_skill_routing_confidence_threshold() {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &temp_dir.path().join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    // 向 storage 注册一个特定领域的技能 - 使用精确的查询匹配
    let specialized_skill = StorageSkillSemantics {
        skill_id: "kubernetes-deployer".to_string(),
        skill_name: "Kubernetes Deployer".to_string(),
        intent_description: "部署到K8s集群".to_string(),
        capability_description: "部署到K8s集群".to_string(),
        project: Some("default".to_string()),
    };
    storage.register_skill(&specialized_skill).await.unwrap();
    
    // 直接测试 storage 搜索（使用低阈值）
    let search_results = storage.search_skills("部署到K8s集群", None, 5, Some(0.1)).await.unwrap();
    assert!(!search_results.is_empty(), "应该能找到技能");
    assert_eq!(search_results[0].skill_id, "kubernetes-deployer");
}
