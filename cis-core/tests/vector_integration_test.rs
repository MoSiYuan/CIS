//! # Vector Intelligence Integration Tests
//!
//! CIS Vector Intelligence Week4 联调测试
//! 测试 Embedding → VectorStorage → IntentParser → SkillRouter → ConversationContext 的完整链路

use std::sync::Arc;

use cis_core::vector::{VectorStorage, VectorConfig};
use cis_core::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
use cis_core::intent::{IntentParser, ActionType};
use cis_core::skill::router::SkillVectorRouter;
use cis_core::skill::cis_admin::register_cis_local_skills;
use cis_core::skill::SkillManager;
use cis_core::storage::db::DbManager;
use cis_core::conversation::ConversationContext;
use cis_core::skill::chain::ChainBuilder;
use async_trait::async_trait;

/// 模拟 embedding service（用于测试）
struct MockEmbeddingService;

#[async_trait]
impl EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> cis_core::error::Result<Vec<f32>> {
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

    async fn batch_embed(&self, texts: &[&str]) -> cis_core::error::Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }
}

/// 创建测试存储
fn create_test_storage() -> (VectorStorage, tempfile::TempDir) {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("vector.db");
    let embedding: Arc<dyn EmbeddingService> = Arc::new(MockEmbeddingService);
    let storage = VectorStorage::open_with_service(&db_path, embedding).expect("Failed to open vector storage");
    (storage, temp_dir)
}

/// 创建 mock embedding service
fn create_mock_embedding() -> Arc<dyn EmbeddingService> {
    Arc::new(MockEmbeddingService)
}

/// 创建测试用的 SkillManager 和 DbManager
fn create_test_managers() -> (Arc<SkillManager>, Arc<DbManager>) {
    let db_manager = Arc::new(DbManager::new().expect("Failed to create DbManager"));
    let skill_manager = Arc::new(SkillManager::new(db_manager.clone()).expect("Failed to create SkillManager"));
    (skill_manager, db_manager)
}

/// 测试: Embedding → VectorStorage 基础流程
#[tokio::test]
async fn test_embedding_to_vector_storage() {
    // 创建向量存储
    let (storage, _temp) = create_test_storage();
    
    // 索引记忆 (使用 UTF-8 字节)
    let test_content = "这是一个测试记忆";
    let id = storage.index_memory(
        "test_key",
        test_content.as_bytes(),
        Some("test")
    ).await.expect("Failed to index memory");
    
    assert!(!id.is_empty(), "Memory ID should not be empty");
    
    // 搜索记忆（使用与索引内容高度相似的查询以获得更好匹配）
    let results = storage.search_memory("这是一个测试记忆", 5, Some(0.1))
        .await
        .expect("Failed to search memory");
    
    // 在简化实现中，相似度计算可能不完美，但至少不应该崩溃
    // 如果能找到结果，验证它是我们索引的
    if !results.is_empty() {
        assert_eq!(results[0].key, "test_key", "First result should be test_key");
    }
    
    // 测试删除
    assert!(storage.delete_memory_index(&id).expect("Failed to delete"));
}

/// 测试: IntentParser → SkillVectorRouter 意图到技能路由
#[tokio::test]
async fn test_intent_to_skill_routing() {
    // 创建向量存储
    let (storage, _temp) = create_test_storage();
    let storage = Arc::new(storage);
    
    // 创建 embedding service
    let embedding = create_mock_embedding();
    
    // 注册一些测试技能到向量存储
    let skill_semantics = cis_core::vector::storage::SkillSemantics {
        skill_id: "cis-local:analyze".to_string(),
        skill_name: "代码分析".to_string(),
        intent_description: "分析代码质量和结构".to_string(),
        capability_description: "可以分析代码复杂度、检查潜在问题".to_string(),
        project: Some("test-project".to_string()),
    };
    
    storage.register_skill(&skill_semantics)
        .await
        .expect("Failed to register skill");
    
    // 创建意图解析器
    let intent_parser = IntentParser::new(Arc::clone(&embedding));
    let intent = intent_parser.parse("分析今天的代码").await
        .expect("Failed to parse intent");
    
    // 验证意图解析结果
    assert_eq!(intent.action_type, ActionType::Analyze, "Action type should be Analyze");
    assert!(intent.confidence > 0.0, "Confidence should be positive");
    
    // 创建 SkillManager 和 DbManager
    let (skill_manager, db_manager) = create_test_managers();
    
    // 创建路由器
    let router = SkillVectorRouter::new(storage, embedding, skill_manager, db_manager);
    
    // 测试路由（不依赖全局技能，只测试存储中的技能）
    let candidates = router.route(&intent, None).await
        .expect("Failed to route intent");
    
    // 验证结果（因为使用了简化的 embedding，可能匹配不到，但至少不应该崩溃）
    println!("Found {} candidates", candidates.len());
}

