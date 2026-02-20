# CIS v1.2.0 - ZeroClaw Trait 模块拆分任务列表

> **目标**: 将 CIS 重构为 ZeroClaw 式的 Trait 模块拆分架构，实现可插拔后端和运行时多态

## 📋 总览

基于以下报告的分析：
- [cis_trait_refactor_analysis.md](../kimi/cis_trait_refactor_analysis.md)
- [cis_zeroclaw_plugin_strategy.md](../kimi/cis_zeroclaw_plugin_strategy.md)

**实施策略**: 渐进式重构（3个Phase，优先高价值模块）

---

## Phase 1: 核心 Trait 抽象层（Week 1-2）🔥 **优先**

### 1.1 创建 Trait 模块结构

**文件**: `cis-core/src/traits/`

```
traits/
├── mod.rs              # 模块导出
├── memory.rs           # Memory trait 定义
├── transport.rs        # Transport trait 定义
└── encryption.rs       # Encryption trait 定义
```

#### Task 1.1.1: 创建 Memory Trait
- [ ] 定义 `Memory` trait（核心方法：get, set, delete, search）
- [ ] 定义 `MemoryEntry` 结构体
- [ ] 定义 `SearchResult` 结构体
- [ ] 添加 trait 文档和示例

**代码框架**:
```rust
#[async_trait]
pub trait Memory: Send + Sync {
    fn name(&self) -> &str;
    async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<MemoryEntry>>;
    async fn delete(&self, key: &str) -> Result<bool>;
    async fn search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<SearchResult>>;
}
```

#### Task 1.1.2: 创建 Transport Trait
- [ ] 定义 `Transport` trait（send, receive, broadcast）
- [ ] 定义 `NodeId` 类型
- [ ] 添加 trait 文档

#### Task 1.1.3: 创建 Encryption Trait
- [ ] 定义 `Encryption` trait（encrypt, decrypt, derive_key）
- [ ] 定义 `EncryptionKey` 类型
- [ ] 添加 trait 文档

---

### 1.2 实现 CIS 默认 Trait 实现

**文件**: `cis-core/src/traits/implementations/`

#### Task 1.2.1: 实现 CisMemoryBackend
- [ ] 创建 `CisMemoryBackend` 结构体（包装 `MemoryService`）
- [ ] 为 `MemoryService` 实现 `Memory` trait
- [ ] 添加构造函数 `from_memory_service()`

**文件**: `cis-core/src/traits/implementations/memory.rs`

#### Task 1.2.2: 实现 CisTransportBackend
- [ ] 创建 `CisTransportBackend` 结构体
- [ ] 为现有网络层实现 `Transport` trait

**文件**: `cis-core/src/traits/implementations/transport.rs`

#### Task 1.2.3: 实现 CisEncryptionBackend
- [ ] 创建 `CisEncryptionBackend` 结构体
- [ ] 为现有加密层实现 `Encryption` trait

**文件**: `cis-core/src/traits/implementations/encryption.rs`

---

### 1.3 创建 Mock 实现（测试友好）

#### Task 1.3.1: MockMemory
- [ ] 创建基于 `HashMap` 的内存实现
- [ ] 实现 `Memory` trait
- [ ] 添加测试辅助方法

**文件**: `cis-core/src/traits/mock/memory.rs`

#### Task 1.3.2: MockTransport
- [ ] 创建基于 `channel` 的模拟传输
- [ ] 实现 `Transport` trait

**文件**: `cis-core/src/traits/mock/transport.rs`

---

### 1.4 更新 lib.rs 导出

**文件**: `cis-core/src/lib.rs`

- [ ] 添加 `pub mod traits;`
- [ ] 导出核心 traits: `Memory, Transport, Encryption`
- [ ] 导出实现: `CisMemoryBackend, CisTransportBackend`

---

## Phase 2: ZeroClaw 兼容层（Week 3-4）🔥 **高优先**

### 2.1 创建 ZeroClaw 适配器

**目录**: `cis-core/src/zeroclaw/`

#### Task 2.1.1: ZeroClaw Memory 适配器
- [ ] 实现 ZeroClaw `Memory` trait 的 CIS 适配器
- [ ] 映射 ZeroClaw 概念到 CIS 概念
  - ZeroClaw `MemoryCategory` → CIS `MemoryDomain`
  - ZeroClaw `session_id` → CIS `scope_id`

**文件**: `cis-core/src/zeroclaw/memory_adapter.rs`

```rust
// zeroclaw-cis-memory crate (独立 crate 或集成)
pub struct ZeroClawCisMemory {
    inner: Box<dyn Memory>,
}

#[async_trait]
impl zeroclaw::memory::Memory for ZeroClawCisMemory {
    async fn store(&self, key: &str, content: &str, category: MemoryCategory, session_id: Option<&str>) -> anyhow::Result<()> {
        // 映射到 CIS Memory trait
    }
}
```

#### Task 2.1.2: ZeroClaw 配置支持
- [ ] 添加配置解析支持
- [ ] 支持 `[memory.backend = "cis"]` 配置

**配置示例**:
```toml
# zeroclaw-config.toml
[memory]
backend = "cis"  # 使用 CIS 作为后端

[memory.cis]
node_id = "my-workstation"
data_dir = "~/.cis"
enable_p2p = true
enable_encryption = true
```

