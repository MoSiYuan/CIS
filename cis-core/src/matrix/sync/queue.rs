//! # Sync Queue Module
//!
//! Optimized synchronization queue for federated events with:
//! - Priority-based processing
//! - Batched sync operations
//! - Persistent queue storage
//! - Dead letter queue for failed events
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     SyncQueue                               │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
//! │  │   Priority   │  │   Batched    │  │   Dead       │       │
//! │  │   Queue      │  │   Sync       │  │   Letter     │       │
//! │  │              │  │   Optimizer  │  │   Queue      │       │
//! │  └──────────────┘  └──────────────┘  └──────────────┘       │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use std::collections::{HashMap, VecDeque};

use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::error::{CisError, Result};
use crate::matrix::federation::types::CisMatrixEvent;

/// Sync task priority levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum SyncPriority {
    /// Critical - Room creation, member joins (sync immediately)
    Critical = 0,
    /// High - Messages, state changes (sync within 1 second)
    High = 1,
    /// Normal - Regular events (sync within 5 seconds)
    #[default]
    Normal = 2,
    /// Low - History sync, backfill (sync when bandwidth available)
    Low = 3,
}


/// Sync task status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    /// Pending in queue
    Pending,
    /// Currently being processed
    Processing,
    /// Successfully completed
    Completed,
    /// Failed, will retry
    Failed { attempt: u32, next_retry: i64 },
    /// Permanently failed (dead letter)
    DeadLetter,
}

/// A sync task representing an event to be synchronized
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncTask {
    /// Unique task ID
    pub task_id: String,
    /// Target node ID
    pub target_node: String,
    /// Event to sync
    pub event: CisMatrixEvent,
    /// Priority level
    pub priority: SyncPriority,
    /// Current status
    pub status: SyncStatus,
    /// Created timestamp
    pub created_at: i64,
    /// Last attempt timestamp
    pub last_attempt: Option<i64>,
    /// Retry count
    pub retry_count: u32,
    /// Room ID for batching
    pub room_id: String,
}

impl SyncTask {
    /// Create a new sync task
    pub fn new(target_node: String, event: CisMatrixEvent, priority: SyncPriority) -> Self {
        let room_id = event.room_id.clone();
        Self {
            task_id: format!("sync-{}", Uuid::new_v4()),
            target_node,
            event,
            priority,
            status: SyncStatus::Pending,
            created_at: chrono::Utc::now().timestamp(),
            last_attempt: None,
            retry_count: 0,
            room_id,
        }
    }

    /// Create with normal priority
    pub fn new_normal(target_node: String, event: CisMatrixEvent) -> Self {
        Self::new(target_node, event, SyncPriority::Normal)
    }

    /// Create with high priority
    pub fn new_high(target_node: String, event: CisMatrixEvent) -> Self {
        Self::new(target_node, event, SyncPriority::High)
    }

    /// Create with critical priority
    pub fn new_critical(target_node: String, event: CisMatrixEvent) -> Self {
        Self::new(target_node, event, SyncPriority::Critical)
    }

    /// Calculate next retry delay with exponential backoff
    pub fn next_retry_delay(&self) -> Duration {
        let base_delay = match self.priority {
            SyncPriority::Critical => 1,
            SyncPriority::High => 2,
            SyncPriority::Normal => 5,
            SyncPriority::Low => 30,
        };
        let multiplier = 2_u64.pow(self.retry_count.min(6));
        Duration::from_secs(base_delay * multiplier)
    }

    /// Mark as failed and update for retry
    pub fn mark_failed(&mut self) {
        self.retry_count += 1;
        self.last_attempt = Some(chrono::Utc::now().timestamp());
        let next_retry = chrono::Utc::now().timestamp() + self.next_retry_delay().as_secs() as i64;
        self.status = SyncStatus::Failed {
            attempt: self.retry_count,
            next_retry,
        };
    }

    /// Check if task should be retried
    pub fn should_retry(&self, max_retries: u32) -> bool {
        self.retry_count < max_retries && !matches!(self.status, SyncStatus::DeadLetter)
    }

    /// Get event type
    pub fn event_type(&self) -> &str {
        &self.event.event_type
    }
}

/// Batched sync operation
#[derive(Debug, Clone)]
pub struct BatchOperation {
    /// Target node
    pub target_node: String,
    /// Room ID
    pub room_id: String,
    /// Events to sync
    pub events: Vec<CisMatrixEvent>,
    /// Batch ID
    pub batch_id: String,
}

