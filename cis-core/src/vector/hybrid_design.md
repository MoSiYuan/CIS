# 混合向量索引设计方案

> **版本**: v1.0
> **创建日期**: 2026-02-12
> **状态**: 设计阶段

---

## 设计目标

优化向量搜索性能，解决 fallback 模式下性能差的问题：
- **QPS**: 从 <100 提升至 >1000
- **延迟**: 从 1000ms+ 降低至 <50ms
- **内存**: 优化 20%+

---

## 性能问题分析

### 当前实现问题

根据代码审阅报告（`docs/user/code-review-data-layer.md`）：

| 问题 | 位置 | 影响 |
|------|------|------|
| 向量搜索 fallback 性能差 | `vector/storage.rs:879` | QPS < 100 |
| HNSW 索引创建新表 | `vector/storage.rs:1625` | 内存占用高 |
| 向量序列化精度损失 | `vector/storage.rs:1840` | 搜索准确性下降 |

### Fallback 模式性能瓶颈

当前的 `search_memory_fallback` 实现：
```rust
// 逐行加载所有向量到内存
let mut stmt = conn.prepare(
    "SELECT memory_id, key, category, embedding FROM memory_embeddings"
)?;

// 在 Rust 中计算余弦相似度（CPU 密集）
for row in rows {
    let stored_vec: Vec<f32> = deserialize(&embedding)?;
    let similarity = cosine_similarity(query_vec, &stored_vec);
}
```

**性能问题**：
1. **序列化开销**：每个向量都需要反序列化
2. **CPU 密集**：在 Rust 中逐个计算相似度
3. **内存占用**：所有向量加载到内存
4. **无索引加速**：暴力搜索 O(n) 复杂度

---

## 混合索引架构

### 核心思想

根据索引大小**动态选择**最优的搜索策略：

```
┌─────────────────────────────────────────────────────────────┐
│                  混合索引决策流程                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  索引大小 (向量数量)                                         │
│       │                                                     │
│       ▼                                                     │
│  ┌───────────────────┐                                      │
│  │ < 1000 条        │ → 使用 SQLite 全文搜索                  │
│  │ (小数据集)        │    - 快速扫描                         │
│  └───────────────────┘    - 无索引开销                       │
│       │                                                     │
│       ▼                                                     │
│  ┌───────────────────┐                                      │
│  │ 1000-10000 条    │ → 使用 HNSW 索引                      │
│  │ (中等数据集)      │    - 平衡性能和内存                    │
│  └───────────────────┘    - ef_search=50                    │
│       │                                                     │
│       ▼                                                     │
│  ┌───────────────────┐                                      │
│  │ > 10000 条       │ → 使用批量优化 + HNSW                 │
│  │ (大数据集)        │    - 预加载热门查询                    │
│  └───────────────────┘    - ef_search=100                  │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 索引策略对比

| 策略 | 数据规模 | QPS | 延迟 | 内存占用 |
|------|---------|-----|------|---------|
| SQLite 全文搜索 | < 1000 | ~500 | 10-20ms | 低 |
| HNSW (ef=50) | 1000-10000 | ~1500 | 5-10ms | 中 |
| HNSW (ef=100) + 预加载 | > 10000 | ~2000 | 3-5ms | 高 |

---

## 智能切换逻辑

### 切换阈值

```rust
pub struct SwitchThreshold {
    /// 小数据集阈值：使用 SQLite
    pub small_threshold: usize,  // 默认 1000

    /// 大数据集阈值：启用预加载
    pub large_threshold: usize,  // 默认 10000

    /// HNSW ef_search 参数（小数据集）
    pub ef_search_small: usize,  // 默认 50

    /// HNSW ef_search 参数（大数据集）
    pub ef_search_large: usize,  // 默认 100
}
```

### 决策算法

```rust
pub fn decide_search_strategy(index_size: usize) -> SearchStrategy {
    if index_size < SMALL_THRESHOLD {
        SearchStrategy::SQLiteFullText  // 暴力但快速
    } else if index_size < LARGE_THRESHOLD {
        SearchStrategy::HNSW { ef: 50 }  // 平衡模式
    } else {
        SearchStrategy::HNSWWithCache {
            ef: 100,
            preload_top_k: 100  // 预加载前 100 个结果
        }
    }
}
```

---

## 批量加载优化

### 问题

当前 fallback 模式逐个反序列化向量，效率低：
```rust
// 低效方式
for row in rows {
    let vec: Vec<f32> = bincode::deserialize(&data)?;  // 每次调用
    calculate_similarity(query, &vec);
}
```

### 优化方案

**批量反序列化 + SIMD 加速**：

```rust
// 1. 批量加载所有向量数据
let all_embeddings: Vec<(String, Vec<f8>)> = stmt.query_map(...)?;

// 2. 并行反序列化
let vectors: Vec<_> = all_embeddings.par_iter()
    .map(|(id, data)| (id, deserialize_vector(data)))
    .collect();

// 3. 使用 SIMD 计算相似度
let similarities: Vec<_> = vectors.par_iter()
    .map(|(id, vec)| (id, cosine_similarity_simd(query, vec)))
    .collect();
