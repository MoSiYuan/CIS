//! # Mock Storage Service
//!
//! 存储服务的 Mock 实现，用于测试数据持久化功能。

use super::MockCallTracker;
use crate::error::{CisError, Result};
use crate::traits::{StorageService, StorageQuery, StorageRecord, StorageStats};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock as AsyncRwLock;

/// Mock 存储项
#[derive(Debug, Clone)]
pub struct MockStorageItem {
    pub key: String,
    pub value: Vec<u8>,
    pub version: u64,
    pub created_at: u64,
    pub updated_at: u64,
}

impl MockStorageItem {
    pub fn new(key: impl Into<String>, value: impl Into<Vec<u8>>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            key: key.into(),
            value: value.into(),
            version: 1,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn increment_version(&mut self) {
        self.version += 1;
        self.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }
}

/// 存储服务 Mock
#[derive(Debug, Clone)]
pub struct MockStorageService {
    tracker: MockCallTracker,
    data: Arc<AsyncRwLock<HashMap<String, MockStorageItem>>>,
    get_behaviors: Arc<AsyncRwLock<HashMap<String, Option<Vec<u8>>>>>,
    should_fail_next: Arc<Mutex<Option<CisError>>>,
    fail_pattern: Arc<Mutex<Option<String>>>,
}

impl MockStorageService {
    /// 创建新的 Mock
    pub fn new() -> Self {
        Self {
            tracker: MockCallTracker::new(),
            data: Arc::new(AsyncRwLock::new(HashMap::new())),
            get_behaviors: Arc::new(AsyncRwLock::new(HashMap::new())),
            should_fail_next: Arc::new(Mutex::new(None)),
            fail_pattern: Arc::new(Mutex::new(None)),
        }
    }

    /// 预设 get 行为
    pub async fn preset_get(&self, key: impl Into<String>, value: Option<Vec<u8>>) {
        let mut behaviors = self.get_behaviors.write().await;
        behaviors.insert(key.into(), value);
    }

    /// 模拟获取值（内部方法）
    pub async fn mock_get(&self, key: impl Into<String>) -> Result<Option<Vec<u8>>> {
        let key = key.into();
        self.tracker.record("get", vec![key.clone()]);

        // 检查是否有预设的错误
        if let Some(err) = self.should_fail_next.lock().unwrap().take() {
            return Err(err);
        }

        // 检查是否有匹配的错误模式
        if let Some(pattern) = self.fail_pattern.lock().unwrap().as_ref() {
            if key.contains(pattern) {
                return Err(CisError::storage(format!(
                    "Simulated error for key matching pattern: {}",
                    pattern
                )));
            }
        }

        // 检查行为预设
        let behaviors = self.get_behaviors.read().await;
        if let Some(value) = behaviors.get(&key) {
            return Ok(value.clone());
        }

        // 从存储中查找
        let data = self.data.read().await;
        Ok(data.get(&key).map(|item| item.value.clone()))
    }

    /// 模拟设置值（内部方法）
    pub async fn mock_set(&self, key: impl Into<String>, value: impl Into<Vec<u8>>) -> Result<()> {
        let key = key.into();
        let value = value.into();
        self.tracker.record("set", vec![key.clone(), format!("{} bytes", value.len())]);

        let mut data = self.data.write().await;
        data.insert(key.clone(), MockStorageItem::new(key, value));
        Ok(())
    }

    /// 模拟删除值（内部方法）
    pub async fn mock_delete(&self, key: impl Into<String>) -> Result<()> {
        let key = key.into();
        self.tracker.record("delete", vec![key.clone()]);

        let mut data = self.data.write().await;
        data.remove(&key);
        Ok(())
    }

    /// 模拟扫描（内部方法）
    pub async fn mock_scan(&self, prefix: impl Into<String>) -> Result<Vec<(String, Vec<u8>)>> {
        let prefix = prefix.into();
        self.tracker.record("scan", vec![prefix.clone()]);

        let data = self.data.read().await;
        let results: Vec<_> = data
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        Ok(results)
    }

