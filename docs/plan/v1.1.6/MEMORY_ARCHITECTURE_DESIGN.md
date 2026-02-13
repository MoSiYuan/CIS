# CIS 记忆架构优化设计 - 向量索引 + 日志归档

> **设计日期**: 2026-02-12
> **版本**: v1.1.6
> **核心问题**: 当前向量记忆是全量记忆，导致检索失真
> **解决方案**: 向量索引 + 日志归档混合架构

---

## 问题分析

### 当前记忆架构问题

```
┌─────────────────────────────────────────────────────────────┐
│              当前 CIS 记忆架构                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌────────────────┐         ┌────────────────┐           │
│  │ MemoryService  │────────▶│ VectorStorage  │           │
│  │ (KeyValue)    │         │ (HNSW Index)  │           │
│  └────────────────┘         └────────────────┘           │
│         │                           ▲                    │
│         └───────────────────────────┘                    │
│                                                             │
│  set_with_embedding()                                  │
│  ──▶ 向量化所有内容 (1000 tokens → 768 维向量)        │
│                                                             │
│  semantic_search()                                    │
│  ──▶ 向量相似度搜索 (会返回模糊匹配结果)              │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 核心问题

| 问题 | 影响 | 根源原因 |
|------|------|----------|
| **检索失真** | 语义搜索返回不精确结果 | 向量是近似搜索，非精确匹配 |
| **性能压力** | 向量数据库持续增长 | 所有记忆都存储向量（包括临时内容） |
| **索引混乱** | 热点数据和冷数据混合 | 用户偏好、一次性查询都平等索引 |
| **无归档机制** | 旧数据无法清理 | 缺少按周/年归档提炼 |
| **内存占用** | 向量索引占用大量内存 | HNSW 需要全部向量在内存中 |

---

## 优化架构设计

### 新架构：向量索引 + 日志归档

```
┌─────────────────────────────────────────────────────────────────┐
│             CIS 混合记忆架构                            │
├─────────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌───────────────────────────────────────────────────┐        │
│  │          MemoryService V2                  │        │
│  │  ┌─────────────────┐  ┌─────────────────┐       │
│  │  │ LogMemory      │  │ VectorIndex    │       │
│  │  │ (主存储)       │  │ (精准索引)     │       │
│  │  └────────┬────────┘  └───────┬───────┘       │
│  │           │                      │                  │
│  │           ▼                      ▼                  │
│  │  ┌──────────────┐   ┌──────────────┐         │
│  │  │ WeeklyLogs   │   │ PrecisionIndex│         │
│  │  │ (按周归档)    │   │ (精选索引)    │         │
│  │  └──────┬───────┘   └──────┬───────┘         │
│  │         │                      │                  │
│  │         ▼                      ▼                  │
│  │  ┌─────────────────────────────────┐             │
│  │  │     ArchiveCompactor          │             │
│  │  │     (归档提炼器)             │             │
│  │  └─────────────┬───────────────┘             │
│  │                │                             │
│  │         ┌──────▼──────┐                   │
│  │         │ YearlyArchives                   │
│  │         │ (按年冷存)                      │
│  │         └───────────────────────────┘          │
│  └───────────────────────────────────────────────────┘        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 核心概念

### 1. 日志记忆 (LogMemory)

**职责**: 完整的记忆日志存储，按周归档