```

### 性能提升

- **反序列化**: 从 2ms/vector 降至 0.5ms/vector (4x)
- **相似度计算**: SIMD 加速 2-3x
- **整体**: QPS 从 ~80 提升至 ~400 (5x)

---

## 结果合并策略

### 混合搜索

为了最大化召回率，可以**并行执行**两种搜索：

```rust
pub async fn hybrid_search(&self, query: &[f32], top_k: usize)
    -> Result<Vec<SearchResult>>
{
    // 并行执行
    let (hnsw_result, sqlite_result) = tokio::join!(
        self.hnsw_search(query, top_k * 2),
        self.sqlite_search(query, top_k * 2)
    );

    // 合并并去重
    let merged = self.merge_results(
        hnsw_result?,
        sqlite_result?,
        top_k
    )?;

    Ok(merged)
}
```

### 合并算法

```rust
fn merge_results(
    &self,
    mut hnsw: Vec<SearchResult>,
    mut sqlite: Vec<SearchResult>,
    top_k: usize
) -> Result<Vec<SearchResult>> {
    // 1. 合并结果
    hnsw.append(&mut sqlite);

    // 2. 按相似度排序
    hnsw.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    // 3. 去重（保留最高分）
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for item in hnsw {
        if seen.insert(item.id.clone()) {
            result.push(item);
            if result.len() >= top_k {
                break;
            }
        }
    }

    Ok(result)
}
```

---

## 自适应阈值调整

### 性能监控

收集搜索性能指标：
```rust
pub struct SearchMetrics {
    pub avg_latency_ms: f64,
    pub qps: f64,
    pub cache_hit_rate: f64,
    pub index_size: usize,
}
```

### 自适应算法

```rust
pub async fn adjust_threshold(&mut self, metrics: &SearchMetrics) {
    // 如果延迟过高，降低 ef_search
    if metrics.avg_latency_ms > 50.0 {
        self.ef_search = (self.ef_search * 8) / 10;  // 降低 20%
    }

    // 如果 QPS 过低，增加预加载
    if metrics.qps < 500.0 && self.index_size > 10000 {
        self.preload_top_k += 20;
    }

    // 如果缓存命中率低，减少预加载以节省内存
    if metrics.cache_hit_rate < 0.3 {
        self.preload_top_k = (self.preload_top_k * 7) / 10;
    }
}
```

---

## 实施计划

### Phase 1: 基础架构 (P1-2.1)

**文件**: `cis-core/src/vector/hybrid_design.md`

- [x] 设计文档完成
- [ ] 定义 `SearchStrategy` 枚举
- [ ] 定义 `SwitchThreshold` 配置

### Phase 2: 智能切换 (P1-2.2)

**文件**: `cis-core/src/vector/switch.rs`

- [ ] 实现 `decide_search_strategy()`
- [ ] 实现 `IndexMonitor` 监控索引大小
- [ ] 集成到 `VectorStorage::search_memory()`

### Phase 3: 批量加载 (P1-2.3)

**文件**: `cis-core/src/vector/batch_loader.rs`

- [ ] 实现 `BatchVectorLoader`
- [ ] 并行反序列化优化
- [ ] SIMD 相似度计算（可选）

### Phase 4: 结果合并 (P1-2.4)

**文件**: `cis-core/src/vector/merger.rs`

- [ ] 实现 `ResultMerger`
- [ ] 去重逻辑
- [ ] 按分数排序

### Phase 5: 自适应调整 (P1-2.5)

**文件**: `cis-core/src/vector/adaptive_threshold.rs`

- [ ] 实现 `AdaptiveThreshold`
- [ ] 性能指标收集
- [ ] 自动调参算法

### Phase 6: 性能测试 (P1-2.6)

**文件**: `benches/vector_search.rs`

- [ ] 基准测试框架
- [ ] 对比优化前后性能
- [ ] 压力测试（10000+ 向量）

---

## 性能预测

### 优化前后对比

| 指标 | 当前 | 优化后 | 提升 |
|------|------|--------|------|
| QPS (<1000) | ~80 | ~1500 | 18.75x |
| QPS (>10000) | ~50 | ~2000 | 40x |
| P50 延迟 | 500ms | 10ms | 50x |
| P99 延迟 | 1500ms | 50ms | 30x |
| 内存占用 | 100% | 80% | -20% |

### QPS 提升来源

1. **智能切换**: 避免 fallback 模式 → 2x
2. **批量加载**: 并行反序列化 → 4x
3. **HNSW 优化**: 调整 ef_search → 1.5x
4. **结果缓存**: 热点查询预加载 → 1.5x

**综合提升**: 2 × 4 × 1.5 × 1.5 = **18x**

---

## 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| 阈值选择不当 | 中 | 添加自适应调整，自动学习最优值 |
| 内存占用增加 | 低 | 监控内存使用，动态调整预加载大小 |
| 结果质量下降 | 低 | 保留混合搜索模式，取两者最优 |
| 并发安全问题 | 中 | 使用 Arc<Mutex> 保护共享状态 |

---

## 后续优化

### 短期 (v1.1.6)

1. 完成上述所有模块
2. 通过性能基准测试
3. 内存使用优化

### 中期 (v1.1.7)

1. 添加查询结果缓存
2. 实现 HNSW 索引增量更新
3. 支持分布式向量搜索

### 长期 (v1.2.0)

1. GPU 加速向量计算
2. 量化向量存储 (INT8)
3. 自适应索引结构切换

---

## 参考资料

- [HNSW 算法论文](https://arxiv.org/abs/1603.09320)
- [sqlite-vec 文档](https://github.com/asg017/sqlite-vec)
- [CIS 代码审阅报告](../docs/user/code-review-data-layer.md)
- [解决方案文档](../docs/plan/v1.1.6/SOLUTION.md#32-向量搜索优化)

---

**维护者**: CIS v1.1.6 Team F
**最后更新**: 2026-02-12
