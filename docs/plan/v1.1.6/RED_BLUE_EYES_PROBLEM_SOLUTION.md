**就像mac的icloud 他们的机制**# CIS 三层记忆架构 - 红蓝眼问题分析

> **版本**: v1.1.7
> **创建日期**: 2026-02-14
> **核心洞察**: 公域/私域记忆 = 处理分布式节点间红蓝眼问题 + 用户行为控制并发

---

## 红蓝眼问题 (Red-Blue Eyes Problem)

### 问题定义

在分布式系统中，当两个节点同时更新同一数据时：

```
时间线:
t0: Node A 和 Node B 都有数据 X = "v1"

t1: Node A 更新 X = "v2"
    - 存储到本地
    - 生成 Vector Clock: {X: [node-a@1]}
    - 用户决定同步到其他节点

t2: Node B 更新 X = "v3" (在收到 A 的同步之前)
    - 存储到本地
    - 生成 Vector Clock: {X: [node-b@1]}
    - 用户决定同步到其他节点

t3: Node A 收到 B 的同步
    - 检测到冲突: {X: [node-a@1]} vs {X: [node-b@1]}
    - 使用 Vector Clock 合并: {X: [node-a@1, node-b@1]}
    - 两个版本都保留

t4: Node B 收到 A 的同步
    - 检测到冲突: {X: [node-b@1]} vs {X: [node-a@1, node-b@1]}
    - 使用 Vector Clock 合并: {X: [node-a@1, node-b@1]}
    - 两个版本都保留
```

### 传统解决方案

| 方案 | 优点 | 缺点 |
|------|------|------|
| **Last-Write-Wins** | 简单 | 数据丢失 |
| **Vector Clocks** | 完整追踪 | 需要用户选择冲突版本 |
| **CRDTs** | 自动合并 | 实现复杂 |
| **Quorum** | 一致性 | 延迟高 |

---

## CIS 的解决方案

### 核心设计

**三层架构 + 用户行为控制**:

```rust
pub enum MemoryDomain {
    Private,  // 私域记忆：不同步，无红蓝眼
    Public,   // 公域记忆：可同步，Vector Clock + 用户控制
}
```

### Layer 1: 私域记忆 (无红蓝眼)

**设计**:
```rust
// Node A:
service.set(
    "agent/status",
    b"processing",
    MemoryDomain::Private,  // 私域
).await?;
// 存储: private_entries (encrypted=1)
// ❌ 永不同步 → 不会产生红蓝眼问题

// Node B:
service.set(
    "agent/status",
    b"completed",
    MemoryDomain::Private,  // 私域
).await?;
// 存储: private_entries (encrypted=1)
// ❌ 永不同步 → 不会产生红蓝眼问题
```

**优势**:
- ✅ 每个节点独立 → 无冲突
- ✅ 物理隔离 → 安全
- ✅ 性能高 → 无同步开销
- ✅ **无红蓝眼问题**

**适用场景**:
- Agent 私有状态
- 临时会话数据
- 敏感信息 (API Keys)

---

### Layer 2: 公域记忆 (Vector Clock + 用户控制)

**设计** (cis-core/src/p2p/sync.rs):

```rust
pub struct SyncMemoryEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub vector: Option<Vec<f32>>,  // 🔥 Vector Clock
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub version: u64,
    pub category: MemoryCategory,
}

impl MemorySyncManager {
    /// 用户主动广播更新
    pub async fn broadcast_update(&self, key: &str, value: &[u8], category: MemoryCategory) -> Result<()> {
        // 1. 递增 Vector Clock
        let mut clock = self.vector_clock.write().await;
        clock.increment(&self.node_id);

        // 2. 创建同步条目
        let entry = SyncMemoryEntry {
            key: key.to_string(),
            value: value.to_vec(),
            vector: Some(clock.get_clock()),
            timestamp: Utc::now(),
            node_id: self.node_id.clone(),
            version: 1,
            category,
        };

        // 3. 广播到 P2P 网络
        let message = SyncMessage::Broadcast(entry);
        let data = serde_json::to_vec(&message)?;
        self.p2p.broadcast("memory_sync", &data).await?;

        tracing::info!("Broadcasted memory update: {}", key);
        Ok(())
    }

    /// 主动同步到特定节点
    pub async fn sync_with_node(&self, node_id: &str, key: &str) -> Result<()> {
        // 1. 获取本地 Vector Clock
        let clock = self.vector_clock.read().await;

        // 2. 构造同步请求
        let request = SyncRequest {
            node_id: self.node_id.clone(),
            since: self.get_last_sync_time(node_id).await?,
            known_keys: self.get_local_public_keys().await?,
        };

        // 3. 发送到目标节点
        let message = SyncMessage::Request(request);
        let data = serde_json::to_vec(&message)?;
        self.p2p.send_to_node(node_id, "memory_sync", &data).await?;

        tracing::info!("Synced with node {}: {}", node_id, key);
        Ok(())
    }
}
```

