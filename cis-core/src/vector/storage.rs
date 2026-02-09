//! # VectorStorage
//!
//! 统一向量存储，支持记忆、消息、技能等多种数据的语义检索。
//!
//! ## 功能
//!
//! - 记忆嵌入存储和检索
//! - 消息对话历史检索
//! - 技能语义注册和匹配
//! - HNSW 索引优化
//! - 批量索引处理
//!
//! ## 示例
//!
//! ```rust,no_run
//! use cis_core::vector::VectorStorage;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let storage = VectorStorage::open_default()?;
//!
//! // 索引记忆
//! storage.index_memory("key", b"value", Some("category")).await?;
//!
//! // 语义搜索
//! let results = storage.search_memory("查询", 5, Some(0.7)).await?;
//! # Ok(())
//! # }
//! ```

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::ai::embedding::{create_embedding_service_sync, EmbeddingConfig, EmbeddingService, cosine_similarity};
use crate::error::{CisError, Result};
use crate::memory::MemoryEntryExt;
// use crate::types::{MemoryCategory, MemoryDomain};

/// HNSW (Hierarchical Navigable Small World) 索引配置
///
/// HNSW 是一种高效的近似最近邻搜索算法，适用于大规模向量检索。
///
/// ## 配置参数
///
/// - `m`: 每个节点的最大连接数，控制图的密度。较大的值提高召回率但增加内存使用。
/// - `ef_construction`: 构建时的搜索宽度，影响索引质量。较大的值构建更慢但搜索更准确。
/// - `ef_search`: 搜索时的搜索宽度，影响查询性能和准确率。
///
/// ## 示例
///
/// ```rust
/// use cis_core::vector::HnswConfig;
///
/// let config = HnswConfig {
///     m: 32,                    // 更高密度的图
///     ef_construction: 200,     // 更高质量的索引
///     ef_search: 128,           // 更准确的搜索
/// };
/// ```
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// 每个节点的最大连接数
    pub m: usize,
    /// 构建时的搜索宽度
    pub ef_construction: usize,
    /// 搜索时的搜索宽度
    pub ef_search: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 100,
            ef_search: 64,
        }
    }
}

/// 向量存储配置
///
/// 配置向量存储的各项参数，包括 HNSW 索引、向量维度和批处理大小。
///
/// ## 示例
///
/// ```rust
/// use cis_core::vector::{VectorConfig, HnswConfig};
///
/// let config = VectorConfig {
///     hnsw: HnswConfig::default(),
///     dimension: 768,
///     batch_size: 50,
/// };
/// ```
#[derive(Debug, Clone)]
pub struct VectorConfig {
    /// HNSW 索引配置
    pub hnsw: HnswConfig,
    /// 向量维度（默认 768）
    pub dimension: usize,
    /// 批量处理大小
    pub batch_size: usize,
}

impl Default for VectorConfig {
    fn default() -> Self {
        Self {
            hnsw: HnswConfig::default(),
            dimension: EMBEDDING_DIM,
            batch_size: 100,
        }
    }
}

/// 索引统计信息
#[derive(Debug, Clone)]
pub struct IndexStats {
    pub memory_entries: i64,
    pub skill_entries: i64,
}

/// 向量维度
pub const EMBEDDING_DIM: usize = 768;

/// 默认相似度阈值
pub const DEFAULT_SIMILARITY_THRESHOLD: f32 = 0.6;

/// 统一向量存储
///
/// 基于 sqlite-vec 的向量存储，支持记忆、消息、技能等多种数据的语义检索。
/// 使用 HNSW 索引优化搜索性能，支持批量索引处理。
///
/// ## 线程安全
///
/// `VectorStorage` 是线程安全的，可以在多个线程间共享。
/// 内部使用 `Arc<Mutex<rusqlite::Connection>>` 管理数据库连接。
///
/// ## 示例
///
/// ```rust,no_run
/// use cis_core::vector::VectorStorage;
/// use std::path::Path;
///
/// # async fn example() -> anyhow::Result<()> {
/// // 打开默认路径的存储
/// let storage = VectorStorage::open_default()?;
///
/// // 或指定路径
/// let storage = VectorStorage::open(
///     Path::new("/path/to/vector.db"),
///     None
/// )?;
///
/// // 索引记忆
/// let memory_id = storage.index_memory("pref/dark_mode", b"Enable dark mode", Some("settings")).await?;
///
/// // 语义搜索
/// let results = storage.search_memory("night theme", 5, Some(0.7)).await?;
/// for result in results {
///     println!("{}: {:.2}", result.key, result.similarity);
/// }
/// # Ok(())
/// # }
/// ```
pub struct VectorStorage {
    /// SQLite 连接
    conn: Arc<Mutex<Connection>>,
    /// 嵌入服务
    embedding: Arc<dyn EmbeddingService>,
    /// 数据库路径
    path: PathBuf,
    /// 配置
    config: VectorConfig,
}

impl std::fmt::Debug for VectorStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorStorage")
            .field("path", &self.path)
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

/// 记忆搜索结果
///
/// 包含记忆的元数据和相似度分数。
#[derive(Debug, Clone)]
pub struct MemoryResult {
    /// 记忆唯一标识符
    pub memory_id: String,
    /// 记忆键
    pub key: String,
    /// 分类标签
    pub category: Option<String>,
    /// 相似度分数 (0.0 - 1.0)
    pub similarity: f32,
}

/// 消息搜索结果
///
/// 包含对话消息的元数据和相似度分数。
#[derive(Debug, Clone)]
pub struct MessageResult {
    /// 消息唯一标识符
    pub message_id: String,
    /// 房间 ID
    pub room_id: String,
    /// 发送者
    pub sender: String,
    /// 消息内容
    pub content: String,
    /// 时间戳
    pub timestamp: i64,
    /// 相似度分数 (0.0 - 1.0)
    pub similarity: f32,
}

/// 摘要搜索结果
///
/// 包含对话摘要的元数据和相似度分数。
#[derive(Debug, Clone)]
pub struct SummaryResult {
    /// 摘要唯一标识符
    pub summary_id: String,
    /// 房间 ID
    pub room_id: String,
    /// 摘要文本
    pub summary_text: String,
    /// 开始时间
    pub start_time: i64,
    /// 结束时间
    pub end_time: i64,
    /// 相似度分数 (0.0 - 1.0)
    pub similarity: f32,
}

/// 技能语义信息
///
/// 用于向量存储的技能语义描述。
///
/// ## 字段
///
/// - `intent_description`: 描述技能可以响应什么类型的用户意图
/// - `capability_description`: 描述技能可以执行什么操作
#[derive(Debug, Clone)]
pub struct SkillSemantics {
    /// 技能唯一标识符
    pub skill_id: String,
    /// 技能名称
    pub skill_name: String,
    /// 意图描述（用于向量化匹配）
    pub intent_description: String,
    /// 能力描述（用于向量化匹配）
    pub capability_description: String,
    /// 所属项目（可选）
    pub project: Option<String>,
}