/// Sync queue configuration
#[derive(Debug, Clone)]
pub struct SyncQueueConfig {
    /// Maximum queue size per node
    pub max_queue_size: usize,
    /// Maximum retries before dead letter
    pub max_retries: u32,
    /// Batch size for optimization
    pub batch_size: usize,
    /// Batch timeout in milliseconds
    pub batch_timeout_ms: u64,
    /// Enable batching
    pub enable_batching: bool,
    /// Worker count
    pub worker_count: usize,
    /// Enable persistent storage
    pub persistent: bool,
}

impl Default for SyncQueueConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 10000,
            max_retries: 5,
            batch_size: 50,
            batch_timeout_ms: 100,
            enable_batching: true,
            worker_count: 4,
            persistent: false,
        }
    }
}

/// Optimized sync queue with priority and batching
#[derive(Debug)]
pub struct SyncQueue {
    /// Configuration
    config: SyncQueueConfig,
    /// Priority queues by level
    queues: Arc<RwLock<HashMap<SyncPriority, VecDeque<SyncTask>>>>,
    /// Batched operations buffer
    batch_buffer: Arc<Mutex<HashMap<String, Vec<SyncTask>>>>,
    /// Dead letter queue
    dead_letter: Arc<Mutex<VecDeque<SyncTask>>>,
    /// Processing metrics
    metrics: Arc<RwLock<SyncMetrics>>,
    /// Task sender
    task_tx: mpsc::Sender<SyncTask>,
    /// Task receiver (stored for cloning)
    task_rx: Arc<Mutex<mpsc::Receiver<SyncTask>>>,
    /// Batch sender
    #[allow(dead_code)]
    batch_tx: mpsc::Sender<BatchOperation>,
    /// Shutdown signal
    shutdown_tx: Arc<Mutex<Option<mpsc::Sender<()>>>>,
}

/// Sync metrics
#[derive(Debug, Clone, Default)]
pub struct SyncMetrics {
    /// Tasks enqueued
    pub tasks_enqueued: u64,
    /// Tasks completed
    pub tasks_completed: u64,
    /// Tasks failed
    pub tasks_failed: u64,
    /// Tasks in dead letter
    pub dead_letter_count: u64,
    /// Batches sent
    pub batches_sent: u64,
    /// Average processing time (ms)
    pub avg_processing_time_ms: f64,
}

