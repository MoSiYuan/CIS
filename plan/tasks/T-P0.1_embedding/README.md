# T-P0.1: AI/Embedding 模拟实现替换

**优先级**: 🔴 P0 (最高)  
**预估时间**: 4h  
**依赖**: embedding_service.rs (已实现)  
**分配**: Agent-F

---

## 问题描述

当前代码仍在使用基于哈希的模拟向量生成，而非真实的 fastembed。

**问题文件**:
- `cis-core/src/ai/embedding.rs:380`
- `cis-core/src/memory/service.rs:929`
- `cis-core/src/task/vector.rs:415`

**当前模拟代码**:
```rust
// 简单的确定性模拟：根据文本哈希生成向量
let hash = calculate_hash(text);
let embedding: Vec<f32> = (0..768).map(|i| ((hash + i as u64) % 100) as f32 / 100.0).collect();
```

---

## 修复方案

### 1. 替换 `ai/embedding.rs`

```rust
use crate::ai::embedding_service::EmbeddingService;

pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let service = EmbeddingService::global().await?;
    service.embed(text).await
}
```

### 2. 替换 `memory/service.rs`

```rust
// 删除模拟代码，使用 EmbeddingService
let embedding = EmbeddingService::global().await?.embed(text).await?;
```

### 3. 替换 `task/vector.rs`

```rust
// 使用真实 embedding
let embedding = EmbeddingService::global().await?.embed(&task.content).await?;
```

---

## 验收标准

- [ ] 相同文本生成相同向量（确定性）
- [ ] 相似文本余弦相似度 > 0.8
- [ ] 删除所有 "模拟" 注释
- [ ] 单元测试通过

---

## 检查命令

```bash
# 检查是否还有模拟代码
grep -n "模拟.*embedding\|模拟.*向量" cis-core/src/ai/*.rs cis-core/src/memory/*.rs cis-core/src/task/*.rs

# 期望: 无输出
```
