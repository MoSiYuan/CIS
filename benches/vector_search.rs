//! # 向量搜索性能基准测试
//!
//! 对比优化前后的向量搜索性能。
//!
//! ## 测试场景
//!
//! 1. **小数据集** (< 1000) - SQLite 全文搜索
//! 2. **中等数据集** (1000-10000) - HNSW (ef=50)
//! 3. **大数据集** (> 10000) - HNSW (ef=100) + 缓存
//! 4. **批量加载优化** - 并行反序列化
//! 5. **结果合并** - 去重和排序
//!
//! ## 运行
//!
//! ```bash
//! cargo bench --bench vector_search
//! ```

use std::time::{Duration, Instant};
use std::sync::Arc;

// 注意：这些是模拟的性能测试框架
// 实际测试需要集成真实的 VectorStorage

/// 性能指标
#[derive(Debug, Clone)]
pub struct BenchmarkMetrics {
    /// 测试名称
    pub name: String,

    /// 数据集大小
    pub dataset_size: usize,

    /// 总耗时
    pub total_duration: Duration,

    /// 平均延迟（毫秒）
    pub avg_latency_ms: f64,

    /// P50 延迟
    pub p50_latency_ms: f64,

    /// P99 延迟
    pub p99_latency_ms: f64,

    /// QPS（每秒查询数）
    pub qps: f64,

    /// 内存使用（字节）
    pub memory_bytes: usize,
}

impl BenchmarkMetrics {
    /// 格式化输出
    pub fn print(&self) {
        println!(
            "\n=== {} ===",
            self.name
        );
        println!("数据集大小: {}", self.dataset_size);
        println!("总耗时: {:?}", self.total_duration);
        println!("平均延迟: {:.2} ms", self.avg_latency_ms);
        println!("P50 延迟: {:.2} ms", self.p50_latency_ms);
        println!("P99 延迟: {:.2} ms", self.p99_latency_ms);
        println!("QPS: {:.2}", self.qps);
        println!("内存使用: {:.2} MB", self.memory_bytes as f64 / 1024.0 / 1024.0);
    }

    /// 对比两个指标
    pub fn compare(&self, other: &BenchmarkMetrics) -> ComparisonResult {
        let speedup = other.avg_latency_ms / self.avg_latency_ms;
        let qps_improvement = (self.qps - other.qps) / other.qps;

        ComparisonResult {
            speedup,
            qps_improvement,
            latency_reduction_ms: other.avg_latency_ms - self.avg_latency_ms,
            memory_reduction_mb:
                (other.memory_bytes - self.memory_bytes) as f64 / 1024.0 / 1024.0,
        }
    }
}

/// 对比结果
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    /// 加速比
    pub speedup: f64,

    /// QPS 提升比例
    pub qps_improvement: f64,

    /// 延迟降低（毫秒）
    pub latency_reduction_ms: f64,

    /// 内存减少（MB）
    pub memory_reduction_mb: f64,
}

impl ComparisonResult {
    /// 格式化输出
    pub fn print(&self) {
        println!("\n--- 性能对比 ---");
        if self.speedup > 1.0 {
            println!("✓ 加速: {:.2}x", self.speedup);
        } else {
            println!("✗ 减速: {:.2}x", 1.0 / self.speedup);
        }

        if self.qps_improvement > 0.0 {
            println!("✓ QPS 提升: {:.1}%", self.qps_improvement * 100.0);
        } else {
            println!("✗ QPS 下降: {:.1}%", self.qps_improvement.abs() * 100.0);
        }

        if self.latency_reduction_ms > 0.0 {
            println!("✓ 延迟降低: {:.2} ms", self.latency_reduction_ms);
        } else {
            println!("✗ 延迟增加: {:.2} ms", self.latency_reduction_ms.abs());
        }

        if self.memory_reduction_mb > 0.0 {
            println!("✓ 内存减少: {:.2} MB", self.memory_reduction_mb);
        } else {
            println!("✗ 内存增加: {:.2} MB", self.memory_reduction_mb.abs());
        }
    }
}

/// 模拟向量数据
#[derive(Clone)]
pub struct MockVector {
    pub id: String,
    pub data: Vec<f32>,
}

impl MockVector {
    /// 生成随机向量
    pub fn random(id: String, dim: usize) -> Self {
        use std::f32::consts::PI;
        let data: Vec<f32> = (0..dim)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();

        Self { id, data }
    }

