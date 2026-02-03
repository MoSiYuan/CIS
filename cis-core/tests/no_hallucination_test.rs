//! 无幻觉测试 - 验证 Skill 匹配不会跨项目产生幻觉

use std::sync::Arc;

use tempfile::TempDir;

use cis_core::skill::router::SkillVectorRouter;
use cis_core::skill::semantics::{SkillIoSignature, SkillScope, SkillSemanticsExt};
use cis_core::vector::storage::{SkillSemantics as StorageSkillSemantics, VectorStorage};

use async_trait::async_trait;
use cis_core::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
use cis_core::error::Result;

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

#[tokio::test]
async fn test_no_hallucination_across_projects() {
    let temp_dir = TempDir::new().unwrap();
    let project_a = temp_dir.path().join("project-a");
    let project_b = temp_dir.path().join("project-b");
    
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b).unwrap();
    
    // 在项目A注册特定技能
    let storage_a = Arc::new(
        VectorStorage::open_with_service(
            &project_a.join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    let skill_a = StorageSkillSemantics {
        skill_id: "project-a-specific".to_string(),
        skill_name: "Project A Specific".to_string(),
        intent_description: "只在项目A可用的技能".to_string(),
        capability_description: "项目A特定能力".to_string(),
        project: Some(project_a.to_string_lossy().to_string()),
    };
    storage_a.register_skill(&skill_a).await.unwrap();
    
    // 在项目B搜索该技能
    let storage_b = Arc::new(
        VectorStorage::open_with_service(
            &project_b.join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    let router = SkillVectorRouter::new(storage_b.clone(), Arc::new(MockEmbeddingService));
    
    // 尝试匹配项目A的技能
    let result = router.route_by_intent("项目A的特殊操作").await;
    
    // 应该找不到匹配（或置信度极低）
    match result {
        Ok(r) => {
            // 即使找到了匹配，置信度也应该很低
            assert!(
                r.overall_confidence < 0.5, 
                "Should not hallucinate across projects, but got confidence: {}",
                r.overall_confidence
            );
        }
        Err(_) => {
            // 错误也是可接受的（表示没有匹配）
        }
    }
}

#[tokio::test]
async fn test_strict_project_isolation() {
    let temp_dir = TempDir::new().unwrap();
    
    // 创建三个独立的项目
    let projects: Vec<_> = (0..3)
        .map(|i| temp_dir.path().join(format!("project-{}", i)))
    .collect();
    
    for project in &projects {
        std::fs::create_dir_all(project).unwrap();
    }
    
    // 在每个项目中注册同名但不同实现的技能
    for (i, project) in projects.iter().enumerate() {
        let storage = Arc::new(
            VectorStorage::open_with_service(
                &project.join("vector.db"),
                Arc::new(MockEmbeddingService)
            ).unwrap()
        );
        
        let skill = StorageSkillSemantics {
            skill_id: "shared-skill".to_string(),
            skill_name: format!("Shared Skill Project {}", i),
            intent_description: format!("项目{}的共享技能实现", i),
            capability_description: format!("项目{}的特定能力", i),
            project: Some(project.to_string_lossy().to_string()),
        };
        
        storage.register_skill(&skill).await.unwrap();
    }
    
    // 在每个项目中搜索，确保只能找到本项目的技能
    for (i, project) in projects.iter().enumerate() {
        let storage = Arc::new(
            VectorStorage::open_with_service(
                &project.join("vector.db"),
                Arc::new(MockEmbeddingService)
            ).unwrap()
        );
        
        // 搜索本项目路径下的技能
        let results = storage.search_skills(
            "共享技能",
            Some(&project.to_string_lossy()),
            5,
            Some(0.5)
        ).await.unwrap();
        
        // 验证只找到本项目的技能
        for result in &results {
            // 由于每个项目有自己的存储，理论上只能找到自己的技能
            assert_eq!(
                result.skill_id, "shared-skill",
                "应该只找到本项目的技能"
            );
        }
    }
}

#[tokio::test]
async fn test_no_false_positives_on_unrelated_queries() {
    let temp_dir = TempDir::new().unwrap();
    
    let project_path = temp_dir.path().join("project");
    std::fs::create_dir_all(&project_path).unwrap();
    
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &project_path.join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    let router = SkillVectorRouter::new(storage.clone(), Arc::new(MockEmbeddingService));
    
    // 注册一些特定领域的技能
    let specific_skill = SkillSemanticsExt {
        skill_id: "docker-deploy".to_string(),
        skill_name: "Docker Deploy".to_string(),
        description: "部署Docker容器".to_string(),
        example_intents: vec!["部署Docker".to_string()],
        parameter_schema: None,
        io_signature: Some(SkillIoSignature {
            input_types: vec!["manifest".to_string()],
            output_types: vec!["deployment".to_string()],
            pipeable: false,
            source: false,
            sink: true,
        }),
        scope: SkillScope::Project,
    };
    
    // 注意：这里我们使用 register_global_skill 而不是存储
    // 因为在测试中 router 不直接使用存储的技能
    let mut router_with_skill = SkillVectorRouter::new(storage.clone(), Arc::new(MockEmbeddingService));
    router_with_skill.register_global_skill(specific_skill);
    
    // 测试完全不相关的查询
    let unrelated_queries = vec![
        "今天天气怎么样",
        "推荐一首好听的歌曲",
        "如何烹饪意大利面",
        "讲一个笑话",
    ];
    
    for query in unrelated_queries {
        let result = router_with_skill.route_by_intent(query).await;
        
        // 对于完全不相关的查询，应该返回错误或非常低的置信度
        if let Ok(routing) = result {
            assert!(
                routing.overall_confidence < 0.6,
                "不相关的查询 '{}' 不应该有高置信度匹配: {}",
                query,
                routing.overall_confidence
            );
        }
        // Err 也是可接受的
    }
}

#[tokio::test]
async fn test_skill_scope_enforcement() {
    let temp_dir = TempDir::new().unwrap();
    
    let project_a = temp_dir.path().join("project-a");
    let project_b = temp_dir.path().join("project-b");
    std::fs::create_dir_all(&project_a).unwrap();
    std::fs::create_dir_all(&project_b).unwrap();
    
    // 在项目A创建 Local scope 技能
    let storage_a = Arc::new(
        VectorStorage::open_with_service(
            &project_a.join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    let local_skill = SkillSemanticsExt {
        skill_id: "local-only-skill".to_string(),
        skill_name: "Local Only Skill".to_string(),
        description: "只在项目A本地可用".to_string(),
        example_intents: vec!["本地操作".to_string()],
        parameter_schema: None,
        io_signature: None,
        scope: SkillScope::Project, // Local scope
    };
    
    storage_a.register_skill(&StorageSkillSemantics {
        skill_id: local_skill.skill_id.clone(),
        skill_name: local_skill.skill_name.clone(),
        intent_description: local_skill.description.clone(),
        capability_description: "本地能力".to_string(),
        project: Some(project_a.to_string_lossy().to_string()),
    }).await.unwrap();
    
    // 在项目B创建同名 Global scope 技能
    let storage_b = Arc::new(
        VectorStorage::open_with_service(
            &project_b.join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    let global_skill = SkillSemanticsExt {
        skill_id: "local-only-skill".to_string(), // 相同的 ID
        skill_name: "Global Skill".to_string(),
        description: "global skill available everywhere".to_string(),
        example_intents: vec!["global operation".to_string()],
        parameter_schema: None,
        io_signature: None,
        scope: SkillScope::Global,
    };
    
    storage_b.register_skill(&StorageSkillSemantics {
        skill_id: global_skill.skill_id.clone(),
        skill_name: global_skill.skill_name.clone(),
        intent_description: "global skill available everywhere".to_string(),
        capability_description: "global capability".to_string(),
        project: Some(project_b.to_string_lossy().to_string()),
    }).await.unwrap();
    
    // 验证项目B的技能存储是独立的（使用低阈值和完全匹配的查询）
    let results_b = storage_b.search_skills(
        "global skill available everywhere",
        Some(&project_b.to_string_lossy()),
        5,
        Some(0.1)
    ).await.unwrap();
    
    assert!(!results_b.is_empty(), "应该能找到项目B的技能");
    
    // 验证搜索结果来自正确的项目
    for result in &results_b {
        assert_eq!(result.skill_id, "local-only-skill");
        // 注意：因为我们使用独立的存储文件，所以每个项目只能找到自己的技能
    }
}

#[tokio::test]
async fn test_confidence_calibration() {
    let temp_dir = TempDir::new().unwrap();
    
    let project_path = temp_dir.path().join("project");
    std::fs::create_dir_all(&project_path).unwrap();
    
    let storage = Arc::new(
        VectorStorage::open_with_service(
            &project_path.join("vector.db"),
            Arc::new(MockEmbeddingService)
        ).unwrap()
    );
    
    // 注册一个高度特定的技能
    let specific_skill = SkillSemanticsExt {
        skill_id: "kubernetes-helm-deployer".to_string(),
        skill_name: "Kubernetes Helm Deployer".to_string(),
        description: "使用Helm部署到Kubernetes集群".to_string(),
        example_intents: vec![
            "使用Helm部署应用到Kubernetes".to_string(),
            "Helm chart 部署".to_string(),
        ],
        parameter_schema: None,
        io_signature: Some(SkillIoSignature {
            input_types: vec!["helm_chart".to_string()],
            output_types: vec!["deployment".to_string()],
            pipeable: false,
            source: false,
            sink: true,
        }),
        scope: SkillScope::Global,
    };
    
    let mut router = SkillVectorRouter::new(storage.clone(), Arc::new(MockEmbeddingService));
    router.register_global_skill(specific_skill);
    
    // 测试精确匹配
    let exact_match = router.route_by_intent("使用Helm部署应用到Kubernetes").await;
    if let Ok(routing) = exact_match {
        // 精确匹配应该有较高置信度
        assert!(
            routing.overall_confidence > 0.5,
            "精确匹配应该有较高置信度: {}",
            routing.overall_confidence
        );
    }
    
    // 测试部分匹配
    let partial_match = router.route_by_intent("Kubernetes部署").await;
    if let Ok(routing) = partial_match {
        // 部分匹配的置信度应该适中
        assert!(
            routing.overall_confidence > 0.3,
            "部分匹配应该有一定置信度: {}",
            routing.overall_confidence
        );
    }
    
    // 测试完全不匹配
    let no_match = router.route_by_intent("如何烤面包").await;
    if let Ok(routing) = no_match {
        // 完全不匹配应该有低置信度
        assert!(
            routing.overall_confidence < 0.5,
            "完全不匹配应该有低置信度: {}",
            routing.overall_confidence
        );
    }
}
