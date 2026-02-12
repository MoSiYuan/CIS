# CIS 记忆服务缓存架构设计

> **版本**: v1.1.6
> **日期**: 2026-02-12
> **作者**: Team J
> **目标**: 实现高性能 LRU 缓存层，提升查询性能 70%+

---

## 概述

### 问题背景

根据代码审阅报告 (`docs/user/code-review-data-layer.md`)，当前记忆服务存在以下性能问题：

1. **缺少查询缓存** - 每次查询都需要访问数据库
2. **多表查询效率低** - 公域/私域分离导致查询需要访问多个表
3. **向量搜索 fallback 性能差** - 缺少缓存导致频繁降级查询

### 设计目标

- ✅ **缓存命中率 > 70%** - 热数据常驻内存
- ✅ **查询延迟 < 1ms** (缓存命中时)
- ✅ **内存占用可控** - LRU 淘汰机制
- ✅ **线程安全** - 支持并发读写
- ✅ **数据一致性** - 与数据库保持同步

---

## 核心架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                     MemoryService                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐      ┌──────────────┐      ┌──────────────┐│
│  │ GetOperations │      │ SetOperations │      │ SearchOps    ││
│  └──────┬───────┘      └──────┬───────┘      └──────┬───────┘│
│         │                     │                     │          │
│         ▼                     ▼                     ▼          │
│  ┌──────────────────────────────────────────────────────────────┐│
│  │                     MemoryCache Layer                      ││
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐  ││
│  │  │   LRU   │  │   TTL   │  │ Metrics │  │  Stats  │  ││
│  │  │  Cache  │  │ Manager │  │ Collector│  │Reporter │  ││
│  │  └────┬────┘  └────┬────┘  └────┬────┘  └────┬────┘  ││
│  └───────┼────────────┼────────────┼────────────┼──────────┘│
│          │            │            │            │            │
└──────────┼────────────┼────────────┼────────────┼────────────┘
           │            │            │            │
           ▼            ▼            ▼            ▼
    ┌──────────────────────────────────────────────────────┐
    │            Arc<RwLock<CacheState>>                   │
    │  ┌────────────────────────────────────────────────┐  │
    │  │ HashMap<Key, CacheEntry>                      │  │
    │  │   - key: String                              │  │
    │  │   - value: Vec<u8>                           │  │
    │  │   - created_at: SystemTime                   │  │
    │  │   - last_accessed: SystemTime                │  │
    │  │   - access_count: u64                        │  │
    │  │   - ttl: Duration                           │  │
    │  └────────────────────────────────────────────────┘  │
    └──────────────────────────────────────────────────────┘
                          │
                          ▼
           ┌──────────────────────────────┐
           │    Storage Layer           │
           │  ┌────────────────────┐   │
           │  │   MemoryDb        │   │
           │  │  - private_table  │   │
           │  │  - public_table   │   │
           │  └────────────────────┘   │
           └────────────────────────────┘
```

### 缓存流程

#### 读流程 (GET)

```
1. GetOperations::get(key)
   │
   ├─▶ 2. 检查缓存是否启用
   │   └─▶ CacheConfig::enabled
   │
   ├─▶ 3. 尝试从缓存读取
   │   └─▶ LruCache::get(key)
   │       │
   │       ├─▶ HIT ─▶ 更新访问时间 ─▶ 返回缓存值 (<<1ms)
   │       │
   │       └─▶ MISS ─▶ 继续下一步
   │
   └─▶ 4. 从数据库读取
       └─▶ MemoryDb::get(key)
           │
           ├─▶ Found ─▶ 更新缓存 ─▶ 返回值
           │
           └─▶ Not Found ─▶ 返回 None
```

#### 写流程 (SET)

```
1. SetOperations::set(key, value, domain, category)
   │
   ├─▶ 2. 写入数据库
   │   └─▶ MemoryDb::set(...)
   │
   ├─▶ 3. 失效缓存 (Cache Invalidation)
   │   └─▶ LruCache::invalidate(key)
   │       └─▶ 从 HashMap 中移除
   │
   └─▶ 4. (可选) 写透缓存
       └─▶ LruCache::put(key, value)
           └─▶ 如果缓存未满，直接添加
               └─▶ 如果缓存已满，LRU 淘汰
