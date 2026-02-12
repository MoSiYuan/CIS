# MemoryService 拆分重构计划

> **版本**: v1.1.6
> **目标**: 将 MemoryService (1100+ 行) 拆分为职责单一的小模块
> **日期**: 2026-02-12

---

## 重构目标

将当前庞大的 `MemoryService` 拆分为更小、职责单一的模块，提高代码可维护性和可测试性。

---

## 当前问题

### 代码规模
- **service.rs**: 1144 行
- **职责过多**: GET、SET、SEARCH、SYNC、加密、命名空间管理
- **测试困难**: 单一文件包含所有功能，难以单独测试

### 依赖关系
```
MemoryService
├── MemoryDb (数据库)
├── VectorStorage (向量检索)
├── MemoryEncryption (加密)
└── 命名空间管理
```

---

## 目标架构

```
cis-core/src/memory/
├── mod.rs                    # 模块导出
├── service.rs                # 主服务（精简，~200 行）
├── refactor_plan.md          # 本文档
├── encryption.rs             # 加密服务（已存在）
└── ops/                     # 操作模块目录
    ├── mod.rs               # 操作模块导出
    ├── get.rs               # GET 操作（~150 行）
    ├── set.rs               # SET 操作（~150 行）
    ├── search.rs            # 搜索操作（~200 行）
    └── sync.rs              # 同步操作（~150 行）
```

---

## 模块职责

### 1. service.rs（精简后的主服务）
**职责**:
- 服务初始化和配置
- 命名空间管理
- 操作模块协调
- 公共 API 暴露

**依赖**:
- `ops::*` - 所有操作模块
- `MemoryEncryption` - 加密
- `VectorStorage` - 向量存储
- `MemoryDb` - 数据库

**精简目标**:
- 从 1144 行减少到 ~200 行
- 只保留配置和协调逻辑
- 将具体操作委托给 ops 模块

### 2. ops/get.rs
**职责**:
- 单个记忆读取
- 批量记忆读取
- 私域/公域记忆解密

**依赖**:
- `MemoryDb` - 数据库读取
- `MemoryEncryption` - 解密
- `VectorStorage` - 向量索引更新

**提取方法**:
```rust
pub async fn get(&self, key: &str) -> Result<Option<MemoryItem>>
pub async fn get_with_domain(&self, key: &str, domain: MemoryDomain) -> Result<Option<MemoryItem>>
pub async fn batch_get(&self, keys: &[String]) -> Result<Vec<Option<MemoryItem>>>
pub async fn get_private(&self, key: &str) -> Result<Option<MemoryItem>>
pub async fn get_public(&self, key: &str) -> Result<Option<MemoryItem>>
```

### 3. ops/set.rs
**职责**:
- 单个记忆存储
- 批量记忆存储
- 私域记忆加密
- 公域记忆标记

**依赖**:
- `MemoryDb` - 数据库写入
- `MemoryEncryption` - 加密
- `VectorStorage` - 向量索引更新

**提取方法**:
```rust
pub async fn set(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>
pub async fn set_with_domain(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>
pub async fn batch_set(&self, items: Vec<(String, Vec<u8>, MemoryDomain, MemoryCategory)>) -> Result<()>
pub async fn set_private(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()>
pub async fn set_public(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()>
pub async fn set_with_embedding(&self, key: &str, value: &[u8], domain: MemoryDomain, category: MemoryCategory) -> Result<()>
```

### 4. ops/search.rs
**职责**:
- 关键词搜索
- 语义搜索
- 向量相似度检索
- 搜索结果过滤和排序

**依赖**:
- `VectorStorage` - 向量搜索
- `MemoryDb` - 完整条目获取
- `MemoryEncryption` - 解密

**提取方法**:
```rust
pub async fn search(&self, query: &str, options: SearchOptions) -> Result<Vec<MemoryItem>>
pub async fn semantic_search(&self, query: &str, limit: usize, threshold: f32) -> Result<Vec<MemorySearchResult>>
pub async fn list_keys(&self, domain: Option<MemoryDomain>) -> Result<Vec<String>>
pub async fn list_with_filter(&self, filter: MemoryFilter) -> Result<Vec<MemoryItem>>
```

### 5. ops/sync.rs
**职责**:
- P2P 同步标记管理
- 公域记忆导出/导入
- 同步状态管理

**依赖**:
- `MemoryDb` - 同步标记操作

**提取方法**:
```rust
pub async fn get_pending_sync(&self, limit: usize) -> Result<Vec<SyncMarker>>
pub async fn mark_synced(&self, key: &str) -> Result<()>
pub async fn export_public(&self, since: i64) -> Result<Vec<MemoryItem>>
pub async fn import_public(&self, items: Vec<MemoryItem>) -> Result<()>
pub async fn on_sync_complete(&self, key: &str, peer_id: &str) -> Result<()>
```

---

## 实施计划

### Phase 1: 创建操作模块（并行）

#### P1-6.1: 设计拆分方案 (0.5 天) ✅
- [x] 创建 refactor_plan.md
- [x] 定义模块结构
- [x] 定义各模块职责和接口

#### P1-6.2: 提取 GET 操作（1 天）
- [ ] 创建 `ops/mod.rs`
- [ ] 创建 `ops/get.rs`
- [ ] 提取所有 get 相关方法
- [ ] 添加单元测试

