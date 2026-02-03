//! # Vector Intelligence Module
//!
//! 提供基于向量的语义检索能力，支持记忆、消息、技能等数据的智能搜索。
//!
//! ## 主要组件
//!
//! - `storage::VectorStorage`: 核心向量存储，基于 sqlite-vec
//! - `batch::BatchProcessor`: 批量处理器，异步批量化索引
//! - `embedding`: 文本向量化服务（见 `crate::ai::embedding`）
//!
//! ## 使用示例
//!
//! ```rust
//! use cis_core::vector::VectorStorage;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     // 打开向量存储
//!     let storage = VectorStorage::open_default()?;
//!
//!     // 索引记忆
//!     storage.index_memory("pref/dark_mode", b"Enable dark mode").await?;
//!
//!     // 语义搜索
//!     let results = storage.search_memory("night theme", 5).await?;
//!     for result in results {
//!         println!("Found: {} (similarity: {})", result.key, result.similarity);
//!     }
//!
//!     Ok(())
//! }
//! ```

pub mod storage;
pub mod batch;

pub use storage::{
    ConversationMessage, HnswConfig, IndexStats, MemoryResult, MessageResult, 
    SkillMatch, SkillSemantics, SummaryResult, VectorConfig, VectorStorage, 
    DEFAULT_SIMILARITY_THRESHOLD, EMBEDDING_DIM,
};
pub use batch::{BatchProcessor, BatchStats};