```

---

## 核心组件设计

### 1. CacheConfig - 缓存配置

```rust
/// 缓存配置
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// 是否启用缓存
    pub enabled: bool;

    /// 最大缓存条目数
    pub max_entries: usize,

    /// 默认 TTL (Time To Live)
    pub default_ttl: Duration,

    /// 缓存键前缀 (用于多实例隔离)
    pub key_prefix: Option<String>,

    /// 是否启用统计信息收集
    pub enable_metrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_entries: 1000,              // 默认 1000 条
            default_ttl: Duration::from_secs(300),  // 默认 5 分钟
            key_prefix: None,
            enable_metrics: true,
        }
    }
}
```

**配置说明**:
- `enabled`: 全局开关，可快速禁用缓存
- `max_entries`: 限制内存占用，超过时触发 LRU 淘汰
- `default_ttl`: 防止缓存数据过期，支持自定义 TTL
- `key_prefix`: 支持命名空间隔离
- `enable_metrics`: 生产环境建议开启，测试环境可关闭

---

### 2. CacheEntry - 缓存条目

```rust
/// 缓存条目
#[derive(Debug, Clone)]
struct CacheEntry {
    /// 缓存的值
    value: Vec<u8>,

    /// 创建时间
    created_at: SystemTime,

    /// 最后访问时间 (用于 LRU)
    last_accessed: SystemTime,

    /// 访问次数 (用于统计热度)
    access_count: u64,

    /// TTL (Time To Live)
    ttl: Duration,

    /// 是否加密 (与私域记忆对应)
    encrypted: bool,

    /// 域类型 (Private/Public)
    domain: MemoryDomain,

    /// 分类
    category: MemoryCategory,
}

impl CacheEntry {
    /// 创建新缓存条目
    fn new(value: Vec<u8>, ttl: Duration, domain: MemoryDomain, category: MemoryCategory) -> Self {
        let now = SystemTime::now();
        Self {
            value,
            created_at: now,
            last_accessed: now,
            access_count: 0,
            ttl,
            encrypted: matches!(domain, MemoryDomain::Private),
            domain,
            category,
        }
    }

    /// 检查是否过期
    fn is_expired(&self) -> bool {
        let elapsed = self.created_at
            .elapsed()
            .unwrap_or(Duration::from_secs(0));
        elapsed > self.ttl
    }

    /// 记录访问
    fn record_access(&mut self) {
        self.last_accessed = SystemTime::now();
        self.access_count += 1;
    }

    /// 获取年龄 (创建时长)
    fn age(&self) -> Duration {
        self.created_at
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
    }

    /// 获取空闲时间 (未访问时长)
    fn idle_time(&self) -> Duration {
        self.last_accessed
            .elapsed()
            .unwrap_or(Duration::from_secs(0))
    }
}
```

---

### 3. LruCache - LRU 缓存核心

```rust
/// LRU (Least Recently Used) 缓存
pub struct LruCache {
    /// 缓存配置
    config: CacheConfig,

    /// 缓存存储 (HashMap + 双向链表实现 LRU)
    cache: Arc<RwLock<LruCacheState>>,

    /// 缓存统计
    metrics: Arc<CacheMetrics>,
}

/// LRU 缓存内部状态
struct LruCacheState {
    /// 主存储
    entries: HashMap<String, CacheEntry>,

    /// 访问顺序链表 (最久未访问 -> 最近访问)
    access_order: std::collections::VecDeque<String>,

    /// 当前条目数
    size: usize,

    /// 淘汰计数
    evictions: u64,

    /// 过期清理计数
    expirations: u64,
}

impl LruCache {
    /// 创建新的 LRU 缓存
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config: config.clone(),
            cache: Arc::new(RwLock::new(LruCacheState::new())),
            metrics: Arc::new(CacheMetrics::new()),
        }
    }

    /// 获取缓存值
    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let mut cache = self.cache.write().await;
        self.metrics.record_request();

        // 检查过期
        cache.remove_expired_entries();

        match cache.entries.get_mut(key) {
            Some(entry) if !entry.is_expired() => {
                // 缓存命中
                entry.record_access();
                cache.update_access_order(key);
                self.metrics.record_hit();
                Some(entry.value.clone())
            }
            Some(_) => {
                // 过期，视为未命中
                cache.entries.remove(key);
                cache.remove_from_access_order(key);
                self.metrics.record_miss();
                self.metrics.record_expiration();
                None
            }
            None => {
                // 缓存未命中
                self.metrics.record_miss();
                None
            }
        }
    }

    /// 添加或更新缓存
    pub async fn put(&self, key: String, value: Vec<u8>, ttl: Option<Duration>) {
        let mut cache = self.cache.write().await;

        // 检查容量，必要时淘汰
        if cache.entries.len() >= self.config.max_entries {
            if !cache.entries.contains_key(&key) {
                // 新增且缓存已满，淘汰最久未访问的条目
                if let Some(evicted_key) = cache.pop_lru() {
                    tracing::debug!("Cache evicted: {}", evicted_key);
                    self.metrics.record_eviction();
                }
            }
        }

        let ttl = ttl.unwrap_or(self.config.default_ttl);
        let entry = CacheEntry::new(value, ttl, MemoryDomain::Public, MemoryCategory::Context);

        cache.entries.insert(key.clone(), entry);
        cache.update_access_order(&key);
    }

    /// 使缓存失效
    pub async fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().await;
        cache.entries.remove(key);
        cache.remove_from_access_order(key);
        self.metrics.record_invalidation();
    }

    /// 清空缓存
    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.entries.clear();
        cache.access_order.clear();
        cache.size = 0;
        self.metrics.reset();
    }
}
```

---

### 4. CacheMetrics - 缓存统计

```rust
/// 缓存统计指标
#[derive(Debug, Default)]
pub struct CacheMetrics {
    /// 请求总数
    total_requests: AtomicU64,