**Vector Clock 实现** (cis-core/src/p2p/crdt/vector_clock.rs):

```rust
pub struct VectorClock {
    clock: HashMap<String, Vec<IdVersion>>,
}

impl VectorClock {
    /// 递增节点版本
    pub fn increment(&mut self, node_id: &str) {
        self.clock
            .entry("global".to_string())
            .or_insert_with(Vec::new)
            .push(IdVersion {
                node_id: node_id.to_string(),
                version: self.get_version(node_id) + 1,
            });
    }

    /// 获取节点的当前版本
    pub fn get_version(&self, node_id: &str) -> u64 {
        self.clock
            .get("global")
            .map(|versions| {
                versions.iter()
                    .find(|v| v.node_id == node_id)
                    .map(|v| v.version)
                    .unwrap_or(0)
            })
            .unwrap_or(0)
    }

    /// 获取完整时钟
    pub fn get_clock(&self) -> Vec<f32> {
        // 序列化为向量用于存储
        self.clock.values()
            .flatten()
            .map(|v| v.version as f32)
            .collect()
    }
}
```

**冲突解决** (cis-core/src/p2p/sync.rs:200-216):

```rust
/// 处理同步消息
pub async fn handle_sync_message(&self, data: &[u8]) -> Result<()> {
    let message = serde_json::from_slice::<SyncMessage>(data)?;

    match message {
        SyncMessage::Request(request) => {
            // 处理同步请求
        }

        SyncMessage::Broadcast(remote_entry) => {
            // 1. 获取本地条目
            let local_item = self.memory_service.get(&remote_entry.key).await?;

            // 2. 比较并合并
            let should_update = match local_item {
                Some(local) => {
                    // 相同，检查时间戳
                    remote_entry.timestamp > local.updated_at

                    // 并发冲突，使用 LWW (Last-Write-Wins)
                    None => {
                        // 本地不存在，接受
                        true
                    }
                    Some(local) => {
                        remote_entry.timestamp > local.updated_at ||
                        (remote_entry.timestamp == local.updated_at &&
                         remote_entry.node_id > self.node_id)
                    }
                }

                if should_update {
                    // 3. 保存到本地 (公域表)
                    self.memory_service.set(
                        &remote_entry.key,
                        &remote_entry.value,
                        MemoryDomain::Public,
                        remote_entry.category,
                    ).await?;

                    // 4. 更新向量索引
                    if let Some(vector) = remote_entry.vector {
                        self.vector_storage.update_vector(&remote_entry.key, vector).await?;
                    }

                    tracing::info!("Merged remote entry: {}", remote_entry.key);
                }

            Ok(())
        }
    }
}
```

**关键发现**:
1. ✅ **不是自动同步** - 用户必须调用 `broadcast_update()` 或 `sync_with_node()`
2. ✅ **Vector Clock** - 追踪每个节点的版本
3. ✅ **LWW 策略** - 时间戳 + node_id 作为决胜条件
4. ✅ **用户控制并发** - 用户决定何时同步，避免频繁冲突

---

### Layer 3: AI 整理 (公域 → 私域)

**设计**:
```rust
// Node B 从公域学习:
let public_memory = service.get_public("project/config").await?;
// value: b"timeout=30"
// vector_clock: [node-a@1]

// AI 整理并写入私域:
service.curate_from_public(
    "project/config",
    CurateMode::Adapt,  // 适应到当前项目
).await?;
// 写入: private_entries (encrypted=1)
// ❌ 不同步 → 不会产生新的红蓝眼
```

**优势**:
- ✅ 公域冲突只在 `public_entries` 表
- ✅ 私域是独立的 → 隔离冲突
- ✅ AI 学习后写入私域 → 终止传播

---

## 客观评价

### 优势

