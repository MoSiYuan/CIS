//! # Offline Queue for P2P Messages
//!
//! Provides message persistence and retry mechanism for weak network conditions.
//! Messages are cached when send fails and automatically retried after reconnection.

use crate::error::{CisError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use super::peer::Message;

/// Maximum default queue size
const DEFAULT_MAX_QUEUE_SIZE: usize = 1000;

/// Queued message with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    /// Target node ID (None = broadcast)
    pub target: Option<String>,
    /// Message payload
    pub message: Message,
    /// Timestamp when queued
    pub queued_at: DateTime<Utc>,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Last error (if any)
    pub last_error: Option<String>,
}

impl QueuedMessage {
    /// Create a new queued message
    pub fn new(target: Option<String>, message: Message) -> Self {
        Self {
            target,
            message,
            queued_at: Utc::now(),
            retry_count: 0,
            last_error: None,
        }
    }

    /// Increment retry counter
    pub fn increment_retry(&mut self, error: String) {
        self.retry_count += 1;
        self.last_error = Some(error);
    }

    /// Check if message has exceeded max retries
    pub fn exceeded_max_retries(&self, max_retries: u32) -> bool {
        self.retry_count >= max_retries
    }
}

/// Offline queue configuration
#[derive(Debug, Clone)]
pub struct OfflineQueueConfig {
    /// Maximum queue size
    pub max_size: usize,
    /// Persist to disk
    pub persist_to_disk: bool,
    /// Storage path (if persisting)
    pub storage_path: Option<PathBuf>,
    /// Maximum retry attempts per message
    pub max_retries: u32,
    /// Retry interval (seconds)
    pub retry_interval_secs: u64,
}

impl Default for OfflineQueueConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_MAX_QUEUE_SIZE,
            persist_to_disk: false,
            storage_path: None,
            max_retries: 5,
            retry_interval_secs: 60,
        }
    }
}

/// Offline queue for P2P messages
///
/// Caches messages when send fails and provides retry mechanism.
pub struct OfflineQueue {
    /// Message queue
    queue: Arc<Mutex<VecDeque<QueuedMessage>>>,
    /// Configuration
    config: OfflineQueueConfig,
    /// Queue statistics
    stats: Arc<RwLock<QueueStats>>,
}

/// Queue statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QueueStats {
    /// Total queued messages
    pub total_queued: u64,
    /// Total successfully sent
    pub total_sent: u64,
    /// Total failed (after max retries)
    pub total_failed: u64,
    /// Current queue size
    pub current_size: usize,
    /// Last retry time
    pub last_retry_at: Option<DateTime<Utc>>,
}

