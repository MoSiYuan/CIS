# TASK 1.2: 提取 cis-types

> **Phase**: 1 - cis-common 基础
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week 1

---

## 任务概述

将 `cis-core` 中的公共类型提取到独立的 `cis-types` crate。

## 工作内容

### 1. 分析现有类型

审查 `cis-core/src/types/` 和 `cis-core/src/common/`：

```rust
// 需要提取的类型列表
- PeerId, DeviceId
- TaskId, TaskHandle
- TaskType, TaskStatus
- SessionId
- MemoryId
- Priority (High, Normal, Low)
```

### 2. 创建 cis-types crate

```
crates/cis-types/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── peer.rs
│   ├── task.rs
│   ├── session.rs
│   └── priority.rs
```

### 3. 实现 newtype 模式

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PeerId(String);

impl PeerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
```

### 4. 确保向后兼容

```rust
// cis-core/src/types.rs
pub use cis_types::*; // 重导出以保持兼容
```

## 验收标准

- [ ] 所有公共类型成功提取
- [ ] 使用 newtype 模式封装
- [ ] 实现必要的 trait (Display, FromStr, etc.)
- [ ] cis-core 可通过重导出兼容
- [ ] 单元测试覆盖边界情况

## 依赖

- Task 1.1 (创建 cis-common)

## 阻塞

- Task 2.1 (提取 cis-storage)

---