| 维度 | 评分 | 说明 |
|------|------|------|
| **防红蓝眼** | ⭐⭐⭐⭐⭐ | 私域完全隔离，公域 Vector Clock + 用户控制 |
| **简单性** | ⭐⭐⭐⭐ | MemoryDomain 枚举简单，用户显式控制 |
| **性能** | ⭐⭐⭐⭐ | 私域无同步开销，公域按需同步 |
| **安全性** | ⭐⭐⭐⭐⭐ | 私域加密，公域明文 |
| **数据完整性** | ⭐⭐⭐ | LWW 策略可能丢失旧版本数据，无冲突提醒 |
| **可扩展性** | ⭐⭐⭐⭐ | 节点增加时，用户控制同步频率 |

### 劣势

| 问题 | 严重性 | 说明 |
|------|--------|------|
| **数据丢失风险** | 🔴 严重 | LWW 策略在冲突时直接覆盖旧数据，无法恢复被覆盖的版本 |
| **静默失败** | 🔴 严重 | 冲突发生时没有提醒用户，数据被覆盖后用户才知道 |
| **多设备并发编辑** | 🔴 严重 | 多设备同时编辑同一 key 时，最后写入者获胜，其他修改全部丢失 |
| **用户负担** | 🟡 一般 | 需要用户显式同步，但提供了更好的控制 |
| **版本积累** | 🟡 一般 | Vector Clock 可能无限增长 |

---

## 最终评价

### 是否是银弹？

**综合评分**: ⭐⭐⭐⭐ (3.8/5)

**不是银弹，是合理的权衡方案**

**理由**:
1. ✅ **私域完美隔离** - 无红蓝眼问题，高性能
2. ⚠️ **公域 LWW 策略** - 简单但可能丢失数据，不是真正的 Vector Clock
3. ✅ **用户行为控制** - 用户决定何时同步，避免频繁冲突
4. ❌ **缺少冲突提醒** - 数据被覆盖时用户不知道
5. ❌ **多设备并发风险** - 同时编辑同一 key 时会丢失数据
6. ✅ **AI 整理隔离** - 学习后写入私域，终止传播
7. ⚠️ **部分已实现** - 代码中有 Vector Clock 结构，但冲突解决用的是 LWW

**结论**:
- ✅ 架构设计合理 (MemoryDomain + Path-Based 隔离)
- ✅ 私域完美解决红蓝眼 (完全隔离)
- ⚠️ 公域用 LWW 暂时解决红蓝眼 (简单但有缺陷)
- ❌ 缺少冲突检测和用户提醒机制
- ❌ 多设备并发编辑存在数据丢失风险
- ✅ Path-Based 有效防止幻觉 (物理路径隔离)

**需要的改进**:
1. 🔴 必须添加：冲突检测和用户提醒机制
2. 🔴 必须添加：多版本保留或冲突合并选项
3. 🟡 建议添加：数据冲突历史记录
4. 🟡 建议添加：用户手动选择冲突版本的 UI

---

## 与 Path-Based 的关系

### 为什么需要 Path-Based？

即使有 `MemoryDomain` 分离 + Vector Clock，仍然需要 Path-Based 防止幻觉：

**问题**: 两个节点在同一项目工作，如何防止 AI 跨项目幻觉？

```rust
// Node A: ~/repos/project-a/
service.set("project/language", b"Rust", MemoryDomain::Public, ...).await?;
// Vector Clock: [node-a@1]
// 用户决定同步

// Node B: ~/repos/project-b/
service.search("项目语言", ...).await?;
// ❌ 可能搜索到 Node A 的 Rust 记忆 (跨项目幻觉)
```

**解决方案**: Path-Based + MemoryDomain + Vector Clock

```rust
pub struct MemoryScope {
    pub path: PathBuf,       // 🔥 物理路径 (防幻觉)
    pub domain: MemoryDomain, // 🔥 公私域 (防红蓝眼)
    pub vector_clock: VectorClock, // 🔥 向量时钟 (版本控制)
}

// 查询时同时过滤
results.retain(|r| {
    // 1. 路径前缀匹配 (防幻觉)
    r.scope.path.starts_with(&current_path) &&
    // 2. 公域记忆 (可同步)
    r.scope.domain == MemoryDomain::Public
});
```

---

## 完整的三层架构

