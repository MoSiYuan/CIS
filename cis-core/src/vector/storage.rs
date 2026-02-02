//! # Vector Storage
//!
//! 基于 sqlite-vec 的向量存储，支持记忆、消息、技能等多种数据的语义检索。
//!
//! ## 功能
//!
//! - 记忆嵌入存储和检索
//! - 消息对话历史检索
//! - 技能语义注册和匹配
//! - HNSW 索引优化
//! - 批量索引处理

use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::ai::embedding::{create_embedding_service, EmbeddingConfig, EmbeddingService, cosine_similarity};
use crate::error::{CisError, Result};
use crate::memory::MemoryEntryExt;
// use crate::types::{MemoryCategory, MemoryDomain};

/// HNSW索引配置
#[derive(Debug, Clone)]
pub struct HnswConfig {
    pub m: usize,                    // 每个节点的最大连接数
    pub ef_construction: usize,      // 构建时的搜索宽度
    pub ef_search: usize,            // 搜索时的搜索宽度
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
#[derive(Debug, Clone)]
pub struct VectorConfig {
    pub hnsw: HnswConfig,
    pub dimension: usize,
    pub batch_size: usize,           // 批量处理大小
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

/// 向量存储主结构
pub struct VectorStorage {
    conn: Arc<Mutex<Connection>>,
    embedding: Arc<dyn EmbeddingService>,
    path: PathBuf,
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
#[derive(Debug, Clone)]
pub struct MemoryResult {
    pub memory_id: String,
    pub key: String,
    pub category: Option<String>,
    pub similarity: f32,
}

/// 消息搜索结果
#[derive(Debug, Clone)]
pub struct MessageResult {
    pub message_id: String,
    pub room_id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
    pub similarity: f32,
}

/// 摘要搜索结果
#[derive(Debug, Clone)]
pub struct SummaryResult {
    pub summary_id: String,
    pub room_id: String,
    pub summary_text: String,
    pub start_time: i64,
    pub end_time: i64,
    pub similarity: f32,
}

/// 技能语义信息
#[derive(Debug, Clone)]
pub struct SkillSemantics {
    pub skill_id: String,
    pub skill_name: String,
    pub intent_description: String,
    pub capability_description: String,
    pub project: Option<String>,
}

/// 技能匹配结果
#[derive(Debug, Clone)]
pub struct SkillMatch {
    pub skill_id: String,
    pub skill_name: String,
    pub intent_similarity: f32,
    pub capability_similarity: f32,
    pub combined_score: f32,
}

/// 对话消息（用于索引）
#[derive(Debug, Clone)]
pub struct ConversationMessage {
    pub message_id: String,
    pub room_id: String,
    pub sender: String,
    pub content: String,
    pub timestamp: i64,
    pub message_type: String,
}

impl VectorStorage {
    /// 打开向量存储（如果不存在则创建）
    pub fn open(path: &Path, embedding_config: Option<&EmbeddingConfig>) -> Result<Self> {
        // 确保目录存在
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| CisError::storage(format!("Failed to create directory: {}", e)))?;
        }

        // 打开连接
        let conn = Connection::open(path)
            .map_err(|e| CisError::storage(format!("Failed to open vector db: {}", e)))?;

        // 配置 WAL 模式
        Self::configure_wal(&conn)?;

        // 初始化 sqlite-vec
        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        {
            unsafe {
                sqlite_vec::sqlite3_vec_init();
            }
        }

        let embedding = create_embedding_service(embedding_config)?;
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

    /// 使用默认路径打开
    pub fn open_default() -> Result<Self> {
        use crate::storage::paths::Paths;
        Self::open(&Paths::vector_db(), None)
    }

    /// 使用指定 embedding service 打开（用于测试）
    pub fn open_with_service(path: &Path, embedding: Arc<dyn EmbeddingService>) -> Result<Self> {
        let conn = Connection::open(path)
            .map_err(|e| CisError::storage(format!("Failed to open vector db: {}", e)))?;

        Self::configure_wal(&conn)?;

        #[cfg(all(feature = "vector", feature = "sqlite-vec"))]
        {
            unsafe {
                sqlite_vec::sqlite3_vec_init();
            }
        }

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

        Ok(())
    }

    /// 创建索引
    fn create_indexes(&self) -> Result<()> {
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

        Ok(())
    }

    // ==================== Memory 操作 ====================

