//! 断线同步队列消费者
//!
//! 定期消费 pending_sync 表中的任务，从远端节点获取缺失事件

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, timeout};
use tracing::{debug, error, info, warn};

use crate::error::Result;
use crate::matrix::error::MatrixResult;
use crate::matrix::store::MatrixStore;
use crate::matrix::websocket::{
    protocol::{EventMessage, SyncRequest, SyncResponse, WsMessage},
    tunnel::TunnelManager,
};
use crate::storage::federation_db::{FederationDb, SyncTask};

/// 同步消费者配置
#[derive(Debug, Clone)]
pub struct SyncConfig {
    /// 消费间隔（秒）
    pub interval_secs: u64,
    /// 每次处理的最大任务数
    pub batch_size: usize,
    /// 最大重试次数
    pub max_retries: i32,
    /// 重试退避基数（秒）
    pub retry_backoff_secs: u64,
    /// 是否启用自动同步
    pub enabled: bool,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            interval_secs: 30,
            batch_size: 10,
            max_retries: 5,
            retry_backoff_secs: 60,
            enabled: true,
        }
    }
}

/// 同步结果
#[derive(Debug, Clone)]
pub enum SyncResult {
    /// 成功同步 N 个事件
    Success { events_count: usize },
    /// 节点离线，已重排
    PeerOffline,
    /// 同步失败，已增加重试计数
    Failed { error: String },
    /// 任务已完成（无需同步）
    AlreadyUpToDate,
}

/// 等待中的同步请求
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct PendingRequest {
    /// 请求 ID
    request_id: String,
    /// 目标节点
    target_node: String,
    /// 创建时间
    created_at: tokio::time::Instant,
}

/// 断线同步消费者
///
/// 后台任务，定期从 pending_sync 表中读取任务，
/// 通过 WebSocket 向目标节点请求缺失事件
pub struct SyncConsumer {
    /// 联邦数据库
    federation_db: Arc<Mutex<FederationDb>>,
    /// WebSocket 隧道管理器
    tunnel_manager: Option<Arc<TunnelManager>>,
    /// Matrix 存储
    store: Arc<MatrixStore>,
    /// 配置
    config: SyncConfig,
    /// 运行状态
    running: Arc<std::sync::atomic::AtomicBool>,
    /// 等待中的请求 (request_id -> PendingRequest)
    pending_requests: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<SyncResponse>>>>,
}