```rust
/// 日志记忆条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// 唯一 ID
    pub id: String,

    /// 记忆键（类似文件路径）
    pub key: String,

    /// 记忆值（原始内容）
    pub value: Vec<u8>,

    /// 域（私域/公域）
    pub domain: MemoryDomain,

    /// 分类
    pub category: MemoryCategory,

    /// 创建时间戳
    pub created_at: DateTime<Utc>,

    /// 所属周（格式: 2026-W06）
    pub week_id: String,

    /// 所属年
    pub year: u32,

    /// 是否被索引（向量索引是否包含此条目）
    pub indexed: bool,

    /// 访问次数（用于热点识别）
    pub access_count: u32,

    /// 最后访问时间
    pub last_accessed_at: Option<DateTime<Utc>>,
}

/// 日志存储
pub struct LogMemory {
    db: Arc<Mutex<SqliteConnection>>,

    /// 当前周（格式: 2026-W06）
    current_week: String,

    /// 写入缓冲区（批量写入优化）
    write_buffer: Arc<Mutex<Vec<LogEntry>>>,
}

impl LogMemory {
    /// 添加日志条目
    pub async fn append(&self, entry: LogEntry) -> Result<()> {
        let mut buffer = self.write_buffer.lock().await;
        buffer.push(entry);

        // 批量写入（每 100 条或每 5 秒）
        if buffer.len() >= 100 {
            self.flush().await?;
        }

        Ok(())
    }

    /// 读取日志条目（精确键查询）
    pub async fn get(&self, key: &str) -> Result<Option<LogEntry>> {
        // 精确匹配查询
        let query = "SELECT * FROM log_entries WHERE key = ?";
        self.db.execute(query, [key]).await
    }

    /// 按周归档
    pub async fn archive_week(&self, week_id: &str) -> Result<String> {
        // 1. 创建归档文件
        let archive_path = format!("logs/archive/{}.db", week_id);

        // 2. 迁移该周的所有数据到归档
        let query = "ATTACH DATABASE ? AS archive SELECT * FROM log_entries WHERE week_id = ?";
        // ... 执行归档

        // 3. 从主库删除已归档数据
        let delete_query = "DELETE FROM log_entries WHERE week_id = ?";
        self.db.execute(delete_query, [week_id]).await;

        Ok(archive_path)
    }
}
```

**归档策略**：
- 每周日 23:59 自动归档当前周
- 保留最近 4 周在主库（热数据）
- 旧周数据迁移到独立归档文件
- 归档文件压缩（gzip）
- **54 周按周分 db**：一年 52-53 周，54 周用于覆盖跨年周期
  - 例如：2025-W52 到 2026-W01（跨年周期）
  - 每周一个独立的 .db 文件
  - 文件命名格式：`YYYY-WWW.db`（如 2026-W06.db）
  - 最多保持 54 个周 db 文件（滚动删除最旧的）

**数据量估算**：
- 假设每周新增 1000 条记忆
- 4 周热数据 = 4,000 条
- 单条 ~1KB → 4MB 热数据
- **54 周分摊**：平均每周 db = 总数据 / 54 ≈ 减轻压力

---

### 2. 向量索引 (VectorIndex)

**职责**: 精准索引，只索引重要记忆，指向日志记忆

```rust
/// 精准索引条目
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
}

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

    /// API Key 等敏感信息（不建向量）
    Sensitive,
}

/// 向量索引（精准索引，非全量）
pub struct VectorIndex {
    /// HNSW 索引（只包含精准条目）
    hnsw: Hnsw,

    /// 索引策略
    strategy: IndexStrategy,
}

/// 索引策略
pub struct IndexStrategy {
    /// 最大索引数量（限制索引规模）
    max_entries: usize,  // 默认 10,000

    /// 索引类型过滤
    allowed_types: Vec<IndexType>,

    /// 最小访问次数（只索引热点数据）
    min_access_count: u32,  // 默认 3

    /// 权重计算
    weight_calculator: WeightCalculator,
}

impl VectorIndex {
    /// 添加到索引（判断是否建立向量）
    pub async fn index_entry(&mut self, log_entry: &LogEntry) -> Result<bool> {
        // 1. 判断是否应该索引
        let index_type = self.classify_entry(log_entry)?;
        if !self.should_index(&index_type) {
            return Ok(false);  // 不索引，但保留日志
        }

        // 2. 权重计算（访问次数、新鲜度）
        let weight = self.calculate_weight(log_entry);

        // 3. 向量化（只在需要时）
        let embedding = if index_type != IndexType::Sensitive {
            Some(create_embedding(&log_entry.value).await?)
        } else {
            None;  // 敏感信息不建向量
        };

        // 4. 插入 HNSW
        let index_entry = IndexEntry {
            id: self.next_id(),
            log_entry_id: log_entry.id.clone(),
            key: log_entry.key.clone(),
            embedding: embedding.unwrap_or_default(),
            index_type,
            weight,
            created_at: Utc::now(),
        };

        self.hnsw.insert(index_entry.embedding, index_entry)?;
        Ok(true)
    }

    /// 分类条目（决定索引类型）
    fn classify_entry(&self, entry: &LogEntry) -> Result<IndexType> {
        // 基于键的模式匹配
        if entry.key.starts_with("user/preference/") {
            Ok(IndexType::UserPreference)
        } else if entry.key.starts_with("project/") {
            Ok(IndexType::ProjectConfig)
        } else if entry.category == MemoryCategory::Result {
            Ok(IndexType::FrequentlyQueried)
        } else {
            Ok(IndexType::Sensitive)  // 其他不索引
        }
    }

    /// 语义搜索（返回日志条目 ID）
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<String>> {
        let query_embedding = create_query_embedding(query).await?;

        // HNSW 搜索
        let results = self.hnsw.search(&query_embedding, limit)?;

        // 返回日志条目 ID（从索引获取）
        Ok(results.into_iter().map(|r| r.log_entry_id).collect())
    }
}
```