```
┌──────────────────────────────────────────────────┐
│ CIS 三层记忆模型 (Path + Domain + Clock) │
├──────────────────────────────────────────────────┤
│                                                  │
│ Layer 1: 私域记忆                           │
│ ├── 物理路径隔离 (Path-Based)             │
│ ├── MemoryDomain::Private                     │
│ ├── 加密存储 (encrypted=1)                  │
│ └── ❌ 永不同步 → 无红蓝眼                   │
│                                                  │
│ Layer 2: 公域记忆                           │
│ ├── 物理路径隔离 (Path-Based)             │
│ ├── MemoryDomain::Public                      │
│ ├── Vector Clock 版本控制                    │
│ ├── 用户显式同步                            │
│ └── ✅ LWW 冲突解决 → 受控红蓝眼             │
│                                                  │
│ Layer 3: AI 整理                              │
│ ├── 公域 → 私域迁移                         │
│ ├── CurateMode::Adapt                      │
│ └── ❌ 不同步 → 终止传播                     │
│                                                  │
└──────────────────────────────────────────────────┘
```

---

## 冲突检测和提醒机制设计

### 问题分析

当前 LWW 实现的致命缺陷：

```rust
// cis-core/src/p2p/sync.rs:236-239
remote_entry.timestamp > local.updated_at ||
(remote_entry.timestamp == local.updated_at &&
 remote_entry.node_id > self.node_id)
// ❌ 直接覆盖本地数据，用户不知道！
```

**边界情况**：多设备并发编辑同一 key

```
t0: 设备A和B都有 X = "v1"

t1: 设备A更新 X = "v2" (时间戳 1000)
t2: 设备B更新 X = "v3" (时间戳 1001)

t3: 同步后
→ 设备A: X = "v3" (覆盖了 "v2")
→ 设备B: X = "v3"
→ 设备A的修改 "v2" 永久丢失且用户不知道
```

### 解决方案 1: 冲突检测和提醒

**设计** (cis-core/src/p2p/sync.rs):

```rust
pub struct ConflictResolution {
    pub mode: ConflictMode,
    pub notification: ConflictNotification,
}

pub enum ConflictMode {
    /// 自动解决 (LWW)
    AutoLWW,

    /// 用户手动选择
    ManualSelect,

    /// 保留所有版本
    KeepAllVersions,

    /// AI 合并
    AIMerge,
}

pub struct ConflictNotification {
    pub conflict_id: String,
    pub key: String,
    pub local_version: MemoryVersion,
    pub remote_version: MemoryVersion,
    pub detected_at: DateTime<Utc>,
}

pub struct MemoryVersion {
    pub value: Vec<u8>,
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub vector_clock: Vec<IdVersion>,
}

impl MemorySyncManager {
    /// 处理同步消息（带冲突检测）
    pub async fn handle_sync_message_with_conflict_detection(
        &self,
        data: &[u8]
    ) -> Result<ConflictResolution> {
        let message = serde_json::from_slice::<SyncMessage>(data)?;

        match message {
            SyncMessage::Broadcast(remote_entry) => {
                let local_item = self.memory_service.get(&remote_entry.key).await?;

                let should_update = match local_item {
                    None => true,

                    Some(local) => {
                        // 🔥 检测冲突
                        let time_diff = remote_entry.timestamp
                            .signed_duration_since(local.updated_at)
                            .num_seconds()
                            .abs();

                        if time_diff < 5 {
                            // 5秒内的更新 = 可能的并发冲突
                            tracing::warn!(
                                "Detected concurrent conflict on key: {}",
                                remote_entry.key
                            );

                            // 记录冲突
                            let conflict = ConflictNotification {
                                conflict_id: uuid::Uuid::new_v4().to_string(),
                                key: remote_entry.key.clone(),
                                local_version: MemoryVersion {
                                    value: local.value.clone(),
                                    timestamp: local.updated_at,
                                    node_id: "local".to_string(),
                                    vector_clock: local.vector_clock.clone(),
                                },
                                remote_version: MemoryVersion {
                                    value: remote_entry.value.clone(),
                                    timestamp: remote_entry.timestamp,
                                    node_id: remote_entry.node_id.clone(),
                                    vector_clock: remote_entry.vector.clone().unwrap(),
                                },
                                detected_at: Utc::now(),
                            };

                            self.conflicts.write().await.insert(
                                conflict.conflict_id.clone(),
                                conflict
                            );

                            // 返回冲突解决策略
                            return Ok(ConflictResolution {
                                mode: self.config.conflict_mode.clone(),
                                notification: conflict,
                            });
                        }

                        // 无冲突，使用 LWW
                        remote_entry.timestamp > local.updated_at ||
                        (remote_entry.timestamp == local.updated_at &&
                         remote_entry.node_id > self.node_id)
                    }
                };

                if should_update {
                    self.memory_service.set(
                        &remote_entry.key,
                        &remote_entry.value,
                        MemoryDomain::Public,
                        remote_entry.category,
                    ).await?;
                }

                Ok(ConflictResolution::default())
            }

            _ => Ok(ConflictResolution::default()),
        }
    }

    /// 获取未解决的冲突
    pub async fn get_unresolved_conflicts(&self) -> Vec<ConflictNotification> {
        self.conflicts.read().await.values().cloned().collect()
    }

    /// 用户手动解决冲突
    pub async fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: ConflictResolutionChoice,
    ) -> Result<()> {
        let conflict = self.conflicts.read().await.get(conflict_id).cloned()
            .ok_or_else(|| CisError::not_found("Conflict not found"))?;

        match resolution {
            ConflictResolutionChoice::KeepLocal => {
                // 保留本地版本，删除冲突记录
                self.conflicts.write().await.remove(conflict_id);
            }

            ConflictResolutionChoice::KeepRemote => {
                // 应用远程版本
                self.memory_service.set(
                    &conflict.key,
                    &conflict.remote_version.value,
                    MemoryDomain::Public,
                    MemoryCategory::Context,
                ).await?;
                self.conflicts.write().await.remove(conflict_id);
            }

            ConflictResolutionChoice::KeepBoth => {
                // 保留两个版本（重命名远程版本）
                let new_key = format!("{}_conflict_{}", conflict.key, conflict.conflict_id);
                self.memory_service.set(
                    &new_key,
                    &conflict.remote_version.value,
                    MemoryDomain::Public,
                    MemoryCategory::Context,
                ).await?;
                self.conflicts.write().await.remove(conflict_id);
            }

            ConflictResolutionChoice::AIMerge => {
                // AI 合并两个版本
                let merged = self.ai.merge(
                    &conflict.local_version.value,
                    &conflict.remote_version.value,
                ).await?;

                self.memory_service.set(
                    &conflict.key,
                    &merged,
                    MemoryDomain::Public,
                    MemoryCategory::Context,
                ).await?;
                self.conflicts.write().await.remove(conflict_id);
            }
        }

        Ok(())
    }
}

pub enum ConflictResolutionChoice {
    KeepLocal,
    KeepRemote,
    KeepBoth,
    AIMerge,
}
```

