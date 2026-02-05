# CIS-DAG 触发机制与并发控制设计

> 解决多 Agent 环境下的任务争抢与编辑冲突问题

---

## 1. 当前触发方式

### 1.1 人工触发（CLI）
```bash
# 用户手动触发，单用户场景无并发问题
cis dag run simple.json
cis dag run test_pipeline.toml
```

### 1.2 API 触发（Agent）
```
GLM Agent → POST /api/v1/dag/publish → 等待确认 → 用户确认 → 执行
```

### 1.3 Matrix Room 触发（分布式）
```
Room 消息 "!dag publish" → 所有节点接收 → 本地决策是否执行
```

---

## 2. 多 Agent 场景问题分析

### 2.1 场景 A：多 GLM 推送同一 DAG
```
Agent-1 ──→ POST /publish (dag_id: backup_daily)
              ↓
Agent-2 ──→ POST /publish (dag_id: backup_daily)  ← 重复！
              ↓
Agent-3 ──→ GET /status (询问状态)
```
**问题**: 同一 DAG 被多次创建，浪费资源

### 2.2 场景 B：任务争抢执行
```
Node-A ──→ mark_running("task-1")  ← 同时！
Node-B ──→ mark_running("task-1")  ← 冲突！
```
**问题**: 同一任务被多个节点同时执行

### 2.3 场景 C：状态更新竞争
```
Node-A: task completed → update status → write DB
Node-B: task failed    → update status → write DB  ← 覆盖！
```
**问题**: 最终结果不一致

### 2.4 场景 D：确认阶段并发
```
User-GUI ──→ confirm dag-1  ← 同时点击
GLM-API ──→ confirm dag-1   ← 重复请求
```
**问题**: DAG 被重复发布到 Room

---

## 3. 解决方案设计

### 3.1 核心原则

| 原则 | 说明 |
|------|------|
| **Single Source of Truth** | SQLite 数据库作为唯一状态源 |
| **乐观并发控制** | 版本号机制检测冲突 |
| **任务认领制** | 执行前先获取独占锁 |
| **幂等设计** | 同一操作多次执行结果一致 |

### 3.2 DAG 去重机制

#### DAG ID 唯一性约束
```rust
// 数据库层面约束
CREATE TABLE dag_runs (
    dag_id TEXT PRIMARY KEY,  -- 唯一约束
    ...
);

// 插入时冲突处理
INSERT OR IGNORE INTO dag_runs (...)  -- 已存在则跳过
```

#### DAG 内容哈希
```rust
pub struct DagDefinition {
    pub dag_id: String,
    pub content_hash: String,  // SHA256(tasks_json)
    ...
}

// 同一内容不重复创建
if existing.content_hash == new.content_hash {
    return Err(DagError::DuplicateContent);
}
```

#### GLM 场景处理
```
Agent-1: POST /publish (dag_id: backup, tasks: [...])
    ↓
Server: 检查 dag_id 存在？
    ├── 不存在 → 创建 pending DAG → 返回 success
    └── 存在且内容相同 → 返回 AlreadyExists
    └── 存在但内容不同 → 返回 Conflict (需修改 dag_id)
```

### 3.3 任务执行锁（Task Claim）

#### 数据库层乐观锁
```rust
// 任务表增加版本号
CREATE TABLE tasks (
    task_id TEXT PRIMARY KEY,
    status TEXT NOT NULL,
    version INTEGER DEFAULT 1,  -- 乐观锁版本
    executor_node TEXT,          -- 执行节点标识
    claimed_at TIMESTAMP,
    ...
);

// 认领任务
UPDATE tasks 
SET status = 'Running', 
    version = version + 1,
    executor_node = 'node-abc123',
    claimed_at = datetime('now')
WHERE task_id = ?1 
  AND status = 'Pending'      -- 只能认领 Pending 状态
  AND version = ?2;           -- 版本号匹配

// 检查更新结果
if rows_affected == 0 {
    return Err("Task already claimed or version conflict");
}
```

