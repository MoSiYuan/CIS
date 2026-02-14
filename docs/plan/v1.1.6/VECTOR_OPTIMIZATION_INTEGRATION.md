# 向量搜索优化 - 快速集成指南

> **版本**: v1.1.6
> **团队**: Team F
> **最后更新**: 2026-02-12

---

## 概述

本指南帮助开发者快速集成向量搜索性能优化到现有代码中。

## 新增模块

### 1. 智能切换 (`switch`)

```rust
use cis_core::vector::switch::{IndexMonitor, SearchStrategy};

// 创建监控器
let monitor = IndexMonitor::new();

// 更新索引大小
monitor.update_index_size(5000).await;

// 获取推荐策略
let strategy = monitor.decide_strategy().await;
match strategy {
    SearchStrategy::SQLiteFullText => {
        // 使用 SQLite 全文搜索
    }
    SearchStrategy::HNSW { ef_search } => {
        // 使用 HNSW，ef_search 参数
    }
    SearchStrategy::HNSWWithCache { ef_search, preload_top_k } => {
        // 使用 HNSW + 缓存
    }
}
```

### 2. 批量加载 (`batch_loader`)

```rust
use cis_core::vector::batch_loader::{BatchVectorLoader, VectorBatch};

// 创建加载器
let loader = BatchVectorLoader::new()
    .with_batch_size(100)
    .with_parallelism(4);

// 从数据库加载
let batch = loader.load_from_loader(|| async {
    // 从数据库加载向量
    Ok(vec![("id1".to_string(), vec![0.1f32, 0.2, 0.3])])
}).await?;

// 批量计算相似度
let query = vec![0.1f32; 768];
let results = batch.compute_similarities(&query, 10)?;
```

### 3. 结果合并 (`merger`)

```rust
use cis_core::vector::merger::{ResultMerger, SearchResult, MergeStrategy};

let mut merger = ResultMerger::new();

// 合并 HNSW 和 SQLite 结果
let merged = merger.merge(
    hnsw_results,
    sqlite_results,
    MergeStrategy::Union,  // 并集，去重
    10                     // top_k
)?;

// 查看统计
if let Some(stats) = merger.last_stats() {
    println!("去重数量: {}", stats.deduped_count);
}
```

### 4. 自适应调整 (`adaptive_threshold`)

```rust
use cis_core::vector::adaptive_threshold::{AdaptiveThreshold, PerformanceTarget};
use cis_core::vector::switch::SearchMetrics;

// 创建调整器
let mut adapter = AdaptiveThreshold::new();

// 提供性能指标
let metrics = SearchMetrics::new(80.0, 400.0, 0.5, 5000);

// 获取调整建议
let actions = adapter.adjust(&metrics)?;

for action in actions {
    if action.is_action_required() {
        println!("建议: {}", action.describe());
    }
}
```

## 集成到 VectorStorage

### 修改 `search_memory` 方法

在 `cis-core/src/vector/storage.rs` 中：

```rust
use crate::vector::{
    switch::{IndexMonitor, SearchStrategy},
    merger::ResultMerger,
};

pub struct VectorStorage {
    // ... 现有字段

    // v1.1.6 新增
    monitor: Arc<IndexMonitor>,
    merger: Arc<ResultMerger>,
}

impl VectorStorage {
    pub async fn search_memory(
        &self,
        query: &str,
        limit: usize,
        threshold: Option<f32>,
    ) -> Result<Vec<MemoryResult>> {
        let query_vec = self.embedding.embed(query).await?;
        let threshold = threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);

        // 获取搜索策略
        let strategy = self.monitor.decide_strategy().await;
        let start = Instant::now();

        let results = match strategy {
            SearchStrategy::SQLiteFullText => {
                // 使用 SQLite 批量加载
                self.search_memory_batch(&query_vec, limit, threshold).await?
            }

            SearchStrategy::HNSW { ef_search } => {
                // 使用 HNSW 索引
                self.search_memory_hnsw(&query_vec, limit, threshold, ef_search).await?
            }

            SearchStrategy::HNSWWithCache { ef_search, preload_top_k } => {
                // 并行搜索并合并
                let (hnsw, sqlite) = tokio::join!(
                    self.search_memory_hnsw(&query_vec, limit * 2, threshold, ef_search),
                    self.search_memory_batch(&query_vec, limit * 2, threshold)
                );

                // 合并结果
                let merger = ResultMerger::new();
                let merged = merger.merge(
                    hnsw?,
                    sqlite?,
                    MergeStrategy::Union,
                    limit,
                )?;

                // 转换为 MemoryResult
                merged.into_iter().map(|r| MemoryResult {
                    memory_id: r.id.clone(),
                    key: r.id,
                    category: None,
                    similarity: r.score,
                }).collect()
            }
        };

        // 记录延迟
        self.monitor.record_search_latency(start.elapsed()).await;

        Ok(results)
    }

    // 新增：批量搜索（优化版）
    async fn search_memory_batch(
        &self,
        query_vec: &[f32],
        limit: usize,
        threshold: f32,
    ) -> Result<Vec<MemoryResult>> {
        // 使用 BatchVectorLoader 批量加载
        // ... 实现细节
    }
}
```

## 性能测试

### 运行基准测试