    /// 检查键是否存在（内部方法）
    pub async fn mock_exists(&self, key: impl Into<String>) -> Result<bool> {
        let key = key.into();
        self.tracker.record("exists", vec![key.clone()]);

        let data = self.data.read().await;
        Ok(data.contains_key(&key))
    }

    /// 设置下一次操作失败
    pub fn will_fail_next(&self, error: CisError) {
        *self.should_fail_next.lock().unwrap() = Some(error);
    }

    /// 设置失败模式（匹配键的子字符串）
    pub fn will_fail_on_pattern(&self, pattern: impl Into<String>) {
        *self.fail_pattern.lock().unwrap() = Some(pattern.into());
    }

    /// 预填充数据
    pub async fn seed(&self, key: impl Into<String>, value: impl Into<Vec<u8>>) {
        let key = key.into();
        let value = value.into();
        let mut data = self.data.write().await;
        data.insert(key.clone(), MockStorageItem::new(key, value));
    }

    /// 预填充多条数据
    pub async fn seed_many(&self, items: &[(String, Vec<u8>)]) {
        let mut data = self.data.write().await;
        for (key, value) in items {
            data.insert(key.clone(), MockStorageItem::new(key.clone(), value.clone()));
        }
    }

    /// 获取存储中的所有键（内部方法）
    pub async fn mock_keys(&self) -> Vec<String> {
        let data = self.data.read().await;
        data.keys().cloned().collect()
    }

    /// 清空存储
    pub async fn clear_data(&self) {
        let mut data = self.data.write().await;
        data.clear();
    }

    /// 获取存储项
    pub async fn get_item(&self, key: impl AsRef<str>) -> Option<MockStorageItem> {
        let data = self.data.read().await;
        data.get(key.as_ref()).cloned()
    }

    // === 验证方法 ===

    /// 断言：方法被调用
    pub fn assert_called(&self, method: &str) {
        self.tracker.assert_called(method);
    }

    /// 断言：方法被调用指定次数
    pub fn assert_call_count(&self, method: &str, expected: usize) {
        self.tracker.assert_call_count(method, expected);
    }

    /// 断言：键被访问
    pub fn assert_key_accessed(&self, key: impl AsRef<str>) {
        let key = key.as_ref();
        let get_calls = self.tracker.get_calls_for("get");
        let set_calls = self.tracker.get_calls_for("set");
        let delete_calls = self.tracker.get_calls_for("delete");

        let accessed = get_calls.iter().any(|c| c.args.first().map(|a| a == key).unwrap_or(false))
            || set_calls.iter().any(|c| c.args.first().map(|a| a == key).unwrap_or(false))
            || delete_calls.iter().any(|c| c.args.first().map(|a| a == key).unwrap_or(false));

        assert!(
            accessed,
            "Expected key '{}' to be accessed (get/set/delete), but it wasn't",
            key
        );
    }

    /// 断言：键存在
    pub async fn assert_key_exists(&self, key: impl AsRef<str>) {
        let key = key.as_ref();
        let data = self.data.read().await;
        assert!(
            data.contains_key(key),
            "Expected key '{}' to exist in storage",
            key
        );
    }

    /// 断言：键不存在
    pub async fn assert_key_not_exists(&self, key: impl AsRef<str>) {
        let key = key.as_ref();
        let data = self.data.read().await;
        assert!(
            !data.contains_key(key),
            "Expected key '{}' to not exist in storage",
            key
        );
    }

    /// 断言：键的值匹配
    pub async fn assert_value_equals(&self, key: impl AsRef<str>, expected: impl AsRef<[u8]>) {
        let key = key.as_ref();
        let data = self.data.read().await;
        let item = data
            .get(key)
            .expect(&format!("Key '{}' not found in storage", key));
        assert_eq!(
            item.value, expected.as_ref(),
            "Value for key '{}' doesn't match expected",
            key
        );
    }