    /// 索引单个记忆
    pub async fn index_memory(&self, key: &str, value: &[u8], category: Option<&str>) -> Result<String> {
        let text = String::from_utf8_lossy(value);
        let vec = self.embedding.embed(&text).await?;
        let memory_id = uuid::Uuid::new_v4().to_string();

        // 序列化向量
        let vec_bytes = serialize_f32_vec(&vec);

        self.conn.lock().unwrap().execute(
            "INSERT INTO memory_embeddings (memory_id, embedding, key, category)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(memory_id) DO UPDATE SET 
                embedding = excluded.embedding,
                key = excluded.key,
                category = excluded.category",
            rusqlite::params![&memory_id, &vec_bytes, key, category],
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
            let vec_bytes = serialize_f32_vec(&vec);

            tx.execute(
                "INSERT INTO memory_embeddings (memory_id, embedding, key, category)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(memory_id) DO UPDATE SET 
                    embedding = excluded.embedding,
                    key = excluded.key,
                    category = excluded.category",
                rusqlite::params![&memory_id, &vec_bytes, key, category],
            ).map_err(|e| CisError::storage(format!("Failed to index memory: {}", e)))?;

            ids.push(memory_id);
        }

        tx.commit()
            .map_err(|e| CisError::storage(format!("Failed to commit transaction: {}", e)))?;

        Ok(ids)
    }

    /// 语义搜索记忆
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
        let query_bytes = serialize_f32_vec(query_vec);

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT memory_id, key, category, distance
             FROM memory_embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance
             LIMIT ?2"
        ).map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

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
            let embedding_bytes: Vec<u8> = row.get(3)?;
            let embedding = deserialize_f32_vec(&embedding_bytes);
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
        let vec_bytes = serialize_f32_vec(&vec);

        self.conn.lock().unwrap().execute(
            "INSERT INTO message_embeddings (message_id, embedding, room_id, sender, content, timestamp, message_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(message_id) DO UPDATE SET 
                embedding = excluded.embedding,
                content = excluded.content",
            rusqlite::params![
                &msg.message_id,
                &vec_bytes,
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
            let vec_bytes = serialize_f32_vec(&vec);
            tx.execute(
                "INSERT INTO message_embeddings (message_id, embedding, room_id, sender, content, timestamp, message_type)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(message_id) DO UPDATE SET 
                    embedding = excluded.embedding,
                    content = excluded.content",
                rusqlite::params![
                    &msg.message_id,
                    &vec_bytes,
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
        let query_bytes = serialize_f32_vec(query_vec);

        let sql = if room_id.is_some() {
            "SELECT message_id, room_id, sender, content, timestamp, distance
             FROM message_embeddings
             WHERE embedding MATCH ?1 AND room_id = ?2 AND k = ?3
             ORDER BY distance
             LIMIT ?3"
        } else {
            "SELECT message_id, room_id, sender, content, timestamp, distance
             FROM message_embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance
             LIMIT ?2"
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let mut results = Vec::new();
        
        if let Some(room) = room_id {
            let rows = stmt.query_map(
                rusqlite::params![&query_bytes, room, limit as i64],
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
                rusqlite::params![&query_bytes, limit as i64],
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
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Vec<u8>>(5)?,
            ))
        }).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results: Vec<(MessageResult, f32)> = Vec::new();
        for row in rows {
            let (id, room, sender, content, timestamp, emb_bytes) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            let embedding = deserialize_f32_vec(&emb_bytes);
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
        let vec_bytes = serialize_f32_vec(&vec);

        self.conn.lock().unwrap().execute(
            "INSERT INTO summary_embeddings (summary_id, embedding, room_id, summary_text, start_time, end_time)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(summary_id) DO UPDATE SET 
                embedding = excluded.embedding,
                summary_text = excluded.summary_text",
            rusqlite::params![summary_id, &vec_bytes, room_id, summary_text, start_time, end_time],
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
        let query_bytes = serialize_f32_vec(query_vec);

        let sql = if room_id.is_some() {
            "SELECT summary_id, room_id, summary_text, start_time, end_time, distance
             FROM summary_embeddings
             WHERE embedding MATCH ?1 AND room_id = ?2 AND k = ?3
             ORDER BY distance
             LIMIT ?3"
        } else {
            "SELECT summary_id, room_id, summary_text, start_time, end_time, distance
             FROM summary_embeddings
             WHERE embedding MATCH ?1 AND k = ?2
             ORDER BY distance
             LIMIT ?2"
        };

        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| CisError::storage(format!("Failed to prepare query: {}", e)))?;

        let mut results = Vec::new();
        
        if let Some(room) = room_id {
            let rows = stmt.query_map(
                rusqlite::params![&query_bytes, room, limit as i64],
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
                rusqlite::params![&query_bytes, limit as i64],
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
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Vec<u8>>(5)?,
            ))
        }).map_err(|e| CisError::storage(format!("Failed to query: {}", e)))?;

        let mut results: Vec<(SummaryResult, f32)> = Vec::new();
        for row in rows {
            let (id, room, text, start, end, emb_bytes) = row
                .map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
            let embedding = deserialize_f32_vec(&emb_bytes);
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
        let intent_bytes = serialize_f32_vec(&intent_vec);

        self.conn.lock().unwrap().execute(
            "INSERT INTO skill_intent_vec (skill_id, embedding, skill_name, description, project)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(skill_id) DO UPDATE SET 
                embedding = excluded.embedding,
                skill_name = excluded.skill_name,
                description = excluded.description,
                project = excluded.project",
            rusqlite::params![
                &semantics.skill_id,
                &intent_bytes,
                &semantics.skill_name,
                &semantics.intent_description,
                &semantics.project
            ],
        ).map_err(|e| CisError::storage(format!("Failed to register skill intent: {}", e)))?;

        // 索引能力描述
        let cap_vec = self.embedding.embed(&semantics.capability_description).await?;
        let cap_bytes = serialize_f32_vec(&cap_vec);

        self.conn.lock().unwrap().execute(
            "INSERT INTO skill_capability_vec (skill_id, embedding, skill_name, description, project)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(skill_id) DO UPDATE SET 
                embedding = excluded.embedding,
                skill_name = excluded.skill_name,
                description = excluded.description,
                project = excluded.project",
            rusqlite::params![
                &semantics.skill_id,
                &cap_bytes,
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
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            }).map_err(|e| CisError::storage(format!("Failed to query intent: {}", e)))?;

            for row in rows {
                let (id, name, emb_bytes) = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                let embedding = deserialize_f32_vec(&emb_bytes);
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
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                ))
            }).map_err(|e| CisError::storage(format!("Failed to query capability: {}", e)))?;

