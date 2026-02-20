use async_trait::async_trait;
use cis_types::{MemoryDomain, MemoryCategory};
use chrono::{DateTime, Utc};
use std::error::Error;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub key: String,
    pub value: Vec<u8>,
    pub score: f32,
}

#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    pub key: String,
    pub value: Vec<u8>,
    pub vector_score: f32,
    pub keyword_score: f32,
    pub final_score: f32,
}

#[derive(Debug, Clone)]
pub struct SyncMarker {
    pub key: String,
    pub domain: MemoryDomain,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub sync_peers: Vec<String>,
}

#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;

    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>, Box<dyn Error + Send + Sync>>;
    async fn delete(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;

    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>, Box<dyn Error + Send + Sync>>;

    async fn health_check(&self) -> bool;
}

#[async_trait]
pub trait MemoryVectorIndex: Memory {
    async fn semantic_search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>, Box<dyn Error + Send + Sync>>;
    async fn hybrid_search(&self, query: &str, limit: usize, domain: Option<MemoryDomain>) -> Result<Vec<HybridSearchResult>, Box<dyn Error + Send + Sync>>;
}

#[async_trait]
pub trait MemorySync: Memory {
    async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>, Box<dyn Error + Send + Sync>>;
    async fn mark_synced(&self, key: &str, peers: Vec<String>) -> Result<(), Box<dyn Error + Send + Sync>>;
}
