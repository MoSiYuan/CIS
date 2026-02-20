//! # Vector Intelligence Module
//!
//! Provides vector-based semantic retrieval capabilities, supporting intelligent search of memory, messages, skills, and other data.
//!
//! ## Phase 3 Migration Note
//!
//! This module is kept for backward compatibility. The vector functionality has been migrated
//! to cis-common/cis-vector crate. New code should use:
//!
//! ```rust
//! use cis_vector::*;  // Recommended
//! ```
//!
//! This module re-exports from cis_vector for backward compatibility.

pub use cis_vector::*;
//!
//! ## Main Components
//!
//! - `storage::VectorStorage`: Core vector storage based on sqlite-vec
//! - `batch::BatchProcessor`: Batch processor for asynchronous batch indexing
//! - `batch_loader::BatchVectorLoader`: Batch vector loading optimization
//! - `switch::IndexMonitor`: Smart index switching strategy
//! - `merger::ResultMerger`: Search result merger
//! - `adaptive_threshold::AdaptiveThreshold`: Adaptive threshold adjuster
//! - `embedding`: Text vectorization service (see `crate::ai::embedding`)
//!
//! ## 使用示例
//!
//! ```rust,no_run
//! use cis_core::vector::VectorStorage;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // 打开向量存储
//! let storage = VectorStorage::open_default()?;
//!
//! // 索引记忆
//! storage.index_memory("pref/dark_mode", b"Enable dark mode", None).await?;
//!
//! // 语义搜索
//! let results = storage.search_memory("night theme", 5, None).await?;
//! for result in results {
//!     println!("Found: {} (similarity: {})", result.key, result.similarity);
//! }
//!
//! # Ok(())
//! # }
//! ```

pub mod storage;
pub mod batch;

// v1.1.6 性能优化模块
pub mod batch_loader;
pub mod switch;
pub mod merger;
pub mod adaptive_threshold;

pub use storage::{
    ConversationMessage, HnswConfig, IndexStats, MemoryResult, MessageResult,
    SkillMatch, SkillSemantics, SummaryResult, VectorConfig, VectorStorage,
    DEFAULT_SIMILARITY_THRESHOLD, EMBEDDING_DIM,
};
pub use batch::{BatchProcessor, BatchStats};

// v1.1.6 新增导出
pub use batch_loader::{BatchVectorLoader, VectorBatch, VectorData};
pub use switch::{IndexMonitor, SearchStrategy, SearchMetrics, SwitchThreshold};
pub use merger::{ResultMerger, SearchResult, SearchSource, MergeStrategy, MergeStats};
pub use adaptive_threshold::{AdaptiveThreshold, ThresholdAction, PerformanceTarget};
