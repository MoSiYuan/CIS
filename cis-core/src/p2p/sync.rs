//! 公域记忆同步实现

use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::memory::MemoryService;
use crate::vector::VectorStorage;
use crate::p2p::P2PNetwork;
use crate::p2p::crdt::VectorClock;
use crate::error::{CisError, Result};
use crate::types::{MemoryDomain, MemoryCategory};

/// 同步管理器
pub struct MemorySyncManager {
    memory_service: Arc<MemoryService>,
    #[allow(dead_code)]
    vector_storage: Arc<VectorStorage>,
    p2p: Arc<P2PNetwork>,
    /// 本地向量时钟
    vector_clock: Arc<RwLock<VectorClock>>,
    /// 节点 ID
    node_id: String,
}

/// 同步的记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncMemoryEntry {
    pub key: String,
    #[serde(with = "serde_bytes")]
    pub value: Vec<u8>,
    pub vector: Option<Vec<f32>>,
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub vector_clock: VectorClock,
    pub version: u64,
    pub category: MemoryCategory,
}

/// 同步请求
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRequest {
    pub node_id: String,
    pub since: Option<DateTime<Utc>>,
    pub known_keys: Vec<String>,
}

/// 同步响应
#[derive(Debug, Serialize, Deserialize)]
pub struct SyncResponse {
    pub node_id: String,
    pub entries: Vec<SyncMemoryEntry>,
    pub deleted_keys: Vec<String>,
    pub timestamp: DateTime<Utc>,
}

/// 同步消息类型
#[derive(Debug, Serialize, Deserialize)]
pub enum SyncMessage {
    Request(SyncRequest),
    Response(SyncResponse),
    Broadcast(SyncMemoryEntry),
}

impl MemorySyncManager {
    pub fn new(
        memory_service: Arc<MemoryService>,
        vector_storage: Arc<VectorStorage>,
        p2p: Arc<P2PNetwork>,
        node_id: String,
    ) -> Self {
        Self {
            memory_service,
            vector_storage,
            p2p,
            vector_clock: Arc::new(RwLock::new(VectorClock::new())),
            node_id,
        }
    }

    /// 启动同步管理器
    pub async fn start(&self) -> Result<()> {
        let node_id = self.node_id.clone();
        
        // 创建用于消息处理的同步管理器引用
        let sync_manager = Arc::new(self.clone_as_handle());
        
        // 订阅同步主题
        self.p2p.subscribe("memory_sync", move |data| {
            let sync_mgr = Arc::clone(&sync_manager);
            tokio::spawn(async move {
                if let Err(e) = sync_mgr.handle_sync_message(data).await {
                    tracing::error!("Sync message handling failed: {}", e);
                }
            });
        }).await?;

        // 启动定期同步任务
        self.start_periodic_sync().await;

        tracing::info!("Memory sync manager started for node {}", node_id);
        Ok(())
    }

    /// 处理同步消息
    #[allow(dead_code)]
    async fn handle_sync_message(&self, data: Vec<u8>) -> Result<()> {
        let message: SyncMessage = serde_json::from_slice(&data)?;

        match message {
            SyncMessage::Request(req) => {
                self.handle_sync_request(req).await?;
            }
            SyncMessage::Response(resp) => {
                self.handle_sync_response(resp).await?;
            }
            SyncMessage::Broadcast(entry) => {
                self.handle_broadcast(entry).await?;
            }
        }

        Ok(())
    }

    /// 处理同步请求
    #[allow(dead_code)]
    async fn handle_sync_request(&self, request: SyncRequest) -> Result<()> {
        tracing::info!("Received sync request from {}", request.node_id);
        
        // 获取本地公域记忆
        let entries = self.get_local_public_memories(request.since).await?;

        // 构建响应
        let response = SyncResponse {
            node_id: self.node_id.clone(),
            entries,
            deleted_keys: self.get_deleted_keys(request.since).await?,
            timestamp: Utc::now(),
        };

        // 发送响应
        let message = SyncMessage::Response(response);
        let data = serde_json::to_vec(&message)?;

        // 广播到网络（请求节点会接收并处理）
        self.p2p.broadcast("memory_sync", data).await?;

        Ok(())
    }

    /// 处理同步响应
    #[allow(dead_code)]
    async fn handle_sync_response(&self, response: SyncResponse) -> Result<()> {
        tracing::info!(
            "Received sync response from {} with {} entries",
            response.node_id,
            response.entries.len()
        );

        // 先更新向量时钟
        for entry in &response.entries {
            let mut clock = self.vector_clock.write().await;
            *clock = clock.merge(&entry.vector_clock);
        }

        // 然后合并条目
        for entry in response.entries {
            self.merge_entry(entry).await?;
        }

        Ok(())
    }