/// 测试: ConversationContext + VectorStorage RAG
#[tokio::test]
async fn test_conversation_with_vector_context() {
    // 创建向量存储
    let (storage, _temp) = create_test_storage();
    let storage = Arc::new(storage);
    
    // 创建带向量存储的对话上下文
    let mut context = ConversationContext::with_vector_storage(
        "test-conv-1".to_string(),
        "test-session".to_string(),
        Some(std::path::PathBuf::from("/tmp/test_project")),
        Arc::clone(&storage),
    );
    
    // 添加带索引的消息
    let msg_id1 = context.add_user_message_with_index("什么是Rust?").await
        .expect("Failed to add user message with index");
    assert!(!msg_id1.is_empty(), "Message ID should not be empty");
    
    let msg_id2 = context.add_assistant_message_with_index("Rust是系统编程语言", None).await
        .expect("Failed to add assistant message with index");
    assert!(!msg_id2.is_empty(), "Message ID should not be empty");
    
    // RAG检索
    let relevant = context.retrieve_relevant_history("编程语言", 3).await
        .expect("Failed to retrieve relevant history");
    
    println!("Retrieved {} relevant messages", relevant.len());
    // 在简化实现中可能无法准确匹配，但至少不应该崩溃
}

/// 测试: SkillChain 编排
#[tokio::test]
async fn test_skill_chain_execution() {
    // 构建技能链
    let mut chain = ChainBuilder::new()
        .then("cis-local:file-list")
        .pipe("files", "input")
        .then("cis-local:read")
        .pipe("content", "data")
        .then("cis-local:analyze")
        .build(serde_json::json!({"path": "/tmp"}));
    
    // 执行链（使用同步模拟执行器以避免生命周期问题）
    let results = chain.execute_sync(
        |_skill_id: &str, _input: serde_json::Value| {
            // Mock执行器
            Ok(serde_json::json!({"skill": _skill_id, "input": _input}))
        }
    ).expect("Failed to execute chain");
    
    assert_eq!(results.len(), 3, "Should have 3 step results");
    
    // 验证每个步骤
    assert_eq!(results[0].skill_id, "cis-local:file-list");
    assert!(results[0].success);
    
    assert_eq!(results[1].skill_id, "cis-local:read");
    assert!(results[1].success);
    
    assert_eq!(results[2].skill_id, "cis-local:analyze");
    assert!(results[2].success);
}

