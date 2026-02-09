# T4.3: Embedding 服务替换

**任务编号**: T4.3  
**任务名称**: Replace Mock Embedding  
**优先级**: P2  
**预估时间**: 4h  
**依赖**: 无  
**分配状态**: 待分配

---

## 任务概述

替换所有 mock embedding 实现，使用真实 fastembed。

---

## 输入

### 待修改文件
- `cis-core/src/memory/service.rs:929`
- `cis-core/src/task/vector.rs:415-421`
- `cis-core/src/vector/storage.rs:1876-1882`
- `cis-core/src/ai/embedding.rs:380`

### 依赖 crate
- `fastembed = "4.0"` (已配置)

### 当前问题
```rust
/// 模拟 embedding service（用于测试）
/// 简单的确定性模拟：根据文本哈希生成向量
```

---

## 输出要求

```rust
pub struct EmbeddingService {
    model: TextEmbedding,
}

impl EmbeddingService {
    pub async fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15)
        )?;
        Ok(Self { model })
    }
    
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.model.embed(vec![text], None)?;
        Ok(embeddings[0].clone())
    }
}
```

---

## 验收标准

- [ ] 相同文本生成相同向量
- [ ] 相似文本向量距离近
- [ ] 批处理性能 >100 texts/sec
- [ ] 模型自动下载（首次使用）

---

## 阻塞关系

**依赖**: 无

**注意**: 可立即并行执行
