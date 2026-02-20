# TASK 2.1: 提取 cis-storage

> **Phase**: 2 - 模块提取
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 3

---

## 任务概述

将存储模块从 cis-core 提取为独立的 `cis-storage` crate。

## 工作内容

### 1. 分析现有存储实现

审查 `cis-core/src/storage/`：
- `Storage` trait 定义
- `RocksDbStorage` 实现
- `SledStorage` 实现
- `MemoryStorage` 实现（测试用）

### 2. 创建 crate 结构

```
crates/cis-storage/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── storage.rs      # Storage trait (来自 cis-traits)
│   ├── rocksdb.rs      # RocksDB 实现
│   ├── sled.rs         # Sled 实现
│   └── memory.rs       # 内存实现
└── tests/
    └── integration_tests.rs
```

### 3. 实现存储 backends

```rust
// rocksdb.rs
pub struct RocksDbStorage {
    db: rocksdb::DB,
    path: PathBuf,
}

impl RocksDbStorage {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let db = rocksdb::DB::open_default(path.as_ref())?;
        Ok(Self {
            db,
            path: path.as_ref().to_path_buf(),
        })
    }
}

#[async_trait]
impl Storage for RocksDbStorage {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        // 实现...
    }
    // ...
}
```

### 4. 配置 feature flags

```toml
[features]
default = ["rocksdb"]
rocksdb = ["dep:rocksdb"]
sled = ["dep:sled"]
memory = []  # 内存存储始终可用
```

## 验收标准

- [ ] 所有存储后端成功提取
- [ ] RocksDB 实现正常工作
- [ ] Sled 实现正常工作
- [ ] 内存存储用于测试
- [ ] 集成测试通过
- [ ] 文档包含使用示例

## 依赖

- Task 1.1, 1.2, 1.3 (cis-common 基础)

## 阻塞

- Task 3.2 (重构 cis-core)

---