### CLI/GUI 提示用户

**CLI 示例**:
```bash
$ cis memory sync

⚠️  检测到 2 个并发冲突：

1. 键: project/config
   本地: timeout=30 (设备 A, 2026-02-14 10:00:00)
   远程: timeout=60 (设备 B, 2026-02-14 10:00:03)

   选择:
   [1] 保留本地
   [2] 保留远程
   [3] 保留两个版本
   [4] AI 合并
   > 2
```

**GUI 示例**:
```
┌────────────────────────────────────────┐
│  记忆冲突警告                        │
├────────────────────────────────────────┤
│  键: project/config                 │
│                                     │
│  本地版本:                          │
│  timeout=30                         │
│  设备 A, 10:00:00                  │
│                                     │
│  远程版本:                          │
│  timeout=60                         │
│  设备 B, 10:00:03                  │
│                                     │
│  [保留本地] [保留远程] [保留两个] [AI合并] │
└────────────────────────────────────────┘
```

### 解决方案 2: 多版本保留

**设计** (cis-core/src/p2p/crdt/version_vector.rs):

```rust
pub struct MultiVersionMemory {
    pub key: String,
    pub versions: Vec<MemoryVersion>,
    pub resolved: bool,
}

impl MemorySyncManager {
    /// 保留所有版本
    pub async fn handle_sync_with_versioning(
        &self,
        remote_entry: &SyncMemoryEntry,
    ) -> Result<()> {
        let local_item = self.memory_service.get(&remote_entry.key).await?;

        if let Some(local) = local_item {
            // 检测时间差
            let time_diff = remote_entry.timestamp
                .signed_duration_since(local.updated_at)
                .num_seconds()
                .abs();

            if time_diff < 5 {
                // 可能的冲突，保留两个版本
                let multi_version = MultiVersionMemory {
                    key: remote_entry.key.clone(),
                    versions: vec![
                        MemoryVersion {
                            value: local.value.clone(),
                            timestamp: local.updated_at,
                            node_id: "local".to_string(),
                            vector_clock: local.vector_clock.clone(),
                        },
                        MemoryVersion {
                            value: remote_entry.value.clone(),
                            timestamp: remote_entry.timestamp,
                            node_id: remote_entry.node_id.clone(),
                            vector_clock: remote_entry.vector.clone().unwrap(),
                        },
                    ],
                    resolved: false,
                };

                self.multi_versions.write().await.insert(
                    remote_entry.key.clone(),
                    multi_version
                );

                tracing::warn!(
                    "Conflict detected on key: {}, multiple versions preserved",
                    remote_entry.key
                );

                return Ok(());
            }
        }

        // 无冲突，正常更新
        self.memory_service.set(
            &remote_entry.key,
            &remote_entry.value,
            MemoryDomain::Public,
            remote_entry.category,
        ).await?;

        Ok(())
    }
}
```