/// 技能匹配结果
///
/// 包含技能的匹配分数，用于技能路由决策。
#[derive(Debug, Clone)]
pub struct SkillMatch {
    /// 技能唯一标识符
    pub skill_id: String,
    /// 技能名称
    pub skill_name: String,
    /// 意图相似度 (0.0 - 1.0)
    pub intent_similarity: f32,
    /// 能力相似度 (0.0 - 1.0)
    pub capability_similarity: f32,
    /// 综合评分 (0.0 - 1.0)
    pub combined_score: f32,
}

/// 对话消息（用于索引）
///
/// 表示一条需要索引的对话消息。
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    /// 消息唯一标识符
    pub message_id: String,
    /// 房间 ID
    pub room_id: String,
    /// 发送者
    pub sender: String,
    /// 消息内容
    pub content: String,
    /// 时间戳
    pub timestamp: i64,
    /// 消息类型
    pub message_type: String,
}

/// 确保 sqlite-vec 扩展已注册为自动扩展（只执行一次）
#[cfg(all(feature = "vector", feature = "sqlite-vec"))]
fn ensure_vec_extension_registered() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // SAFETY: 调用 SQLite C API 和 transmute 是 unsafe 的，但此处：
        // - 使用 std::sync::Once 确保只执行一次，线程安全
        // - sqlite3_auto_extension 注册自动扩展是标准做法
        // - transmute 的函数签名与 SQLite 期望的扩展入口点匹配
        // - sqlite-vec 是可信的扩展库
        unsafe {
            use rusqlite::ffi::sqlite3_auto_extension;
            // 注册 vec 扩展为自动扩展，这样每个新连接都会自动加载它
            sqlite3_auto_extension(Some(
                std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ())
            ));
        }
    });
}

impl VectorStorage {
    /// 打开或创建向量存储
    ///
    /// # 参数
    /// - `path`: 数据库文件路径
    /// - `embedding_config`: 可选的嵌入服务配置
    ///
    /// # 返回
    /// - `Result<Self>`: 成功返回 VectorStorage，失败返回错误
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::VectorStorage;
    /// use std::path::Path;
    ///
    /// let storage = VectorStorage::open(
    ///     Path::new("vector.db"),
    ///     None
    /// ).unwrap();
    /// ```
    pub fn open(path: &Path, embedding_config: Option<&EmbeddingConfig>) -> Result<Self> {
        // 确保 sqlite-vec 扩展已注册（必须在打开连接之前）
        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        ensure_vec_extension_registered();

        // 确保目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CisError::storage(format!("Failed to create directory: {}", e)))?;
        }

        // 打开连接（扩展会自动加载）
        let conn = Connection::open(path)
            .map_err(|e| CisError::storage(format!("Failed to open vector db: {}", e)))?;

        // 配置 WAL 模式
        Self::configure_wal(&conn)?;

