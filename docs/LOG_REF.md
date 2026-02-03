
**CIS-Memory 自代谢记忆系统 v2.0**
*全量技术方案（Agent 任务拆分版）*

---

## 1. 架构总览

### 1.1 核心哲学
- **双轨制**：vec 模糊定位（语义），日志精准搜索（时序）
- **54 周上限**：物理存储硬边界，超期即焚
- **热度传导**：记忆权重决定日志生命周期
- **邦联自治**：节点本地决策，跨节点按需拉取

### 1.2 数据流图
```text
┌─────────────────────────────────────────────────────────────────┐
│                        CIS Node                                  │
│  ┌──────────────┐         ┌──────────────┐                     │
│  │   Skill      │────────▶│  Memory      │                     │
│  │   (Agent)    │ 写入    │  Entropy     │                     │
│  └──────────────┘         │  Reducer     │                     │
│          │                └──────┬───────┘                     │
│          │                       │                              │
│          ▼                       ▼                              │
│  ┌──────────────────────────────────────┐                      │
│  │        memory_vec.db (热库)           │                      │
│  │  ┌──────────┬──────────┬──────────┐  │                      │
│  │  │ hot_mem  │log_heat  │persona   │  │                      │
│  │  │(语义核)  │(热度映射)│(人格固化) │  │                      │
│  │  └──────────┴──────────┴──────────┘  │                      │
│  └──────────────┬───────────────────────┘                      │
│                 │ log_ref (周标识|rowid)                       │
│                 ▼                                               │
│  ┌──────────────────────────────────────┐                      │
│  │   logs/ (周库目录，54文件滚动)         │                      │
│  │  logs_2026_W05.db  (当前写入)         │                      │
│  │  logs_2026_W04.db  (上周，可能引用)    │                      │
│  │  ...                                 │                      │
│  │  [2025_W50..W52]  (待删除)            │                      │
│  └──────────────┬───────────────────────┘                      │
│                 │ 超54周/无引用                                  │
│                 ▼                                               │
│  ┌──────────────────────────────────────┐                      │
│  │   Munin Node (冷归档，可选)            │                      │
│  │   (邦联其他节点/低功耗设备)             │                      │
│  └──────────────────────────────────────┘                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. 模块划分（Agent 可拆分）

### 模块 A：周级日志管理器（`log_manager`）
**职责**：日志追加写入、周文件生命周期管理

**核心功能**：
- 按 ISO 周创建/切换日志库文件
- 高并发写入优化（WAL 模式调优）
- 54 周滚动删除（物理文件删除）

**接口定义**：
```rust
pub trait LogManager {
    /// 写入日志，自动路由到当前周文件
    fn append(&self, level: LogLevel, skill_id: &str, content: &[u8]) -> Result<LogRef>;
    
    /// 根据 LogRef 精准查询单条
    fn get(&self, ref: &LogRef) -> Result<LogEntry>;
    
    /// 时间范围查询（跨周聚合）
    fn query_range(&self, start: u64, end: u64) -> Result<Vec<LogEntry>>;
    
    /// 维护：删除超期周文件
    fn purge_older_than(&self, weeks: u8) -> Result<Vec<String>>; // 返回已删除文件列表
}

/// 格式：2026-W05|8848
pub struct LogRef {
    pub week: String,      // 2026-W05
    pub rowid: i64,
}
```

**数据表（logs_YYYY_WWW.db）**：
```sql
CREATE TABLE log_entries (
    rowid INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL,
    level INTEGER NOT NULL,     -- 1:DEBUG 2:INFO 3:WARN 4:ERROR
    skill_id TEXT NOT NULL,
    content BLOB NOT NULL,      -- zstd 压缩
    tag TEXT                    -- JSON 标签，快速过滤
);
CREATE INDEX idx_ts ON log_entries(ts);
CREATE INDEX idx_skill ON log_entries(skill_id);
```

---

### 模块 B：记忆核引擎（`memory_kernel`）
**职责**：向量存储、语义检索、热度管理

**核心功能**：
- HNSW 向量索引（sqlite-vec）
- 热度衰减计算（指数衰减模型）
- 记忆分级（热/温/冷）判定

**接口定义**：
```rust
pub trait MemoryKernel {
    /// 插入记忆（ Skill 调用）
    fn memorize(&self, vec: &[f32], content: &str, log_ref: LogRef) -> Result<MemoryId>;
    
    /// 语义搜索（模糊定位）
    fn search(&self, query_vec: &[f32], top_k: usize) -> Result<Vec<Memory>>;
    
    /// 访问记忆（触发热度更新）
    fn touch(&self, id: MemoryId) -> Result<()>;
    
    /// 获取待清理的冷却记忆（供 Entropy Reducer 调用）
    fn get_cooled_memories(&self, threshold: f32) -> Result<Vec<Memory>>;
    