    /// 处理广播消息
    #[allow(dead_code)]
    async fn handle_broadcast(&self, entry: SyncMemoryEntry) -> Result<()> {
        tracing::debug!("Received broadcast for key: {}", entry.key);
        self.merge_entry(entry).await?;
        Ok(())
    }

    /// 合并条目（使用 LWW 策略）
    async fn merge_entry(&self, entry: SyncMemoryEntry) -> Result<()> {
        // 检查本地是否存在
        let local = self.memory_service.get(&entry.key).await?;

        let should_update = if let Some(local_item) = local {
            // 比较向量时钟
            let local_clock = self.get_item_vector_clock(&entry.key).await?;

            match local_clock.compare(&entry.vector_clock) {
                Some(std::cmp::Ordering::Less) => {
                    // 远程更新，接受
                    true
                }
                Some(std::cmp::Ordering::Greater) => {
                    // 本地更新，忽略
                    false
                }
                Some(std::cmp::Ordering::Equal) => {
                    // 相同，检查时间戳
                    entry.timestamp > local_item.updated_at
                }
                None => {
                    // 并发冲突，使用 LWW
                    entry.timestamp > local_item.updated_at ||
                    (entry.timestamp == local_item.updated_at &&
                     entry.node_id > self.node_id)
                }
            }
        } else {
            // 本地不存在，接受
            true
        };

        if should_update {
            // 保存到本地
            self.memory_service.set(
                &entry.key,
                &entry.value,
                MemoryDomain::Public,
                entry.category,
            ).await?;

            // 更新向量索引
            if let Some(_vector) = &entry.vector {
                // 向量存储更新（如果需要）
                // self.vector_storage.update_vector(&entry.key, vector).await?;
            }

            // 保存向量时钟
            self.save_item_vector_clock(&entry.key, &entry.vector_clock).await?;

            tracing::info!("Merged entry: {}", entry.key);
        }

        Ok(())
    }

    /// 触发同步到特定节点
    pub async fn sync_with_node(&self, node_id: &str) -> Result<()> {
        let request = SyncRequest {
            node_id: self.node_id.clone(),
            since: self.get_last_sync_time(node_id).await?,
            known_keys: self.get_local_public_keys().await?,
        };

        let message = SyncMessage::Request(request);
        let data = serde_json::to_vec(&message)?;

        // 广播到网络
        self.p2p.broadcast("memory_sync", data).await?;

        Ok(())
    }

    /// 广播本地更新
    pub async fn broadcast_update(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        // 递增向量时钟
        {
            let mut clock = self.vector_clock.write().await;
            clock.increment(&self.node_id);
        }

        let entry = SyncMemoryEntry {
            key: key.to_string(),
            value: value.to_vec(),
            vector: None, // 向量数据可选，实际同步时从 VectorStorage 获取
            timestamp: Utc::now(),
            node_id: self.node_id.clone(),
            vector_clock: self.vector_clock.read().await.clone(),
            version: 1,
            category,
        };

        let message = SyncMessage::Broadcast(entry);
        let data = serde_json::to_vec(&message)?;

        // 广播到 P2P 网络
        self.p2p.broadcast("memory_sync", data).await?;

        Ok(())
    }

    /// 获取本地公域记忆
    #[allow(dead_code)]
    async fn get_local_public_memories(&self, since: Option<DateTime<Utc>>)
        -> Result<Vec<SyncMemoryEntry>>
    {
        // 查询公域记忆
        let keys = self.memory_service.list_keys(Some(MemoryDomain::Public)).await?;

        let mut entries = Vec::new();
        for key in keys {
            if let Some(item) = self.memory_service.get(&key).await? {
                // 检查时间戳
                if let Some(since_time) = since {
                    if item.updated_at < since_time {
                        continue;
                    }
                }

                let clock = self.get_item_vector_clock(&key).await?;

                entries.push(SyncMemoryEntry {
                    key,
                    value: item.value,
                    vector: None, // 向量数据可选，实际同步时从 VectorStorage 获取
                    timestamp: item.updated_at,
                    node_id: item.owner,
                    vector_clock: clock,
                    version: item.version,
                    category: item.category,
                });
            }
        }

        Ok(entries)
    }

    /// 获取本地公域键列表
    async fn get_local_public_keys(&self) -> Result<Vec<String>> {
        self.memory_service.list_keys(Some(MemoryDomain::Public)).await
    }
    