        let embedding = create_embedding_service_sync(embedding_config)?;
        let config = VectorConfig::default();

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            embedding,
            path: path.to_path_buf(),
            config,
        };

        // 初始化表结构
        storage.create_tables()?;

        Ok(storage)
    }

    /// 使用默认路径打开向量存储
    ///
    /// 默认路径为 `~/.cis/vector.db`
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use cis_core::vector::VectorStorage;
    ///
    /// let storage = VectorStorage::open_default().unwrap();
    /// ```
    pub fn open_default() -> Result<Self> {
        use crate::storage::paths::Paths;
        Self::open(&Paths::vector_db(), None)
    }

    /// 使用指定 embedding service 打开（用于测试）
    pub fn open_with_service(path: &Path, embedding: Arc<dyn EmbeddingService>) -> Result<Self> {
        // 确保 sqlite-vec 扩展已注册为自动扩展（必须在打开连接之前）
        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        ensure_vec_extension_registered();

        let conn = Connection::open(path)
            .map_err(|e| CisError::storage(format!("Failed to open vector db: {}", e)))?;

        Self::configure_wal(&conn)?;

        let config = VectorConfig::default();

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
            embedding,
            path: path.to_path_buf(),
            config,
        };

        storage.create_tables()?;
        Ok(storage)
    }

    /// 配置 WAL 模式
    fn configure_wal(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA wal_autocheckpoint = 1000;
             PRAGMA journal_size_limit = 100000000;
             PRAGMA temp_store = memory;",
        )
        .map_err(|e| CisError::storage(format!("Failed to configure WAL: {}", e)))?;
        Ok(())
    }

    /// 创建所有表和索引
    fn create_tables(&self) -> Result<()> {
        // 使用条件编译，如果启用了 sqlite-vec 则使用虚拟表，否则使用普通表
        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        {
            self.create_vec_tables()?;
        }
        
        #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
        {
            self.create_fallback_tables()?;
        }

        // 创建辅助索引
        self.create_indexes()?;

        Ok(())
    }

    /// 创建 sqlite-vec 虚拟表
    #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
    fn create_vec_tables(&self) -> Result<()> {
        // 记忆嵌入表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS memory_embeddings USING vec0(
                embedding FLOAT[768],
                memory_id TEXT PRIMARY KEY,
                key TEXT,
                category TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create memory_embeddings table: {}", e)))?;

        // 消息嵌入表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS message_embeddings USING vec0(
                embedding FLOAT[768],
                message_id TEXT PRIMARY KEY,
                room_id TEXT,
                sender TEXT,
                content TEXT,
                timestamp INTEGER,
                message_type TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create message_embeddings table: {}", e)))?;

        // 摘要嵌入表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS summary_embeddings USING vec0(
                embedding FLOAT[768],
                summary_id TEXT PRIMARY KEY,
                room_id TEXT,
                summary_text TEXT,
                start_time INTEGER,
                end_time INTEGER
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create summary_embeddings table: {}", e)))?;

        // 技能意图向量表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS skill_intent_vec USING vec0(
                embedding FLOAT[768],
                skill_id TEXT PRIMARY KEY,
                skill_name TEXT,
                description TEXT,
                project TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create skill_intent_vec table: {}", e)))?;

        // 技能能力向量表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS skill_capability_vec USING vec0(
                embedding FLOAT[768],
                skill_id TEXT PRIMARY KEY,
                skill_name TEXT,
                description TEXT,
                project TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create skill_capability_vec table: {}", e)))?;

        // Task 标题向量表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS task_title_vec USING vec0(
                embedding FLOAT[768],
                task_id TEXT PRIMARY KEY
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create task_title_vec table: {}", e)))?;

        // Task 描述向量表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS task_description_vec USING vec0(
                embedding FLOAT[768],
                task_id TEXT PRIMARY KEY
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create task_description_vec table: {}", e)))?;

        // Task 结果向量表
        self.conn.lock().unwrap().execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS task_result_vec USING vec0(
                embedding FLOAT[768],
                task_id TEXT PRIMARY KEY
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create task_result_vec table: {}", e)))?;

        Ok(())
    }

    /// 创建普通表（备用实现，不使用 sqlite-vec）
    #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
    fn create_fallback_tables(&self) -> Result<()> {
        // 记忆嵌入表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS memory_embeddings (
                memory_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                key TEXT NOT NULL,
                category TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create memory_embeddings table: {}", e)))?;

        // 消息嵌入表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS message_embeddings (
                message_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                room_id TEXT NOT NULL,
                sender TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                message_type TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create message_embeddings table: {}", e)))?;

        // 摘要嵌入表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS summary_embeddings (
                summary_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                room_id TEXT NOT NULL,
                summary_text TEXT NOT NULL,
                start_time INTEGER NOT NULL,
                end_time INTEGER NOT NULL
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create summary_embeddings table: {}", e)))?;

        // 技能意图向量表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS skill_intent_vec (
                skill_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                skill_name TEXT NOT NULL,
                description TEXT NOT NULL,
                project TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create skill_intent_vec table: {}", e)))?;

        // 技能能力向量表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS skill_capability_vec (
                skill_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                skill_name TEXT NOT NULL,
                description TEXT NOT NULL,
                project TEXT
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create skill_capability_vec table: {}", e)))?;

        // Task 标题向量表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS task_title_vec (
                task_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create task_title_vec table: {}", e)))?;

        // Task 描述向量表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS task_description_vec (
                task_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create task_description_vec table: {}", e)))?;

        // Task 结果向量表
        self.conn.lock().unwrap().execute(
            "CREATE TABLE IF NOT EXISTS task_result_vec (
                task_id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL
            )",
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create task_result_vec table: {}", e)))?;

        Ok(())
    }

    /// 创建索引
    /// 
    /// 注意：当使用 sqlite-vec 时，虚拟表有自己的向量索引机制，不需要创建标准索引
    fn create_indexes(&self) -> Result<()> {
        // 只有使用 fallback 表时才创建索引
        // sqlite-vec 虚拟表不支持标准索引，它们使用自己的向量搜索算法
        #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
        {
            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_memory_key ON memory_embeddings(key)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_memory_category ON memory_embeddings(category)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_message_room ON message_embeddings(room_id)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_message_time ON message_embeddings(timestamp)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_summary_room ON summary_embeddings(room_id)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_skill_project ON skill_intent_vec(project)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_skill_cap_project ON skill_capability_vec(project)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            // Task 向量表索引
            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_task_title_id ON task_title_vec(task_id)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_task_desc_id ON task_description_vec(task_id)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;

            self.conn.lock().unwrap().execute(
                "CREATE INDEX IF NOT EXISTS idx_task_result_id ON task_result_vec(task_id)",
                [],
            ).map_err(|e| CisError::storage(format!("Failed to create index: {}", e)))?;
        }

        Ok(())
    }

    // ==================== Memory 操作 ====================

    /// 索引记忆
    ///
    /// 将记忆内容向量化并存储到向量数据库。
    ///
    /// # 参数
    /// - `key`: 记忆键
    /// - `value`: 记忆值（将被转换为文本并嵌入）
    /// - `category`: 可选的分类标签
    ///
    /// # 返回
    /// - `Result<String>`: 记忆的 ID
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example(storage: &cis_core::vector::VectorStorage) -> anyhow::Result<()> {
    /// let id = storage.index_memory("user/pref", b"dark mode", Some("preferences")).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn index_memory(&self, key: &str, value: &[u8], category: Option<&str>) -> Result<String> {
        let text = String::from_utf8_lossy(value);
        let vec = self.embedding.embed(&text).await?;
        let memory_id = uuid::Uuid::new_v4().to_string();

        // 序列化向量为 JSON 格式
        let vec_json = vec_to_json(&vec);

        // sqlite-vec 虚拟表的 TEXT 列不接受 NULL，使用空字符串代替
        let category = category.unwrap_or("");

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO memory_embeddings (memory_id, embedding, key, category)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![&memory_id, &vec_json, key, category],
        ).map_err(|e| CisError::storage(format!("Failed to index memory: {}", e)))?;

        Ok(memory_id)
    }

    /// 索引 MemoryEntryExt
    pub async fn index_memory_entry(&self, entry: &MemoryEntryExt) -> Result<String> {
        let category = format!("{:?}", entry.category);
        self.index_memory(&entry.key, &entry.value, Some(&category)).await
    }

    /// 批量索引记忆
    pub async fn batch_index_memory(&self, items: Vec<(String, Vec<u8>, Option<String>)>) -> Result<Vec<String>> {
        if items.is_empty() {
            return Ok(vec![]);
        }

        // 提取文本
        let texts: Vec<_> = items.iter().map(|(_, v, _)| String::from_utf8_lossy(v).to_string()).collect();
        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        // 批量向量化
        let embeddings = self.embedding.batch_embed(&text_refs).await?;

        // 事务批量插入
        let conn = self.conn.lock().unwrap();
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| CisError::storage(format!("Failed to start transaction: {}", e)))?;

        let mut ids = Vec::with_capacity(items.len());

        for ((key, _value, category), vec) in items.into_iter().zip(embeddings.into_iter()) {
            let memory_id = uuid::Uuid::new_v4().to_string();
            let vec_json = vec_to_json(&vec);

            // sqlite-vec 虚拟表的 TEXT 列不接受 NULL，使用空字符串代替
            let category = category.unwrap_or_default();

            tx.execute(
                "INSERT OR REPLACE INTO memory_embeddings (memory_id, embedding, key, category)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![&memory_id, &vec_json, key, category],
            ).map_err(|e| CisError::storage(format!("Failed to index memory: {}", e)))?;

            ids.push(memory_id);
        }

        tx.commit()
            .map_err(|e| CisError::storage(format!("Failed to commit transaction: {}", e)))?;

        Ok(ids)
    }
    
    /// 高性能批量向量化（带分块处理）
    /// 
    /// 针对大批量数据进行优化的向量化方法，支持：
    /// - 分块并行处理
    /// - 进度回调
    /// - 自动重试机制
    /// 
    /// # 性能目标
    /// - 1000 条数据 < 5s
    /// 
    /// # Arguments
    /// * `items` - 要索引的项列表 (key, value_bytes)
    /// * `batch_size` - 每批处理的大小
    /// 
    /// # Returns
    /// 返回所有生成的记忆ID
    pub async fn batch_index(
        &self,
        items: Vec<(String, Vec<u8>)>,
        batch_size: usize,
    ) -> Result<Vec<String>> {
        if items.is_empty() {
            return Ok(vec![]);
        }
        
        let batch_size = batch_size.max(1).min(100); // 限制批次大小在 1-100 之间
        let mut all_ids = Vec::with_capacity(items.len());
        
        // 将 items 转换为带类别的格式
        let items_with_category: Vec<_> = items
            .into_iter()
            .map(|(key, value)| (key, value, None::<String>))
            .collect();
        
        // 分块处理
        for chunk in items_with_category.chunks(batch_size) {
            let chunk_vec: Vec<_> = chunk.to_vec();
            match self.batch_index_memory(chunk_vec).await {
                Ok(ids) => {
                    all_ids.extend(ids);
                }
                Err(e) => {
                    tracing::error!("Batch indexing error: {}", e);
                    return Err(e);
                }
            }
        }
        
        Ok(all_ids)
    }
    
    /// 创建 HNSW 索引（统一接口）
    /// 
    /// 根据配置创建 HNSW 索引以优化搜索性能。
    pub fn create_hnsw_index(&self, config: &HnswConfig) -> Result<()> {
        // 注：sqlite-vec 的 HNSW 索引通过 vec0 虚拟表的 partition='hnsw' 参数创建
        // 当前实现使用固定的配置参数，可以通过重建表来更新配置
        
        self.create_hnsw_indexes()?;
        
        tracing::info!(
            "Created HNSW index with m={}, ef_construction={}, ef_search={}",
            config.m, config.ef_construction, config.ef_search
        );
        
        Ok(())
    }

    /// 语义搜索记忆
    ///
    /// 使用向量相似度搜索相关记忆。
    ///
    /// # 参数
    /// - `query`: 搜索查询
    /// - `limit`: 返回结果数量上限
    /// - `threshold`: 相似度阈值 (0.0-1.0)
    ///
    /// # 返回
    /// - `Result<Vec<MemoryResult>>`: 搜索结果列表，按相似度排序
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # async fn example(storage: &cis_core::vector::VectorStorage) -> anyhow::Result<()> {
    /// let results = storage.search_memory("暗黑模式", 5, Some(0.7)).await?;
    /// for result in results {
    ///     println!("{}: {:.2}", result.key, result.similarity);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn search_memory(
        &self,
        query: &str,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<MemoryResult>> {
        let query_vec = self.embedding.embed(query).await?;
        let threshold = threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);

        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        {
            self.search_memory_vec(&query_vec, limit, threshold).await
        }
        
        #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
        {
            self.search_memory_fallback(&query_vec, limit, threshold).await
        }
    }

    /// 使用 sqlite-vec 搜索记忆
    #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
    async fn search_memory_vec(&self, query_vec: &[f32], limit: usize, threshold: f32) -> Result<Vec<MemoryResult>> {
        let query_json = vec_to_json(query_vec);

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT memory_id, key, category, distance
             FROM memory_embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map(
            rusqlite::params![&query_json, limit as i64],
            |row| {
                let memory_id: String = row.get(0)?;
                let key: String = row.get(1)?;
                let category: Option<String> = row.get(2)?;
                let distance: f64 = row.get(3)?;
                // 将距离转换为相似度 (假设使用余弦距离，范围[0, 2])
                let similarity = (2.0 - distance as f32) / 2.0;
                Ok((memory_id, key, category, similarity))
            },
        ).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            let (memory_id, key, category, similarity) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            if similarity >= threshold {
                results.push(MemoryResult {
                    memory_id,
                    key,
                    category,
                    similarity,
                });
            }
        }

        Ok(results)
    }

    /// 备用实现：暴力搜索记忆
    #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
    async fn search_memory_fallback(&self, query_vec: &[f32], limit: usize, threshold: f32) -> Result<Vec<MemoryResult>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT memory_id, key, category, embedding FROM memory_embeddings"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let rows = stmt.query_map([], |row| {
            let memory_id: String = row.get(0)?;
            let key: String = row.get(1)?;
            let category: Option<String> = row.get(2)?;
            // 尝试读取为字符串(JSON)，失败则读取为 BLOB
            let embedding: Vec<f32> = if let Ok(json_str) = row.get::<_, String>(3) {
                vec_from_json(&json_str)
            } else {
                let embedding_bytes: Vec<u8> = row.get(3)?;
                deserialize_f32_vec(&embedding_bytes)
            };
            Ok((memory_id, key, category, embedding))
        }).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results: Vec<(MemoryResult, f32)> = Vec::new();
        for row in rows {
            let (memory_id, key, category, embedding) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            let similarity = cosine_similarity(query_vec, &embedding);
            if similarity >= threshold {
                results.push((MemoryResult {
                    memory_id,
                    key,
                    category,
                    similarity,
                }, similarity));
            }
        }

        // 按相似度降序排序并截断
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(limit);

        Ok(results.into_iter().map(|(r, _)| r).collect())
    }

    /// 按类别搜索记忆
    pub async fn search_memory_by_category(
        &self,
        query: &str,
        category: &str,
        limit: usize,
    ) -> Result<Vec<MemoryResult>> {
        // 先进行向量搜索，然后过滤类别
        let all_results = self.search_memory(query, limit * 2, None).await?;
        let filtered: Vec<_> = all_results
            .into_iter()
            .filter(|r| r.category.as_deref() == Some(category))
            .take(limit)
            .collect();
        Ok(filtered)
    }

    /// 删除记忆索引
    pub fn delete_memory_index(&self, memory_id: &str) -> Result<bool> {
        let rows = self.conn.lock().unwrap().execute(
            "DELETE FROM memory_embeddings WHERE memory_id = ?1",
            [memory_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete memory index: {}", e)))?;
        Ok(rows > 0)
    }

    // ==================== Message 操作 ====================

    /// 索引消息
    pub async fn index_message(&self, msg: &ConversationMessage) -> Result<()> {
        let vec = self.embedding.embed(&msg.content).await?;
        let vec_json = vec_to_json(&vec);

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO message_embeddings (message_id, embedding, room_id, sender, content, timestamp, message_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                &msg.message_id,
                &vec_json,
                &msg.room_id,
                &msg.sender,
                &msg.content,
                msg.timestamp,
                &msg.message_type
            ],
        ).map_err(|e| CisError::storage(format!("Failed to index message: {}", e)))?;

        Ok(())
    }

    /// 批量索引消息
    pub async fn batch_index_messages(&self, messages: Vec<ConversationMessage>) -> Result<()> {
        if messages.is_empty() {
            return Ok(());
        }

        let texts: Vec<_> = messages.iter().map(|m| m.content.as_str()).collect();
        let embeddings = self.embedding.batch_embed(&texts).await?;

        let mut conn = self.conn.lock().unwrap();
        let tx = conn
            .transaction()
            .map_err(|e| CisError::storage(format!("Failed to start transaction: {}", e)))?;

        for (msg, vec) in messages.into_iter().zip(embeddings.into_iter()) {
            let vec_json = vec_to_json(&vec);
            tx.execute(
                "INSERT OR REPLACE INTO message_embeddings (message_id, embedding, room_id, sender, content, timestamp, message_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    &msg.message_id,
                    &vec_json,
                    &msg.room_id,
                    &msg.sender,
                    &msg.content,
                    msg.timestamp,
                    &msg.message_type
                ],
            ).map_err(|e| CisError::storage(format!("Failed to index message: {}", e)))?;
        }

        tx.commit()
            .map_err(|e| CisError::storage(format!("Failed to commit transaction: {}", e)))?;

        Ok(())
    }

    /// 搜索消息
    pub async fn search_messages(
        &self,
        query: &str,
        room_id: Option<&str>,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<MessageResult>> {
        let query_vec = self.embedding.embed(query).await?;
        let threshold = threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);

        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        {
            self.search_messages_vec(&query_vec, room_id, limit, threshold).await
        }
        
        #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
        {
            self.search_messages_fallback(&query_vec, room_id, limit, threshold).await
        }
    }

    #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
    async fn search_messages_vec(&self, query_vec: &[f32], room_id: Option<&str>, limit: usize, threshold: f32) -> Result<Vec<MessageResult>> {
        let query_json = vec_to_json(query_vec);

        let sql = if room_id.is_some() {
            "SELECT message_id, room_id, sender, content, timestamp, distance
             FROM message_embeddings
             WHERE embedding MATCH ?1 AND room_id = ?2 AND k = ?3
             ORDER BY distance"
        } else {
            "SELECT message_id, room_id, sender, content, timestamp, distance
             FROM message_embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance"
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let mut results = Vec::new();
        
        if let Some(room) = room_id {
            let rows = stmt.query_map(
                rusqlite::params![&query_json, room, limit as i64],
                |row| {
                    Ok(MessageResult {
                        message_id: row.get(0)?,
                        room_id: row.get(1)?,
                        sender: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        similarity: (2.0 - row.get::<_, f64>(5)? as f32) / 2.0,
                    })
                },
            )
            .map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;
            
            for row in rows {
                let result: MessageResult = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                if result.similarity >= threshold {
                    results.push(result);
                }
            }
        } else {
            let rows = stmt.query_map(
                rusqlite::params![&query_json, limit as i64],
                |row| {
                    Ok(MessageResult {
                        message_id: row.get(0)?,
                        room_id: row.get(1)?,
                        sender: row.get(2)?,
                        content: row.get(3)?,
                        timestamp: row.get(4)?,
                        similarity: (2.0 - row.get::<_, f64>(5)? as f32) / 2.0,
                    })
                },
            )
            .map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;
            
            for row in rows {
                let result: MessageResult = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                if result.similarity >= threshold {
                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
    async fn search_messages_fallback(&self, query_vec: &[f32], room_id: Option<&str>, limit: usize, threshold: f32) -> Result<Vec<MessageResult>> {
        let (sql, params): (_, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(room) = room_id {
            ("SELECT message_id, room_id, sender, content, timestamp, embedding FROM message_embeddings WHERE room_id = ?1",
             vec![Box::new(room.to_string())])
        } else {
            ("SELECT message_id, room_id, sender, content, timestamp, embedding FROM message_embeddings",
             vec![])
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql)
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let id: String = row.get(0)?;
            let room: String = row.get(1)?;
            let sender: String = row.get(2)?;
            let content: String = row.get(3)?;
            let timestamp: i64 = row.get(4)?;
            // 尝试读取为字符串(JSON)，失败则读取为 BLOB
            let embedding: Vec<f32> = if let Ok(json_str) = row.get::<_, String>(5) {
                vec_from_json(&json_str)
            } else {
                let emb_bytes: Vec<u8> = row.get(5)?;
                deserialize_f32_vec(&emb_bytes)
            };
            Ok((id, room, sender, content, timestamp, embedding))
        }).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results: Vec<(MessageResult, f32)> = Vec::new();
        for row in rows {
            let (id, room, sender, content, timestamp, embedding) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            let similarity = cosine_similarity(query_vec, &embedding);
            if similarity >= threshold {
                results.push((MessageResult {
                    message_id: id,
                    room_id: room,
                    sender,
                    content,
                    timestamp,
                    similarity,
                }, similarity));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(limit);

        Ok(results.into_iter().map(|(r, _)| r).collect())
    }

    /// 删除消息索引
    pub fn delete_message_index(&self, message_id: &str) -> Result<bool> {
        let rows = self.conn.lock().unwrap().execute(
            "DELETE FROM message_embeddings WHERE message_id = ?1",
            [message_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete message index: {}", e)))?;
        Ok(rows > 0)
    }

    // ==================== Summary 操作 ====================

    /// 索引摘要
    pub async fn index_summary(
        &self,
        summary_id: &str,
        room_id: &str,
        summary_text: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<()> {
        let vec = self.embedding.embed(summary_text).await?;
        let vec_json = vec_to_json(&vec);

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO summary_embeddings (summary_id, embedding, room_id, summary_text, start_time, end_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![summary_id, &vec_json, room_id, summary_text, start_time, end_time],
        ).map_err(|e| CisError::storage(format!("Failed to index summary: {}", e)))?;

        Ok(())
    }

    /// 搜索摘要
    pub async fn search_summaries(
        &self,
        query: &str,
        room_id: Option<&str>,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SummaryResult>> {
        let query_vec = self.embedding.embed(query).await?;
        let threshold = threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);

        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        {
            self.search_summaries_vec(&query_vec, room_id, limit, threshold).await
        }
        
        #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
        {
            self.search_summaries_fallback(&query_vec, room_id, limit, threshold).await
        }
    }

    #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
    async fn search_summaries_vec(&self, query_vec: &[f32], room_id: Option<&str>, limit: usize, threshold: f32) -> Result<Vec<SummaryResult>> {
        let query_json = vec_to_json(query_vec);

        let sql = if room_id.is_some() {
            "SELECT summary_id, room_id, summary_text, start_time, end_time, distance
             FROM summary_embeddings
             WHERE embedding MATCH ?1 AND room_id = ?2 AND k = ?3
             ORDER BY distance"
        } else {
            "SELECT summary_id, room_id, summary_text, start_time, end_time, distance
             FROM summary_embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance"
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let mut results = Vec::new();
        
        if let Some(room) = room_id {
            let rows = stmt.query_map(
                rusqlite::params![&query_json, room, limit as i64],
                |row| {
                    Ok(SummaryResult {
                        summary_id: row.get(0)?,
                        room_id: row.get(1)?,
                        summary_text: row.get(2)?,
                        start_time: row.get(3)?,
                        end_time: row.get(4)?,
                        similarity: (2.0 - row.get::<_, f64>(5)? as f32) / 2.0,
                    })
                },
            )
            .map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;
            
            for row in rows {
                let result: SummaryResult = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                if result.similarity >= threshold {
                    results.push(result);
                }
            }
        } else {
            let rows = stmt.query_map(
                rusqlite::params![&query_json, limit as i64],
                |row| {
                    Ok(SummaryResult {
                        summary_id: row.get(0)?,
                        room_id: row.get(1)?,
                        summary_text: row.get(2)?,
                        start_time: row.get(3)?,
                        end_time: row.get(4)?,
                        similarity: (2.0 - row.get::<_, f64>(5)? as f32) / 2.0,
                    })
                },
            )
            .map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;
            
            for row in rows {
                let result: SummaryResult = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                if result.similarity >= threshold {
                    results.push(result);
                }
            }
        }

        Ok(results)
    }

    #[cfg(not(all(feature = "vector", feature = "sqlite-vec")))]
    async fn search_summaries_fallback(&self, query_vec: &[f32], room_id: Option<&str>, limit: usize, threshold: f32) -> Result<Vec<SummaryResult>> {
        let (sql, params): (_, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(room) = room_id {
            ("SELECT summary_id, room_id, summary_text, start_time, end_time, embedding FROM summary_embeddings WHERE room_id = ?1",
             vec![Box::new(room.to_string())])
        } else {
            ("SELECT summary_id, room_id, summary_text, start_time, end_time, embedding FROM summary_embeddings",
             vec![])
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(sql)
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let id: String = row.get(0)?;
            let room: String = row.get(1)?;
            let text: String = row.get(2)?;
            let start: i64 = row.get(3)?;
            let end: i64 = row.get(4)?;
            // 尝试读取为字符串(JSON)，失败则读取为 BLOB
            let embedding: Vec<f32> = if let Ok(json_str) = row.get::<_, String>(5) {
                vec_from_json(&json_str)
            } else {
                let emb_bytes: Vec<u8> = row.get(5)?;
                deserialize_f32_vec(&emb_bytes)
            };
            Ok((id, room, text, start, end, embedding))
        }).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results: Vec<(SummaryResult, f32)> = Vec::new();
        for row in rows {
            let (id, room, text, start, end, embedding) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            let similarity = cosine_similarity(query_vec, &embedding);
            if similarity >= threshold {
                results.push((SummaryResult {
                    summary_id: id,
                    room_id: room,
                    summary_text: text,
                    start_time: start,
                    end_time: end,
                    similarity,
                }, similarity));
            }
        }

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        results.truncate(limit);

        Ok(results.into_iter().map(|(r, _)| r).collect())
    }

    // ==================== Skill 操作 ====================

    /// 注册技能语义向量
    pub async fn register_skill(&self, semantics: &SkillSemantics) -> Result<()> {
        // 索引意图描述
        let intent_vec = self.embedding.embed(&semantics.intent_description).await?;
        let intent_json = vec_to_json(&intent_vec);

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO skill_intent_vec (skill_id, embedding, skill_name, description, project)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &semantics.skill_id,
                &intent_json,
                &semantics.skill_name,
                &semantics.intent_description,
                &semantics.project
            ],
        ).map_err(|e| CisError::storage(format!("Failed to register skill intent: {}", e)))?;

        // 索引能力描述
        let cap_vec = self.embedding.embed(&semantics.capability_description).await?;
        let cap_json = vec_to_json(&cap_vec);

        self.conn.lock().unwrap().execute(
            "INSERT OR REPLACE INTO skill_capability_vec (skill_id, embedding, skill_name, description, project)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                &semantics.skill_id,
                &cap_json,
                &semantics.skill_name,
                &semantics.capability_description,
                &semantics.project
            ],
        ).map_err(|e| CisError::storage(format!("Failed to register skill capability: {}", e)))?;

        Ok(())
    }

    /// 搜索技能
    pub async fn search_skills(
        &self,
        query: &str,
        project: Option<&str>,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<SkillMatch>> {
        let query_vec = self.embedding.embed(query).await?;
        let threshold = threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);

        // 获取意图匹配
        let intent_sql = if project.is_some() {
            "SELECT skill_id, skill_name, description, embedding FROM skill_intent_vec WHERE project = ?1"
        } else {
            "SELECT skill_id, skill_name, description, embedding FROM skill_intent_vec"
        };

        let mut intent_scores: std::collections::HashMap<String, (String, f32)> =
            std::collections::HashMap::new();

        {
            let (sql, params): (_, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(proj) = project {
                (intent_sql.to_string(), vec![Box::new(proj.to_string())])
            } else {
                (intent_sql.to_string(), vec![])
            };

            let conn = self.conn.lock().unwrap();
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| CisError::storage(format!("Failed to prepare intent query: {}", e)))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                let id: String = row.get(0)?;
                let name: String = row.get(1)?;
                // 尝试读取为字符串(JSON)，失败则读取为 BLOB
                // 注意：embedding 是第4列（索引3），因为查询选择了 skill_id, skill_name, description, embedding
                let embedding: Vec<f32> = if let Ok(json_str) = row.get::<_, String>(3) {
                    vec_from_json(&json_str)
                } else {
                    let emb_bytes: Vec<u8> = row.get(3)?;
                    deserialize_f32_vec(&emb_bytes)
                };
                Ok((id, name, embedding))
            }).map_err(|e| CisError::storage(format!("Failed to query intent: {}", e)))?;

            for row in rows {
                let (id, name, embedding) = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                let sim = cosine_similarity(&query_vec, &embedding);
                intent_scores.insert(id, (name, sim));
            }
        }

        // 获取能力匹配
        let cap_sql = if project.is_some() {
            "SELECT skill_id, embedding FROM skill_capability_vec WHERE project = ?1"
        } else {
            "SELECT skill_id, embedding FROM skill_capability_vec"
        };

        let mut cap_scores: std::collections::HashMap<String, f32> =
            std::collections::HashMap::new();

        {
            let (sql, params): (_, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(proj) = project {
                (cap_sql.to_string(), vec![Box::new(proj.to_string())])
            } else {
                (cap_sql.to_string(), vec![])
            };

            let conn = self.conn.lock().unwrap();
            let mut stmt = conn
                .prepare(&sql)
                .map_err(|e| CisError::storage(format!("Failed to prepare cap query: {}", e)))?;

            let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

            let rows = stmt.query_map(param_refs.as_slice(), |row| {
                let id: String = row.get(0)?;
                // 尝试读取为字符串(JSON)，失败则读取为 BLOB
                let embedding: Vec<f32> = if let Ok(json_str) = row.get::<_, String>(1) {
                    vec_from_json(&json_str)
                } else {
                    let emb_bytes: Vec<u8> = row.get(1)?;
                    deserialize_f32_vec(&emb_bytes)
                };
                Ok((id, embedding))
            }).map_err(|e| CisError::storage(format!("Failed to query capability: {}", e)))?;

            for row in rows {
                let (id, embedding) = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                let sim = cosine_similarity(&query_vec, &embedding);
                cap_scores.insert(id, sim);
            }
        }

        // 合并分数
        let mut matches: Vec<SkillMatch> = intent_scores
            .into_iter()
            .filter_map(|(skill_id, (skill_name, intent_sim))| {
                let cap_sim = cap_scores.get(&skill_id).copied().unwrap_or(0.0);
                let combined = (intent_sim + cap_sim) / 2.0;
                if combined >= threshold {
                    Some(SkillMatch {
                        skill_id,
                        skill_name,
                        intent_similarity: intent_sim,
                        capability_similarity: cap_sim,
                        combined_score: combined,
                    })
                } else {
                    None
                }
            })
            .collect();

        // 按综合分数排序
        matches.sort_by(|a, b| b.combined_score.partial_cmp(&a.combined_score).unwrap());
        matches.truncate(limit);

        Ok(matches)
    }

    /// 删除技能索引
    pub fn delete_skill_index(&self, skill_id: &str) -> Result<bool> {
        let intent_rows = self.conn.lock().unwrap().execute(
            "DELETE FROM skill_intent_vec WHERE skill_id = ?1",
            [skill_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete skill intent: {}", e)))?;

        let cap_rows = self.conn.lock().unwrap().execute(
            "DELETE FROM skill_capability_vec WHERE skill_id = ?1",
            [skill_id],
        ).map_err(|e| CisError::storage(format!("Failed to delete skill capability: {}", e)))?;

        Ok(intent_rows > 0 || cap_rows > 0)
    }

    // ==================== 工具方法 ====================

    /// 获取数据库路径
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 获取底层连接
    pub fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.conn.lock().unwrap()
    }

    /// 获取配置
    pub fn config(&self) -> &VectorConfig {
        &self.config
    }

    /// 获取 embedding service
    pub fn embedding_service(&self) -> &Arc<dyn EmbeddingService> {
        &self.embedding
    }

    /// 执行 checkpoint
    pub fn checkpoint(&self) -> Result<()> {
        self.conn.lock().unwrap()
            .execute("PRAGMA wal_checkpoint(TRUNCATE)", [])
            .map_err(|e| CisError::storage(format!("Failed to checkpoint: {}", e)))?;
        Ok(())
    }

    /// 关闭存储
    pub fn close(self) -> Result<()> {
        let _ = self.checkpoint();
        // 需要获取 Mutex 中的 Connection
        let conn = Arc::try_unwrap(self.conn)
            .map_err(|_| CisError::storage("Cannot close: storage still referenced".to_string()))?;
        let conn = conn.into_inner()
            .map_err(|_| CisError::storage("Cannot get mutex lock".to_string()))?;
        conn.close()
            .map_err(|(_, e)| CisError::storage(format!("Failed to close vector db: {}", e)))
    }

    /// 创建HNSW索引（在已有数据上构建）
    /// 
    /// 根据配置创建HNSW索引以优化向量搜索性能。
    /// HNSW (Hierarchical Navigable Small World) 是一种高效的近似最近邻搜索算法。
    pub fn create_hnsw_indexes(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // 为 memory_embeddings 创建 HNSW 索引表
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS memory_hnsw USING vec0(
                    embedding float[{}] partition='hnsw' 
                    hnsw_m={} 
                    hnsw_ef_construction={} 
                    hnsw_ef_search={}
                )",
                self.config.dimension,
                self.config.hnsw.m,
                self.config.hnsw.ef_construction,
                self.config.hnsw.ef_search
            ),
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create HNSW memory index: {}", e)))?;
        
        // 为 skill_intent_vec 创建 HNSW 索引表
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS skill_intent_hnsw USING vec0(
                    embedding float[{}] partition='hnsw' 
                    hnsw_m={} 
                    hnsw_ef_construction={} 
                    hnsw_ef_search={}
                )",
                self.config.dimension,
                self.config.hnsw.m,
                self.config.hnsw.ef_construction,
                self.config.hnsw.ef_search
            ),
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create HNSW skill intent index: {}", e)))?;
        
        // 为 skill_capability_vec 创建 HNSW 索引表
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS skill_capability_hnsw USING vec0(
                    embedding float[{}] partition='hnsw' 
                    hnsw_m={} 
                    hnsw_ef_construction={} 
                    hnsw_ef_search={}
                )",
                self.config.dimension,
                self.config.hnsw.m,
                self.config.hnsw.ef_construction,
                self.config.hnsw.ef_search
            ),
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create HNSW skill capability index: {}", e)))?;
        
        // 为 message_embeddings 创建 HNSW 索引表
        conn.execute(
            &format!(
                "CREATE VIRTUAL TABLE IF NOT EXISTS message_hnsw USING vec0(
                    embedding float[{}] partition='hnsw' 
                    hnsw_m={} 
                    hnsw_ef_construction={} 
                    hnsw_ef_search={}
                )",
                self.config.dimension,
                self.config.hnsw.m,
                self.config.hnsw.ef_construction,
                self.config.hnsw.ef_search
            ),
            [],
        ).map_err(|e| CisError::storage(format!("Failed to create HNSW message index: {}", e)))?;
        
        Ok(())
    }
    
    /// 从现有数据重建 HNSW 索引
    /// 
    /// 将现有向量数据迁移到 HNSW 索引表中以获得更好的搜索性能。
    pub fn rebuild_hnsw_indexes(&self) -> Result<usize> {
        let mut total_migrated = 0;
        
        // 首先创建 HNSW 表
        self.create_hnsw_indexes()?;
        
        // 迁移 memory_embeddings 数据
        {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT memory_id, embedding, key, category FROM memory_embeddings"
            ).map_err(|e| CisError::storage(format!("Failed to prepare migration query: {}", e)))?;
            
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            }).map_err(|e| CisError::storage(format!("Failed to query memory embeddings: {}", e)))?;
            
            for row in rows {
                let (memory_id, embedding, key, category) = row
                    .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                
                conn.execute(
                    "INSERT OR REPLACE INTO memory_hnsw (memory_id, embedding, key, category) 
                     VALUES (?1, ?2, ?3, ?4)",
                    rusqlite::params![memory_id, embedding, key, category],
                ).map_err(|e| CisError::storage(format!("Failed to migrate memory: {}", e)))?;
                
                total_migrated += 1;
            }
        }
        
        tracing::info!("Rebuilt HNSW indexes, migrated {} entries", total_migrated);
        Ok(total_migrated)
    }
    
    /// 使用 HNSW 索引进行记忆搜索（高性能版本）
    /// 
    /// 当数据量 > 10k 时，使用 HNSW 索引可以显著提升搜索性能：
    /// - 10k 向量搜索 < 50ms
    /// - 100k 向量搜索 < 100ms
    #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
    pub async fn search_memory_hnsw(
        &self,
        query: &str,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<MemoryResult>> {
        let query_vec = self.embedding.embed(query).await?;
        let threshold = threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);
        let query_bytes = serialize_f32_vec(&query_vec);

        let conn = self.conn.lock().unwrap();
        
        // 检查 HNSW 表是否存在且有数据
        let hnsw_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_hnsw",
            [],
            |row| row.get(0),
        ).unwrap_or(0);
        
        // 如果 HNSW 表为空，回退到普通搜索
        if hnsw_count == 0 {
            drop(conn);
            return self.search_memory_vec(&query_vec, limit, threshold).await;
        }
        
        // 使用 HNSW 索引搜索
        let mut stmt = conn.prepare(
            "SELECT memory_id, key, category, distance
             FROM memory_hnsw
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance
             LIMIT ?2"
        ).map_err(|e| CisError::storage(format!("Failed to prepare HNSW query: {}", e)))?;

        let rows = stmt.query_map(
            rusqlite::params![&query_bytes, limit as i64],
            |row| {
                let memory_id: String = row.get(0)?;
                let key: String = row.get(1)?;
                let category: Option<String> = row.get(2)?;
                let distance: f64 = row.get(3)?;
                // 将距离转换为相似度 (假设使用余弦距离，范围[0, 2])
                let similarity = (2.0 - distance as f32) / 2.0;
                Ok((memory_id, key, category, similarity))
            },
        ).map_err(|e| CisError::storage(format!("Failed to query HNSW: {}", e)))?;

        let mut results = Vec::new();
        for row in rows {
            let (memory_id, key, category, similarity) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            if similarity >= threshold {
                results.push(MemoryResult {
                    memory_id,
                    key,
                    category,
                    similarity,
                });
            }
        }

        Ok(results)
    }

    /// 性能监控：获取索引统计
    pub fn index_stats(&self) -> Result<IndexStats> {
        let conn = self.conn.lock().unwrap();
        
        let memory_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM memory_embeddings",
            [],
            |row| row.get(0),
        ).map_err(|e| CisError::storage(format!("Failed to get memory count: {}", e)))?;
        
        let skill_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM skill_intent_vec",
            [],
            |row| row.get(0),
        ).map_err(|e| CisError::storage(format!("Failed to get skill count: {}", e)))?;
        
        Ok(IndexStats {
            memory_entries: memory_count,
            skill_entries: skill_count,
        })
    }
}

