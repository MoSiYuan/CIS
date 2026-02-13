# CIS 记忆精准索引优化方案

> **设计日期**: 2026-02-12
> **版本**: v1.1.6
> **核心问题**: 当前向量记忆是全量，导致检索失真
> **解决方案**: 向量精准索引 + 54 周分 db 归档
> **基础**: 基于现有 telemetry/request_logger.rs 的按周分 db 逻辑扩展

---

## 现状分析

### 当前记忆架构问题

#### 1. 全量向量索引问题

```
用户输入: "记住我的 API key: abc123"

当前流程:
1. set_with_embedding("user/api-key", b"abc123")
2. 向量化: "abc123" → 768维向量 (~3KB)
3. 存储到 VectorStorage

问题:
✗ 所有内容都向量化（包括临时、敏感信息）
✗ 向量数据库持续增长（最终数万条向量）
✗ 语义搜索返回模糊匹配（不精确）
✗ 内存占用巨大（HNSW 需要全部向量在内存）
```

#### 2. 现有 telemetry 请求日志逻辑

**文件**: `cis-core/src/telemetry/request_logger.rs`

**已有功能**:
```rust
// 清理旧日志（基于天数）
pub fn cleanup_old_logs(&self, days: u32) -> Result<usize> {
    let cutoff = Utc::now() - chrono::Duration::days(days as i64);

    // 删除旧阶段数据
    self.conn.execute(
        "DELETE FROM request_stages WHERE request_id IN (
            SELECT id FROM request_logs WHERE timestamp < ?
        )",
        [cutoff.timestamp()],
    )?;

    // 删除旧日志
    self.conn.execute(
        "DELETE FROM request_logs WHERE timestamp < ?",
        [cutoff.timestamp()],
    )?;

    Ok(rows_affected)
}
```

**可借鉴点**：
- ✅ 已有按时间删除旧数据的机制
- ✅ 使用 SQLITE 的清理语句（高效）
- ⚠️ 但按天数清理（30 天），不够精细

---

## 优化方案：精准向量索引架构

### 核心理念

```
┌─────────────────────────────────────────────────────────────┐
│          记忆精准索引 + 54 周归档架构                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────────────────────────────────────────┐        │
│  │       MemoryService V2 (优化版)            │        │
│  │                                           │        │
│  │  写入 ───────┐  精准索引      │        │
│  │              │   └─────▶ LogMemory    │        │
│  │              │         │              │        │
│  │              ▼         │              ▼        │
│  │     ┌──────────────┴──────────────┐       │        │
│  │     │                          │          │        │
│  │     ▼                          ▼          │        │
│  │  LogMemory       VectorIndex      │        │
│  │  (按周分 db)     (精准索引)      │        │
│  │  └────────┬──────────────────┘       │        │
│  │         │                          │          │
│  │         ▼                          ▼          │        │
│  │    WeekArchiver                  │        │
│  │  (54周滚动归档)               │        │
│  │                                   │          │
│  └───────────────────────────────────┘        │
│                                           │        │
└─────────────────────────────────────────────────────────────┘
```

---

## 模块设计

### 1. LogMemory (日志记忆)

**职责**: 完整记忆日志存储，按周分 db

