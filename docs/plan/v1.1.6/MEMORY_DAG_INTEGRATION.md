# 记忆系统改版 DAG 集成方案

> **日期**: 2026-02-12
> **目的**: 将54周按周分db和精准索引纳入v1.1.6 DAG任务编排

---

## 1. 记忆系统改版任务定义

### M-1: 记忆周归档实现（P0 - 关键）

**任务 ID**: M-1
**名称**: 实现54周按周分db归档机制
**类型**: `module_refactoring`
**优先级**: P0
**预估工作量**: 10天
**依赖**: V-1 (CLI架构修复)

**描述**:
实现记忆系统的周级别归档，每周一个独立的数据库文件，支持自动归档和清理。

**Prompt 模板**:
```
你的任务是实现 CIS 记忆系统的54周按周分db归档机制。

## 背景
- 当前所有记忆存储在单一数据库，检索失真
- 需要按周归档，减少单数据库压力
- 参考: docs/plan/v1.1.6/MEMORY_ARCHITECTURE_DESIGN.md

## 实现要求

### 1. 周数据库管理
- 每周一个独立数据库文件: `week-{YEAR}-W{WEEK}.db`
- 例如: `week-2026-W07.db` (2026年第7周)
- 自动创建当前周数据库
- 最多保留54周数据库（约1年+边界）

### 2. 自动归档
- 检测周切换，自动创建新周数据库
- 旧周数据库自动归档为 `.archive` 文件
- 压缩归档文件（gzip）
- 删除超过54周的数据库

### 3. 连接池管理
- 支持同时打开多个周数据库
- LRU 缓存最近使用的周连接
- 自动关闭不活跃的周连接

### 4. Schema 设计
```sql
-- 日志记忆表（全量存储）
CREATE TABLE log_memory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key TEXT NOT NULL UNIQUE,
    value BLOB NOT NULL,
    domain TEXT NOT NULL,  -- 'private' | 'public'
    category TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    indexed BOOLEAN NOT NULL DEFAULT 0,  -- 是否已索引
    created_at INTEGER NOT NULL
);

-- 精准向量索引表（只有10%数据）
CREATE TABLE precision_index (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    log_id INTEGER NOT NULL UNIQUE,
    key TEXT NOT NULL,
    embedding BLOB NOT NULL,  -- 384维向量
    indexed_at INTEGER NOT NULL,
    FOREIGN KEY (log_id) REFERENCES log_memory(id) ON DELETE CASCADE
);
```

### 5. 测试
- 单元测试: 周ID计算、归档触发、连接池
- 集成测试: 跨周查询、归档流程
- 性能测试: 54周数据库的查询性能

## 验收标准
- [ ] 实现 WeeklyArchivedMemory 结构
- [ ] 支持 CRUD 操作（跨周自动路由）
- [ ] 自动归档超过54周的数据库
- [ ] 连接池缓存工作正常
- [ ] 测试覆盖率 >80%

## 输出
- 代码文件: `cis-core/src/memory/weekly_archived.rs`
- 单元测试: `cis-core/src/memory/weekly_archived.rs` (tests module)
- 集成测试: 端到端测试
```

**上下文变量**:
```json
{
  "implementation_file": "cis-core/src/memory/weekly_archived.rs",
  "design_doc": "docs/plan/v1.1.6/MEMORY_ARCHITECTURE_DESIGN.md",
  "max_weeks": 54,
  "base_dir": "~/.cis/data/memory/weeks"
}
```

---

### M-2: 精准向量索引实现（P1 - 高优先级）

**任务 ID**: M-2
**名称**: 实现精准向量索引策略（只有10%数据）
**类型**: `module_refactoring`
**优先级**: P1
**预估工作量**: 8天
**依赖**: M-1 (记忆周归档)

**描述**:
实现精准索引策略，只对重要记忆（约10%）创建向量索引，避免全量索引导致检索失真。