    /// 物理删除记忆（级联影响日志）
    fn delete(&self, id: MemoryId) -> Result<()>;
}

pub struct Memory {
    pub id: i64,
    pub vec: Vec<f32>,
    pub content: String,        // 压缩后的记忆核
    pub weight: f32,            // 0.0-1.0
    pub heat_level: u8,         // 0:冷 1:温 2:热
    pub log_ref: LogRef,
    pub last_access: u64,
}
```

**数据表（memory_vec.db）**：
```sql
-- 主记忆表
CREATE TABLE hot_mem (
    id INTEGER PRIMARY KEY,
    vec REAL(768) NOT NULL,
    content TEXT NOT NULL,
    weight REAL NOT NULL DEFAULT 1.0,
    heat_level INTEGER CHECK(heat_level IN (0,1,2)),
    log_week TEXT NOT NULL,     -- 冗余，便于 JOIN
    log_rowid INTEGER NOT NULL,
    last_access INTEGER NOT NULL,
    
    -- 外键约束指向日志（逻辑引用）
    UNIQUE(log_week, log_rowid)
);

-- 虚拟向量表（sqlite-vec）
CREATE VIRTUAL TABLE vec_mem USING vec0(
    embedding float[768]
);

-- 热度映射（加速清理决策）
CREATE TABLE log_heat_map (
    week TEXT NOT NULL,
    rowid INTEGER NOT NULL,
    weight REAL NOT NULL,
    access_count INTEGER DEFAULT 0,
    PRIMARY KEY (week, rowid)
) WITHOUT ROWID;
```

---

### 模块 C：熵减执行器（`entropy_reducer`）
**职责**：触发压缩、物理清理、邦联同步

**触发条件**：
- 容量触发：`memory_vec.db > 500MB`
- 时间触发：每日 UTC 00:00
- 手动触发：用户/Agent 显式调用

**决策矩阵**（单次扫描执行）：
```rust
enum Decision {
    Keep,       // 保留在 hot_mem（热）
    Compact,    // 压缩内容（温）
    Archive,    // 迁移到 Munin（冷）
    Purge,      // 物理删除（垃圾）
}

fn evaluate(weight: f32, age_days: u32, access_freq: f32) -> Decision {
    match (weight, age_days) {
        (w, _) if w > 0.8 => Decision::Keep,
        (w, d) if w > 0.3 && d < 30 => Decision::Keep,
        (w, d) if w > 0.2 && d < 365 => Decision::Compact,
        (_, d) if d < 365 => Decision::Archive,
        _ => Decision::Purge,
    }
}
```

**接口定义**：
```rust
pub trait EntropyReducer {
    /// 执行全量熵减（单次事务）
    fn reduce(&self, trigger: TriggerEvent) -> Result<ReductionReport>;
    
    /// 级联检查：某周日志是否可被删除
    fn check_week_removable(&self, week: &str) -> Result<bool>;
    
    /// 生成邦联同步报告
    fn generate_sync_report(&self) -> Result<EntropyReport>;
}

pub struct ReductionReport {
    pub scanned: usize,
    pub kept: usize,
    pub compacted: usize,
    pub archived: usize,
    pub purged: usize,
    pub freed_bytes: u64,
}
```

---

### 模块 D：邦联同步器（`federation_sync`）
**职责**：跨节点记忆同步、冷日志按需拉取

**同步策略**：
- **热记忆**：实时广播（weight > 0.7）
- **温记忆**：批量同步（每小时）
- **冷日志**：永不主动同步，按需通过 DID 拉取

**接口定义**：
```rust
pub trait FederationSync {
    /// 广播记忆更新（轻量：只传向量摘要，不传日志）
    fn broadcast_kernel(&self, kernel: &MemoryKernel) -> Result<()>;
    
    /// 从其他节点拉取记忆（通过 vec 相似度预匹配）
    fn fetch_remote_memories(&self, query_vec: &[f32], nodes: &[DID]) -> Result<Vec<Memory>>;
    
    /// 按需拉取冷日志（当本地无日志，但记忆引用存在时）
    fn fetch_cold_logs(&self, week: &str, from_node: DID) -> Result<LogArchive>;
    
    /// 接收他节点的熵减报告（用于更新本地路由表）
    fn receive_report(&self, report: &EntropyReport) -> Result<()>;
}

pub struct EntropyReport {
    pub node_did: DID,
    pub week_archived: Vec<String>,      // 已归档到 Munin 的周
    pub kernel_hashes: Vec<String>,      // 新生成的记忆核哈希
    pub purged_weeks: Vec<String>,       // 已删除的周（他节点可停止引用）
}
```

---

## 3. 数据流时序图

### 场景 A：Skill 写入记忆
```text
Skill ──▶ MemoryKernel.memorize(vec, content)
            │
            ├──▶ sqlite-vec INSERT (语义向量)
            │
            ├──▶ hot_mem INSERT (元数据)
            │
            └──▶ LogManager.append(skill_id, raw_content)
                    │
                    └──▶ logs_2026_W05.db INSERT (原始日志)
                            │
                            └──▶ 返回 LogRef(2026-W05|rowid)