```bash
# 编译并运行
cargo bench --bench vector_search

# 查看详细输出
cargo bench --bench vector_search -- --verbose

# 生成报告
cargo bench --bench vector_search -- --output-format bencher | tee benchmark_results.txt
```

### 解释结果

输出示例：

```
=== HNSW (ef=100) ===
数据集大小: 15000
总耗时: 1.234s
平均延迟: 12.34 ms
P50 延迟: 10.50 ms
P99 延迟: 25.80 ms
QPS: 81.04
内存使用: 45.67 MB

--- 性能对比 ---
✓ 加速: 18.50x
✓ QPS 提升: 1750.0%
✓ 延迟降低: 987.65 ms
✓ 内存减少: 12.34 MB
```

## 监控和调优

### 启用自适应调整

```rust
use tokio::time::{interval, Duration};

// 每小时自动调优
async fn auto_tune_task(storage: Arc<VectorStorage>) {
    let mut ticker = interval(Duration::from_secs(3600));

    loop {
        ticker.tick().await;

        // 获取当前指标
        let metrics = storage.monitor.get_metrics().await.unwrap();

        // 调整参数
        let actions = storage.adapter.adjust(&metrics).unwrap();

        for action in actions {
            match action {
                ThresholdAction::DecreaseEfSearch { suggested, .. } => {
                    // 更新 ef_search
                    storage.set_ef_search(suggested).await;
                }
                ThresholdAction::IncreasePreload { suggested, .. } => {
                    // 更新预加载大小
                    storage.set_preload_size(suggested).await;
                }
                // ... 处理其他动作
            }
        }
    }
}
```

### 性能指标收集

```rust
// 记录每次搜索
let start = Instant::now();
let results = storage.search_memory(query, limit, threshold).await?;
storage.monitor.record_search_latency(start.elapsed()).await;

// 定期查看指标
let metrics = storage.monitor.get_metrics().await?;
println!("平均延迟: {:.2} ms", metrics.avg_latency_ms);
println!("QPS: {:.2}", metrics.qps);
println!("缓存命中率: {:.1}%", metrics.cache_hit_rate * 100.0);
```

## 故障排查

### 问题 1: 性能提升不明显

**可能原因**:
- 索引太小（< 100），应使用 SQLite
- ef_search 参数不合适

**解决方案**:
```rust
// 检查当前策略
let strategy = monitor.current_strategy().await;
println!("当前策略: {:?}", strategy);

// 手动调整阈值
let custom_threshold = SwitchThreshold::new(500, 5000, 30, 80, 50);
monitor.set_threshold(custom_threshold).await?;
```

### 问题 2: 内存占用过高

**可能原因**:
- 预加载量过大
- 缓存命中率低

**解决方案**:
```rust
// 减少预加载
let threshold = monitor.get_threshold().await;
let adjusted = SwitchThreshold::new(
    threshold.small_threshold,
    threshold.large_threshold,
    threshold.ef_search_small,
    threshold.ef_search_large,
    50,  // 减少预加载到 50
);
monitor.set_threshold(adjusted).await?;
```

### 问题 3: 搜索质量下降

**可能原因**:
- ef_search 过小导致召回率低
- 应该使用混合搜索

**解决方案**:
```rust
// 使用 RRF 策略合并结果
let merged = merger.merge(
    hnsw_results,
    sqlite_results,
    MergeStrategy::Rrf { k: 60 },
    limit,
)?;
```

## 最佳实践

### 1. 定期监控性能

```rust
// 每天生成性能报告
async fn daily_report(storage: Arc<VectorStorage>) {
    let metrics = storage.monitor.get_metrics().await?;
    let avg_latency = storage.monitor.avg_latency_last_n(1000).await;

    println!("=== 性能报告 ===");
    println!("平均延迟: {:.2} ms", avg_latency.as_millis());
    println!("QPS: {:.2}", metrics.qps);
    println!("索引大小: {}", metrics.index_size);
}
```

### 2. 测试不同阈值

```rust
// A/B 测试不同 ef_search
async fn test_ef_search(storage: &VectorStorage) {
    for ef in &[30, 50, 80, 100, 150] {
        let start = Instant::now();
        let _ = storage.search_memory("test", 10, None).await;
        let latency = start.elapsed();

        println!("ef={}: {:?}", ef, latency);
    }
}
```

### 3. 预热缓存

```rust
// 启动时预热热门查询
async fn warmup_cache(storage: &VectorStorage) {
    let hot_queries = vec![
        "用户偏好",
        "配置设置",
        "架构决策",
    ];

    for query in hot_queries {
        let _ = storage.search_memory(query, 10, None).await;
    }

    println!("缓存预热完成");
}
```

## 参考文档

- [设计文档](../../cis-core/src/vector/hybrid_design.md)
- [Team F 报告](./TEAM_F_REPORT.md)
- [解决方案](./SOLUTION.md#32-向量搜索优化)

---

**快速链接**:

- [switch.rs API](../../cis-core/src/vector/switch.rs)
- [batch_loader.rs API](../../cis-core/src/vector/batch_loader.rs)
- [merger.rs API](../../cis-core/src/vector/merger.rs)
- [adaptive_threshold.rs API](../../cis-core/src/vector/adaptive_threshold.rs)