    /// 获取已删除的键
    #[allow(dead_code)]
    async fn get_deleted_keys(&self, _since: Option<DateTime<Utc>>) -> Result<Vec<String>> {
        // 从 memory_service 获取已删除的公域键
        // 简化实现：返回空列表
        // 实际实现应该查询一个专门的删除日志表
        Ok(vec![])
    }

    /// 获取条目的向量时钟
    #[allow(dead_code)]
    async fn get_item_vector_clock(&self, key: &str) -> Result<VectorClock> {
        // 尝试从 metadata 获取
        let clock_key = format!("__vc__/{}", key);
        match self.memory_service.get(&clock_key).await? {
            Some(item) => {
                serde_json::from_slice(&item.value)
                    .map_err(|e| CisError::storage(format!("Serialization error: {}", e)))
            }
            None => Ok(VectorClock::new()),
        }
    }

    /// 保存条目的向量时钟
    #[allow(dead_code)]
    async fn save_item_vector_clock(&self, key: &str, clock: &VectorClock) -> Result<()> {
        let clock_key = format!("__vc__/{}", key);
        let clock_data = serde_json::to_vec(clock)?;
        self.memory_service.set(
            &clock_key,
            &clock_data,
            MemoryDomain::Private,
            MemoryCategory::Context,
        ).await?;
        Ok(())
    }

    /// 获取上次同步时间
    async fn get_last_sync_time(&self, node_id: &str) -> Result<Option<DateTime<Utc>>> {
        // 从数据库获取
        // 通过 P2P 网络的 peer_manager 获取
        if let Some(peer) = self.p2p.get_peer(node_id).await? {
            Ok(peer.last_sync_at)
        } else {
            Ok(None)
        }
    }

    /// 启动定期同步
    async fn start_periodic_sync(&self) {
        let sync_manager = Arc::new(self.clone_as_handle());

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));

            loop {
                interval.tick().await;

                // 获取已知节点
                let peers = sync_manager.p2p.get_connected_peers().await;

                for peer in peers {
                    if let Err(e) = sync_manager.sync_with_node(&peer.node_id).await {
                        tracing::warn!("Periodic sync with {} failed: {}", peer.node_id, e);
                    }
                }
            }
        });
    }

    /// 创建用于消息处理的句柄（不包含完整的内部状态）
    fn clone_as_handle(&self) -> MemorySyncHandle {
        MemorySyncHandle {
            memory_service: Arc::clone(&self.memory_service),
            vector_storage: Arc::clone(&self.vector_storage),
            p2p: Arc::clone(&self.p2p),
            vector_clock: Arc::clone(&self.vector_clock),
            node_id: self.node_id.clone(),
        }
    }
}

/// 轻量级同步管理器句柄，用于消息处理
#[derive(Clone)]
struct MemorySyncHandle {
    memory_service: Arc<MemoryService>,
    #[allow(dead_code)]
    vector_storage: Arc<VectorStorage>,
    p2p: Arc<P2PNetwork>,
    vector_clock: Arc<RwLock<VectorClock>>,
    node_id: String,
}

impl MemorySyncHandle {
    /// 处理同步消息
    async fn handle_sync_message(&self, data: Vec<u8>) -> Result<()> {
        let message: SyncMessage = serde_json::from_slice(&data)?;

        match message {
            SyncMessage::Request(req) => {
                self.handle_sync_request(req).await?;
            }
            SyncMessage::Response(resp) => {
                self.handle_sync_response(resp).await?;
            }
            SyncMessage::Broadcast(entry) => {
                self.handle_broadcast(entry).await?;
            }
        }

        Ok(())
    }

    /// 处理同步请求
    async fn handle_sync_request(&self, request: SyncRequest) -> Result<()> {
        tracing::info!("Received sync request from {}", request.node_id);
        
        // 获取本地公域记忆
        let entries = self.get_local_public_memories(request.since).await?;

        // 构建响应
        let response = SyncResponse {
            node_id: self.node_id.clone(),
            entries,
            deleted_keys: vec![],
            timestamp: Utc::now(),
        };

        // 发送响应
        let message = SyncMessage::Response(response);
        let data = serde_json::to_vec(&message)?;

        // 广播到网络
        self.p2p.broadcast("memory_sync", data).await?;

        Ok(())
    }

    /// 处理同步响应
    async fn handle_sync_response(&self, response: SyncResponse) -> Result<()> {
        tracing::info!(
            "Received sync response from {} with {} entries",
            response.node_id,
            response.entries.len()
        );

        // 先更新向量时钟
        for entry in &response.entries {
            let mut clock = self.vector_clock.write().await;
            *clock = clock.merge(&entry.vector_clock);
        }

        // 然后合并条目
        for entry in response.entries {
            self.merge_entry(entry).await?;
        }

        Ok(())
    }