---

### 2.2 创建 Factory 模式

#### Task 2.2.1: MemoryFactory
- [ ] 创建 `MemoryFactory` trait
- [ ] 实现基于配置的后端创建
- [ ] 支持运行时切换后端

**文件**: `cis-core/src/traits/factory.rs`

```rust
pub struct MemoryFactory;

impl MemoryFactory {
    pub fn create(backend: &str, config: &Config) -> Result<Box<dyn Memory>> {
        match backend {
            "cis" => Ok(Box::new(CisMemoryBackend::new(config)?)),
            "sqlite" => Ok(Box::new(SqliteMemory::new(config)?)),
            "mock" => Ok(Box::new(MockMemory::new())),
            _ => Err(...),
        }
    }
}
```

---

## Phase 3: 测试和文档（Week 5）📝

### 3.1 Trait 单元测试

#### Task 3.1.1: Memory Trait 测试
- [ ] 为 `MockMemory` 编写单元测试
- [ ] 为 `CisMemoryBackend` 编写集成测试
- [ ] 测试 trait object 多态

**文件**: `cis-core/src/traits/tests/memory_tests.rs`

#### Task 3.1.2: Transport Trait 测试
- [ ] 测试网络传输抽象
- [ ] 测试错误处理

---

### 3.2 文档更新

#### Task 3.2.1: Trait 使用指南
- [ ] 创建 `docs/traits-guide.md`
- [ ] 添加如何使用 trait 抽象的示例
- [ ] 添加如何实现自定义后端的指南

#### Task 3.2.2: ZeroClaw 集成文档
- [ ] 创建 `docs/zeroclaw-integration.md`
- [ ] 添加配置示例
- [ ] 添加迁移指南

---

## Phase 4: 清理和优化（Week 6+）🔧 可选

### 4.1 弃用旧 API（向后兼容）

#### Task 4.1.1: 标记旧 API 为 deprecated
- [ ] 保持 `MemoryService` 可用
- [ ] 添加 `#[deprecated]` 注解
- [ ] 提供迁移指南

### 4.2 性能优化

#### Task 4.2.1: 基准测试
- [ ] 对比重构前后性能
- [ ] 优化动态分发开销

---

## 🎯 关键决策点

| 决策 | 建议 | 理由 |
|------|------|------|
| **是否重构？** | ✅ 是 | 长期价值显著 |
| **何时重构？** | 当前 | 代码量相对较小，重构成本低 |
| **重构范围？** | Phase 1-2 | 80% 价值，50% 工作量 |
| **泛型 vs Trait Object？** | 混合 | 性能敏感用泛型，配置灵活用 trait object |

---

## 📊 进度追踪

### Phase 1 进度
- [ ] 1.1 Trait 定义 (0/3)
- [ ] 1.2 CIS 实现 (0/3)
- [ ] 1.3 Mock 实现 (0/2)
- [ ] 1.4 lib.rs 更新 (0/1)

**Phase 1 完成度**: 0% (0/9)

### Phase 2 进度
- [ ] 2.1 ZeroClaw 适配器 (0/2)
- [ ] 2.2 Factory 模式 (0/1)

**Phase 2 完成度**: 0% (0/3)

### Phase 3 进度
- [ ] 3.1 单元测试 (0/2)
- [ ] 3.2 文档更新 (0/2)

**Phase 3 完成度**: 0% (0/4)

---

## 🚀 快速开始

### 最小可行实现（MVP）

**Week 1 目标**: 实现 Memory trait + 基本测试

```bash
# 1. 创建 trait 模块
mkdir -p cis-core/src/traits

# 2. 实现核心 trait
# - traits/memory.rs
# - traits/implementations/cis_memory.rs

# 3. 添加测试
# - traits/mock/memory.rs
# - traits/tests/memory_tests.rs

# 4. 运行测试
cargo test --package cis-core --lib traits
```

---

## 📚 参考文档

- [ZeroClaw 插件开发指南](https://github.com/your-repo/zeroclaw/plugins)
- [Rust async trait 模式](https://rust-lang.github.io/async-book/07_working_with_traits.html)
- [CIS 架构文档](../../architecture.md)

---

## ⚠️ 风险和缓解

| 风险 | 缓解措施 |
|------|---------|
| 动态分发性能开销 | 使用 `Box<dyn>`，开销 <1% |
| API 破坏性变更 | 保持旧 API，标记为 deprecated |
| 编译时间增加 | 使用泛型替代部分动态分发 |
| 测试覆盖率下降 | 重构期间保持测试，新增 Mock 测试 |

---

## ✅ 验收标准

### Phase 1 验收
- [ ] 可以使用 `Box<dyn Memory>` 替代 `MemoryService`
- [ ] 单元测试可以使用 `MockMemory`
- [ ] 所有现有测试通过

### Phase 2 验收
- [ ] ZeroClaw 可以使用 CIS 作为 Memory 后端
- [ ] 配置文件可以切换后端
- [ ] 文档完整，示例可运行

### Phase 3 验收
- [ ] 测试覆盖率 > 75%
- [ ] 文档完整
- [ ] 性能基准测试通过

---

**创建日期**: 2026-02-20
**最后更新**: 2026-02-20
**负责人**: Claude AI
**状态**: 📋 计划中
