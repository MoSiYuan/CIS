# AgentFlow → CIS 迁移任务分析

## 设计哲学对比

| 维度 | AgentFlow | CIS (独联体) | 兼容性 |
|------|-----------|--------------|--------|
| **架构** | Master-Worker 集中式 | P2P 去中心化 | ❌ 冲突 |
| **记忆存储** | SQLite + 可选云端同步 | 本地 SQLite 强制加密 | ⚠️ 部分兼容 |
| **project_id** | 多项目数据隔离 | 单节点独立，无多项目概念 | ⚠️ 语义需调整 |
| **跨节点通信** | gRPC/HTTP API | gRPC over QUIC P2P | ⚠️ 协议需调整 |
| **硬件绑定** | 无 | DID + 硬件指纹强制绑定 | ❌ 缺失 |
| **私域/公域** | 逻辑隔离 | 物理隔离 + 加密 | ⚠️ 增强需求 |
| **容器化** | 支持 | 明确禁止 Docker | ❌ 冲突 |

## 迁移任务适配性分析

### ✅ 完全适用

#### TASK-002: Database Migration Framework
- **适用性**: ✅ 高
- **原因**: CIS 使用 SQLite，需要 schema 版本管理
- **调整**: 需添加加密密钥派生相关的迁移支持
- **文件**: `db/migration.rs` → `cis-core/src/storage/migration.rs`

#### TASK-006: Index Optimization  
- **适用性**: ✅ 高
- **原因**: CIS 同样需要高效查询本地记忆
- **调整**: 索引字段从 `project_id` 改为 `domain` 或 `namespace`
- **文件**: `migrations/004_add_composite_indexes.sql` → 适配 CIS schema

### ⚠️ 需要重大调整

#### TASK-001/003/004/005: project_id 相关功能
- **适用性**: ⚠️ 中
- **核心问题**: CIS 没有"多项目"概念，每个节点是独立的
- **建议改造**:
  ```
  project_id → domain/namespace（应用域概念）
  ```
- **CIS 语义**: 
  - `domain` = 应用/业务域（如 "system", "user", "skill"）
  - 用于单节点内的数据分类，非跨节点隔离

### ❌ 不适用（需废弃）

#### AgentFlow Push/Pull 系统
- **不适用原因**: 
  - CIS 禁止云端记忆同步（违反本地性原则）
  - CIS 使用"内联打包"替代远程同步
- **替代方案**: 
  - 任务移交时打包上下文 (`task_context` inline packing)
  - 公域只同步 Merkle DAG 元数据

#### AgentFlow Master-Worker 模式
- **不适用原因**:
  - CIS 是无中心 P2P 架构
  - 没有 Master 节点概念
- **替代方案**:
  - 守望者模式（Watcher）用于离线告警
  - 任务调度由本地 scheduler 决策

## 优化后的 CIS 迁移计划

### Phase 1: 存储层适配（高优先级）

```rust
// CIS 存储核心 - 替换 AgentFlow MemoryCore
pub struct LocalStorage {
    pool: SqlitePool,
    cipher: ChaCha20Poly1305,  // CIS 新增: 硬件密钥派生加密
    node_did: Did,             // CIS 新增: 节点身份
}
```

1. **Schema Migration 框架** (from TASK-002)
   - 保留 MigrationManager
   - 添加加密表结构迁移
   
2. **复合索引** (from TASK-006)
   - 改为 `(domain, namespace, key)`
   - 添加 `(task_id, created_at)` 任务查询索引

### Phase 2: 域分类系统（中优先级）

将 project_id 改造为 domain 分类：

```rust
/// CIS 记忆域分类（替代 project_id）
pub enum MemoryDomain {
    System,     // 系统配置、节点状态
    User,       // 用户偏好、历史
    Skill,      // Skill 扩展数据
    Task,       // 任务执行上下文
    Session,    // 临时会话数据
}

pub struct MemoryEntry {
    // ... 其他字段
    domain: MemoryDomain,      // 替代 project_id
    namespace: String,         // 保留：细分子空间
    encrypted: bool,           // CIS 新增：是否加密存储
}
```

### Phase 3: 数据迁移工具（低优先级）

改造 AgentFlow 的 migrate-data 工具：

```bash
# CIS 数据迁移场景
# 场景1: 硬件迁移（助记词恢复）
cis-node migrate --from-backup backup.db --mnemonic "word1 word2..."

# 场景2: 域重分类（原 project_id 映射到 domain）
cis-node migrate --remap-domain --mapping old_project:new_domain
```

### Phase 4: 项目发现 → 节点初始化（可选）

AgentFlow 的 `ProjectContext::discover()` 改造为 CIS `NodeInit`：

```rust
/// CIS 节点初始化（替代 ProjectContext）
pub struct NodeInit {
    pub mnemonic: Mnemonic,           // 助记词（硬件迁移用）
    pub machine_fingerprint: String,  // 硬件指纹
    pub work_dir: PathBuf,            // 工作目录
}

impl NodeInit {
    /// 从环境初始化节点
    pub fn from_env() -> Result<Self> {
        // 1. 读取 CIS_MNEMONIC 环境变量或生成新的
        // 2. 计算硬件指纹
        // 3. 派生 DID
    }
}
```

## 代码迁移映射

| AgentFlow 文件 | CIS 目标 | 状态 |
|----------------|----------|------|
| `db/migration.rs` | `cis-core/src/storage/migration.rs` | ✅ 直接迁移 |
| `db/data_migration.rs` | `cis-core/src/storage/recovery.rs` | ⚠️ 改造为助记词恢复 |
| `memory/mod.rs` | `cis-core/src/storage/local.rs` | ⚠️ 移除 push/pull，添加加密 |
| `project.rs` | `cis-core/src/node/init.rs` | ⚠️ 改造为节点初始化 |
| `types.rs` | `cis-core/src/types.rs` | ⚠️ 已存在，需合并 |
| `push/*.rs` | ❌ 废弃 | 用 P2P 内联打包替代 |

## 关键决策

### 决策 1: project_id 的命运
- **建议**: 废弃 project_id，改用 domain 枚举
- **理由**: CIS 单节点无多项目需求，domain 更适合分类

### 决策 2: 数据迁移工具的定位
- **建议**: 从"逻辑迁移"改为"物理恢复"
- **场景**: 硬件损毁后的助记词恢复，而非改 project_id

### 决策 3: 跨节点记忆共享
- **建议**: 完全废弃 push/pull
- **替代**: 任务移交时内联打包（`task_context` 包含所需记忆）

## 执行建议

1. **本周**: 迁移 Migration Framework + Index Optimization
2. **下周**: 实现 domain 系统替换 project_id
3. **第三周**: 改造数据迁移工具为恢复工具
4. **第四周**: 集成测试，验证硬件迁移流程

---

**结论**: AgentFlow 的 8 个任务中，约 40% 可直接迁移，40% 需改造，20% 需废弃。核心是理解 CIS "硬件锚定、本地优先、P2P 无中心"的设计哲学。