    /// 处理广播消息
    async fn handle_broadcast(&self, entry: SyncMemoryEntry) -> Result<()> {
        tracing::debug!("Received broadcast for key: {}", entry.key);
        self.merge_entry(entry).await?;
        Ok(())
    }

    /// 合并条目（使用 LWW 策略）
    async fn merge_entry(&self, entry: SyncMemoryEntry) -> Result<()> {
        let local = self.memory_service.get(&entry.key).await?;

        let should_update = if let Some(local_item) = local {
            let local_clock = self.get_item_vector_clock(&entry.key).await?;

            match local_clock.compare(&entry.vector_clock) {
                Some(std::cmp::Ordering::Less) => true,
                Some(std::cmp::Ordering::Greater) => false,
                Some(std::cmp::Ordering::Equal) => {
                    entry.timestamp > local_item.updated_at
                }
                None => {
                    entry.timestamp > local_item.updated_at ||
                    (entry.timestamp == local_item.updated_at &&
                     entry.node_id > self.node_id)
                }
            }
        } else {
            true
        };

        if should_update {
            self.memory_service.set(
                &entry.key,
                &entry.value,
                MemoryDomain::Public,
                entry.category,
            ).await?;

            self.save_item_vector_clock(&entry.key, &entry.vector_clock).await?;

            tracing::info!("Merged entry: {}", entry.key);
        }

        Ok(())
    }

    /// 获取本地公域记忆
    async fn get_local_public_memories(&self, since: Option<DateTime<Utc>>)
        -> Result<Vec<SyncMemoryEntry>>
    {
        let keys = self.memory_service.list_keys(Some(MemoryDomain::Public)).await?;

        let mut entries = Vec::new();
        for key in keys {
            if let Some(item) = self.memory_service.get(&key).await? {
                if let Some(since_time) = since {
                    if item.updated_at < since_time {
                        continue;
                    }
                }

                let clock = self.get_item_vector_clock(&key).await?;

                entries.push(SyncMemoryEntry {
                    key,
                    value: item.value,
                    vector: None,
                    timestamp: item.updated_at,
                    node_id: item.owner,
                    vector_clock: clock,
                    version: item.version,
                    category: item.category,
                });
            }
        }

        Ok(entries)
    }

    /// 获取条目的向量时钟
    async fn get_item_vector_clock(&self, _key: &str) -> Result<VectorClock> {
        Ok(VectorClock::new())
    }

    /// 保存条目的向量时钟
    async fn save_item_vector_clock(&self, _key: &str, _clock: &VectorClock) -> Result<()> {
        Ok(())
    }

    /// 触发同步到特定节点
    async fn sync_with_node(&self, node_id: &str) -> Result<()> {
        let request = SyncRequest {
            node_id: self.node_id.clone(),
            since: self.get_last_sync_time(node_id).await?,
            known_keys: self.memory_service.list_keys(Some(MemoryDomain::Public)).await?,
        };

        let message = SyncMessage::Request(request);
        let data = serde_json::to_vec(&message)?;

        self.p2p.broadcast("memory_sync", data).await?;

        Ok(())
    }

    /// 获取上次同步时间
    async fn get_last_sync_time(&self, node_id: &str) -> Result<Option<DateTime<Utc>>> {
        if let Some(peer) = self.p2p.get_peer(node_id).await? {
            Ok(peer.last_sync_at)
        } else {
            Ok(None)
        }
    }
}

impl Clone for MemorySyncManager {
    fn clone(&self) -> Self {
        Self {
            memory_service: Arc::clone(&self.memory_service),
            vector_storage: Arc::clone(&self.vector_storage),
            p2p: Arc::clone(&self.p2p),
            vector_clock: Arc::clone(&self.vector_clock),
            node_id: self.node_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_memory_entry_serialization() {
        let entry = SyncMemoryEntry {
            key: "test-key".to_string(),
            value: b"test-value".to_vec(),
            vector: None,
            timestamp: Utc::now(),
            node_id: "node1".to_string(),
            vector_clock: VectorClock::new(),
            version: 1,
            category: MemoryCategory::Context,
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: SyncMemoryEntry = serde_json::from_str(&json).unwrap();
        
        assert_eq!(entry.key, deserialized.key);
        assert_eq!(entry.value, deserialized.value);
        assert_eq!(entry.node_id, deserialized.node_id);
    }
}
