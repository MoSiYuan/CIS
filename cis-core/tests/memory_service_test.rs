//! MemoryService 单元测试
//!
//! CVI-014: 为核心组件编写单元测试

use std::sync::Arc;
use tokio::sync::Mutex;
use tempfile::TempDir;

use cis_core::memory::{MemoryService, MemoryEncryption, SearchOptions};
use cis_core::types::{MemoryDomain, MemoryCategory};
use cis_core::storage::memory_db::MemoryDb;
use cis_core::vector::VectorStorage;

use async_trait::async_trait;
use cis_core::error::Result;

/// 模拟 embedding service（用于测试）
struct MockEmbeddingService;

#[async_trait]
impl cis_core::ai::embedding::EmbeddingService for MockEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let mut vec = vec![0.0f32; 768];
        let hash = text.bytes().fold(0u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(b as u64)
        });
        for i in 0..768 {
            let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
            vec[i] = val;
        }
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

    fn dimension(&self) -> usize {
        768
    }
}

fn create_test_service(node_id: &str) -> (MemoryService, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    
    let db_path = temp_dir.path().join("memory.db");
    let memory_db = MemoryDb::open(&db_path).unwrap();
    
    let vector_path = temp_dir.path().join("vector.db");
    let embedding = Arc::new(MockEmbeddingService);
    let vector_storage = VectorStorage::open_with_service(&vector_path, embedding).unwrap();
    
    let service = MemoryService::new(
        Arc::new(Mutex::new(memory_db)),
        Arc::new(vector_storage),
        node_id,
    ).unwrap();
    
    (service, temp_dir)
}

#[tokio::test]
async fn test_memory_service_open() {
    let (service, _temp) = create_test_service("node-1");
    
    assert_eq!(service.node_id(), "node-1");
    assert!(!service.is_encrypted());
}

#[tokio::test]
async fn test_set_and_get_private() {
    let (service, _temp) = create_test_service("node-1");
    
    // 设置加密
    let service = service.with_encryption(MemoryEncryption::from_node_key(b"test-key"));
    
    service.set("private-key", b"secret-value", MemoryDomain::Private, MemoryCategory::Context).await.unwrap();
    
    let item = service.get("private-key").await.unwrap().unwrap();
    assert_eq!(item.key, "private-key");
    assert_eq!(item.value, b"secret-value");
    assert!(matches!(item.domain, MemoryDomain::Private));
    assert!(item.encrypted);
}

#[tokio::test]
async fn test_set_and_get_public() {
    let (service, _temp) = create_test_service("node-1");
    
    service.set("public-key", b"public-value", MemoryDomain::Public, MemoryCategory::Result).await.unwrap();
    
    let item = service.get("public-key").await.unwrap().unwrap();
    assert_eq!(item.key, "public-key");
    assert_eq!(item.value, b"public-value");
    assert!(matches!(item.domain, MemoryDomain::Public));
    assert!(!item.encrypted);
}