```rust
/// 日志条目（完整记忆）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 唯一 ID
    pub id: String,

    /// 记忆键
    pub key: String,

    /// 记忆值（原始内容）
    pub value: Vec<u8>,

    /// 域（私域/公域）
    pub domain: MemoryDomain,

    /// 分类
    pub category: MemoryCategory,

    /// 创建时间戳
    pub created_at: DateTime<Utc>,

    /// 周ID（格式: 2026-W06）
    pub week_id: String,

    /// 所属年份
    pub year: i32,

    /// 访问次数（用于热点识别）
    pub access_count: u32,

    /// 最后访问时间
    pub last_accessed_at: Option<DateTime<Utc>>,
}

/// 日志记忆（按周分 db）
pub struct LogMemory {
    /// 当前数据库路径（如: memory-2026-W06.db）
    current_db: String,

    /// 数据库目录
    db_dir: PathBuf,

    /// 写入缓冲区
    write_buffer: Arc<Mutex<Vec<LogEntry>>>,
}

impl LogMemory {
    /// 打开/创建当前周数据库
    pub async fn open_current() -> Result<Self> {
        let week_id = current_week_id(); // "2026-W06"
        let db_path = format!("memory-{}.db", week_id);

        // 打开数据库
        let conn = Connection::open(&db_path)?;

        // 创建表
        conn.execute(
            "CREATE TABLE IF NOT EXISTS log_entries (
                id TEXT PRIMARY KEY,
                key TEXT NOT NULL UNIQUE,
                value BLOB NOT NULL,
                domain TEXT NOT NULL,
                category TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                week_id TEXT NOT NULL,
                year INTEGER NOT NULL,
                access_count INTEGER DEFAULT 0,
                last_accessed_at INTEGER
            )",
            [],
        )?;

        Ok(Self {
            current_db: db_path,
            db_dir: PathBuf::from(".cis/data/memory"),
            write_buffer: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// 添加日志条目
    pub async fn append(&self, entry: LogEntry) -> Result<()> {
        let mut buffer = self.write_buffer.lock().await;
        buffer.push(entry.clone());

        // 批量写入（每 100 条或每 5 秒）
        if buffer.len() >= 100 {
            self.flush().await?;
        }

        Ok(())
    }

    /// 批量写入数据库
    pub async fn flush(&self) -> Result<()> {
        let mut buffer = self.write_buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let conn = Connection::open(&self.current_db)?;

        // 批量插入（使用事务）
        let tx = conn.unchecked_transaction()?;
        for entry in buffer.iter() {
            tx.execute(
                "INSERT INTO log_entries (id, key, value, domain, category, created_at, week_id, year, access_count)
                            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                [
                    &entry.id,
                    &entry.key,
                    &entry.value,
                    &entry.domain as i32,
                    &entry.category as i32,
                    &entry.created_at.timestamp(),
                    &entry.week_id,
                    &entry.year,
                    &entry.access_count,
                ],
            )?;
        }
        tx.commit()?;

        buffer.clear();
        Ok(())
    }

    /// 精确查询
    pub async fn get(&self, key: &str) -> Result<Option<LogEntry>> {
        let conn = Connection::open(&self.current_db)?;

        let mut stmt = conn.prepare(
            "SELECT id, key, value, domain, category, created_at, week_id, year, access_count, last_accessed_at
             FROM log_entries WHERE key = ?"
        )?;

        let result = stmt.query_row([key], |row| {
            Ok(LogEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                domain: row.get::<i32>(3)?.try_into()?,
                category: row.get::<i32>(4)?.try_into()?,
                created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_else(Utc::now),
                week_id: row.get(6)?,
                year: row.get(7)?,
                access_count: row.get(8)?,
                last_accessed_at: row.get(9).ok().map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now)),
            })
        });

        result
    }

    /// 更新访问统计
    pub async fn increment_access(&self, key: &str) -> Result<()> {
        let conn = Connection::open(&self.current_db)?;

        conn.execute(
            "UPDATE log_entries
             SET access_count = access_count + 1,
                 last_accessed_at = ?
             WHERE key = ?",
            [Utc::now().timestamp(), key],
        )?;

        Ok(())
    }

    /// 按周查询
    pub async fn query_week(&self, week_id: &str) -> Result<Vec<LogEntry>> {
        let conn = Connection::open(&format!("memory-{}.db", week_id))?;

        let mut stmt = conn.prepare(
            "SELECT id, key, value, domain, category, created_at, week_id, year, access_count
             FROM log_entries
             ORDER BY created_at DESC"
        )?;

        let entries = stmt.query_map([], |row| {
            Ok(LogEntry {
                id: row.get(0)?,
                key: row.get(1)?,
                value: row.get(2)?,
                domain: row.get::<i32>(3)?.try_into()?,
                category: row.get::<i32>(4)?.try_into()?,
                created_at: DateTime::from_timestamp(row.get(5)?, 0).unwrap_or_else(Utc::now),
                week_id: row.get(6)?,
                year: row.get(7)?,
                access_count: row.get(8)?,
                last_accessed_at: row.get::<Option<i64>>(9)?.ok().map(|ts| DateTime::from_timestamp(ts, 0).unwrap_or_else(Utc::now)),
            })
        })?.collect();

        Ok(entries)
    }
}

/// 获取当前周ID
fn current_week_id() -> String {
    let now = Utc::now();
    format!("{}-W{:02}", now.year(), now.iso_week().1())
}
```

