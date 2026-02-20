use async_trait::async_trait;
use cis_types::{MemoryCategory, MemoryDomain};
use cis_traits::memory::Memory as MemoryTrait;
use cis_storage::MemoryStorage;
use std::error::Error;
use std::sync::Arc;
use chrono::Utc;
use super::entry::MemoryEntry;

pub struct CISMemory {
    storage: Arc<MemoryStorage>,
    namespace: String,
}

impl CISMemory {
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            storage: Arc::new(MemoryStorage::new()),
            namespace: namespace.into(),
        }
    }

    pub fn with_storage(mut self, storage: Arc<MemoryStorage>) -> Self {
        self.storage = storage;
        self
    }

    fn make_key(&self, key: &str) -> String {
        format!("{}/{}", self.namespace, key)
    }
}

#[async_trait]
impl MemoryTrait for CISMemory {
    fn name(&self) -> &str {
        "cis-memory"
    }

    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<(), Box<dyn Error + Send + Sync>> {
        let full_key = self.make_key(key);
        self.storage.set(&full_key, value).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>, Box<dyn Error + Send + Sync>> {
        let full_key = self.make_key(key);
        self.storage.get(&full_key).await?;
        Ok(None)
    }

    async fn delete(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let full_key = self.make_key(key);
        self.storage.delete(&full_key).await
    }

    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let prefix = prefix.map(|p| self.make_key(p));
        self.storage.list_keys(prefix.as_deref()).await
    }

    async fn health_check(&self) -> bool {
        self.storage.health_check().await
    }
}