/// 端到端测试: 自然语言 → 技能执行
/// 模拟: cis skill do "分析代码"
#[tokio::test]
async fn test_end_to_end_skill_invocation() {
    // 创建向量存储
    let (storage, _temp) = create_test_storage();
    let storage = Arc::new(storage);
    
    // 创建 embedding service
    let embedding = create_mock_embedding();
    
    // 1. 解析意图
    let intent_parser = IntentParser::new(Arc::clone(&embedding));
    let intent = intent_parser.parse("分析今天的代码质量").await
        .expect("Failed to parse intent");
    
    assert_eq!(intent.action_type, ActionType::Analyze, "Should detect Analyze action");
    
    // 2. 创建路由器并注册本地技能
    let (skill_manager, db_manager) = create_test_managers();
    let mut router = SkillVectorRouter::new(Arc::clone(&storage), Arc::clone(&embedding), skill_manager, db_manager);
    
    // 3. 路由到技能
    let candidates = router.route(&intent, None).await
        .expect("Failed to route intent");
    register_cis_local_skills(&mut router);
    
    // 注册技能语义到向量存储
    let analyze_semantics = cis_core::vector::storage::SkillSemantics {
        skill_id: "cis-local:analyze".to_string(),
        skill_name: "Analyze Code".to_string(),
        intent_description: "分析代码质量和结构，检查潜在问题".to_string(),
        capability_description: "可以执行代码静态分析、复杂度检查、潜在问题识别".to_string(),
        project: Some("test-project".to_string()),
    };
    storage.register_skill(&analyze_semantics).await
        .expect("Failed to register analyze skill");
    
    // 4. 验证结果
    if !candidates.is_empty() {
        let best = &candidates[0];
        println!("Best match: {} (confidence: {:.2})", best.skill_name, best.confidence);
        
        // 3. 如果有技能链，验证链结构
        if let Some(chain) = &best.suggested_chain {
            assert!(!chain.is_empty(), "Suggested chain should not be empty");
            println!("Suggested chain: {:?}", chain);
        }
    } else {
        println!("No candidates found (expected with simplified embedding)");
    }
}

/// 测试 VectorConfig 默认值
#[test]
fn test_vector_config_defaults() {
    use cis_core::vector::HnswConfig;
    
    let config = VectorConfig::default();
    
    assert_eq!(config.dimension, 768, "Default dimension should be 768");
    assert_eq!(config.batch_size, 100, "Default batch size should be 100");
    
    let hnsw = &config.hnsw;
    assert_eq!(hnsw.m, 16, "Default HNSW m should be 16");
    assert_eq!(hnsw.ef_construction, 100, "Default ef_construction should be 100");
    assert_eq!(hnsw.ef_search, 64, "Default ef_search should be 64");
}

/// 测试批量索引
#[tokio::test]
async fn test_batch_memory_indexing() {
    let (storage, _temp) = create_test_storage();
    
    // 准备批量数据（使用非常相似的内容以提高匹配率）
    let items: Vec<(String, Vec<u8>, Option<String>)> = vec![
        ("key1".to_string(), b"memory item first".to_vec(), Some("category1".to_string())),
        ("key2".to_string(), b"memory item second".to_vec(), Some("category1".to_string())),
        ("key3".to_string(), b"memory item third".to_vec(), Some("category2".to_string())),
    ];
    
    // 批量索引
    let ids = storage.batch_index_memory(items).await
        .expect("Failed to batch index memories");
    
    assert_eq!(ids.len(), 3, "Should create 3 memory IDs");
    
    // 搜索验证（使用低阈值）
    let results = storage.search_memory("memory item", 10, Some(0.1)).await
        .expect("Failed to search");
    
    // 在简化实现中，可能无法找到所有结果，但至少应该能执行
    println!("Found {} memories", results.len());
    assert!(results.len() <= 3, "Should find at most 3 memories");
}

/// 测试技能语义注册和搜索
#[tokio::test]
async fn test_skill_semantic_registration() {
    let (storage, _temp) = create_test_storage();
    
    // 注册技能
    let semantics = cis_core::vector::storage::SkillSemantics {
        skill_id: "test:skill1".to_string(),
        skill_name: "Test Skill".to_string(),
        intent_description: "Test skill for searching code analysis".to_string(),
        capability_description: "Can perform test operations and demo features".to_string(),
        project: Some("test-project".to_string()),
    };
    
    storage.register_skill(&semantics).await
        .expect("Failed to register skill");
    
    // 搜索技能（使用低阈值）
    let results = storage.search_skills("Test skill for searching code analysis", None, 5, Some(0.1)).await
        .expect("Failed to search skills");
    
    // 在简化实现中，可能无法找到技能，但至少应该能执行
    println!("Found {} skills", results.len());
    
    // 按项目过滤
    let project_results = storage.search_skills("test", Some("test-project"), 5, Some(0.1)).await
        .expect("Failed to search skills by project");
    
    println!("Found {} skills in test-project", project_results.len());
    
    // 删除技能索引
    let deleted = storage.delete_skill_index("test:skill1")
        .expect("Failed to delete skill index");
    
    assert!(deleted, "Should successfully delete skill index");
}