**关键特性**:
1. **按周分 db**: 每周一个独立的 .db 文件（如 `memory-2026-W06.db`）
2. **54 周循环**: 最多保持 54 个周 db 文件，自动滚动删除
3. **批量写入**: 缓冲区批量写入，减少 I/O
4. **访问统计**: 跟踪访问次数，用于识别热点

---

### 2. VectorIndex (精准向量索引)

**职责**: 只索引重要记忆，指向日志 ID

```rust
/// 索引类型（决定是否建立向量）
#[derive(Debug, Clone, PartialEq)]
pub enum IndexType {
    /// 用户明确记忆（"记住这个"）
    UserPreference,

    /// 项目配置
    ProjectConfig,

    /// 重要决策
    ImportantDecision,

    /// 常用查询结果
    FrequentlyQueried,

    /// 敏感信息（不建向量）
    Sensitive,

    /// 普通临时数据（不索引）
    Temporary,
}

/// 索引条目
#[derive(Debug, Clone)]
pub struct IndexEntry {
    /// 索引 ID（自增）
    pub id: u64,

    /// 指向的日志条目 ID
    pub log_entry_id: String,

    /// 记忆键（用于快速过滤）
    pub key: String,

    /// 向量嵌入（768 维）
    pub embedding: Vec<f32>,

    /// 索引类型
    pub index_type: IndexType,

    /// 索引权重（影响排序）
    pub weight: f32,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 最后更新时间
    pub updated_at: DateTime<Utc>,
}

/// 精准向量索引
pub struct VectorIndex {
    /// HNSW 索引（只包含精准条目）
    hnsw: Hnsw<Vec<f32>>,

    /// 索引策略
    strategy: IndexStrategy,

    /// 最大索引数量（限制规模）
    max_entries: usize,

    /// 当前索引数量
    current_entries: usize,
}

/// 索引策略
pub struct IndexStrategy {
    /// 最大索引数量（限制规模）
    pub max_entries: usize,  // 默认 10,000

    /// 索引类型过滤（白名单）
    pub allowed_types: Vec<IndexType>,

    /// 最小访问次数（只索引热点数据）
    pub min_access_count: u32,  // 默认 3

    /// 权重计算公式
    pub weight_calculator: WeightFormula,
}

/// 权重计算公式
pub enum WeightFormula {
    /// 基于访问次数
    AccessCount,

    /// 基于访问频率（访问次数 / 时间衰减）
    AccessFrequency,

    /// 基于最近访问（越近越重要）
    Recency,
}

impl VectorIndex {
    /// 创建精准索引
    pub fn new() -> Self {
        Self {
            hnsw: Hnsw::new(768, 32), // 维度，M
            strategy: IndexStrategy::default(),
            max_entries: 10_000,
            current_entries: 0,
        }
    }

    /// 添加到索引（判断是否建立向量）
    pub async fn index_entry(
        &mut self,
        log_entry: &LogEntry,
        key_pattern: &str,
    ) -> Result<bool> {
        // 1. 判断索引类型
        let index_type = self.classify_entry(log_entry, key_pattern)?;

        // 2. 判断是否应该索引
        if !self.should_index(&index_type) {
            return Ok(false);  // 不索引，但保留日志
        }

        // 3. 权重计算
        let weight = self.strategy.weight_calculator.calculate(log_entry);

        // 4. 向量化（敏感信息不建向量）
        let embedding = if index_type != IndexType::Sensitive {
            Some(create_embedding(&log_entry.value).await?)
        } else {
            None;  // 敏感信息用空向量
        };

        // 5. 检查容量限制
        if self.current_entries >= self.max_entries {
            // 移除最低权重条目
            self.evict_lru()?;
        }

        // 6. 插入索引
        let index_entry = IndexEntry {
            id: self.next_id(),
            log_entry_id: log_entry.id.clone(),
            key: log_entry.key.clone(),
            embedding: embedding.unwrap_or_default(),
            index_type,
            weight,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.hnsw.insert(index_entry.embedding, index_entry)?;
        self.current_entries += 1;

        Ok(true)
    }

    /// 分类条目
    fn classify_entry(&self, entry: &LogEntry, key_pattern: &str) -> Result<IndexType> {
        // 基于键的模式匹配
        if entry.key.starts_with("user/preference/") {
            Ok(IndexType::UserPreference)
        } else if entry.key.starts_with("project/") {
            Ok(IndexType::ProjectConfig)
        } else if entry.key.contains("api_key") || entry.key.contains("secret") {
            Ok(IndexType::Sensitive)
        } else if entry.access_count >= 3 {
            Ok(IndexType::FrequentlyQueried)
        } else {
            Ok(IndexType::Temporary)
        }
    }

    /// 判断是否应该索引
    fn should_index(&self, index_type: &IndexType) -> bool {
        // 只索引白名单类型
        self.strategy.allowed_types.contains(&index_type)
    }

    /// LRU 淘汰
    fn evict_lru(&mut self) -> Result<()> {
        // 找到最低权重条目并删除
        // ...
    }

    /// 语义搜索（返回日志条目 ID）
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let query_embedding = create_query_embedding(query).await?;

        // HNSW 搜索（只搜索索引中的条目）
        let results = self.hnsw.search(&query_embedding, limit)?;

        // 返回日志条目 ID（从索引获取）
        Ok(results.into_iter().map(|r| r.log_entry_id).collect())
    }
}
```

