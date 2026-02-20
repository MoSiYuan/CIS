use async_trait::async_trait;
use cis_types::{MemoryCategory, MemoryDomain};
use cis_traits::memory::{MemoryEntry, Memory as MemoryTrait};
use std::error::Error;
use std::sync::RwLock;
use chrono::Utc;

pub struct InMemoryDatabase {
    entries: RwLock<std::collections::HashMap<String, MemoryEntry>>,
}

impl InMemoryDatabase {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for InMemoryDatabase {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryTrait for InMemoryDatabase {
    fn name(&self) -> &str {
        "in-memory-database"
    }

    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<(), Box<dyn Error + Send + Sync>> {
        let now = Utc::now();
        let entry = MemoryEntry {
            key: key.to_string(),
            value: value.to_vec(),
            domain,
            category,
            created_at: now,
            updated_at: now,
        };
        let mut entries = self.entries.write().unwrap();
        entries.insert(key.to_string(), entry);
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>, Box<dyn Error + Send + Sync>> {
        let entries = self.entries.read().unwrap();
        Ok(entries.get(key).cloned())
    }

    async fn delete(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let mut entries = self.entries.write().unwrap();
        Ok(entries.remove(key).is_some())
    }

    async fn list_keys(&self, domain: Option<MemoryDomain>, category: Option<MemoryCategory>, prefix: Option<&str>) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let entries = self.entries.read().unwrap();
        let keys: Vec<String> = entries.values()
            .filter(|e| {
                domain.map_or(true, |d| e.domain == d) &&
                category.map_or(true, |c| e.category == c) &&
                prefix.map_or(true, |p| e.key.starts_with(p))
            })
            .map(|e| e.key.clone())
            .collect();
        Ok(keys)
    }

    async fn health_check(&self) -> bool {
        true
    }
}
