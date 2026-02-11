//! Kademlia 本地键值存储
//!
//! 带 TTL 的本地存储，支持值的过期清理。

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 存储项
#[derive(Debug, Clone)]
pub struct StorageItem {
    pub value: Vec<u8>,
    pub expires_at: Instant,
}

impl StorageItem {
    pub fn new(value: Vec<u8>, ttl: Duration) -> Self {
        Self {
            value,
            expires_at: Instant::now() + ttl,
        }
    }

    pub fn is_expired(&self) -> bool {
        Instant::now() > self.expires_at
    }
}

/// 本地键值存储
pub struct LocalStorage {
    data: Arc<RwLock<HashMap<String, StorageItem>>>,
    default_ttl: Duration,
}

impl LocalStorage {
    /// 创建新的存储
    pub fn new(default_ttl: Duration) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            default_ttl,
        }
    }

    /// 存储键值对
    pub async fn put(&self, key: impl Into<String>, value: Vec<u8>, ttl: Option<Duration>) -> crate::error::Result<()> {
        let key = key.into();
        let ttl = ttl.unwrap_or(self.default_ttl);
        let item = StorageItem::new(value, ttl);
        
        let mut data = self.data.write().await;
        data.insert(key, item);
        
        Ok(())
    }

    /// 获取值
    pub async fn get(&self, key: &str) -> crate::error::Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        
        if let Some(item) = data.get(key) {
            if !item.is_expired() {
                return Ok(Some(item.value.clone()));
            }
        }
        
        Ok(None)
    }

    /// 检查键是否存在
    pub async fn contains(&self, key: &str) -> bool {
        let data = self.data.read().await;
        
        if let Some(item) = data.get(key) {
            return !item.is_expired();
        }
        
        false
    }

    /// 删除键值对
    pub async fn delete(&self, key: &str) -> crate::error::Result<bool> {
        let mut data = self.data.write().await;
        Ok(data.remove(key).is_some())
    }

    /// 清理过期项
    pub async fn cleanup(&self) -> usize {
        let mut data = self.data.write().await;
        let before = data.len();
        data.retain(|_, item| !item.is_expired());
        before - data.len()
    }

    /// 获取存储统计
    pub async fn stats(&self) -> StorageStats {
        let data = self.data.read().await;
        let total = data.len();
        let expired = data.values().filter(|item| item.is_expired()).count();
        
        StorageStats {
            total_keys: total,
            expired_keys: expired,
            active_keys: total - expired,
        }
    }
    
    /// 获取所有键的列表
    pub async fn keys(&self) -> Vec<String> {
        let data = self.data.read().await;
        data.keys().cloned().collect()
    }
    
    /// 获取带有指定前缀的所有键
    pub async fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let data = self.data.read().await;
        data.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
}

/// 存储统计
#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_keys: usize,
    pub expired_keys: usize,
    pub active_keys: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_storage_basic() {
        let storage = LocalStorage::new(Duration::from_secs(3600));
        
        // 存储
        storage.put("key1", b"value1".to_vec(), None).await.unwrap();
        
        // 获取
        let value = storage.get("key1").await.unwrap();
        assert_eq!(value, Some(b"value1".to_vec()));
        
        // 不存在的键
        let value = storage.get("nonexistent").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_storage_expiration() {
        let storage = LocalStorage::new(Duration::from_millis(100));
        
        // 存储短 TTL 的值
        storage.put("key1", b"value1".to_vec(), Some(Duration::from_millis(50))).await.unwrap();
        
        // 立即获取应该存在
        assert!(storage.contains("key1").await);
        
        // 等待过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 过期后应该不存在
        assert!(!storage.contains("key1").await);
        let value = storage.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_storage_cleanup() {
        let storage = LocalStorage::new(Duration::from_millis(50));
        
        storage.put("key1", b"value1".to_vec(), None).await.unwrap();
        storage.put("key2", b"value2".to_vec(), Some(Duration::from_secs(3600))).await.unwrap();
        
        // 等待 key1 过期
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // 清理
        let cleaned = storage.cleanup().await;
        assert_eq!(cleaned, 1);
        
        // 验证统计
        let stats = storage.stats().await;
        assert_eq!(stats.total_keys, 1);
        assert_eq!(stats.expired_keys, 0);
        assert_eq!(stats.active_keys, 1);
    }

    #[tokio::test]
    async fn test_storage_delete() {
        let storage = LocalStorage::new(Duration::from_secs(3600));
        
        storage.put("key1", b"value1".to_vec(), None).await.unwrap();
        assert!(storage.delete("key1").await.unwrap());
        assert!(!storage.delete("key1").await.unwrap());
    }
}