impl SyncConsumer {
    /// 创建新的同步消费者
    pub fn new(
        federation_db: Arc<Mutex<FederationDb>>,
        store: Arc<MatrixStore>,
    ) -> Self {
        Self {
            federation_db,
            tunnel_manager: None,
            store,
            config: SyncConfig::default(),
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 配置 WebSocket 隧道管理器
    pub fn with_tunnel_manager(mut self, tunnel_manager: Arc<TunnelManager>) -> Self {
        self.tunnel_manager = Some(tunnel_manager);
        self
    }

    /// 设置配置
    pub fn with_config(mut self, config: SyncConfig) -> Self {
        self.config = config;
        self
    }

    /// 启动后台同步任务
    pub fn spawn(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let running = self.running.clone();
        running.store(true, std::sync::atomic::Ordering::SeqCst);

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(self.config.interval_secs));

            while running.load(std::sync::atomic::Ordering::SeqCst) {
                ticker.tick().await;

                if !self.config.enabled {
                    continue;
                }

                if let Err(e) = self.process_batch().await {
                    error!("Sync batch failed: {}", e);
                }
            }

            info!("Sync consumer stopped");
        })
    }

    /// 停止同步任务
    pub fn stop(&self) {
        self.running
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// 处理一批同步任务
    async fn process_batch(&self) -> Result<()> {
        let tasks = {
            let db = self.federation_db.lock().await;
            db.get_pending_tasks(self.config.batch_size)?
        };

        if tasks.is_empty() {
            return Ok(());
        }

        info!("Processing {} sync tasks", tasks.len());

        for task in tasks {
            match self.process_task(&task).await {
                Ok(result) => {
                    debug!("Sync task {} completed: {:?}", task.id.unwrap_or(-1), result);

                    // 根据结果处理任务
                    match result {
                        SyncResult::Success { .. } | SyncResult::AlreadyUpToDate => {
                            // 完成任务
                            if let Err(e) = self.complete_task(task.id.unwrap()).await {
                                error!("Failed to complete sync task: {}", e);
                            }
                        }
                        SyncResult::PeerOffline => {
                            // 节点离线，稍后重试
                            debug!("Peer {} is offline, will retry later", task.target_node);
                        }
                        SyncResult::Failed { error } => {
                            // 增加重试计数
                            warn!("Sync task failed: {}, error: {}", task.id.unwrap_or(-1), error);
                            if let Err(e) = self.retry_task(task.id.unwrap()).await {
                                error!("Failed to retry sync task: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Sync task {} error: {}", task.id.unwrap_or(-1), e);
                    if let Err(e) = self.retry_task(task.id.unwrap()).await {
                        error!("Failed to retry sync task: {}", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// 处理单个同步任务
    async fn process_task(&self, task: &SyncTask) -> Result<SyncResult> {
        debug!(
            "Processing sync task: target={}, room={}, since={}",
            task.target_node, task.room_id, task.since_event_id
        );

        // 检查是否有 WebSocket 隧道管理器
        let tunnel_manager = match &self.tunnel_manager {
            Some(tm) => tm,
            None => {
                return Ok(SyncResult::Failed {
                    error: "No tunnel manager available".to_string(),
                });
            }
        };

        // 检查目标节点是否在线
        if !tunnel_manager.is_connected(&task.target_node).await {
            return Ok(SyncResult::PeerOffline);
        }

        // 构建同步请求
        let request = SyncRequest::new(
            task.room_id.clone(),
            Some(task.since_event_id.clone()),
            100, // 每次最多 100 个事件
        );

        // 生成请求 ID
        let request_id = uuid::Uuid::new_v4().to_string();
        
        // 创建 oneshot channel 等待响应
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        // 注册等待中的请求
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(request_id.clone(), tx);
        }
        
        // 发送同步请求
        let request_msg = WsMessage::SyncRequest(request);

        match tunnel_manager
            .send_message(&task.target_node, request_msg)
            .await
        {
            Ok(_) => {
                info!("Sent sync request {} to {}", request_id, task.target_node);
                
                // 等待响应（带超时）
                match timeout(Duration::from_secs(30), rx).await {
                    Ok(Ok(response)) => {
                        // 收到响应，处理事件
                        let events_count = response.events.len();
                        info!("Received sync response for {}: {} events", request_id, events_count);
                        
                        // 保存同步的事件
                        let mut inserted = 0;
                        for event in response.events {
                            match self.save_synced_event(&event).await {
                                Ok(_) => inserted += 1,
                                Err(e) => {
                                    error!("Failed to save synced event {}: {}", event.message_id, e);
                                }
                            }
                        }
                        
                        Ok(SyncResult::Success { events_count: inserted })
                    }
                    Ok(Err(_)) => {
                        warn!("Sync response channel closed for {}", request_id);
                        Ok(SyncResult::Failed {
                            error: "Response channel closed".to_string(),
                        })
                    }
                    Err(_) => {
                        warn!("Sync request {} timed out", request_id);
                        // 超时后清理请求
                        let mut pending = self.pending_requests.write().await;
                        pending.remove(&request_id);
                        Ok(SyncResult::Failed {
                            error: "Request timeout".to_string(),
                        })
                    }
                }
            }
            Err(e) => {
                // 发送失败，清理请求
                let mut pending = self.pending_requests.write().await;
                pending.remove(&request_id);
                Ok(SyncResult::Failed {
                    error: format!("Failed to send sync request: {}", e),
                })
            }
        }
    }

    /// 处理同步响应
    /// 
    /// 将响应发送给等待中的请求
    pub async fn handle_sync_response(
        &self,
        from_node: &str,
        response: SyncResponse,
    ) -> Result<usize> {
        let events_count = response.events.len();
        info!(
            "Received sync response from {}: {} events",
            from_node,
            events_count
        );
        
        // 检查是否有等待中的请求 (使用 response 中的 request_id 如果存在)
        // 注意：目前 SyncResponse 没有 request_id 字段，我们假设是最近的请求
        // 在实际实现中，SyncResponse 应该包含对应的 request_id
        let request_id = {
            let pending = self.pending_requests.read().await;
            // 找到第一个匹配的请求（简化处理）
            pending.keys().next().cloned()
        };
        
        if let Some(request_id) = request_id {
            let sender = {
                let mut pending = self.pending_requests.write().await;
                pending.remove(&request_id)
            };
            
            if let Some(sender) = sender {
                // 发送响应给等待的 task
                let _ = sender.send(response.clone());
                info!("Delivered sync response to request {}", request_id);
            }
        }

        let mut inserted = 0;

        for event in response.events {
            // 转换并保存事件到本地存储
            match self.save_synced_event(&event).await {
                Ok(_) => inserted += 1,
                Err(e) => {
                    error!("Failed to save synced event {}: {}", event.message_id, e);
                }
            }
        }

        // 记录联邦日志
        {
            let db = self.federation_db.lock().await;
            db.log_federation(&crate::storage::federation_db::FederationLog {
                direction: "in".to_string(),
                node_id: from_node.to_string(),
                event_type: "sync.response".to_string(),
                event_id: format!("batch-{}-events", events_count),
                size_bytes: Some(inserted as i32),
                status: "success".to_string(),
            })?;
        }

        Ok(inserted)
    }

    /// 保存同步的事件
    async fn save_synced_event(&self, event: &EventMessage) -> MatrixResult<()> {
        
        
        // 将 EventMessage 转换为 Matrix 存储格式
        // EventMessage 中的 event_data 是字节数组，需要解析

        // 生成 event_id（如果 event.message_id 是 Matrix 格式，直接使用）
        let event_id = format!("${}", event.message_id);
        
        // 检查事件是否已存在
        if self.store.event_exists(&event_id)? {
            debug!("Event {} already exists, skipping", event_id);
            return Ok(());
        }

        // 解析 event_data 为 JSON
        let content: serde_json::Value = match serde_json::from_slice(&event.event_data) {
            Ok(v) => v,
            Err(e) => {
                return Err(crate::matrix::error::MatrixError::Store(
                    format!("Failed to parse event data: {}", e)
                ));
            }
        };

        // 保存事件
        let room_id = event.room_id.clone().unwrap_or_else(|| "!unknown:cis.local".to_string());
        self.store.save_raw_event(
            &room_id,
            &event_id,
            &event.event_type,
            &content,
            event.timestamp as i64,
        )?;

        Ok(())
    }

    /// 完成同步任务
    async fn complete_task(&self, task_id: i64) -> Result<()> {
        let db = self.federation_db.lock().await;
        db.complete_sync_task(task_id)?;
        Ok(())
    }

    /// 增加重试计数
    async fn retry_task(&self, task_id: i64) -> Result<()> {
        let db = self.federation_db.lock().await;
        db.increment_retry(task_id)?;

        // 检查是否超过最大重试次数
        // 如果超过，可以移动到 dead letter 队列或记录错误
        Ok(())
    }

    /// 手动触发同步（用于测试或紧急同步）
    pub async fn trigger_sync(&self, target_node: &str, room_id: &str) -> Result<SyncResult> {
        let task = SyncTask {
            id: None,
            target_node: target_node.to_string(),
            room_id: room_id.to_string(),
            since_event_id: "$0".to_string(), // 从头开始同步
            priority: 0,
        };

        self.process_task(&task).await
    }

    /// 获取同步队列状态
    pub async fn queue_status(&self) -> Result<QueueStatus> {
        let db = self.federation_db.lock().await;

        // 获取待处理任务数
        let pending_tasks = db.get_pending_tasks(10000)?;
        let pending_count = pending_tasks.len();

        // 按节点统计
        let mut per_node: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for task in &pending_tasks {
            *per_node.entry(task.target_node.clone()).or_insert(0) += 1;
        }

        Ok(QueueStatus {
            pending_count,
            per_node,
        })
    }
}

/// 队列状态
#[derive(Debug, Clone)]
pub struct QueueStatus {
    pub pending_count: usize,
    pub per_node: std::collections::HashMap<String, usize>,
}

impl std::fmt::Display for QueueStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pending sync tasks: {}", self.pending_count)?;
        for (node, count) in &self.per_node {
            write!(f, "\n  - {}: {}", node, count)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // 这些测试需要完整的依赖注入，这里仅作为结构示例
    // 实际测试应该在集成测试环境中进行

    #[tokio::test]
    async fn test_sync_config_default() {
        let config = SyncConfig::default();
        assert_eq!(config.interval_secs, 30);
        assert_eq!(config.batch_size, 10);
        assert!(config.enabled);
    }

    #[tokio::test]
    async fn test_queue_status_display() {
        let status = QueueStatus {
            pending_count: 5,
            per_node: [
                ("node1".to_string(), 3),
                ("node2".to_string(), 2),
            ]
            .into_iter()
            .collect(),
        };

        let display = format!("{}", status);
        assert!(display.contains("Pending sync tasks: 5"));
        assert!(display.contains("node1: 3"));
        assert!(display.contains("node2: 2"));
    }
}