#### P1-6.3: 提取 SET 操作（1 天）
- [ ] 创建 `ops/set.rs`
- [ ] 提取所有 set 相关方法
- [ ] 添加单元测试

#### P1-6.4: 提取搜索操作（1 天）
- [ ] 创建 `ops/search.rs`
- [ ] 提取所有搜索相关方法
- [ ] 添加单元测试

#### P1-6.5: 提取同步操作（0.5 天）
- [ ] 创建 `ops/sync.rs`
- [ ] 提取所有同步相关方法
- [ ] 添加单元测试

### Phase 2: 重构主服务（2 天）

#### P1-6.6: 精简 service.rs
- [ ] 删除已提取的方法
- [ ] 使用 ops 模块重构公共 API
- [ ] 保持 API 兼容性
- [ ] 添加集成测试

### Phase 3: 验证和测试（1 天）

- [ ] 运行所有单元测试
- [ ] 运行集成测试
- [ ] 性能基准测试
- [ ] 代码审查

---

## 实现细节

### 共享状态

所有操作模块需要访问以下共享状态：

```rust
pub struct MemoryServiceState {
    pub memory_db: Arc<tokio::sync::Mutex<MemoryDb>>,
    pub vector_storage: Arc<VectorStorage>,
    pub encryption: Option<MemoryEncryption>,
    pub node_id: String,
    pub namespace: Option<String>,
}
```

每个操作模块将持有 `Arc<MemoryServiceState>`。

### 命名键管理

```rust
impl MemoryServiceState {
    fn full_key(&self, key: &str) -> String {
        match &self.namespace {
            Some(ns) => format!("{}/{}", ns, key),
            None => key.to_string(),
        }
    }
}
```

### 向量索引更新

```rust
// 后台更新向量索引（共享实现）
fn spawn_index_update(&self, key: &str, value: &[u8], category: &MemoryCategory) {
    // 实现...
}
```

---

## 公共 API 兼容性

重构后的 `MemoryService` 必须保持以下公共 API 不变：

```rust
impl MemoryService {
    // 构造函数
    pub fn new(...) -> Result<Self>
    pub fn open_default(...) -> Result<Self>
    pub fn with_encryption(...) -> Self
    pub fn with_namespace(...) -> Self

    // 核心操作
    pub async fn set(...) -> Result<()>
    pub async fn get(...) -> Result<Option<MemoryItem>>
    pub async fn delete(...) -> Result<bool>

    // 搜索操作
    pub async fn search(...) -> Result<Vec<MemoryItem>>
    pub async fn semantic_search(...) -> Result<Vec<MemorySearchResult>>
    pub async fn list_keys(...) -> Result<Vec<String>>

    // 同步操作
    pub async fn get_pending_sync(...) -> Result<Vec<SyncMarker>>
    pub async fn mark_synced(...) -> Result<()>
    pub async fn export_public(...) -> Result<Vec<MemoryItem>>
    pub async fn import_public(...) -> Result<()>

    // 工具方法
    pub fn node_id(&self) -> &str
    pub fn namespace(&self) -> Option<&str>
    pub fn is_encrypted(&self) -> bool
    pub async fn close(...) -> Result<()>
}
```

---

## 测试策略

### 单元测试

每个 ops 模块都需要独立的单元测试：

```rust
// ops/get.rs 测试
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_get_private() { }
    #[tokio::test]
    async fn test_get_public() { }
    #[tokio::test]
    async fn test_get_with_encryption() { }
}

// ops/set.rs 测试
// ops/search.rs 测试
// ops/sync.rs 测试
```

### 集成测试

在 `service.rs` 中保留集成测试，确保 API 兼容性：

```rust
#[tokio::test]
async fn test_full_workflow() {
    // 设置 -> 读取 -> 搜索 -> 删除
}

#[tokio::test]
async fn test_api_compatibility() {
    // 确保所有旧 API 仍然可用
}
```

---

## 风险与缓解

### 风险 1: 循环依赖
**风险**: ops 模块可能与主模块产生循环依赖

**缓解**:
- 所有共享数据结构放在 `service.rs` 或 `mod.rs`
- ops 模块只依赖基础类型和 trait

### 风险 2: 性能下降
**风险**: 多层间接调用可能影响性能

**缓解**:
- 使用 `Arc` 避免克隆
- 内联关键路径方法
- 性能基准测试验证

### 风险 3: API 不兼容
**风险**: 重构可能导致现有代码编译失败

**缓解**:
- 保持所有公共 API 不变
- 添加编译测试确保兼容性
- 分阶段迁移

---

## 验收标准

- [ ] `service.rs` 减少到 200 行以内
- [ ] 所有 ops 模块独立编译
- [ ] 所有公共 API 保持兼容
- [ ] 单元测试覆盖率 > 80%
- [ ] 集成测试全部通过
- [ ] 性能基准测试无明显下降

---

## 后续优化

1. **引入 trait 抽象**: 将 ops 模块抽象为 trait
2. **依赖注入**: 支持自定义 ops 实现
3. **性能监控**: 添加每个操作的性能指标
4. **批量操作**: 优化批量 set/get 性能

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: Team G