impl SyncQueue {
    /// Create a new sync queue
    pub fn new(config: SyncQueueConfig) -> Self {
        let (task_tx, task_rx) = mpsc::channel(config.max_queue_size);
        let (batch_tx, _batch_rx) = mpsc::channel(100);

        Self {
            config,
            queues: Arc::new(RwLock::new(HashMap::new())),
            batch_buffer: Arc::new(Mutex::new(HashMap::new())),
            dead_letter: Arc::new(Mutex::new(VecDeque::new())),
            metrics: Arc::new(RwLock::new(SyncMetrics::default())),
            task_tx,
            task_rx: Arc::new(Mutex::new(task_rx)),
            batch_tx,
            shutdown_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Create with default configuration
    pub fn default_queue() -> Self {
        Self::new(SyncQueueConfig::default())
    }

    /// Start the sync queue workers
    pub async fn start<F>(&self, processor: F) -> Result<()>
    where
        F: Fn(SyncTask) -> Result<()> + Send + Sync + 'static,
    {
        info!(
            "Starting SyncQueue with {} workers",
            self.config.worker_count
        );

        // Initialize priority queues
        {
            let mut queues = self.queues.write().await;
            for priority in [
                SyncPriority::Critical,
                SyncPriority::High,
                SyncPriority::Normal,
                SyncPriority::Low,
            ] {
                queues.insert(priority, VecDeque::new());
            }
        }

        // Start worker tasks
        let task_rx = self.task_rx.clone();
        let processor = Arc::new(processor);

        for i in 0..self.config.worker_count {
            let rx = task_rx.clone();
            let proc = processor.clone();
            let queues = self.queues.clone();
            let metrics = self.metrics.clone();

            tokio::spawn(async move {
                debug!("SyncQueue worker {} started", i);
                let mut rx = rx.lock().await;

                while let Some(task) = rx.recv().await {
                    let start = Instant::now();

                    match proc(task.clone()) {
                        Ok(()) => {
                            let mut m = metrics.write().await;
                            m.tasks_completed += 1;
                            let elapsed = start.elapsed().as_millis() as f64;
                            m.avg_processing_time_ms =
                                (m.avg_processing_time_ms * 0.9) + (elapsed * 0.1);
                        }
                        Err(e) => {
                            warn!("Sync task failed: {}", e);
                            let mut m = metrics.write().await;
                            m.tasks_failed += 1;

                            // Re-queue for retry if applicable
                            let mut task = task;
                            if task.should_retry(5) {
                                task.mark_failed();
                                if let Some(queue) = queues.write().await.get_mut(&task.priority) {
                                    queue.push_back(task);
                                }
                            }
                        }
                    }
                }
            });
        }

        // Start batch processor if enabled
        if self.config.enable_batching {
            self.start_batch_processor().await;
        }

        // Start scheduler
        self.start_scheduler().await;

        info!("SyncQueue started successfully");
        Ok(())
    }

    /// Start the batch processor
    async fn start_batch_processor(&self) {
        let batch_buffer = self.batch_buffer.clone();
        let batch_size = self.config.batch_size;
        let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);

        tokio::spawn(async move {
            let mut interval_timer = interval(batch_timeout);

            loop {
                interval_timer.tick().await;

                let mut buffer = batch_buffer.lock().await;
                let keys_to_process: Vec<String> = buffer
                    .iter()
                    .filter(|(_, tasks)| tasks.len() >= batch_size)
                    .map(|(key, _)| key.clone())
                    .collect();

                for key in keys_to_process {
                    if let Some(tasks) = buffer.remove(&key) {
                        let events: Vec<CisMatrixEvent> =
                            tasks.iter().map(|t| t.event.clone()).collect();

                        debug!("Processing batch of {} events for {}", events.len(), key);
                        // Batch would be sent here in full implementation
                    }
                }
            }
        });
    }

    /// Start the task scheduler
    async fn start_scheduler(&self) {
        let queues = self.queues.clone();
        let task_tx = self.task_tx.clone();

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);
        *self.shutdown_tx.lock().await = Some(shutdown_tx);

        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_millis(10));