    /// 命中次数
    hits: AtomicU64,

    /// 未命中次数
    misses: AtomicU64,

    /// 淘汰次数
    evictions: AtomicU64,

    /// 过期清理次数
    expirations: AtomicU64,

    /// 失效次数
    invalidations: AtomicU64,
}

impl CacheMetrics {
    /// 记录请求
    fn record_request(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录命中
    fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录未命中
    fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录淘汰
    fn record_eviction(&self) {
        self.evictions.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录过期
    fn record_expiration(&self) {
        self.expirations.fetch_add(1, Ordering::Relaxed);
    }

    /// 记录失效
    fn record_invalidation(&self) {
        self.invalidations.fetch_add(1, Ordering::Relaxed);
    }

    /// 获取命中率
    pub fn hit_rate(&self) -> f64 {
        let total = self.total_requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let hits = self.hits.load(Ordering::Relaxed);
        (hits as f64) / (total as f64)
    }

    /// 获取统计快照
    pub fn snapshot(&self) -> CacheMetricsSnapshot {
        CacheMetricsSnapshot {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(OrderOrdering::Relaxed),
            expirations: self.expirations.load(Ordering::Relaxed),
            invalidations: self.invalidations.load(Ordering::Relaxed),
            hit_rate: self.hit_rate(),
        }
    }

    /// 重置统计
    fn reset(&self) {
        self.total_requests.store(0, Ordering::Relaxed);
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
        self.expirations.store(0, Ordering::Relaxed);
        self.invalidations.store(0, Ordering::Relaxed);
    }
}

/// 缓存统计快照
#[derive(Debug, Clone, Serialize)]
pub struct CacheMetricsSnapshot {
    pub total_requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expirations: u64,
    pub invalidations: u64,
    pub hit_rate: f64,
}
```

---

## 缓存键设计

### 键命名规范

```
{namespace}/{domain}/{category}/{identifier}

示例：
- memory/public/context/user-preference-theme
- memory/private/context/api-keys
- memory/public/result/last-search-query
```

### 键哈希

为了优化内存使用，对长键进行哈希：

```rust
fn cache_key(key: &str) -> String {
    if key.len() > 64 {
        // 对长键进行哈希
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        key.hash(&mut hasher);
        format!("{}-{:016x}", &key[..32], hasher.finish())
    } else {
        key.to_string()
    }
}
```

---

## 缓存失效策略

### 1. TTL 失效 (Time-based)

- **被动检查**: 每次访问时检查是否过期
- **主动清理**: 定期后台任务清理过期条目

```rust
// 后台定期清理任务
async fn start_expiration_task(cache: Arc<LruCache>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cache.remove_expired_entries().await;
        }
    });
}
```

### 2. 写时失效 (Write-through)

- **SET 操作**: 立即使对应键失效
- **DELETE 操作**: 立即使对应键失效
- **原因**: 保证数据一致性

### 3. 容量淘汰 (Capacity-based)

- **LRU 策略**: 淘汰最久未访问的条目
- **触发条件**: `cache.size >= config.max_entries`
- **淘汰比例**: 一次淘汰 10% 的条目 (可配置)

---

## 性能优化

### 1. 分段锁 (Sharding)

减少锁竞争，提高并发性能：

```rust
pub struct ShardedLruCache {
    shards: Vec<Arc<RwLock<LruCacheState>>>,
    shard_count: usize,
}

impl ShardedLruCache {
    fn new(config: CacheConfig, shard_count: usize) -> Self {
        let shards = (0..shard_count)
            .map(|_| Arc::new(RwLock::new(LruCacheState::new())))
            .collect();

        Self { shards, shard_count }
    }