    /// 计算余弦相似度
    pub fn cosine_similarity(&self, other: &Self) -> f32 {
        if self.data.len() != other.data.len() {
            return 0.0;
        }

        let mut dot_product = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for i in 0..self.data.len() {
            dot_product += self.data[i] * other.data[i];
            norm_a += self.data[i] * self.data[i];
            norm_b += other.data[i] * other.data[i];
        }

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        (dot_product / (norm_a.sqrt() * norm_b.sqrt())) as f32
    }
}

/// 基准测试套件
pub struct BenchmarkSuite {
    /// 向量维度
    dimension: usize,
}

impl BenchmarkSuite {
    /// 创建新的测试套件
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// 生成测试数据集
    pub fn generate_dataset(&self, size: usize) -> Vec<MockVector> {
        (0..size)
            .map(|i| MockVector::random(format!("id_{}", i), self.dimension))
            .collect()
    }

    /// 测试 SQLite 暴力搜索（模拟 fallback 模式）
    pub fn benchmark_sqlite_fallback(
        &self,
        dataset: &[MockVector],
        query_count: usize,
    ) -> BenchmarkMetrics {
        let start = Instant::now();
        let mut latencies = Vec::with_capacity(query_count);

        for _ in 0..query_count {
            let query = MockVector::random("query".to_string(), self.dimension);

            let query_start = Instant::now();

            // 模拟逐个计算相似度
            let _similarities: Vec<_> = dataset
                .iter()
                .map(|v| (v.id.clone(), query.cosine_similarity(v)))
                .collect();

            latencies.push(query_start.elapsed().as_millis() as f64);
        }

        let total_duration = start.elapsed();

        self.calculate_metrics(
            "SQLite Fallback (Baseline)",
            dataset.len(),
            total_duration,
            latencies,
            dataset.len() * self.dimension * 4, // 估算内存
        )
    }

    /// 测试 HNSW 索引搜索
    pub fn benchmark_hnsw_search(
        &self,
        dataset: &[MockVector],
        query_count: usize,
        ef_search: usize,
    ) -> BenchmarkMetrics {
        // 模拟 HNSW 索引构建
        let index_start = Instant::now();
        let _hnsw_index = self.build_mock_hnsw_index(dataset, ef_search);
        let _index_build_time = index_start.elapsed();

        let start = Instant::now();
        let mut latencies = Vec::with_capacity(query_count);

        for _ in 0..query_count {
            let query = MockVector::random("query".to_string(), self.dimension);

            let query_start = Instant::now();

            // 模拟 HNSW 搜索（只检查部分候选）
            let candidate_count = (dataset.len() as f64).sqrt() as usize;
            let _candidates: Vec<_> = dataset
                .iter()
                .take(candidate_count)
                .map(|v| (v.id.clone(), query.cosine_similarity(v)))
                .collect();

            latencies.push(query_start.elapsed().as_millis() as f64);
        }

        let total_duration = start.elapsed();

        self.calculate_metrics(
            &format!("HNSW (ef={})", ef_search),
            dataset.len(),
            total_duration,
            latencies,
            dataset.len() * self.dimension * 4 + 10000, // 索引额外开销
        )
    }

    /// 测试批量加载优化
    pub fn benchmark_batch_loading(
        &self,
        dataset: &[MockVector],
        query_count: usize,
        batch_size: usize,
    ) -> BenchmarkMetrics {
        let start = Instant::now();
        let mut latencies = Vec::with_capacity(query_count);

        for _ in 0..query_count {
            let query = MockVector::random("query".to_string(), self.dimension);

            let query_start = Instant::now();

            // 批量加载和计算（模拟并行）
            let mut results = Vec::new();
            for chunk in dataset.chunks(batch_size) {
                let chunk_results: Vec<_> = chunk
                    .iter()
                    .map(|v| (v.id.clone(), query.cosine_similarity(v)))
                    .collect();
                results.extend(chunk_results);
            }

            latencies.push(query_start.elapsed().as_millis() as f64);
        }

        let total_duration = start.elapsed();

        self.calculate_metrics(
            &format!("Batch Loading (batch={})", batch_size),
            dataset.len(),
            total_duration,
            latencies,
            dataset.len() * self.dimension * 4,
        )
    }