**精准索引原则**：
1. **选择性索引** - 只索引重要记忆（~10% 的数据）
2. **权重排序** - 热点数据和新鲜数据优先
3. **敏感保护** - API Key 等不建向量
4. **引用指向** - 索引指向日志 ID，完整数据从日志读取

**数据量估算**：
- 假设每周 1000 条记忆
- 精选索引 100 条（10%）
- 4 周热数据索引 = 400 条
- 向量大小: 400 × 768 × 4 bytes = 1.2MB

**vs 当前全量索引**:
- 当前: 4,000 条 × 768 × 4 = 12MB (全量)
- 优化后: 400 条 × 768 × 4 = 1.2MB (精准)
- **节省 90% 索引内存**

---

### 3. 归档提炼器 (ArchiveCompactor)

**职责**: 按年归档提炼旧记忆，减少系统压力

```rust
/// 归档配置
pub struct ArchiveConfig {
    /// 热数据保留周数
    hot_weeks: usize,  // 默认 4 周

    /// 归档压缩
    compress_archive: bool,  // 默认 true

    /// 归档保留年数
    archive_retention_years: usize,  // 默认 5 年
}

/// 归档提炼器
pub struct ArchiveCompactor {
    log_memory: LogMemory,
    vector_index: VectorIndex,
    config: ArchiveConfig,
}

impl ArchiveCompactor {
    /// 每周执行的归档任务
    pub async fn weekly_archive(&mut self) -> Result<ArchiveReport> {
        // 1. 获取当前周
        let current_week = format!("{}-W{:02}", Utc::now().year(), Utc::now().iso_week().1());

        // 2. 归档当前周
        let archive_path = self.log_memory.archive_week(&current_week).await?;

        // 3. 清理向量索引中的旧条目
        let cleaned = self.cleanup_index(&current_week).await?;

        // 4. 压缩归档（可选）
        if self.config.compress_archive {
            self.compress_archive(&archive_path).await?;
        }

        Ok(ArchiveReport {
            week_id: current_week,
            archive_path,
            entries_archived: cleaned.entries_removed,
            index_cleaned: cleaned.index_removed,
            space_saved: cleaned.space_saved,
        })
    }

    /// 每年执行的提炼任务
    pub async fn yearly_compact(&mut self) -> Result<CompactReport> {
        let year = Utc::now().year();

        // 1. 扫描所有归档文件
        let archives = self.scan_year_archives(year).await?;

        // 2. 提炼重要记忆到索引
        let important_entries = self.extract_important(&archives).await?;

        // 3. 合并重复条目
        let deduped = self.deduplicate_entries(&important_entries).await?;

        // 4. 重建年索引
        self.rebuild_year_index(&deduped).await?;

        // 5. 删除旧归档
        self.delete_old_archives(year - self.config.archive_retention_years).await?;

        Ok(CompactReport {
            year,
            archives_processed: archives.len(),
            important_extracted: important_entries.len(),
            duplicates_removed: deduped.duplicates_count,
            final_index_entries: deduped.entries.len(),
            space_saved: deduped.space_saved,
        })
    }

    /// 提取重要记忆（基于访问模式）
    async fn extract_important(&self, archives: &[ArchiveFile]) -> Result<Vec<LogEntry>> {
        let mut important = Vec::new();

        for archive in archives {
            // 1. 读取归档中的访问统计
            let entries = archive.read_entries().await?;

            // 2. 筛选高频访问的条目
            for entry in entries {
                if entry.access_count >= 3 {  // 至少访问 3 次
                    important.push(entry);
                }
            }
        }

        Ok(important)
    }
}

/// 归档报告
#[derive(Debug, Clone)]
pub struct ArchiveReport {
    pub week_id: String,
    pub archive_path: String,
    pub entries_archived: usize,
    pub index_cleaned: usize,
    pub space_saved: u64,  // bytes
}

/// 提炼报告
#[derive(Debug, Clone)]
pub struct CompactReport {
    pub year: u32,
    pub archives_processed: usize,
    pub important_extracted: usize,
    pub duplicates_removed: usize,
    pub final_index_entries: usize,
    pub space_saved: u64,
}
```