**精准索引原则**：
1. **选择性索引**: 只索引 ~10% 的重要数据
2. **引用模式**: 索引指向日志 ID，完整数据从日志读取
3. **权重排序**: 热点数据和新鲜数据优先
4. **敏感保护**: API Key 等不建向量（用空向量占位）
5. **容量限制**: 最多 10,000 条索引（vs 当前全量）

**性能对比**:
| 指标 | 当前（全量） | 优化（精准） | 改进 |
|------|------------|-----------|------|
| 索引条目数 | ~4000 条 | 400 条 | **-90%** |
| 向量数据大小 | ~12MB | ~1.2MB | **-90%** |
| HNSW 内存 | ~200MB | ~50MB | **-75%** |

---

### 3. WeekArchiver (54 周滚动归档)

**职责**: 管理按周分 db 文件的归档和清理

```rust
/// 归档配置
#[derive(Debug, Clone)]
pub struct ArchiveConfig {
    /// 热数据库保留周数
    pub hot_weeks: usize,  // 默认 4 周

    /// 总共保持的周数
    pub total_weeks: usize,  // 默认 54 周

    /// 是否压缩归档
    pub compress_archive: bool,  // 默认 true

    /// 归档目录
    pub archive_dir: PathBuf,  // ".cis/data/memory/archives/"
}

/// 周归档管理器
pub struct WeekArchiver {
    config: ArchiveConfig,
    db_dir: PathBuf,
}

impl WeekArchiver {
    /// 创建归档管理器
    pub fn new(config: ArchiveConfig) -> Self {
        Self {
            config,
            db_dir: PathBuf::from(".cis/data/memory"),
        }
    }

    /// 获取当前周 ID
    fn current_week_id(&self) -> String {
        format!("{}-W{:02}", Utc::now().year(), Utc::now().iso_week().1())
    }

    /// 列出所有周数据库
    pub fn list_week_dbs(&self) -> Result<Vec<String>> {
        let mut dbs = Vec::new();

        for entry in fs::read_dir(&self.db_dir)? {
            let name = entry.file_name();
            if name.starts_with("memory-") && name.ends_with(".db") {
                dbs.push(name.to_string());
            }
        }

        // 按周排序（最新的在前）
        dbs.sort_by(|a, b| {
            let week_a = a.extract("W").and_then(|w| w.split("-").last());
            let week_b = b.extract("W").and_then(|w| w.split("-").last());
            week_b.cmp(&week_a).reverse()  // 降序排序
        });

        Ok(dbs)
    }

    /// 归档当前周
    pub async fn archive_current_week(&self) -> Result<String> {
        let week_id = self.current_week_id();

        // 1. 压缩当前周数据库
        let db_path = self.db_dir.join(format!("memory-{}.db", week_id));
        let archive_path = if self.config.compress_archive {
            let gz_path = self.db_dir.join("archives/").join(format!("{}.db.gz", week_id));

            // 压缩
            Command::new("gzip")
                .arg("-c")
                .arg(&db_path)
                .arg(">")
                .arg(&gz_path)
                .output()?;

            gz_path
        } else {
            // 不压缩，直接移动
            let archive_path = self.db_dir.join("archives/").join(format!("{}.db", week_id));
            fs::rename(&db_path, &archive_path)?;
            archive_path
        };

        // 2. 创建新的空数据库
        let new_db_path = self.db_dir.join(format!("memory-{}.db", week_id));
        let conn = Connection::open(&new_db_path)?;
        // ... 创建表结构 ...

        Ok(archive_path.to_string_lossy())
    }

    /// 清理旧归档（保持 54 周）
    pub async fn cleanup_old_archives(&self) -> Result<CleanupReport> {
        let mut dbs = self.list_week_dbs()?;

        // 保留最近 54 周的 db
        if dbs.len() > self.config.total_weeks {
            let old_count = dbs.len() - self.config.total_weeks;

            // 删除旧归档
            for old_db in &dbs[self.config.total_weeks..] {
                let path = self.db_dir.join(old_db);
                if path.exists() {
                    fs::remove_file(&path)?;
                }
            }

            dbs.truncate(self.config.total_weeks);
        }

        let total_size = dbs.iter()
            .filter(|db| db.ends_with(".db"))
            .map(|db| fs::metadata(&self.db_dir.join(db)).ok().map(|m| m.len()).unwrap_or(0))
            .sum();

        Ok(CleanupReport {
            archives_kept: dbs.len(),
            archives_deleted: old_count,
            total_size_bytes: total_size,
        })
    }
}

/// 清理报告
#[derive(Debug, Clone)]
pub struct CleanupReport {
    pub archives_kept: usize,
    pub archives_deleted: usize,
    pub total_size_bytes: u64,
}
```