impl OfflineQueue {
    /// Create a new offline queue
    pub fn new(config: OfflineQueueConfig) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
            stats: Arc::new(RwLock::new(QueueStats::default())),
        }
    }

    /// Create with default configuration
    pub fn default_queue() -> Self {
        Self::new(OfflineQueueConfig::default())
    }

    /// Enqueue a message
    ///
    /// Returns error if queue is full.
    pub async fn enqueue(&self, target: Option<String>, message: Message) -> Result<()> {
        let mut queue = self.queue.lock().await;

        // Check queue size
        if queue.len() >= self.config.max_size {
            return Err(CisError::network("Offline queue is full".to_string()));
        }

        // Create and add queued message
        let queued = QueuedMessage::new(target, message);
        queue.push_back(queued);

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_queued += 1;
        stats.current_size = queue.len();

        tracing::debug!(
            target: "offline_queue",
            queued_at = %queued.queued_at,
            queue_size = queue.len(),
            "Message enqueued to offline queue"
        );

        // Persist if enabled
        if self.config.persist_to_disk {
            if let Err(e) = self.persist().await {
                tracing::warn!("Failed to persist offline queue: {}", e);
            }
        }

        Ok(())
    }

    /// Retry sending queued messages
    ///
    /// Attempts to send all queued messages using the provided sender function.
    /// Returns number of successfully sent messages.
    pub async fn retry_send<F, Fut>(&self, mut sender: F) -> Result<usize>
    where
        F: FnMut(Option<String>, Message) -> Fut,
        Fut: std::future::Future<Output = Result<()>>,
    {
        let mut queue = self.queue.lock().await;
        let mut sent_count = 0;
        let mut failed_count = 0;
        let mut new_queue = VecDeque::new();

        // Process each queued message
        while let Some(mut queued) = queue.pop_front() {
            // Check max retries
            if queued.exceeded_max_retries(self.config.max_retries) {
                tracing::warn!(
                    target: "offline_queue",
                    target = ?queued.target,
                    retry_count = queued.retry_count,
                    "Message exceeded max retries, discarding"
                );
                failed_count += 1;
                continue;
            }

            // Try to send
            let result = sender(queued.target.clone(), queued.message.clone()).await;

            match result {
                Ok(()) => {
                    sent_count += 1;
                    tracing::debug!(
                        target: "offline_queue",
                        target = ?queued.target,
                        "Message successfully sent from offline queue"
                    );
                }
                Err(e) => {
                    queued.increment_retry(e.to_string());
                    new_queue.push_back(queued);
                }
            }
        }

        // Replace queue with remaining messages
        *queue = new_queue;

        // Update stats
        let mut stats = self.stats.write().await;
        stats.total_sent += sent_count as u64;
        stats.total_failed += failed_count as u64;
        stats.current_size = queue.len();
        stats.last_retry_at = Some(Utc::now());

        // Persist if enabled
        if self.config.persist_to_disk {
            if let Err(e) = self.persist().await {
                tracing::warn!("Failed to persist offline queue: {}", e);
            }
        }

        tracing::info!(
            target: "offline_queue",
            sent = sent_count,
            failed = failed_count,
            remaining = queue.len(),
            "Offline queue retry completed"
        );

        Ok(sent_count)
    }

    /// Get current queue size
    pub async fn size(&self) -> usize {
        self.queue.lock().await.len()
    }

    /// Check if queue is empty
    pub async fn is_empty(&self) -> bool {
        self.queue.lock().await.is_empty()
    }

    /// Clear all queued messages
    pub async fn clear(&self) -> Result<()> {
        let mut queue = self.queue.lock().await;
        queue.clear();

        let mut stats = self.stats.write().await;
        stats.current_size = 0;

        tracing::info!("Offline queue cleared");

        // Persist if enabled
        if self.config.persist_to_disk {
            if let Err(e) = self.persist().await {
                tracing::warn!("Failed to persist offline queue: {}", e);
            }
        }

        Ok(())
    }

    /// Get queue statistics
    pub async fn stats(&self) -> QueueStats {
        self.stats.read().await.clone()
    }

    /// Persist queue to disk
    async fn persist(&self) -> Result<()> {
        if !self.config.persist_to_disk {
            return Ok(());
        }

        let storage_path = self.config.storage_path.as_ref()
            .ok_or_else(|| CisError::network("Storage path not configured".to_string()))?;

        // Ensure directory exists
        if let Some(parent) = storage_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| CisError::io(format!("Failed to create queue directory: {}", e)))?;
        }

        // Serialize and write
        let queue = self.queue.lock().await;
        let data: Vec<&QueuedMessage> = queue.iter().collect();
        let json = serde_json::to_string_pretty(&data)
            .map_err(|e| CisError::serialization(format!("Failed to serialize queue: {}", e)))?;

        tokio::fs::write(storage_path, json)
            .map_err(|e| CisError::io(format!("Failed to write queue file: {}", e)))?;

        tracing::debug!(
            target = "offline_queue",
            path = %storage_path.display(),
            size = queue.len(),
            "Offline queue persisted"
        );

        Ok(())
    }

    /// Load queue from disk
    pub async fn load(&self) -> Result<()> {
        if !self.config.persist_to_disk {
            return Ok(());
        }

        let storage_path = self.config.storage_path.as_ref()
            .ok_or_else(|| CisError::network("Storage path not configured".to_string()))?;

        // Check if file exists
        if !tokio::fs::try_exists(storage_path).await
            .map_err(|e| CisError::io(format!("Failed to check queue file: {}", e)))?
        {
            return Ok(());
        }

        // Read and deserialize
        let data = tokio::fs::read_to_string(storage_path).await
            .map_err(|e| CisError::io(format!("Failed to read queue file: {}", e)))?;

        let messages: Vec<QueuedMessage> = serde_json::from_str(&data)
            .map_err(|e| CisError::serialization(format!("Failed to deserialize queue: {}", e)))?;

        // Load into queue
        let mut queue = self.queue.lock().await;
        *queue = messages.into_iter().collect();

        let mut stats = self.stats.write().await;
        stats.current_size = queue.len();

        tracing::info!(
            target = "offline_queue",
            path = %storage_path.display(),
            size = queue.len(),
            "Offline queue loaded from disk"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_enqueue_message() {
        let queue = OfflineQueue::default_queue();

        let message = Message::Text("test".to_string());
        queue.enqueue(Some("node-1".to_string()), message).await.unwrap();

        assert_eq!(queue.size().await, 1);
        assert!(!queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let config = OfflineQueueConfig {
            max_size: 2,
            ..Default::default()
        };
        let queue = OfflineQueue::new(config);

        let message = Message::Text("test".to_string());
        queue.enqueue(None, message.clone()).await.unwrap();
        queue.enqueue(None, message.clone()).await.unwrap();

        // Third message should fail
        let result = queue.enqueue(None, message).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_retry_send() {
        let queue = OfflineQueue::default_queue();

        // Enqueue messages
        queue.enqueue(Some("node-1".to_string()), Message::Text("msg1".to_string())).await.unwrap();
        queue.enqueue(Some("node-2".to_string()), Message::Text("msg2".to_string())).await.unwrap();

        // Mock sender that succeeds
        let sent = queue.retry_send(|_, _| async { Ok(()) }).await.unwrap();
        assert_eq!(sent, 2);
        assert!(queue.is_empty().await);
    }

    #[tokio::test]
    async fn test_retry_send_partial_failure() {
        let queue = OfflineQueue::default_queue();

        queue.enqueue(Some("node-1".to_string()), Message::Text("msg1".to_string())).await.unwrap();
        queue.enqueue(Some("node-2".to_string()), Message::Text("msg2".to_string())).await.unwrap();

        // Mock sender that fails for node-1
        let mut call_count = 0;
        let sent = queue.retry_send(|target, _| {
            call_count += 1;
            async move {
                if call_count == 1 {
                    Err(CisError::network("Send failed".to_string()))
                } else {
                    Ok(())
                }
            }
        }).await.unwrap();

        assert_eq!(sent, 1);
        assert_eq!(queue.size().await, 1);
    }

    #[tokio::test]
    async fn test_clear_queue() {
        let queue = OfflineQueue::default_queue();

        queue.enqueue(None, Message::Text("test".to_string())).await.unwrap();
        assert_eq!(queue.size().await, 1);

        queue.clear().await.unwrap();
        assert_eq!(queue.size().await, 0);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let queue = OfflineQueue::default_queue();

        queue.enqueue(None, Message::Text("msg1".to_string())).await.unwrap();
        queue.enqueue(None, Message::Text("msg2".to_string())).await.unwrap();

        let stats = queue.stats().await;
        assert_eq!(stats.total_queued, 2);
        assert_eq!(stats.current_size, 2);
        assert_eq!(stats.total_sent, 0);
    }
}