**归档策略**：
- **周归档** - 每周日自动归档
- **年提炼** - 每年 1 月 15 日执行提炼
- **热数据** - 保留最近 4 周在主库
- **冷数据** - 旧年数据压缩存储
- **保留策略** - 保留 5 年归档，超期删除

---

## 混合架构 API

### 写入记忆

```rust
/// 写入记忆（智能索引）
pub async fn set_memory(
    log_memory: &LogMemory,
    vector_index: &VectorIndex,
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
        week_id: current_week(),
        year: current_year(),
        indexed: false,  // 尚未索引
        access_count: 0,
        last_accessed_at: None,
    };

    log_memory.append(log_entry).await?;

    // 2. 尝试精准索引（异步，不阻塞写入）
    tokio::spawn(async move {
        if let Err(e) = vector_index.index_entry(&log_entry).await {
            tracing::warn!("Failed to index entry {}: {}", key, e);
        }
    });

    Ok(())
}
```

### 搜索记忆

```rust
/// 搜索记忆（混合策略）
pub async fn search_memory(
    log_memory: &LogMemory,
    vector_index: &VectorIndex,
    query: &str,
    limit: usize,
) -> Result<Vec<MemoryItem>> {
    // 1. 精准索引搜索（快速）
    let indexed_ids = vector_index.search(query, limit).await?;

    // 2. 日志精确匹配（补充）
    let exact_matches = log_memory.search_exact(query).await?;

    // 3. 合并去重
    let mut results = Vec::new();
    let mut seen_ids = HashSet::new();

    // 优先返回索引结果（可能有语义相关）
    for id in indexed_ids {
        if let Some(entry) = log_memory.get(&id).await? {
            if !seen_ids.contains(&entry.id) {
                results.push(entry.clone().into());
                seen_ids.insert(entry.id);
            }
        }
    }

    // 补充精确匹配结果
    for entry in exact_matches {
        if !seen_ids.contains(&entry.id) {
            results.push(entry.into());
            seen_ids.insert(entry.id);
        }
    }

    // 4. 更新访问统计（异步）
    for id in &seen_ids {
        tokio::spawn(async move {
            let _ = log_memory.increment_access(id).await;
        });
    }

    Ok(results)
}
```

---

## 存储布局

### 文件系统结构

```
~/.cis/
├── data/
│   ├── memory/
│   │   ├── memory.db              # 主日志数据库（最近 4 周）
│   │   │   ├── log_entries       # 日志条目表
│   │   │   ├── access_stats      # 访问统计表
│   │   │   └── index_meta       # 索引元数据
│   │   │
│   │   ├── index/
│   │   │   ├── hnsw_index.db   # 精准向量索引
│   │   │   └── index_entries   # 索引条目表
│   │   │
│   │   └── archives/              # 归档目录
│   │       ├── 2026-W06.db.gz  # 周归档（压缩）
│   │       ├── 2026-W05.db.gz
│   │       ├── ...
│   │       │
│   │       └── yearly/           # 年度提炼归档
│   │           ├── 2025_compacted.db.gz
│   │           ├── 2024_compacted.db.gz
│   │           └── ...
│   │
│   └── vector/                    # 向量数据
│       ├── embeddings/              # 嵌入向量缓存
│       └── models/                 # 模型文件
│           └── all-MiniLM-L7-v2.npy
│
└── config/
    ├── memory.toml                # 记忆配置
    │   [index]
    │   max_entries = 10000
    │   allowed_types = ["UserPreference", "ProjectConfig", "ImportantDecision"]
    │   min_access_count = 3
    │
    │   [archive]
    │   hot_weeks = 4
    │   compress = true
    │   retention_years = 5
    │
    └── telemetry.toml           # 遥测配置
        [memory]
        log_retention_days = 30
        index_size_limit = 10000
```

---

## 性能对比

### 当前架构 vs 优化架构

| 指标 | 当前架构 | 优化架构 | 改进 |
|------|---------|---------|------|
| **向量索引大小** | 12MB (4000 条) | 1.2MB (400 条) | -90% |
| **搜索延迟** | ~200ms | ~50ms (精准索引) | -75% |
| **搜索准确度** | 模糊匹配 | 精确匹配 | +100% |
| **热数据加载** | 全量加载 | 只加载 4 周 | -90% |
| **归档压力** | 无自动归档 | 自动周归档 | ∞ |
| **内存占用** | ~200MB | ~50MB | -75% |
| **冷数据访问** | 从主库查询 | 从归档按需加载 | 按需 |

