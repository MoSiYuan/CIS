# TASK 2.2: 提取 cis-memory

> **Phase**: 2 - 模块提取
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 3-4

---

## 任务概述

将记忆模块从 cis-core 提取为独立的 `cis-memory` crate，包括 ZeroClaw 兼容层。

## 工作内容

### 1. 分析现有记忆实现

审查 `cis-core/src/memory/`：
- `Memory` trait 定义
- `CISMemory` 实现
- 记忆索引策略
- Embedding 集成

### 2. 创建 crate 结构

```
crates/cis-memory/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── memory.rs       # Memory trait 重导出
│   ├── cis_memory.rs   # CIS 记忆实现
│   ├── entry.rs        # MemoryEntry 定义
│   ├── index.rs        # 记忆索引
│   ├── embedding.rs    # Embedding 集成
│   └── zeroclaw/       # ZeroClaw 兼容层
│       ├── mod.rs
│       ├── adapter.rs
│       └── loader.rs
└── tests/
    └── memory_tests.rs
```

### 3. 实现 CIS Memory

```rust
// cis_memory.rs
pub struct CISMemory<S: Storage> {
    storage: S,
    namespace: String,
    embedding: Arc<dyn EmbeddingProvider>,
    index: MemoryIndex,
}

impl<S: Storage> CISMemory<S> {
    pub fn new(
        storage: S,
        namespace: impl Into<String>,
        embedding: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        Self {
            storage,
            namespace: namespace.into(),
            embedding,
            index: MemoryIndex::new(),
        }
    }
}

#[async_trait]
impl<S: Storage> Memory for CISMemory<S> {
    async fn remember(&self, entry: MemoryEntry) -> Result<(), MemoryError> {
        // 存储 + 索引
    }
    
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryEntry>, MemoryError> {
        // 相似度搜索
    }
    // ...
}
```

### 4. 实现 ZeroClaw 适配器

```rust
// zeroclaw/adapter.rs
#[cfg(feature = "zeroclaw")]
pub struct ZeroclawMemoryAdapter<M: Memory> {
    inner: M,
    session_id: Option<String>,
}

#[cfg(feature = "zeroclaw")]
impl<M: Memory> ZeroclawMemoryAdapter<M> {
    pub fn new(inner: M) -> Self {
        Self { inner, session_id: None }
    }
    
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}
```

### 5. 配置 feature flags

```toml
[features]
default = ["embedding", "index"]
embedding = ["dep:cis-embedding"]
index = []
zeroclaw-compat = ["dep:zeroclaw-memory"]
```

## 验收标准

- [ ] CIS Memory 实现完整
- [ ] 记忆索引策略工作正常
- [ ] Embedding 集成正常
- [ ] ZeroClaw 适配器编译通过
- [ ] 单元测试覆盖率 > 80%

## 依赖

- Task 2.1 (cis-storage)
- Task 1.3 (cis-traits)

## 阻塞

- Task 3.2 (重构 cis-core)
- Task 4.x (ZeroClaw 集成)

---