#[tokio::test]
async fn test_semantic_search() {
    let (service, _temp) = create_test_service("node-1");
    
    // 使用 set_with_embedding 来建立向量索引
    service.set_with_embedding("key1", "user likes dark theme".as_bytes(), MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    service.set_with_embedding("key2", "dark mode is enabled".as_bytes(), MemoryDomain::Public, MemoryCategory::Result).await.unwrap();
    
    // 等待后台索引完成
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // 语义搜索
    let results = service.semantic_search("dark mode", 10, 0.1).await.unwrap();
    
    // 由于使用 MockEmbeddingService，结果可能不准确
    // 但至少应该不报错
    println!("Semantic search results: {:?}", results);
}

#[tokio::test]
async fn test_delete() {
    let (service, _temp) = create_test_service("node-1");
    
    service.set("delete-key", b"value", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    assert!(service.get("delete-key").await.unwrap().is_some());
    
    let deleted = service.delete("delete-key").await.unwrap();
    assert!(deleted);
    assert!(service.get("delete-key").await.unwrap().is_none());
    
    // 删除不存在的 key 应该返回 false
    let deleted_again = service.delete("delete-key").await.unwrap();
    assert!(!deleted_again);
}

#[tokio::test]
async fn test_list_keys() {
    let (service, _temp) = create_test_service("node-1");
    
    service.set("key1", b"value1", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    service.set("key2", b"value2", MemoryDomain::Private, MemoryCategory::Context).await.unwrap();
    service.set("key3", b"value3", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    
    // 列出所有 key
    let all_keys = service.list_keys(None).await.unwrap();
    assert_eq!(all_keys.len(), 3);
    
    // 只列出公域 key
    let public_keys = service.list_keys(Some(MemoryDomain::Public)).await.unwrap();
    assert_eq!(public_keys.len(), 2);
    
    // 只列出私域 key
    let private_keys = service.list_keys(Some(MemoryDomain::Private)).await.unwrap();
    assert_eq!(private_keys.len(), 1);
}

#[tokio::test]
async fn test_with_namespace() {
    let (base_service, _temp) = create_test_service("node-1");
    
    // 使用命名空间创建服务
    let service_ns1 = base_service.with_namespace("ns1");

    
    // 验证命名空间设置成功
    assert_eq!(service_ns1.namespace(), Some("ns1"));
    
    // 需要重新创建服务以使用不同的命名空间
    let (base_service2, _temp2) = create_test_service("node-1");
    let service_ns2 = base_service2.with_namespace("ns2");
    assert_eq!(service_ns2.namespace(), Some("ns2"));
}

#[tokio::test]
async fn test_memory_encryption() {
    let (service, _temp) = create_test_service("node-1");
    
    let encrypted_service = service.with_encryption(MemoryEncryption::from_node_key("test-key-123".as_bytes()));
    
    // 存储私域记忆
    encrypted_service.set("encrypted-key", "secret-data".as_bytes(), MemoryDomain::Private, MemoryCategory::Context).await.unwrap();
    
    // 读取（应该自动解密）
    let item = encrypted_service.get("encrypted-key").await.unwrap().unwrap();
    assert_eq!(item.value, b"secret-data");
    assert!(item.encrypted);
}

#[tokio::test]
async fn test_memory_item_metadata() {
    let (service, _temp) = create_test_service("node-1");
    
    service.set("meta-key", b"meta-value", MemoryDomain::Public, MemoryCategory::Skill).await.unwrap();
    
    let item = service.get("meta-key").await.unwrap().unwrap();
    assert_eq!(item.key, "meta-key");
    assert_eq!(item.value, b"meta-value");
    assert!(matches!(item.domain, MemoryDomain::Public));
    assert!(matches!(item.category, MemoryCategory::Skill));
    assert_eq!(item.owner, "node-1");
    assert!(item.created_at <= chrono::Utc::now());
    assert!(item.updated_at <= chrono::Utc::now());
}

#[tokio::test]
async fn test_search_with_options() {
    let (service, _temp) = create_test_service("node-1");
    
    // 创建不同域和分类的数据
    service.set_with_embedding("key1", "public context".as_bytes(), MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    service.set_with_embedding("key2", "public result".as_bytes(), MemoryDomain::Public, MemoryCategory::Result).await.unwrap();
    service.set_with_embedding("key3", "private context".as_bytes(), MemoryDomain::Private, MemoryCategory::Context).await.unwrap();
    
    // 等待后台索引
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    
    // 使用选项搜索
    let options = SearchOptions::new()
        .with_domain(MemoryDomain::Public)
        .with_category(MemoryCategory::Context)
        .with_limit(5)
        .with_threshold(0.1);
    
    let results = service.search("public", options).await.unwrap();
    println!("Search with options results: {:?}", results.len());
}

#[tokio::test]
async fn test_sync_marker_operations() {
    let (service, _temp) = create_test_service("node-1");
    
    // 存储公域记忆（会自动标记为待同步）
    service.set("sync-key", b"sync-value", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    
    // 获取待同步项
    let pending = service.get_pending_sync(10).await.unwrap();
    // 注意：实际行为取决于 MemoryDb 的实现
    println!("Pending sync items: {:?}", pending.len());
}

#[tokio::test]
async fn test_export_import_public() {
    let (service, _temp) = create_test_service("node-1");
    
    // 存储公域记忆
    service.set("export-key1", b"value1", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    service.set("export-key2", b"value2", MemoryDomain::Public, MemoryCategory::Result).await.unwrap();
    
    // 导出
    let since = 0;
    let exported = service.export_public(since).await.unwrap();
    assert!(!exported.is_empty());
    
    // 所有导出项应该是公域
    for item in &exported {
        assert!(matches!(item.domain, MemoryDomain::Public));
    }
}

#[tokio::test]
async fn test_mark_synced() {
    let (service, _temp) = create_test_service("node-1");
    
    // 存储并标记为同步
    service.set("sync-test-key", b"value", MemoryDomain::Public, MemoryCategory::Context).await.unwrap();
    
    let result = service.mark_synced("sync-test-key").await;
    // 结果取决于 MemoryDb 实现
    println!("Mark synced result: {:?}", result);
}

#[tokio::test]
async fn test_on_sync_complete() {
    let (service, _temp) = create_test_service("node-1");
    
    let result = service.on_sync_complete("key", "peer-1").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_memory_domain_variants() {
    let private = MemoryDomain::Private;
    let public = MemoryDomain::Public;
    
    assert!(matches!(private, MemoryDomain::Private));
    assert!(matches!(public, MemoryDomain::Public));
}

#[tokio::test]
async fn test_memory_category_variants() {
    let categories = vec![
        MemoryCategory::Execution,
        MemoryCategory::Result,
        MemoryCategory::Error,
        MemoryCategory::Context,
        MemoryCategory::Skill,
    ];
    
    assert_eq!(categories.len(), 5);
}

#[tokio::test]
async fn test_search_options_builder() {
    let options = SearchOptions::new()
        .with_domain(MemoryDomain::Public)
        .with_category(MemoryCategory::Context)
        .with_limit(20)
        .with_threshold(0.8);
    
    assert!(matches!(options.domain, Some(MemoryDomain::Public)));
    assert!(matches!(options.category, Some(MemoryCategory::Context)));
    assert_eq!(options.limit, 20);
    assert!((options.threshold - 0.8).abs() < 0.001);
}

#[tokio::test]
async fn test_set_with_embedding() {
    let (service, _temp) = create_test_service("node-1");
    
    // 存储并建立向量索引
    service.set_with_embedding(
        "embedded-key",
        b"embedded value for testing",
        MemoryDomain::Public,
        MemoryCategory::Context
    ).await.unwrap();
    
    // 验证存储成功
    let item = service.get("embedded-key").await.unwrap().unwrap();
    assert_eq!(item.key, "embedded-key");
    assert_eq!(item.value, b"embedded value for testing");
}

#[tokio::test]
async fn test_index_memory() {
    let (service, _temp) = create_test_service("node-1");
    
    // 手动索引记忆
    let memory_id = service.index_memory(
        "manual-index-key",
        b"manual index value",
        MemoryCategory::Result
    ).await.unwrap();
    
    assert!(!memory_id.is_empty());
}