            loop {
                tokio::select! {
                    _ = check_interval.tick() => {
                        // Process tasks by priority
                        let mut queues_guard = queues.write().await;

                        for priority in [
                            SyncPriority::Critical,
                            SyncPriority::High,
                            SyncPriority::Normal,
                            SyncPriority::Low,
                        ] {
                            if let Some(queue) = queues_guard.get_mut(&priority) {
                                // Process up to 10 tasks per priority per tick
                                for _ in 0..10 {
                                    if let Some(task) = queue.pop_front() {
                                        if let Err(e) = task_tx.send(task).await {
                                            error!("Failed to send task to worker: {}", e);
                                            break;
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    _ = shutdown_rx.recv() => {
                        info!("SyncQueue scheduler shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Enqueue a task
    pub async fn enqueue(&self, task: SyncTask) -> Result<()> {
        // Check queue size limit
        let queue_size = self.total_size().await;
        if queue_size >= self.config.max_queue_size {
            return Err(CisError::p2p("Sync queue full".to_string()));
        }

        // Add to appropriate priority queue
        {
            let mut queues = self.queues.write().await;
            let queue = queues
                .entry(task.priority)
                .or_insert_with(VecDeque::new);
            queue.push_back(task);
        }

        // Update metrics
        self.metrics.write().await.tasks_enqueued += 1;

        Ok(())
    }

    /// Enqueue with retry
    pub async fn enqueue_with_retry(&self, mut task: SyncTask) -> Result<()> {
        task.retry_count += 1;
        sleep(task.next_retry_delay()).await;
        self.enqueue(task).await
    }

    /// Enqueue a batch of tasks
    pub async fn enqueue_batch(&self, tasks: Vec<SyncTask>) -> Result<()> {
        for task in tasks {
            self.enqueue(task).await?;
        }
        Ok(())
    }

    /// Dequeue a task (for testing/monitoring)
    pub async fn dequeue(&self) -> Option<SyncTask> {
        let mut queues = self.queues.write().await;

        for priority in [
            SyncPriority::Critical,
            SyncPriority::High,
            SyncPriority::Normal,
            SyncPriority::Low,
        ] {
            if let Some(queue) = queues.get_mut(&priority) {
                if let Some(task) = queue.pop_front() {
                    return Some(task);
                }
            }
        }

        None
    }

    /// Get total queue size
    pub async fn total_size(&self) -> usize {
        let queues = self.queues.read().await;
        queues.values().map(|q| q.len()).sum()
    }

    /// Get queue size by priority
    pub async fn size_by_priority(&self, priority: SyncPriority) -> usize {
        let queues = self.queues.read().await;
        queues.get(&priority).map(|q| q.len()).unwrap_or(0)
    }

    /// Move task to dead letter queue
    pub async fn move_to_dead_letter(&self, task: SyncTask) {
        let mut dlq = self.dead_letter.lock().await;
        dlq.push_back(task);

        // Limit dead letter queue size
        while dlq.len() > 1000 {
            dlq.pop_front();
        }

        self.metrics.write().await.dead_letter_count += 1;
        warn!("Task moved to dead letter queue");
    }

    /// Get dead letter queue contents
    pub async fn get_dead_letter(&self) -> Vec<SyncTask> {
        self.dead_letter.lock().await.iter().cloned().collect()
    }

    /// Get metrics
    pub async fn metrics(&self) -> SyncMetrics {
        self.metrics.read().await.clone()
    }

    /// Shutdown the sync queue
    pub async fn shutdown(&self) {
        info!("Shutting down SyncQueue");

        if let Some(tx) = self.shutdown_tx.lock().await.take() {
            let _ = tx.send(()).await;
        }

        // Close task channel
        // Note: This will cause workers to exit when current tasks complete
    }

    /// Create a batch key from node and room
    #[allow(dead_code)]
    fn batch_key(node_id: &str, room_id: &str) -> String {
        format!("{}:{}", node_id, room_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_event() -> CisMatrixEvent {
        CisMatrixEvent::new(
            "$test123",
            "!test:example.com",
            "@alice:example.com",
            "m.room.message",
            serde_json::json!({"body": "Hello"}),
        )
    }

    #[test]
    fn test_sync_task_creation() {
        let event = create_test_event();
        let task = SyncTask::new("node1".to_string(), event, SyncPriority::High);

        assert_eq!(task.target_node, "node1");
        assert!(matches!(task.priority, SyncPriority::High));
        assert!(matches!(task.status, SyncStatus::Pending));
        assert_eq!(task.retry_count, 0);
    }

    #[test]
    fn test_sync_task_retry_delay() {
        let event = create_test_event();
        let mut task = SyncTask::new("node1".to_string(), event, SyncPriority::Normal);

        let delay1 = task.next_retry_delay();
        task.retry_count = 1;
        let delay2 = task.next_retry_delay();

        assert!(delay2 > delay1);
    }

    #[test]
    fn test_sync_priority_ordering() {
        assert!(SyncPriority::Critical < SyncPriority::High);
        assert!(SyncPriority::High < SyncPriority::Normal);
        assert!(SyncPriority::Normal < SyncPriority::Low);
    }

    #[tokio::test]
    async fn test_sync_queue_enqueue_dequeue() {
        let queue = SyncQueue::default_queue();

        let event = create_test_event();
        let task = SyncTask::new("node1".to_string(), event, SyncPriority::Normal);

        // Enqueue
        queue.enqueue(task.clone()).await.unwrap();
        assert_eq!(queue.total_size().await, 1);

        // Dequeue
        let dequeued = queue.dequeue().await;
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().task_id, task.task_id);
        assert_eq!(queue.total_size().await, 0);
    }

    #[tokio::test]
    async fn test_sync_queue_priority() {
        let queue = SyncQueue::default_queue();

        let event1 = create_test_event();
        let task1 = SyncTask::new("node1".to_string(), event1, SyncPriority::Low);

        let event2 = create_test_event();
        let task2 = SyncTask::new("node2".to_string(), event2, SyncPriority::Critical);

        queue.enqueue(task1).await.unwrap();
        queue.enqueue(task2).await.unwrap();

        // Critical should be dequeued first
        let dequeued = queue.dequeue().await;
        assert!(matches!(dequeued.unwrap().priority, SyncPriority::Critical));
    }

    #[tokio::test]
    async fn test_sync_metrics() {
        let queue = SyncQueue::default_queue();

        let metrics = queue.metrics().await;
        assert_eq!(metrics.tasks_enqueued, 0);
        assert_eq!(metrics.tasks_completed, 0);
    }
}