```

### 场景 B：用户查询（双轨）
```text
User Query ──▶ MemoryKernel.search(query_vec)
                 │
                 ├──▶ HNSW 相似度检索 (Top 5)
                 │
                 └──▶ 返回 Vec<Memory> (含 log_ref)
                     │
                     ├──▶ LogManager.get(log_ref) ──▶ 精准拉取原始日志
                     │
                     └──▶ 融合：Memory.content + LogEntry.content
                                 ↓
                            返回给 LLM (上下文)
```

### 场景 C：熵减触发
```text
Trigger ──▶ EntropyReducer.reduce()
              │
              ├──▶ 扫描 hot_mem (单次查表)
              │
              ├──▶ 决策：Keep/Compact/Archive/Purge
              │
              ├──▶ 物理删除 Purge 项
              │
              ├──▶ 检查被删记忆的 log_week
              │       │
              │       └──▶ LogManager.check_removable(week)
              │               │
              │               └──▶ 查询 log_heat_map
              │                       │
              │                       └──▶ 若引用数=0 ──▶ rm logs_2026_WXX.db
              │
              └──▶ FederationSync.generate_report()
                      │
                      └──▶ 广播到邦联 Matrix Room
```

---

## 4. 关键配置参数

```toml
[memory]
vec_dim = 768
max_hot_size_mb = 500
hot_max_items = 50000

[logs]
week_format = "YYYY-WWW"          # ISO 8601
retention_weeks = 54              # 硬边界
compress_algorithm = "zstd"
compress_level = 3

[entropy]
trigger_capacity_percent = 85
trigger_critical_percent = 95
decay_lambda = 0.1                # weight *= exp(-λ * days)

[federation]
broadcast_heat_threshold = 0.7    # 热记忆实时广播
sync_interval_sec = 3600          # 温记忆批量同步
munin_node_did = "did:cis:munin:01"
```

---

## 5. Agent 任务拆分清单

### Task 1：基础存储层
- [ ] 实现 `LogManager`：周文件创建、追加写入、WAL 调优
- [ ] 实现 `MemoryKernel`：sqlite-vec 封装、基础 CRUD
- [ ] 定义 `LogRef` 编解码（2026-W05|8848）

### Task 2：热度与衰减系统
- [ ] 实现权重衰减算法（指数模型）
- [ ] 实现 `touch()` 方法（访问时更新热度）
- [ ] 设计 `log_heat_map` 更新逻辑

### Task 3：熵减执行引擎
- [ ] 实现三级决策逻辑（Keep/Compact/Archive/Purge）
- [ ] 实现级联删除（记忆删 → 检查周文件 → 物理删除）
- [ ] 实现 `VACUUM` 和文件系统清理

### Task 4：邦联同步协议
- [ ] 实现 `EntropyReport` 序列化/反序列化
- [ ] 集成 Matrix Room 广播（或 CIS 邦联通道）
- [ ] 实现按需拉取冷日志（HTTP/IPFS 协议）

### Task 5：查询融合层
- [ ] 实现双轨查询（vec 模糊 + 日志精准）
- [ ] 上下文融合策略（摘要 + 原始日志拼接）
- [ ] 缓存优化（热记忆常驻内存）

### Task 6：监控与 CLI
- [ ] 存储占用监控（周文件大小、vec.db 大小）
- [ ] 手动触发熵减命令
- [ ] 邦联节点健康检查

---

## 6. 风险与缓解

| 风险               | 影响                               | 缓解方案                                                                    |
| ------------------ | ---------------------------------- | --------------------------------------------------------------------------- |
| **周文件句柄泄漏** | 打开过多周文件（54个）导致 FD 耗尽 | 实现 LRU 缓存，只保持最近 4 周文件打开，其余按需打开                        |
| **HNSW 索引膨胀**  | sqlite-vec 索引文件超过内存        | 限制 `max_hot_items=50000`，超限时强制触发 L3 人格固化                      |
| **邦联网络分区**   | 无法拉取冷日志，记忆引用失效       | 本地保留 `tombstone_log`，记录缺失的引用，网络恢复后异步回补                |
| **并发写入锁**     | 高频 Skill 写入阻塞向量查询        | 日志库独立文件（无锁），记忆库使用 `WAL` 模式，`PRAGMA busy_timeout = 5000` |

---

**文档冻结。** 按此拆分，6 个 Task 可并行开发，通过 `LogRef` 和 `MemoryId` 解耦依赖。