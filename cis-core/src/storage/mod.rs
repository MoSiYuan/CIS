//! # Storage Module
//!
//! 跨平台存储管理，支持数据隔离和热插拔。
//!
//! ## 模块结构
//!
//! - `paths`: 跨平台目录路径管理
//! - `db`: 数据库连接管理（核心 + Skill 隔离）
//! - `connection`: 多库连接管理（ATTACH/DETACH 机制）
//! - `pool`: 连接池管理（多线程支持）
//! - `backup`: 自动备份管理
//! - `memory_db`: 独立的记忆数据库
//! - `wal`: WAL 模式配置
//! - `safety`: 随时关机安全机制

pub mod backup;
pub mod connection;
pub mod conversation_db;
pub mod db;
pub mod federation_db;
pub mod memory_db;
pub mod paths;
pub mod pool;
pub mod safety;
pub mod wal;

pub use backup::BackupManager;
pub use connection::{CrossDbRow, FromSqlValue, MultiDbConnection, SharedMultiDbConnection, SqlValue};
pub use conversation_db::{Conversation, ConversationDb, ConversationMessage};
pub use db::{CoreDb, DbManager, DagDetail, DagLogRecord, DagRecord, DagRunRecord, MemoryIndex, SkillDb};
pub use federation_db::{FederationDb, FederationLog, PeerInfo, PeerStatus, TrustLevel};
pub use memory_db::{MemoryDb, MemoryEntry};
pub use paths::Paths;
pub use pool::{ConnectionPool, PoolConfig, PoolConnectionGuard};
pub use wal::{checkpoint, checkpoint_passive, set_wal_mode, SynchronousMode, WALConfig};
