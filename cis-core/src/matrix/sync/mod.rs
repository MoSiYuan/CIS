//! 联邦同步模块
//!
//! 处理断线同步队列消费、事件同步和冲突解决
//!
//! ## Components
//!
//! - **SyncConsumer**: 断线同步队列消费者，定期消费 pending_sync 表
//! - **SyncQueue**: 优化的同步队列，支持优先级和批处理
//! - **SyncTask**: 同步任务定义

pub mod consumer;
pub mod queue;

pub use consumer::{QueueStatus, SyncConfig, SyncConsumer, SyncResult};
pub use queue::{
    BatchOperation, SyncMetrics, SyncPriority, SyncQueue, SyncQueueConfig, SyncStatus, SyncTask,
};