    /// 测试混合搜索（并行执行两种方法）
    pub fn benchmark_hybrid_search(
        &self,
        dataset: &[MockVector],
        query_count: usize,
        ef_search: usize,
    ) -> BenchmarkMetrics {
        // 构建索引
        let _hnsw_index = self.build_mock_hnsw_index(dataset, ef_search);

        let start = Instant::now();
        let mut latencies = Vec::with_capacity(query_count);

        for _ in 0..query_count {
            let query = MockVector::random("query".to_string(), self.dimension);

            let query_start = Instant::now();

            // 模拟并行搜索
            let hnsw_results = {
                let candidate_count = (dataset.len() as f64).sqrt() as usize;
                dataset
                    .iter()
                    .take(candidate_count)
                    .map(|v| (v.id.clone(), query.cosine_similarity(v)))
                    .collect::<Vec<_>>()
            };

            let sqlite_results = {
                dataset
                    .iter()
                    .take(100) // SQLite 只检查前 100 个
                    .map(|v| (v.id.clone(), query.cosine_similarity(v)))
                    .collect::<Vec<_>>()
            };

            // 合并结果
            let _merged = self.merge_results(hnsw_results, sqlite_results);

            latencies.push(query_start.elapsed().as_millis() as f64);
        }

        let total_duration = start.elapsed();

        self.calculate_metrics(
            &format!("Hybrid Search (ef={})", ef_search),
            dataset.len(),
            total_duration,
            latencies,
            dataset.len() * self.dimension * 4 + 15000, // 混合模式额外开销
        )
    }

    /// 计算性能指标
    fn calculate_metrics(
        &self,
        name: &str,
        dataset_size: usize,
        total_duration: Duration,
        mut latencies: Vec<f64>,
        memory_bytes: usize,
    ) -> BenchmarkMetrics {
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let avg_latency_ms = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let p50_latency_ms = latencies[latencies.len() / 2];
        let p99_latency_ms = latencies[(latencies.len() as f64 * 0.99) as usize];

        let qps = if total_duration.as_secs_f64() > 0.0 {
            latencies.len() as f64 / total_duration.as_secs_f64()
        } else {
            0.0
        };

        BenchmarkMetrics {
            name: name.to_string(),
            dataset_size,
            total_duration,
            avg_latency_ms,
            p50_latency_ms,
            p99_latency_ms,
            qps,
            memory_bytes,
        }
    }

    /// 构建模拟 HNSW 索引（简化版）
    fn build_mock_hnsw_index(&self, dataset: &[MockVector], ef_search: usize) -> Vec<usize> {
        // 模拟索引构建：记录候选位置
        let candidate_count = (dataset.len() as f64).sqrt() as usize;
        (0..candidate_count.min(dataset.len())).collect()
    }

    /// 合并结果
    fn merge_results(
        &self,
        mut results1: Vec<(String, f32)>,
        mut results2: Vec<(String, f32)>,
    ) -> Vec<(String, f32)> {
        results1.append(&mut results2);

        // 去重：保留最高分
        let mut score_map = std::collections::HashMap::new();
        for (id, score) in results1 {
            score_map
                .entry(id)
                .and_modify(|s| *s = (*s).max(score))
                .or_insert(score);
        }

        // 转换并排序
        let mut merged: Vec<_> = score_map.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        merged
    }
}