            for row in rows {
                let (id, emb_bytes) = row.map_err(|e| CisError::storage(format!("Failed to get row: {}", e)))?;
                let embedding = deserialize_f32_vec(&emb_bytes);
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
    pub fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
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
    pub fn create_hnsw_indexes(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        // 为memory索引创建HNSW
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
        ).map_err(|e| CisError::storage(format!("Failed to create HNSW index: {}", e)))?;
        
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::{EmbeddingService, DEFAULT_EMBEDDING_DIM};
    use async_trait::async_trait;

    /// 模拟 embedding service（用于测试）
    struct MockEmbeddingService;

    #[async_trait]
    impl EmbeddingService for MockEmbeddingService {
        async fn embed(&self, text: &str) -> Result<Vec<f32>> {
            // 简单的确定性模拟：根据文本哈希生成向量
            let mut vec = vec![0.0f32; DEFAULT_EMBEDDING_DIM];
            let hash = text.bytes().fold(0u64, |acc, b| {
                acc.wrapping_mul(31).wrapping_add(b as u64)
            });
            for i in 0..DEFAULT_EMBEDDING_DIM {
                let val = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) * 2.0 - 1.0;
                vec[i] = val;
            }
            // 归一化
            let norm = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for x in &mut vec {
                    *x /= norm;
                }
            }
            Ok(vec)
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

        // 搜索暗黑模式相关
        let results = storage.search_memory("dark mode", 5, None).await.unwrap();
        assert!(!results.is_empty(), "Should find at least one result");
        
        // 第一个结果应该是 key1
        if !results.is_empty() {
            assert_eq!(results[0].key, "key1");
            assert!(results[0].similarity > 0.5);
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

        // 验证可以搜索到
        let results = storage.search_memory("batch", 10, None).await.unwrap();
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

        // 搜索消息
        let results = storage.search_messages("dark mode", Some("room1"), 5, None).await.unwrap();
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

        // 搜索技能
        let results = storage.search_skills("night theme", None, 5, None).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].skill_name, "DarkMode");
        assert!(results[0].combined_score > 0.5);

        // 按项目过滤
        let results = storage.search_skills("theme", Some("ui"), 5, None).await.unwrap();
        assert!(!results.is_empty());

        let results = storage.search_skills("theme", Some("other"), 5, None).await.unwrap();
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

        let results = storage.search_summaries("dark mode", Some("room1"), 5, None).await.unwrap();
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
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "key1");
    }
}