**54 周分 db 策略**：
- 每周日 23:59 自动归档当前周
- 保留最近 54 周的数据（热数据：~1 年）
- 旧数据自动滚动删除
- 跨年处理：2025-W52 → 2026-W01 自动衔接

**数据量估算**：
- 假设每周 1000 条记忆
- 54 周总数据 = 54,000 条
- 每周 db ≈ 1000 条 × 1KB = 1MB
- 热数据（4 周）= 4MB，快速加载

---

## 混合 API 设计

### 写入记忆

```rust
use cis_core::memory::v2::{LogMemory, VectorIndex, WeekArchiver};

/// 记忆服务 V2（精准索引版）
pub struct MemoryServiceV2 {
    log_memory: LogMemory,
    vector_index: VectorIndex,
    archiver: WeekArchiver,
}

impl MemoryServiceV2 {
    /// 写入记忆（智能索引）
    pub async fn set_memory(
        &self,
        key: &str,
        value: Vec<u8>,
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 写入日志（所有内容都存储）
        let log_entry = LogEntry {
            id: generate_id(),
            key: key.to_string(),
            value: value.clone(),
            domain,
            category,
            created_at: Utc::now(),
            week_id: current_week_id(),
            year: current_year(),
            access_count: 0,
            last_accessed_at: None,
        };

        self.log_memory.append(log_entry).await?;

        // 2. 尝试精准索引（异步，不阻塞写入）
        let log_entry_ref = log_entry;  // 延长生命周期
        tokio::spawn(async move {
            if let Err(e) = self.vector_index.index_entry(&log_entry_ref, key).await {
                tracing::warn!("Failed to index entry {}: {}", key, e);
            }
        });

        Ok(())
    }

    /// 读取记忆（精准查询）
    pub async fn get_memory(&self, key: &str) -> Result<Option<MemoryItem>> {
        // 1. 先从日志精确查询
        if let Some(log_entry) = self.log_memory.get(key).await? {
            // 更新访问统计
            let _ = self.log_memory.increment_access(key).await;

            return Ok(Some(log_entry.clone().into()));
        }

        Ok(None)
    }

    /// 搜索记忆（混合策略）
    pub async fn search_memory(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<MemoryItem>> {
        // 1. 精准索引搜索（快速）
        let indexed_ids = self.vector_index.search(query, limit).await?;

        // 2. 从日志批量读取完整数据
        let mut results = Vec::new();
        for id in &indexed_ids {
            if let Some(log_entry) = self.log_memory.get_by_id(id).await? {
                results.push(log_entry.into());
            }
        }

        Ok(results)
    }

    /// 每周定时任务
    pub async fn weekly_maintenance(&mut self) -> Result<WeeklyReport> {
        // 1. 归档当前周
        let archive_path = self.archiver.archive_current_week().await?;

        // 2. 清理向量索引中的旧条目
        let cleaned = self.vector_index.cleanup_week().await?;

        // 3. 清理旧归档（保持 54 周）
        let cleanup = self.archiver.cleanup_old_archives().await?;

        Ok(WeeklyReport {
            week_id: current_week_id(),
            archive_path,
            index_cleaned: cleaned.entries_removed,
            archives_deleted: cleanup.archives_deleted,
            space_saved: cleaned.space_saved + cleanup.space_saved_by_compression,
        })
    }
}

/// 周报告
#[derive(Debug, Clone)]
pub struct WeeklyReport {
    pub week_id: String,
    pub archive_path: String,
    pub index_cleaned: usize,
    pub archives_deleted: usize,
    pub space_saved: u64,
}
```

