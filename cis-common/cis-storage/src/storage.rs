use async_trait::async_trait;
use cis_traits::Storage;
use std::error::Error;
use std::sync::RwLock;

pub struct MemoryStorage {
    data: RwLock<std::collections::HashMap<String, Vec<u8>>>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            data: RwLock::new(std::collections::HashMap::new()),
        }
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    fn name(&self) -> &str {
        "memory"
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error + Send + Sync>> {
        let data = self.data.read().unwrap();
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: &str, value: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut data = self.data.write().unwrap();
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let mut data = self.data.write().unwrap();
        Ok(data.remove(key).is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>> {
        let data = self.data.read().unwrap();
        Ok(data.contains_key(key))
    }

    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, Box<dyn Error + Send + Sync>> {
        let data = self.data.read().unwrap();
        let keys: Vec<String> = data.keys()
            .filter(|k| prefix.map_or(true, |p| k.starts_with(p)))
            .cloned()
            .collect();
        Ok(keys)
    }

    async fn health_check(&self) -> bool {
        true
    }
}