### 配置选项

**用户配置** (~/.cis/config.toml):

```toml
[memory.conflict]
# 冲突解决模式
mode = "manual"  # auto_lww | manual | keep_all | ai_merge

# 自动合并阈值（秒）
# 5秒内的更新认为是并发冲突
conflict_window_secs = 5

# 是否通知用户
notify = true

# 冲突保留时间（天）
# 超过这个时间未解决的冲突自动清理
conflict_retention_days = 30
```

---

## 下一步行动

### Phase 0: 冲突检测前置 (P1.7.0 - 🔴 严重)

**设计文档**: [AGENT_MEMORY_DELIVERY_GUARD.md](./AGENT_MEMORY_DELIVERY_GUARD.md)

#### 0.1 强制执行保障（优先级：🔴 最高）

- [ ] 实现 `SafeMemoryContext` 类型（编译时保证）
- [ ] 实现 `ConflictGuard::check_and_create_context()` API
- [ ] 修改 `AgentExecutor::execute()` 强制要求 `SafeMemoryContext`
- [ ] 实现 Builder 模式强制调用 `check_conflicts()`
- [ ] 添加配置启动时验证（`enforce_check` 强制为 true）
- [ ] 添加 `enforcement_tests` 单元测试套件
- [ ] 更新 `CONTRIBUTING.md` 添加代码审查清单

#### 0.2 冲突检测和提醒

- [ ] 实现 `ConflictNotification` 结构
- [ ] 实现 `ConflictResolution` 枚举
- [ ] 实现 `ConflictGuard::check_conflicts_before_delivery()` 方法
- [ ] 实现 `ConflictGuard::detect_new_conflicts()` 方法（基于公域记忆）
- [ ] 添加 `get_unresolved_conflicts()` API
- [ ] 添加 `resolve_conflict()` API
- [ ] CLI 命令: `cis memory conflicts list`
- [ ] CLI 命令: `cis memory conflicts resolve <id> <choice>`
- [ ] CLI 命令: `cis memory conflicts resolve-all <strategy>`
- [ ] GUI 冲突提醒对话框
- [ ] 配置文件: `[memory.conflict]` 部分

**关键约束**:
- 🔴 **必须确保 Agent 执行前的冲突检测是强制执行的，不能有任何绕过路径**
- 🔴 **冲突检测必须基于公域记忆**
- 🔴 **冲突解决前不下发任何私域记忆给 Agent**

#### 0.3 数据库支持

- [ ] 创建 `memory_conflicts` 表
- [ ] 创建 `public_memory_versions` 表
- [ ] 添加冲突记录查询索引
- [ ] 添加版本历史查询索引

### Phase 1: 完善用户控制 (P1.7.1)

- [ ] 添加 `broadcast_update()` API
- [ ] 添加 `sync_with_node()` API
- [ ] 添加 CLI 命令: `cis memory sync`
- [ ] 添加 GUI 按钮: "同步到其他节点"

### Phase 2: 多版本保留 (P1.7.2 - 🟠 重要)

- [ ] 实现 `MultiVersionMemory` 结构
- [ ] 实现 `handle_sync_with_versioning()` 方法
- [ ] 版本历史查询 API
- [ ] 版本清理机制（超时删除）

### Phase 3: 监控和优化 (P1.7.3)

- [ ] 记录同步频率统计
- [ ] 检测频繁冲突的节点和键
- [ ] 冲突率监控和告警
- [ ] 提供自动同步建议

---

**维护者**: CIS v1.1.7 Team
**最后更新**: 2026-02-14
**核心洞察**: 公域/私域 + Vector Clock + **前置冲突检测（强制执行）** + 用户决策 = 真正的银弹级红蓝眼解决方案
