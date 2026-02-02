//! 联邦同步模块
//!
//! 处理断线同步队列消费、事件同步和冲突解决

pub mod consumer;

pub use consumer::{SyncConsumer, SyncConfig, SyncResult};
