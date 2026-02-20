use async_trait::async_trait;
use std::error::Error;

#[async_trait]
pub trait Storage: Send + Sync {
    fn name(&self) -> &str;

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, Box<dyn Error + Send + Sync>>;
    async fn set(&self, key: &str, value: &[u8]) -> Result<(), Box<dyn Error + Send + Sync>>;
    async fn delete(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;
    async fn exists(&self, key: &str) -> Result<bool, Box<dyn Error + Send + Sync>>;
    async fn list_keys(&self, prefix: Option<&str>) -> Result<Vec<String>, Box<dyn Error + Send + Sync>>;

    async fn health_check(&self) -> bool;
}