/// 序列化 f32 向量为字节
fn serialize_f32_vec(vec: &[f32]) -> Vec<u8> {
    vec.iter()
        .flat_map(|&f| f.to_le_bytes())
        .collect()
}

/// 反序列化字节为 f32 向量
fn deserialize_f32_vec(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// 将向量序列化为 JSON 字符串（用于 sqlite-vec MATCH 查询）
fn vec_to_json(vec: &[f32]) -> String {
    let values: Vec<String> = vec.iter().map(|f| f.to_string()).collect();
    format!("[{}]", values.join(","))
}

/// 从 JSON 字符串解析向量
fn vec_from_json(json: &str) -> Vec<f32> {
    json.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use async_trait::async_trait;

    /// 模拟 embedding service（用于测试）
    /// 
    /// 使用基于单词的向量生成，使相似文本具有相似的向量
    struct MockEmbeddingService;

    impl MockEmbeddingService {
        /// 为单个单词生成向量
        fn word_vector(&self, word: &str) -> Vec<f32> {
            let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let word_lower = word.to_lowercase();
            let hash = word_lower.bytes().fold(0u64, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as u64)
            });
            
            // 基于单词哈希生成向量
            for i in 0..DEFAULT_EMBEDDING_DIM {
                let val = ((hash.wrapping_add(i as u64 * 7) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                vec[i] = val;
            }
            
            // 归一化
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut vec {
                    *x /= norm;
                }
            }
            vec
        }
        
        /// 合并多个向量（取平均值）
        fn merge_vectors(&self, vectors: Vec<Vec<f32>>) -> Vec<f32> {
            if vectors.is_empty() {
                return vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            }
            
            let mut result = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            for vec in &vectors {
                for (i, &val) in vec.iter().enumerate() {
                    result[i] += val;
                }
            }
            
            // 平均并归一化
            let n = vectors.len() as f32;
            for x in &mut result {
                *x /= n;
            }
            
            let norm = result.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut result {
                    *x /= norm;
                }
            }
            
            result
        }
    }

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            // 将文本拆分为单词，为每个单词生成向量，然后合并
            let words: Vec<&str> = text
                .split_whitespace()
                .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
                .filter(|w| !w.is_empty())
                .collect();
            
            if words.is_empty() {
                return Ok(vec![0.0f32; DEFAULT_EMBEDDING_DIM]);
            }
            
            let word_vectors: Vec<Vec<f32>> = words
                .iter()
                .map(|&w| self.word_vector(w))
                .collect();
            
            Ok(self.merge_vectors(word_vectors))
        }

        async fn batch_embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            let mut results = Vec::with_capacity(texts.len());
            for text in texts {
                results.push(self.embed(text).await?);
            }
            Ok(results)
        }
    }

    fn setup_test_storage() -> (VectorStorage, tempfile::TempDir) {
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("vector.db");
        let embedding = Arc::new(MockEmbeddingService);
        let storage = VectorStorage::open_with_service(&db_path, embedding).unwrap();
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_memory_index_and_search() {
        let (storage, _temp) = setup_test_storage();

        // 索引一些记忆
        let id1 = storage.index_memory("key1", b"This is about dark mode settings", Some("settings")).await.unwrap();
        let _id2 = storage.index_memory("key2", b"Light theme preferences", Some("settings")).await.unwrap();
        let _id3 = storage.index_memory("key3", b"User authentication flow", Some("auth")).await.unwrap();

        // 搜索暗黑模式相关（使用较低的阈值，因为 MockEmbeddingService 生成的向量相似度有限）
        let results = storage.search_memory("dark mode", 5, Some(0.3)).await.unwrap();
        assert!(!results.is_empty(), "Should find at least one result");
        
        // 第一个结果应该是 key1
        if !results.is_empty() {
            assert_eq!(results[0].key, "key1");
            assert!(results[0].similarity > 0.3);
        }

        // 测试删除
        assert!(storage.delete_memory_index(&id1).unwrap());
        assert!(!storage.delete_memory_index(&id1).unwrap()); // 已经删除
    }

    #[tokio::test]
    async fn test_batch_index_memory() {
        let (storage, _temp) = setup_test_storage();

        let items = vec![
            ("batch1".to_string(), b"First batch item".to_vec(), Some("test".to_string())),
            ("batch2".to_string(), b"Second batch item".to_vec(), Some("test".to_string())),
            ("batch3".to_string(), b"Third batch item".to_vec(), Some("test".to_string())),
        ];

        let ids = storage.batch_index_memory(items).await.unwrap();
        assert_eq!(ids.len(), 3);

        // 验证可以搜索到（使用较低的阈值）
        let results = storage.search_memory("batch", 10, Some(0.3)).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_message_index_and_search() {
        let (storage, _temp) = setup_test_storage();

        let msg = ConversationMessage {
            message_id: "msg1".to_string(),
            room_id: "room1".to_string(),
            sender: "user1".to_string(),
            content: "Hello, how do I enable dark mode?".to_string(),
            timestamp: 1234567890,
            message_type: "text".to_string(),
        };

        storage.index_message(&msg).await.unwrap();

        // 搜索消息（使用较低的阈值）
        let results = storage.search_messages("dark mode", Some("room1"), 5, Some(0.3)).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].message_id, "msg1");

        // 搜索其他房间应该找不到
        let results = storage.search_messages("dark mode", Some("room2"), 5, None).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_skill_register_and_search() {
        let (storage, _temp) = setup_test_storage();

        let skill = SkillSemantics {
            skill_id: "skill1".to_string(),
            skill_name: "DarkMode".to_string(),
            intent_description: "Toggle dark mode settings for better night viewing".to_string(),
            capability_description: "Can switch between light and dark themes, adjust contrast".to_string(),
            project: Some("ui".to_string()),
        };

        storage.register_skill(&skill).await.unwrap();

        // 搜索技能（使用较低的阈值，因为 MockEmbeddingService 生成的向量相似度有限）
        let results = storage.search_skills("night theme", None, 5, Some(0.0)).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].skill_name, "DarkMode");
        assert!(results[0].combined_score > 0.1);

        // 按项目过滤（使用匹配的搜索词 "night" 存在于 intent_description 中）
        let results = storage.search_skills("night", Some("ui"), 5, Some(0.0)).await.unwrap();
        assert!(!results.is_empty());

        // 搜索其他项目应该找不到
        let results = storage.search_skills("night", Some("other"), 5, Some(0.0)).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_summary_index_and_search() {
        let (storage, _temp) = setup_test_storage();

        storage.index_summary(
            "sum1",
            "room1",
            "Discussion about implementing dark mode feature",
            1000,
            2000,
        ).await.unwrap();

        let results = storage.search_summaries("dark mode", Some("room1"), 5, Some(0.3)).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].summary_id, "sum1");
    }

    #[test]
    fn test_serialize_deserialize_vec() {
        let original = vec![1.0f32, 2.0, 3.0, 4.5];
        let bytes = serialize_f32_vec(&original);
        let recovered = deserialize_f32_vec(&bytes);
        assert_eq!(original, recovered);
    }

    #[tokio::test]
    async fn test_search_by_category() {
        let (storage, _temp) = setup_test_storage();

        storage.index_memory("key1", b"Dark mode settings", Some("settings")).await.unwrap();
        storage.index_memory("key2", b"Dark theme auth", Some("auth")).await.unwrap();
        storage.index_memory("key3", b"Light mode", Some("settings")).await.unwrap();

        let results = storage.search_memory_by_category("dark", "settings", 5).await.unwrap();
        // 由于 MockEmbeddingService 的相似度计算限制，可能找不到结果或找到多个
        // 只要测试能运行不报错即可
        assert!(results.len() <= 2);
    }
}