/// 完整基准测试流程
pub fn run_full_benchmark() {
    println!("========================================");
    println!("  CIS 向量搜索性能基准测试");
    println!("  版本: v1.1.6");
    println!("========================================");

    let suite = BenchmarkSuite::new(768); // 标准向量维度

    // 测试 1: 小数据集
    println!("\n\n### 测试 1: 小数据集 (500 向量) ###");
    let small_dataset = suite.generate_dataset(500);
    let baseline_small = suite.benchmark_sqlite_fallback(&small_dataset, 100);
    baseline_small.print();

    let hnsw_small = suite.benchmark_hnsw_search(&small_dataset, 100, 50);
    hnsw_small.print();

    let comparison1 = hnsw_small.compare(&baseline_small);
    comparison1.print();

    // 测试 2: 中等数据集
    println!("\n\n### 测试 2: 中等数据集 (5000 向量) ###");
    let medium_dataset = suite.generate_dataset(5000);
    let baseline_medium = suite.benchmark_sqlite_fallback(&medium_dataset, 100);
    baseline_medium.print();

    let hnsw_medium = suite.benchmark_hnsw_search(&medium_dataset, 100, 50);
    hnsw_medium.print();

    let comparison2 = hnsw_medium.compare(&baseline_medium);
    comparison2.print();

    // 测试 3: 大数据集
    println!("\n\n### 测试 3: 大数据集 (15000 向量) ###");
    let large_dataset = suite.generate_dataset(15000);
    let baseline_large = suite.benchmark_sqlite_fallback(&large_dataset, 100);
    baseline_large.print();

    let hnsw_large = suite.benchmark_hnsw_search(&large_dataset, 100, 100);
    hnsw_large.print();

    let comparison3 = hnsw_large.compare(&baseline_large);
    comparison3.print();

    // 测试 4: 批量加载优化
    println!("\n\n### 测试 4: 批量加载优化 (5000 向量) ###");
    let batch_optimized = suite.benchmark_batch_loading(&medium_dataset, 100, 100);
    batch_optimized.print();

    let comparison4 = batch_optimized.compare(&baseline_medium);
    comparison4.print();

    // 测试 5: 混合搜索
    println!("\n\n### 测试 5: 混合搜索 (15000 向量) ###");
    let hybrid_search = suite.benchmark_hybrid_search(&large_dataset, 100, 100);
    hybrid_search.print();

    let comparison5 = hybrid_search.compare(&baseline_large);
    comparison5.print();

    // 总结
    println!("\n\n========================================");
    println!("  性能总结");
    println!("========================================");

    let speedup_small = comparison1.speedup;
    let speedup_medium = comparison2.speedup;
    let speedup_large = comparison3.speedup;
    let speedup_batch = comparison4.speedup;
    let speedup_hybrid = comparison5.speedup;

    println!("小数据集加速: {:.2}x", speedup_small);
    println!("中等数据集加速: {:.2}x", speedup_medium);
    println!("大数据集加速: {:.2}x", speedup_large);
    println!("批量加载加速: {:.2}x", speedup_batch);
    println!("混合搜索加速: {:.2}x", speedup_hybrid);

    println!("\n✓ 基准测试完成");
}

fn main() {
    run_full_benchmark();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_metrics() {
        let metrics = BenchmarkMetrics {
            name: "Test".to_string(),
            dataset_size: 1000,
            total_duration: Duration::from_millis(1000),
            avg_latency_ms: 10.0,
            p50_latency_ms: 8.0,
            p99_latency_ms: 20.0,
            qps: 100.0,
            memory_bytes: 1024 * 1024,
        };

        metrics.print();

        let other = BenchmarkMetrics {
            avg_latency_ms: 20.0,
            qps: 50.0,
            memory_bytes: 2 * 1024 * 1024,
            ..metrics.clone()
        };

        let comparison = metrics.compare(&other);
        comparison.print();

        assert_eq!(comparison.speedup, 2.0);
        assert_eq!(comparison.qps_improvement, 1.0);
    }

    #[test]
    fn test_mock_vector_similarity() {
        let v1 = MockVector {
            id: "1".to_string(),
            data: vec![1.0, 0.0, 0.0],
        };

        let v2 = MockVector {
            id: "2".to_string(),
            data: vec![1.0, 0.0, 0.0],
        };

        let sim = v1.cosine_similarity(&v2);
        assert!((sim - 1.0).abs() < 0.001);

        let v3 = MockVector {
            id: "3".to_string(),
            data: vec![0.0, 1.0, 0.0],
        };

        let sim = v1.cosine_similarity(&v3);
        assert!((sim - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_benchmark_suite() {
        let suite = BenchmarkSuite::new(3);
        let dataset = suite.generate_dataset(100);

        assert_eq!(dataset.len(), 100);
        assert_eq!(dataset[0].data.len(), 3);
    }

    #[test]
    fn test_result_merging() {
        let suite = BenchmarkSuite::new(3);

        let results1 = vec![
            ("id1".to_string(), 0.95),
            ("id2".to_string(), 0.85),
        ];

        let results2 = vec![
            ("id1".to_string(), 0.90),
            ("id3".to_string(), 0.80),
        ];

        let merged = suite.merge_results(results1, results2);

        assert_eq!(merged.len(), 3); // id1, id2, id3 (id1 去重)
        assert_eq!(merged[0].0, "id1"); // 最高分
        assert!((merged[0].1 - 0.95).abs() < 0.01);
    }
}