#### 执行节点标识
```rust
pub struct NodeIdentity {
    did: String,           // did:cis:node-1:abc123
    session_id: String,    // 本次启动唯一ID
    heartbeat_at: Instant,
}

// 任务执行时标记归属
pub struct TaskExecution {
    task_id: String,
    executor: NodeIdentity,
    started_at: DateTime,
    lease_expires: DateTime,  // 租约过期时间（防止死锁）
}
```

#### 租约续期机制
```rust
// 长时间任务需要续期
async fn extend_lease(task_id: &str) -> Result<()> {
    UPDATE tasks 
    SET lease_expires = datetime('now', '+5 minutes')
    WHERE task_id = ?1 AND executor_node = ?2;
}

// 租约过期后，其他节点可以抢占
async fn claim_expired_task(task_id: &str) -> Result<()> {
    UPDATE tasks 
    SET executor_node = ?1, lease_expires = datetime('now', '+5 minutes')
    WHERE task_id = ?2 
      AND datetime('now') > lease_expires;  -- 租约已过期
}
```

### 3.4 状态更新冲突解决

#### 状态机约束
```rust
pub enum TaskStatus {
    Pending,      // 初始状态
    Running,      // 被认领执行
    Completed,    // 成功完成（终态）
    Failed,       // 执行失败（终态）
    Cancelled,    // 被取消（终态）
}

// 合法状态转换
impl TaskStatus {
    pub fn can_transition_to(&self, new: TaskStatus) -> bool {
        match (self, new) {
            (Pending, Running) => true,
            (Running, Completed) => true,
            (Running, Failed) => true,
            (Running, Cancelled) => true,
            // 终态不可变
            (Completed, _) => false,
            (Failed, _) => false,
            (Cancelled, _) => false,
            _ => false,
        }
    }
}

// 更新时检查
UPDATE tasks 
SET status = 'Completed', version = version + 1
WHERE task_id = ?1 
  AND status = 'Running'      -- 只能从 Running → Completed
  AND version = ?2;
```

#### 冲突检测与重试
```rust
pub struct UpdateResult {
    pub success: bool,
    pub current_version: i64,
    pub current_status: TaskStatus,
}

// 客户端重试逻辑
loop {
    let task = load_task(task_id)?;
    let result = try_update(task_id, new_status, task.version);
    
    match result {
        Ok(_) => break,  // 成功
        Err(Conflict { current_version, current_status }) => {
            if current_status.is_terminal() {
                // 终态，无需更新
                return Ok(());
            }
            // 重试
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }
    }
}
```

### 3.5 确认阶段幂等性

#### 确认令牌（Idempotency Key）
```rust
pub struct DagConfirmation {
    dag_id: String,
    confirmed_by: String,        // User DID
    idempotency_key: String,     // UUID (防止重复提交)
    confirmed_at: DateTime,
}

// 数据表唯一约束
CREATE TABLE dag_confirmations (
    idempotency_key TEXT PRIMARY KEY,
    dag_id TEXT NOT NULL,
    ...
);

// 确认请求
POST /api/v1/dag/{dag_id}/confirm
Header: X-Idempotency-Key: uuid-v4

// 服务端处理
INSERT OR IGNORE INTO dag_confirmations (...)  -- 重复key忽略
```

#### 客户端去重
```javascript
// GLM 端维护已确认集合
const confirmedDags = new Set();

function confirmDag(dagId) {
    if (confirmedDags.has(dagId)) {
        return { alreadyConfirmed: true };
    }
    
    const key = generateUUID();
    const result = await fetch(`/api/v1/dag/${dagId}/confirm`, {
        headers: { 'X-Idempotency-Key': key }
    });
    
    if (result.ok) {
        confirmedDags.add(dagId);  // 本地标记
    }
    return result;
}
```

### 3.6 Matrix Room 分布式协调

#### 消息顺序保证
```rust
// 使用 Matrix 的 event_id 作为顺序依据
pub struct DagRoomMessage {
    event_id: String,           // Matrix 全局唯一
    sender_did: String,
    dag_payload: DagPayload,
    timestamp: i64,
}

// 去重：已处理的 event_id 不重复处理
CREATE TABLE processed_events (
    event_id TEXT PRIMARY KEY,
    processed_at TIMESTAMP
);
```