### 预期收益

**1. 性能提升**：
- 向量搜索快 4 倍（精准索引 vs 全量索引）
- 热数据加载快 10 倍（只加载 4 周）
- 内存占用减少 75%

**2. 准确度提升**：
- 不再返回模糊匹配结果
- 精确查询返回精确内容
- 索引引用保证数据一致性

**3. 存储优化**：
- 自动周归档（无需手动清理）
- 年度提炼（去重、压缩）
- 旧数据自动删除（5 年保留期）

**4. 系统压力降低**：
- 向量索引规模可控（10,000 条上限）
- 冷数据不占用主库空间
- 归档文件压缩节省 70% 空间

---

## 实施计划

### 阶段 1: 基础重构 (Week 1-2)

**负责团队**: Team V (2-3 人)

**任务**:
- [ ] 实现 `LogMemory` 模块
  - [ ] 日志条目存储（SQLite）
  - [ ] 周归档逻辑
  - [ ] 访问统计更新
  - [ ] 批量写入优化
- [ ] 实现 `VectorIndex` 模块
  - [ ] 精准索引策略
  - [ ] 索引类型分类
  - [ ] 权重计算
  - [ ] HNSW 集成
- [ ] 编写单元测试
- [ ] 性能基准测试

**工作量**: 8-10 人日

---

### 阶段 2: 归档系统 (Week 3)

**负责团队**: Team V

**任务**:
- [ ] 实现 `ArchiveCompactor`
  - [ ] 周归档任务
  - [ ] 年提炼任务
  - [ ] 归档压缩
  - [ ] 旧数据删除
- [ ] 实现定时任务调度
  - [ ] 每周日 23:59 触发周归档
  - [ ] 每年 1 月 15 日触发年提炼
- [ ] 编写集成测试
- [ ] 归档恢复测试

**工作量**: 3-5 人日

---

### 阶段 3: 集成和迁移 (Week 4)

**负责团队**: Team V + QA

**任务**:
- [ ] 更新 `MemoryService` 接口
  - [ ] 保持 API 兼容性
  - [ ] 添加 `set_memory()` 新方法
  - [ ] 添加 `search_memory()` 新方法
- [ ] 数据迁移脚本
  - [ ] 从旧 VectorStorage 迁移日志数据
  - [ ] 从旧 MemoryService 迁移到新架构
  - [ ] 重建精准索引
- [ ] 完整回归测试
- [ ] 性能对比测试
- [ ] 更新文档

**工作量**: 5-7 人日

---

## 风险和缓解

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| 数据迁移失败 | 🟡 中 | 🔴 高 | 完整备份 + 迁移验证 |
| 索引策略不准 | 🟡 中 | 🟠 中 | 可配置策略 + A/B 测试 |
| 归档文件损坏 | 🟢 低 | 🟠 中 | 校验和 + 多副本 |
| 性能不如预期 | 🟡 中 | 🟡 中 | 性能基准对比 + 回滚方案 |
| 用户适应困难 | 🟡 中 | 🟡 中 | 逐步迁移 + 旧 API 兼容 |

---

## 成功指标

| 指标 | 当前 | 目标 | 测量方式 |
|------|------|------|----------|
| 平均搜索延迟 | ~200ms | <80ms | 基准测试 |
| 向量索引大小 | ~12MB | <5MB | 文件大小 |
| 热数据加载时间 | ~2s | <200ms | 加载计时 |
| 内存占用 | ~200MB | <80MB | heaptrack |
| 搜索准确度 | ~60% | >95% | 人工评估 |
| 自动归档 | 无 | 100% | 归档任务日志 |

---

## 下一步行动

### 立即执行

1. **创建记忆优化任务 Team**
   - Team V: 记忆架构重构（2-3 人，12-15 人日）

2. **准备开发环境**
   - 创建 feature branch: `feature/memory-architecture-v2`
   - 设置性能基准测试环境
   - 准备测试数据集

3. **开始实施**
   - Week 1-2: 基础重构（LogMemory + VectorIndex）
   - Week 3: 归档系统（ArchiveCompactor）
   - Week 4: 集成迁移

---

**文档版本**: 1.0
**设计完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**审核状态**: 待审核
