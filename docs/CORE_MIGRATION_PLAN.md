# CIS Core 迁移计划（极简版）

## 设计原则

**CIS Core = 私域记忆存储 + P2P 零 Token 通信**

其他一切都是 Skill。

## Core 组件（必须迁移/实现）

### 1. 私域记忆存储

**核心职责**: 硬件绑定的加密 SQLite 存储

```rust
pub struct PrivateMemory {
    db: EncryptedSqlite,
    did: Did,
    fingerprint: HardwareFingerprint,
}
```

**从 AgentFlow 迁移**:
- `db/migration.rs` → Schema 版本管理
- `memory/mod.rs` → 核心存储（移除 project_id，简化）
- 移除: push/pull、云端存储、LLM 辅助

**新增**:
- 硬件密钥派生
- 任务上下文打包

### 2. P2P 通信层

**核心职责**: 零 Token 的节点间任务移交

```rust
pub struct P2PTransport {
    swarm: libp2p::Swarm<CisBehaviour>,
    did: Did,
}

pub struct TaskContext {
    pub task_id: TaskId,
    pub executable: Vec<u8>,
    pub inline_memories: Vec<u8>,
    pub merkle_root: Hash,
}
```

### 3. 节点身份

```rust
pub struct NodeIdentity {
    did: Did,
    mnemonic: Mnemonic,
    fingerprint: HardwareFingerprint,
}
```

## 废弃的 AgentFlow 功能

| 功能 | 原因 | 替代 |
|------|------|------|
| project_id | 单节点无多项目 | namespace（纯组织） |
| push/pull | 违反本地性 | 任务移交 pack |
| WorkerMemory | Master-Worker 废弃 | PrivateMemory |
| LLM 辅助 | Core 不依赖 LLM | Skill 实现 |
| UpdateToken | 私域无冲突 | 无需向量时钟 |

## 迁移执行计划

### Week 1: Core 存储层
- [ ] `PrivateMemory` 结构
- [ ] 硬件密钥派生
- [ ] Schema 迁移框架
- [ ] 复合索引

### Week 2: P2P 通信层
- [ ] libp2p 集成
- [ ] TaskContext 打包/解包
- [ ] 任务移交协议

### Week 3: 节点身份
- [ ] DID 生成
- [ ] 助记词恢复
- [ ] 硬件指纹绑定

### Week 4: Skill 框架
- [ ] Skill 接口定义
- [ ] WASM 加载器
- [ ] 示例 Skill

## 总结

AgentFlow 的 8 个任务在 CIS 中的命运:

| 任务 | CIS Core | CIS Skill | 废弃 |
|------|----------|-----------|------|
| TASK-001 project_id | | | ✅ 废弃 |
| TASK-002 Migration | ✅ 保留 | | |
| TASK-003 DAL Refactor | ✅ 简化 | | |
| TASK-004 Project Discovery | | | ✅ 废弃 |
| TASK-005 Data Migration | ✅ 改造为恢复工具 | | |
| TASK-006 Index Optimization | ✅ 保留 | | |
| TASK-007 Log Level | ✅ 保留 | | |
| TASK-008 Signaling Fix | ✅ 改造为 P2P | | |

**核心原则**: CIS Core 保持极简，功能通过 Skill 涌现。
