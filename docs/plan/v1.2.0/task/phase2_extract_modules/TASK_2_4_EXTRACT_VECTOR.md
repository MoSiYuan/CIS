# TASK 2.4: 提取 cis-vector

> **Phase**: 2 - 模块提取
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 4-5

---

## 任务概述

将向量存储模块从 cis-core 提取为独立的 `cis-vector` crate。

## 工作内容

### 1. 分析现有向量存储

审查 `cis-core/src/vector/`：
- `VectorStore` trait
- HNSW 索引实现
- Embedding 生成接口
- 相似度搜索

### 2. 创建 crate 结构

```
crates/cis-vector/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── store.rs        # VectorStore trait
│   ├── hnsw.rs         # HNSW 实现
│   ├── embedding.rs    # Embedding 接口
│   ├── distance.rs     # 距离计算
│   └── quantized.rs    # 量化支持（可选）
└── tests/
    └── vector_tests.rs
```

### 3. 实现 HNSW 索引

```rust
// hnsw.rs
pub struct HnswIndex {
    max_elements: usize,
    m: usize,              // 每个节点的连接数
    ef_construction: usize,
    layers: Vec<Layer>,
    nodes: Vec<Node>,
}

impl HnswIndex {
    pub fn new(max_elements: usize, m: usize, ef_construction: usize) -> Self {
        // 初始化 HNSW 参数
    }
    
    pub fn add_vector(&mut self, id: VectorId, vector: &[f32]) -> Result<(), VectorError> {
        // HNSW 插入算法
    }
    
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(VectorId, f32)> {
        // HNSW 搜索算法
    }
}
```

### 4. 实现 Embedding 接口

```rust
// embedding.rs
#[async_trait]
pub trait EmbeddingProvider: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError>;
    fn dimensions(&self) -> usize;
}

// OpenAI Embedding 实现
pub struct OpenAIEmbedding {
    client: reqwest::Client,
    api_key: String,
    model: String,
    dimensions: usize,
}
```

## 验收标准

- [ ] HNSW 索引实现正确
- [ ] 相似度搜索精度达标
- [ ] Embedding 接口可扩展
- [ ] 支持批量处理
- [ ] 性能测试通过

## 依赖

- Task 1.3 (cis-traits)

## 阻塞

- Task 2.2 (cis-memory, 需要向量搜索)

---