#### 主节点选举（可选）
```rust
// 同一 Room 内，只有一个节点负责任务调度
pub struct RoomCoordinator {
    room_id: String,
    leader_did: String,         // 当前主节点
    lease_expires: DateTime,
}

// 租约竞争
fn try_become_leader(node_did: &str) -> Result<bool> {
    UPDATE room_coordinators 
    SET leader_did = ?1, lease_expires = now() + 30s
    WHERE room_id = ?2 
      AND (leader_did IS NULL OR lease_expires < now());
}
```

---

## 4. API 变更

### 4.1 新增错误码
```rust
pub enum DagApiError {
    #[error("DAG already exists: {0}")]
    AlreadyExists(String),
    
    #[error("DAG content conflict, different from existing")]
    ContentConflict,
    
    #[error("Task already claimed by {0}")]
    AlreadyClaimed(String),
    
    #[error("Version conflict, current version: {0}")]
    VersionConflict(i64),
    
    #[error("Idempotency key already used")]
    DuplicateIdempotencyKey,
    
    #[error("Task lease expired, can be reclaimed")]
    LeaseExpired,
}
```

### 4.2 响应格式
```json
{
  "success": false,
  "error": {
    "code": "AlreadyClaimed",
    "message": "Task already claimed by node-abc123",
    "details": {
      "task_id": "task-xyz",
      "executor": "node-abc123",
      "claimed_at": "2026-02-04T10:00:00Z"
    }
  }
}
```

---

## 5. GLM Agent 端最佳实践

### 5.1 推送 DAG 前检查
```python
async def publish_dag(dag_id, tasks):
    # 1. 先查询是否已存在
    existing = await query_dag_status(dag_id)
    if existing.status != "not_found":
        if existing.tasks == tasks:
            return {"status": "already_exists", "dag_id": dag_id}
        else:
            # 内容不同，使用新ID
            dag_id = f"{dag_id}-{timestamp}"
    
    # 2. 创建新 DAG
    return await api.post("/api/v1/dag/publish", {...})
```

### 5.2 轮询状态优化
```python
# 指数退避避免频繁轮询
async def poll_status(dag_id, max_wait=300):
    delay = 1
    for _ in range(max_wait):
        status = await query_dag_status(dag_id)
        if status in ["completed", "failed"]:
            return status
        await asyncio.sleep(delay)
        delay = min(delay * 2, 30)  # 最大30秒间隔
```

### 5.3 错误处理
```python
try:
    result = await confirm_dag(dag_id)
except ApiError as e:
    if e.code == "DuplicateIdempotencyKey":
        # 已确认过，视为成功
        return {"success": True}
    raise
```

---

## 6. 实现优先级

| 优先级 | 功能 | 影响 | 工作量 |
|--------|------|------|--------|
| P0 | DAG ID 唯一约束 | 防止重复创建 | 1h |
| P0 | 乐观锁版本控制 | 状态更新安全 | 2h |
| P0 | 确认幂等性 | 防止重复发布 | 1h |
| P1 | 任务认领租约 | 执行不重复 | 3h |
| P1 | Matrix 事件去重 | Room 消息不重复 | 2h |
| P2 | Room 主节点选举 | 简化调度 | 4h |

---

## 7. 总结

### 当前问题
- ❌ 无并发控制，多 Agent 会冲突
- ❌ 无去重机制，DAG 可能重复
- ❌ 状态更新可能覆盖

### 解决方案
- ✅ 数据库唯一约束 + 乐观锁
- ✅ 任务认领 + 租约续期
- ✅ 幂等确认令牌
- ✅ 状态机强制约束

### 下一步
1. 添加 `version` 字段到 tasks/dag_runs 表
2. 实现 `claim_task` 方法
3. 添加 `idempotency_key` 支持

**审阅要点**：
1. 乐观锁版本号策略是否合理？
2. 租约时间（5分钟）是否合适？
3. 是否需要 Room 级别的主节点选举？
4. GLM 端是否需要更多辅助机制？