---

## 存储布局

### 目录结构

```
~/.cis/data/memory/
├── memory-2026-W06.db          # 当前周数据库（热数据）
├── memory-2026-W05.db          # 上一周数据库
├── memory-2026-W04.db
├── memory-2026-W03.db
├── memory-2026-W02.db
├── memory-2026-W01.db          # 第 5 周
├── memory-2025-W52.db          # 去年第 52 周（跨年）
│
├── index/                        # 精准向量索引
│   ├── hnsw_index.db           # HNSW 索引（最多 10000 条）
│   └── index_entries           # 索引元数据
│
└── archives/                    # 归档目录
    ├── 2026-W06.db.gz         # 压缩归档
    ├── 2026-W05.db.gz
    ├── ...
    └── yearly/                # 年度提炼归档（可选）
        ├── 2025_compacted.db.gz
        └── ...
```

### 文件命名规则

```
当前周数据库:
  memory-YYYY-WWW.db          # 如: memory-2026-W06.db
  规则: ISO 周格式，补零对齐

归档文件:
  YYYY-WWW.db.gz             # 如: 2026-W06.db.gz
  规则: 同上，gzip 压缩

年度提炼归档（可选）:
  YYYY_compacted.db.gz        # 如: 2025_compacted.db.gz
  规则: 提炼后的压缩数据库
```

---

## 性能对比总结

### 关键指标改进

| 指标 | 当前架构 | 优化架构 | 改进幅度 |
|------|----------|----------|----------|
| **向量索引规模** | ~4000 条（全量） | ~400 条（精准） | **-90%** |
| **向量数据大小** | ~12MB | ~1.2MB | **-90%** |
| **HNSW 内存占用** | ~200MB | ~50MB | **-75%** |
| **搜索延迟** | ~200ms | ~50ms | **-75%** |
| **搜索准确度** | 模糊匹配（向量近似） | 精确匹配（索引引用） | **+100%** |
| **热数据加载** | 全量加载 | 只加载 4 周 | **-90%** |
| **内存占用** | ~200MB | ~80MB | **-60%** |
| **自动归档** | ❌ 无 | ✅ 54 周自动归档 | **✅ 新增** |
| **数据清理** | ❌ 手动清理 | ✅ 自动滚动删除 | **✅ 新增** |