**Prompt 模板**:
```
你的任务是实现 CIS 记忆系统的精准向量索引策略。

## 背景
- 当前所有记忆都向量索引，导致检索失真
- 需要"索引 → 日志"的两级架构
- 索引是指针，指向日志记忆的具体条目
- 参考: docs/plan/v1.1.6/MEMORY_INDEX_PRECISION_DESIGN.md

## 精准索引策略

### 1. 索引策略（只索引10%）

**应该索引的记忆**:
- 架构决策: `architecture/*`
- API 契约: `api_contract/*`
- 编码约定: `convention/*`
- 用户偏好: `user/preference/*`
- 项目配置: `project/{id}/config`
- 上下文记忆: `context/*`

**不应该索引的记忆**:
- 临时数据: `temp/*`, `cache/*`, `debug/*`
- 日志文件: `logs/*`
- 会话数据: `session/*`
- 结果缓存: `result/cache/*`

### 2. 索引创建流程

```rust
// 存储记忆时判断
if should_index_memory(key) {
    // 生成向量嵌入
    let embedding = generate_embedding(key, value).await?;

    // 存储到 precision_index 表
    INSERT INTO precision_index (log_id, key, embedding, indexed_at)
    VALUES (?, ?, ?, ?)

    // 标记为已索引
    UPDATE log_memory SET indexed = 1 WHERE id = ?
}
```

### 3. 检索流程（两级）

**Level 1: 向量搜索（精准索引）**
```sql
SELECT l.*, p.embedding
FROM log_memory l
INNER JOIN precision_index p ON l.id = p.log_id
WHERE l.indexed = 1
AND vector_distance(p.embedding, ?) < 0.7
LIMIT 10
```

**Level 2: 文本回溯（日志记忆）**
```sql
SELECT * FROM log_memory
WHERE key LIKE ? || '%'
OR value LIKE ? || '%'
AND indexed = 0
LIMIT 20
```

### 4. 向量嵌入生成

- 使用现有的嵌入服务（fastembed）
- 384维向量（与现有一致）
- 批量生成优化（一次生成多个）
- 缓存常用 key 的嵌入

### 5. 统计和监控

- 索引率目标: 10%
- 定期统计: `SELECT COUNT(*) FROM log_memory WHERE indexed = 1`
- 检索性能: 向量搜索 vs 文本搜索

## 验收标准
- [ ] 实现 `should_index_memory()` 判断逻辑
- [ ] 向量索引只包含10%的记忆
- [ ] 两级检索流程工作正常
- [ ] 检索准确率提升 >30%
- [ ] 索引大小减少 >90%

## 输出
- 代码: `cis-core/src/memory/precision_index.rs`
- 嵌入集成: 调用现有 `fastembed` 服务
- 性能测试: 对比全量索引 vs 精准索引
```

**上下文变量**:
```json
{
  "target_index_ratio": 0.1,
  "design_doc": "docs/plan/v1.1.6/MEMORY_INDEX_PRECISION_DESIGN.md",
  "embedding_service": "cis-core/src/vector/embedding.rs",
  "vector_dimension": 384
}
```

---

### M-3: 记忆服务集成（P1 - 高优先级）

**任务 ID**: M-3
**名称**: 集成周归档和精准索引到现有记忆服务
**类型**: `module_refactoring`
**优先级**: P1
**预估工作量**: 5天
**依赖**: M-1, M-2

**描述**:
将新的周归档和精准索引集成到现有的 `MemoryService`，保持向后兼容。

**Prompt 模板**:
```
你的任务是将新的周归档记忆系统集成到现有 CIS 记忆服务。

## 背景
- 现有记忆服务: `cis-core/src/memory/service.rs`
- 新的周归档: `cis-core/src/memory/weekly_archived.rs`
- 新的精准索引: `cis-core/src/memory/precision_index.rs`
- 需要保持向后兼容，不破坏现有 API

## 集成方案

### 1. 适配现有接口

```rust
// 现有接口（保持不变）
impl MemoryService {
    pub async fn set(&self, key: &str, value: &[u8], ...)
        -> Result<()> {
        // 路由到周归档存储
        self.weekly_memory.set(key, value, ...).await?;
        Ok(())
    }

    pub async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        // 从周归档存储获取
        self.weekly_memory.get(key).await
            .map_err(|e| Error::Memory(e.into()))
    }

    pub async fn search(&self, query: &str, limit: usize)
        -> Result<Vec<MemorySearchItem>> {
        // 使用精准索引搜索
        let results = self.weekly_memory.search(query, None, limit).await?;

        // 转换为现有格式
        Ok(results.into_iter().map(|r| MemorySearchItem {
            key: r.key,
            value: r.value,
            domain: match r.domain.as_str() {
                "private" => MemoryDomain::Private,
                "public" => MemoryDomain::Public,
                _ => return Err(Error::InvalidDomain),
            },
            category: match r.category.as_str() {
                "context" => MemoryCategory::Context,
                "result" => MemoryCategory::Result,
                // ...
            },
        }).collect())
    }
}
```

### 2. 数据迁移

- 从现有 `memory.db` 迁移到当前周数据库
- 迁移脚本: `scripts/migrate-memory-to-weekly.sh`
- 保留原数据库备份（`.backup` 后缀）

### 3. 配置兼容

```toml
# ~/.cis/config.toml
[memory]
# 新增: 启用周归档
enable_weekly_archive = true
max_weeks = 54

# 新增: 精准索引
enable_precision_index = true
target_index_ratio = 0.1

# 现有: 保持兼容
default_domain = "public"
encryption_enabled = true
```

### 4. CLI 命令兼容

```bash
# 现有命令保持不变
cis memory set "key" "value"
cis memory get "key"
cis memory search "query"

# 新增命令
cis memory weekly stats      # 统计周数据库
cis memory weekly cleanup    # 清理过期周
cis memory weekly export      # 导出当前周
```

### 5. 测试

- 单元测试: API 兼容性
- 集成测试: 迁移脚本
- 回归测试: 现有功能不破坏

## 验收标准
- [ ] 现有 `MemoryService` API 不变
- [ ] 数据迁移脚本可用
- [ ] CLI 命令向后兼容
- [ ] 现有测试通过
- [ ] 性能不降低

## 输出
- 修改: `cis-core/src/memory/service.rs`
- 新增: `scripts/migrate-memory-to-weekly.sh`
- 文档: `docs/migration/weekly-memory-migration.md`
```

**上下文变量**:
```json
{
  "existing_service": "cis-core/src/memory/service.rs",
  "weekly_memory": "cis-core/src/memory/weekly_archived.rs",
  "migration_script": "scripts/migrate-memory-to-weekly.sh",
  "compatibility_mode": "backward"
}
```

---

## 2. DAG 任务编排

### 任务依赖关系

```
V-1 (CLI架构修复)
  ↓
M-1 (周归档实现)
  ↓
M-2 (精准索引实现)
  ↓
M-3 (记忆服务集成)
  ↓
V-4 (memory精准索引) ← 与M-2并行
```

### 并行执行策略

**Phase 1: 基础架构**
- Week 1-2: V-1 (CLI架构修复)
- Week 3: M-1 (周归档)

**Phase 2: 索引优化**
- Week 4-5: M-2 (精准索引)
- Week 4-5: V-4 (可并行，但需协调)

**Phase 3: 集成和测试**
- Week 6: M-3 (记忆服务集成)
- Week 7-8: 集成测试和性能优化

### 执行层级（可并行）

**Level 1**（可立即开始）:
- V-1: CLI架构修复

**Level 2**（等待V-1）:
- M-1: 周归档实现
- V-2: scheduler拆分（可并行）
- V-3: config拆分（可并行）

**Level 3**（等待M-1）:
- M-2: 精准索引实现
- V-4: memory精准索引（与M-2协调，避免冲突）
- V-5: skill拆分（可并行）

**Level 4**（等待M-2, M-3）:
- M-3: 记忆服务集成
- V-8: 全面测试
- V-9: 性能优化

---

## 3. 测试计划

### 单元测试

**周归档**:
- [ ] 周ID计算正确性
- [ ] 数据库自动创建
- [ ] 连接池缓存
- [ ] 自动归档触发
- [ ] 过期周清理

**精准索引**:
- [ ] 索引策略判断
- [ ] 向量嵌入生成
- [ ] 两级检索流程
- [ ] 索引率统计

### 集成测试

**数据迁移**:
- [ ] 现有数据完整迁移
- [ ] 数据一致性验证
- [ ] 性能对比

**API 兼容性**:
- [ ] 现有 CLI 命令工作
- [ ] Agent 记忆调用正常
- [ ] P2P 同步兼容

### 性能测试

**查询性能**:
- 基准: 现有单一数据库
- 目标: 周归档查询 < 20ms
- 对比: 全量索引 vs 精准索引

**存储性能**:
- 基准: 单数据库写入
- 目标: 周数据库写入 < 50ms
- 对比: 索引大小（-90%预期）

---

## 4. 成功指标

### 存储优化

| 指标 | 当前 | 目标 | 测量方法 |
|--------|--------|--------|----------|
| 向量索引大小 | ~12MB | ~1.2MB | `SELECT COUNT(*) FROM precision_index` |
| 索引率 | 100% | ~10% | `COUNT(indexed)/COUNT(*)` |
| 数据库文件数 | 1 | 54 | `ls ~/.cis/data/memory/weeks/*.db \| wc -l` |

### 检索质量

| 指标 | 当前 | 目标 | 测量方法 |
|--------|--------|--------|----------|
| 检索准确率 | 基线 | +30% | 用户反馈 A/B 测试 |
| 检索延迟 | 基线 | -50% | 查询时间统计 |
| 相关结果比例 | 基线 | +40% | Top-10 结果相关性评分 |

### 系统性能

| 指标 | 当前 | 目标 | 测量方法 |
|--------|--------|--------|----------|
| 查询 QPS | 基线 | >1000 | 压力测试 |
| 写入 QPS | 基线 | >500 | 压力测试 |
| 存储空间 | 基线 | -20% | 磁盘使用统计 |

---

## 5. 回滚计划

### 如果记忆系统改版失败

**触发条件**:
- 数据迁移失败率 > 5%
- 性能下降 > 50%
- 现有功能严重破坏

**回滚步骤**:
1. 停止新记忆服务
2. 恢复原数据库备份 (`memory.db.backup`)
3. 回滚代码到改版前 commit
4. 重启 CIS 服务
5. 验证现有功能正常

**回滚时间**: < 30分钟

---

## 6. 相关文档

- [MEMORY_ARCHITECTURE_DESIGN.md](./MEMORY_ARCHITECTURE_DESIGN.md) - 记忆架构设计
- [MEMORY_INDEX_PRECISION_DESIGN.md](./MEMORY_INDEX_PRECISION_DESIGN.md) - 精准索引设计
- [TASK_STORAGE_SQLITE_DESIGN.md](./TASK_STORAGE_SQLITE_DESIGN.md) - 任务存储设计
- [NEXT_STEPS.md](./NEXT_STEPS.md) - 执行指南

---

**文档版本**: 1.0
**创建日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 设计完成，待执行