    fn get_shard(&self, key: &str) -> usize {
        let hash = key.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64));
        (hash % self.shard_count as u64) as usize
    }

    pub async fn get(&self, key: &str) -> Option<Vec<u8>> {
        let shard_idx = self.get_shard(key);
        let shard = &self.shards[shard_idx];
        // ... 访问对应 shard
    }
}
```

### 2. 零拷贝读取

使用 `Arc` 避免数据复制：

```rust
struct CacheEntry {
    value: Arc<Vec<u8>>,  // 共享数据，避免复制
    // ... 其他字段
}
```

### 3. 批量预热

启动时预加载热数据：

```rust
async fn warm_up_cache(cache: &LruCache, db: &MemoryDb, keys: Vec<String>) {
    for key in keys {
        if let Ok(Some(entry)) = db.get(&key) {
            cache.put(key, entry.value, None).await;
        }
    }
}
```

---

## 监控和调试

### 1. 指标导出

```rust
/// 导出 Prometheus 格式指标
pub fn export_metrics(&self) -> String {
    let snapshot = self.metrics.snapshot();
    format!(
        r#"
# HELP cache_hit_rate Cache hit rate
# TYPE cache_hit_rate gauge
cache_hit_rate {}

# HELP cache_total_requests Total cache requests
# TYPE cache_total_requests counter
cache_total_requests {}

# HELP cache_hits Total cache hits
# TYPE cache_hits counter
cache_hits {}

# HELP cache_misses Total cache misses
# TYPE cache_misses counter
cache_misses {}
"#,
        snapshot.hit_rate,
        snapshot.total_requests,
        snapshot.hits,
        snapshot.misses
    )
}
```

### 2. 健康检查

```rust
pub async fn health_check(&self) -> CacheHealth {
    let cache = self.cache.read().await;
    let snapshot = self.metrics.snapshot();

    CacheHealth {
        size: cache.entries.len(),
        capacity: self.config.max_entries,
        utilization: (cache.entries.len() as f64) / (self.config.max_entries as f64),
        hit_rate: snapshot.hit_rate,
        is_healthy: snapshot.hit_rate > 0.5,  // 命中率 > 50% 视为健康
    }
}
```

---

## 使用示例

### 集成到 GetOperations

```rust
impl GetOperations {
    pub async fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        // 1. 尝试从缓存读取
        if let Some(cache) = &self.state.cache {
            if let Some(value) = cache.get(key).await {
                tracing::debug!("Cache hit for key: {}", key);
                return Ok(Some(deserialize_cached_item(value)));
            }
        }

        // 2. 缓存未命中，从数据库读取
        let full_key = self.state.full_key(key);
        let db = self.state.memory_db.lock().await;

        match db.get(&full_key)? {
            Some(entry) => {
                let mut item = MemoryItem::from(entry);
                item.owner = self.state.node_id.clone();

                // 3. 解密私域记忆
                if item.encrypted {
                    if let Some(ref enc) = self.state.encryption {
                        item.value = enc.decrypt(&item.value)?;
                    }
                }

                // 4. 更新缓存
                if let Some(cache) = &self.state.cache {
                    cache.put(key.to_string(), item.value.clone(), None).await;
                }

                Ok(Some(item))
            }
            None => Ok(None),
        }
    }
}
```

### 集成到 SetOperations

```rust
impl SetOperations {
    pub async fn set(
        &self,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> Result<()> {
        // 1. 写入数据库
        // ... 现有逻辑

        // 2. 使缓存失效
        if let Some(cache) = &self.state.cache {
            cache.invalidate(key).await;
        }

        Ok(())
    }
}
```

---

## 测试计划

### 单元测试

1. **LRU 淘汰测试**
   - 验证超过容量时正确淘汰最久未访问的条目
   - 验证淘汰后命中率变化

2. **TTL 过期测试**
   - 验证过期条目不被返回
   - 验证过期后自动清理

3. **并发安全测试**
   - 多线程并发读写
   - 验证无数据竞争

4. **统计准确性测试**
   - 验证命中率计算正确
   - 验证统计信息完整

### 性能测试

1. **基准测试**
   - 缓存命中延迟 < 1μs
   - 缓存未命中延迟 < 100μs
   - 吞吐量 > 100K ops/sec

2. **压力测试**
   - 100万次读写操作
   - 验证内存占用稳定

3. **模拟真实负载**
   - 使用 Zipf 分布模拟真实访问模式
   - 验证命中率 > 70%

---

## 文件结构

```
cis-core/src/cache/
├── mod.rs              # 模块导出
├── architecture.md     # 本文档
├── config.rs           # CacheConfig
├── lru.rs             # LruCache 核心实现
├── batch_ops.rs       # 批量操作
├── metrics.rs         # CacheMetrics (或集成到 lru.rs)
└── tests/
    └── cache_tests.rs # 集成测试
```

---

## 下一步

- [ ] 实现 `config.rs` (P1-4.5)
- [ ] 实现 `lru.rs` (P1-4.2)
- [ ] 集成到 `get.rs` 和 `set.rs` (P1-4.3)
- [ ] 实现 `batch_ops.rs` (P1-4.4)
- [ ] 编写测试 `cache_tests.rs` (P1-4.6)

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: Team J