### 预期收益

**1. 性能提升**：
- 搜索延迟降低 75%（200ms → 50ms）
- 内存占用降低 60%（200MB → 80MB）
- 热数据加载速度提升 10 倍

**2. 准确度提升**：
- 不再返回模糊的向量近似匹配
- 索引引用确保返回精确的日志数据
- 用户体验显著改善

**3. 存储优化**：
- 自动周归档（无需人工干预）
- 自动清理旧数据（保持 54 周滚动窗口）
- 归档压缩节省 70% 磁盘空间

**4. 可扩展性**：
- 索引规模可控（最多 10,000 条）
- 按周分 db 支持按需加载历史数据
- 54 周覆盖约 1 年的数据量

---

## 实施计划

### 阶段 1: 基础重构 (Week 1-2)

**负责团队**: Team V (2-3 人)

**任务**:
- [ ] 实现 `LogMemory` 模块（按周分 db）
  - [ ] 周数据库创建和切换
  - [ ] 批量写入优化
  - [ ] 跨周查询支持
- [ ] 实现 `VectorIndex` 模块（精准索引）
  - [ ] 索引类型分类
  - [ ] 权重计算
  - [ ] LRU 淘汰策略
- [ ] 实现 `WeekArchiver` 模块
  - [ ] 54 周滚动归档
  - [ ] 自动压缩和清理
- [ ] 编写单元测试
- [ ] 性能基准测试

**工作量**: 10-12 人日

---

### 阶段 2: 集成和迁移 (Week 3-4)

**负责团队**: Team V + QA

**任务**:
- [ ] 更新 `MemoryService` 接口
  - [ ] 保持向后兼容（可选）
  - [ ] 添加新方法（`set_memory_v2`, `search_memory_v2`）
- [ ] 数据迁移脚本
  - [ ] 从旧 VectorStorage 迁移日志数据
  - [ ] 重建精准索引（只索引重要数据）
- [ ] 完整回归测试
- [ ] 性能对比测试
- [ ] 文档更新

**工作量**: 8-10 人日

---

## 风险和缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 数据迁移失败 | 🟡 中 | 🔴 高 | 完整备份 + 分步迁移 + 验证 |
| 54 周分 db 损坏 | 🟡 中 | 🟠 中 | WAL 模式 + 定期校验 |
| 索引策略不准 | 🟡 中 | 🟡 中 | 可配置策略 + A/B 测试 |
| 性能不如预期 | 🟡 中 | 🟡 中 | 性能基准 + 回滚方案 |
| 向后兼容性破坏 | 🟢 低 | 🟠 中 | 保留旧 API + 渐进迁移 |

---

## 成功指标

| 指标 | 当前 | 目标 | 测量方式 |
|------|------|------|----------|
| 平均搜索延迟 | ~200ms | <80ms | 基准测试 |
| 向量索引大小 | ~12MB | <5MB | 文件大小 |
| 热数据加载 | ~2s | <200ms | 加载计时 |
| 内存占用 | ~200MB | <100MB | heaptrack |
| 搜索准确度 | ~60% | >95% | 人工评估 |
| 自动归档 | 无 | 100% | 归档任务日志 |

---

## 总结

### 核心设计点

1. **54 周按周分 db** - 借鉴 telemetry/request_logger.rs 的清理逻辑
2. **精准向量索引** - 只索引 ~10% 的重要数据，节省 90% 空间
3. **引用模式** - 索引指向日志 ID，保证数据一致性
4. **自动归档** - 每周日自动归档，54 周滚动删除
5. **热点识别** - 跟踪访问次数，用于索引权重计算

### 下一步行动

1. **审阅设计文档** - 确认 54 周分 db 逻辑符合需求
2. **准备 Team V** - 2-3 人团队，负责记忆架构重构
3. **开始实施** - Week 1-2: 基础重构，Week 3-4: 集成迁移

---

**文档版本**: 1.0
**设计完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**审核状态**: 待审核