    /// 获取调用追踪器
    pub fn tracker(&self) -> &MockCallTracker {
        &self.tracker
    }
}

impl Default for MockStorageService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StorageService for MockStorageService {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.mock_get(key).await
    }

    async fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        self.mock_set(key, value.to_vec()).await
    }

    async fn put_if_version(
        &self,
        key: &str,
        value: &[u8],
        expected_version: Option<u64>,
    ) -> Result<()> {
        self.tracker.record("put_if_version", vec![
            key.to_string(),
            format!("{:?}", expected_version),
        ]);

        let mut data = self.data.write().await;
        
        match data.get(key) {
            Some(item) => {
                // 键存在，检查版本
                match expected_version {
                    Some(expected) if item.version == expected => {
                        // 版本匹配，更新
                        let mut new_item = MockStorageItem::new(key, value.to_vec());
                        new_item.version = item.version + 1;
                        new_item.created_at = item.created_at;
                        data.insert(key.to_string(), new_item);
                        Ok(())
                    }
                    Some(expected) => {
                        // 版本不匹配
                        Err(CisError::already_exists(format!(
                            "Version mismatch: expected {}, got {}",
                            expected, item.version
                        )))
                    }
                    None => {
                        // 期望不存在但实际存在
                        Err(CisError::already_exists(
                            "Key already exists".to_string()
                        ))
                    }
                }
            }
            None => {
                // 键不存在
                match expected_version {
                    Some(_) => {
                        // 期望存在但实际不存在
                        Err(CisError::not_found(
                            "Key does not exist".to_string()
                        ))
                    }
                    None => {
                        // 创建新键
                        data.insert(key.to_string(), MockStorageItem::new(key, value.to_vec()));
                        Ok(())
                    }
                }
            }
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        self.mock_delete(key).await
    }

    async fn query(&self, query: StorageQuery) -> Result<Vec<StorageRecord>> {
        self.tracker.record("query", vec![format!("{:?}", query)]);
        
        let data = self.data.read().await;
        let mut results: Vec<StorageRecord> = data
            .iter()
            .filter(|(k, _)| {
                // 前缀过滤
                if let Some(ref prefix) = query.prefix {
                    if !k.starts_with(prefix) {
                        return false;
                    }
                }
                // 模式过滤
                if let Some(ref pattern) = query.key_pattern {
                    if !k.contains(pattern) {
                        return false;
                    }
                }
                true
            })
            .map(|(k, v)| StorageRecord {
                key: k.clone(),
                value: v.value.clone(),
                version: v.version,
                created_at: v.created_at,
                updated_at: v.updated_at,
                expires_at: None,
                metadata: HashMap::new(),
            })
            .collect();

        // 应用限制和偏移
        if let Some(offset) = query.offset {
            results = results.into_iter().skip(offset).collect();
        }
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn scan(&self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>> {
        self.mock_scan(prefix).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.mock_exists(key).await
    }

    async fn get_batch(&self, keys: &[String]) -> Result<Vec<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(keys.len());
        for key in keys {
            let value = self.mock_get(key.as_str()).await?;
            results.push(value);
        }
        Ok(results)
    }

    async fn put_batch(&self, items: &[(String, Vec<u8>)]) -> Result<()> {
        for (key, value) in items {
            self.mock_set(key, value.clone()).await?;
        }
        Ok(())
    }

    async fn keys(&self) -> Result<Vec<String>> {
        Ok(self.mock_keys().await)
    }

    async fn clear(&self) -> Result<()> {
        self.clear_data().await;
        Ok(())
    }

    async fn stats(&self) -> Result<StorageStats> {
        let data = self.data.read().await;
        let total_size: u64 = data.values().map(|v| v.value.len() as u64).sum();
        
        Ok(StorageStats {
            total_keys: data.len() as u64,
            total_size,
            index_count: 0,
            last_modified: data.values().map(|v| v.updated_at).max(),
            compression_ratio: None,
        })
    }

    async fn transaction(&self, operations: Vec<(String, String, Option<Vec<u8>>)>) -> Result<()>
    where
        String: Send + Sync,
        Vec<u8>: Send + Sync,
    {
        self.tracker.record("transaction", vec![format!("{} ops", operations.len())]);
        
        // 简化的原子操作模拟
        let mut data = self.data.write().await;
        let backup: HashMap<String, MockStorageItem> = data.clone();
        
        for (op, key, value) in operations {
            match op.as_str() {
                "put" | "set" => {
                    if let Some(val) = value {
                        data.insert(key.clone(), MockStorageItem::new(key, val));
                    }
                }
                "delete" | "del" => {
                    data.remove(&key);
                }
                _ => {
                    // 回滚
                    *data = backup;
                    return Err(CisError::invalid_input(format!(
                        "Unknown operation: {}", op
                    )));
                }
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_get_set() {
        let mock = MockStorageService::new();

        // 设置并获取
        mock.seed("key1", b"value1").await;
        let value = mock.mock_get("key1").await.unwrap();

        assert_eq!(value, Some(b"value1".to_vec()));
        mock.assert_call_count("get", 1);
    }

    #[tokio::test]
    async fn test_mock_delete() {
        let mock = MockStorageService::new();

        mock.seed("key1", b"value1").await;
        mock.assert_key_exists("key1").await;

        mock.mock_delete("key1").await.unwrap();
        mock.assert_key_not_exists("key1").await;
    }

    #[tokio::test]
    async fn test_mock_scan() {
        let mock = MockStorageService::new();

        mock.seed_many(&[
            ("user:1".to_string(), b"Alice".to_vec()),
            ("user:2".to_string(), b"Bob".to_vec()),
            ("post:1".to_string(), b"Hello".to_vec()),
        ])
        .await;

        let results = mock.mock_scan("user:").await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_mock_error_simulation() {
        let mock = MockStorageService::new();

        mock.will_fail_next(CisError::storage("Disk full"));
        let result = mock.mock_get("any_key").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mock_pattern_failure() {
        let mock = MockStorageService::new();

        mock.will_fail_on_pattern("sensitive");
        let result = mock.mock_get("sensitive_data").await;
        assert!(result.is_err());

        // 不匹配模式的应该成功
        mock.seed("other_key", b"value").await;
        let result = mock.mock_get("other_key").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_put_if_version() {
        let mock = MockStorageService::new();

        // 创建新键（期望版本为 None）
        mock.put_if_version("key1", b"value1", None).await.unwrap();
        
        let item = mock.get_item("key1").await.unwrap();
        assert_eq!(item.version, 1);

        // 更新（版本匹配）
        mock.put_if_version("key1", b"value2", Some(1)).await.unwrap();
        let item = mock.get_item("key1").await.unwrap();
        assert_eq!(item.version, 2);

        // 版本不匹配
        let result = mock.put_if_version("key1", b"value3", Some(1)).await;
        assert!(result.is_err());

        // 键已存在但期望不存在
        let result = mock.put_if_version("key1", b"value3", None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stats() {
        let mock = MockStorageService::new();

        mock.seed("key1", b"value1").await;
        mock.seed("key2", b"value2_longer").await;

        let stats = mock.stats().await.unwrap();
        assert_eq!(stats.total_keys, 2);
        assert_eq!(stats.total_size, 6 + 13); // "value1" + "value2_longer"
    }

    #[tokio::test]
    async fn test_transaction() {
        let mock = MockStorageService::new();

        let operations = vec![
            ("put".to_string(), "key1".to_string(), Some(b"value1".to_vec())),
            ("put".to_string(), "key2".to_string(), Some(b"value2".to_vec())),
            ("delete".to_string(), "key1".to_string(), None),
        ];

        mock.transaction(operations).await.unwrap();

        // key1 应该被删除
        assert!(!mock.mock_exists("key1").await.unwrap());
        // key2 应该存在
        assert!(mock.mock_exists("key2").await.unwrap());
    }
}
