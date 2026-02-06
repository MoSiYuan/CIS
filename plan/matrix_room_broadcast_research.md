# Matrix Room 在 CIS 中的应用方式调研

## 执行摘要

**结论**: Matrix Room 是 CIS 中实现节点状态、DAG 执行情况广播的理想机制。CIS 已有完整的 Matrix 联邦基础设施，支持通过 WebSocket 隧道实时广播事件到多个节点。

---

## 1. 现有 Matrix 基础设施

### 1.1 架构概览

```
┌─────────────────────────────────────────────────────────────────┐
│                      CIS Matrix 模块                             │
├─────────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐           │
│  │   Server     │  │   Nucleus    │  │  Federation  │           │
│  │  (7676)      │  │   (Core)     │  │  (6767/6768) │           │
│  └──────────────┘  └──────┬───────┘  └──────────────┘           │
│                           │                                     │
│         ┌─────────────────┼─────────────────┐                   │
│         │                 │                 │                   │
│    ┌────┴────┐      ┌────┴────┐      ┌────┴────┐               │
│    │  Store  │      │  Sync   │      │   WS    │               │
│    │         │      │  Queue  │      │ Tunnel  │               │
│    └─────────┘      └─────────┘      └─────────┘               │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 核心组件

| 组件 | 端口 | 功能 |
|------|------|------|
| Matrix Server | 7676 | Client-Server API，Element 客户端连接 |
| Federation (HTTP) | 6767 | 节点间联邦通信 (BMI) |
| Federation (WebSocket) | 6768 | 低延迟实时通信 |
| EventBroadcaster | - | 房间事件广播到联邦 peers |

### 1.3 关键特性

- **WebSocket 隧道**: `TunnelManager` 管理节点间持久连接
- **事件广播**: `EventBroadcaster` 支持并行发送到多个 peers
- **断线重连**: `FederationManager` 自动重连和状态同步
- **DID 认证**: 基于 DID 的节点身份验证

---

## 2. DAG 状态广播方案

### 2.1 推荐方案：专用 DAG 状态 Room

```
Room ID 格式: !dag-status:{node_id}
示例: !dag-status-node-abc123:cis.local
```

#### 2.1.1 事件类型设计

```rust
// 新增 CIS DAG 事件类型
pub enum MatrixEventType {
    // ... 现有事件 ...
    
    // DAG 状态事件
    CisDagRunCreated,       // DAG run 创建
    CisDagRunStarted,       // DAG run 开始执行
    CisDagRunCompleted,     // DAG run 完成
    CisDagRunFailed,        // DAG run 失败
    CisDagRunPaused,        // DAG run 暂停
    CisDagTaskStatus,       // 任务状态更新
    CisDagTaskOutput,       // 任务输出（可选，大输出走 ContextStore）
    CisDagSessionBlocked,   // Agent session 卡点
    CisDagSessionRecovered, // Agent session 恢复
}
```

#### 2.1.2 事件内容格式

```json
// cis.dag.run.created
{
    "run_id": "dag-run-abc123",
    "dag_name": "refactor-project",
    "source_file": "dag.toml",
    "max_workers": 4,
    "timestamp": "2025-01-20T10:30:00Z",
    "federate": true
}

// cis.dag.task.status
{
    "run_id": "dag-run-abc123",
    "task_id": "analyze",
    "status": "running",  // pending | running | completed | failed | blocked
    "agent_type": "claude",
    "session_id": "dag-run-abc123:analyze",
    "timestamp": "2025-01-20T10:31:00Z",
    "progress": {
        "completed": 2,
        "total": 6,
        "failed": 0
    }
}

// cis.dag.session.blocked
{
    "run_id": "dag-run-abc123",
    "task_id": "update-core",
    "session_id": "dag-run-abc123:update-core",
    "reason": "Merge conflict detected",
    "output_preview": "CONFLICT (content): Merge conflict in src/main.rs",
    "timestamp": "2025-01-20T10:45:00Z"
}
```

### 2.2 集成方式

#### 2.2.1 与 SessionManager 集成

```rust
// cis-core/src/agent/cluster/matrix_broadcast.rs

use crate::matrix::nucleus::{MatrixEvent, RoomId, UserId, EventId};
use crate::matrix::events::MatrixEventType;
use crate::matrix::broadcast::EventBroadcaster;

pub struct DagMatrixBroadcaster {
    broadcaster: Arc<EventBroadcaster>,
    room_id: RoomId,
    node_id: String,
}

impl DagMatrixBroadcaster {
    /// 广播 DAG run 创建
    pub async fn broadcast_run_created(&self, run: &DagRun) -> Result<()> {
        let event = MatrixEvent::new(
            self.room_id.clone(),
            EventId::generate(),
            UserId::new(format!("@{}:{}", self.node_id, self.server_name())),
            MatrixEventType::CisDagRunCreated.as_str(),
            serde_json::json!({
                "run_id": run.run_id,
                "dag_name": run.dag_name,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        );
        
        self.broadcaster.broadcast_event(
            self.room_id.as_str(),
            &event
        ).await?;
        
        Ok(())
    }
    
    /// 广播任务状态更新
    pub async fn broadcast_task_status(
        &self,
        run_id: &str,
        task_id: &str,
        status: DagNodeStatus,
    ) -> Result<()> {
        // ...
    }
}
```

#### 2.2.2 与 AgentClusterExecutor 集成

```rust
// 在 executor.rs 中添加广播逻辑

pub struct AgentClusterExecutor {
    // ... 现有字段 ...
    matrix_broadcaster: Option<Arc<DagMatrixBroadcaster>>,
}

impl AgentClusterExecutor {
    async fn start_task(&self, run: &DagRun, task_id: &str, command: &str) -> Result<()> {
        // ... 现有逻辑 ...
        
        // 广播任务开始
        if let Some(ref broadcaster) = self.matrix_broadcaster {
            broadcaster.broadcast_task_status(
                &run.run_id,
                task_id,
                DagNodeStatus::Running,
            ).await.ok(); // 广播失败不影响执行
        }
        
        Ok(())
    }
}
```

---

## 3. 节点状态广播方案

### 3.1 节点心跳 Room

```
Room ID 格式: !node-heartbeat:{network_id}
示例: !node-heartbeat-cis-network-xyz:cis.local
```

### 3.2 状态事件

```json
// cis.node.heartbeat
{
    "node_id": "node-abc123",
    "did": "did:cis:node-abc123",
    "status": "online",  // online | busy | maintenance | offline
    "capabilities": {
        "agents": ["claude", "kimi"],
        "max_workers": 4,
        "version": "0.2.0"
    },
    "resources": {
        "cpu_percent": 45,
        "memory_percent": 60,
        "active_sessions": 2
    },
    "timestamp": "2025-01-20T10:30:00Z"
}

// cis.node.dag.active
{
    "node_id": "node-abc123",
    "active_runs": [
        {
            "run_id": "dag-run-abc123",
            "dag_name": "refactor-project",
            "progress": "3/6",
            "status": "running"
        }
    ],
    "timestamp": "2025-01-20T10:30:00Z"
}
```

---

## 4. 联邦跨节点 DAG 执行

### 4.1 分布式 DAG 执行流程

```
┌─────────────────┐         ┌─────────────────┐         ┌─────────────────┐
│   Node A        │         │   Node B        │         │   Node C        │
│  (Coordinator)  │         │  (Worker)       │         │  (Worker)       │
└────────┬────────┘         └────────┬────────┘         └────────┬────────┘
         │                           │                           │
         │  1. Create Room           │                           │
         │  !dag-run-123:cis.local   │                           │
         │──────────────────────────>│                           │
         │                           │                           │
         │  2. Broadcast Task-1      │                           │
         │  (assign to B)            │                           │
         │──────────────────────────>│                           │
         │                           │  3. Execute Task-1        │
         │                           │  (Claude session)         │
         │                           │                           │
         │  4. Task-1 Complete       │                           │
         │<──────────────────────────│                           │
         │                           │                           │
         │  5. Broadcast Task-2      │                           │
         │  (assign to C)            │                           │
         │──────────────────────────────────────────────────────>│
         │                           │                           │
         │                           │                           │  6. Execute Task-2
         │                           │                           │
         │  7. Task-2 Blocked        │                           │
         │<──────────────────────────────────────────────────────│
         │                           │                           │
         │  8. Broadcast unblock     │                           │
         │──────────────────────────────────────────────────────>│
```

### 4.2 任务分配事件

```json
// cis.dag.task.assigned
{
    "run_id": "dag-run-abc123",
    "task_id": "analyze",
    "assigned_to": "node-b",
    "assigned_by": "node-a",
    "agent_type": "claude",
    "upstream_tasks": [],
    "timestamp": "2025-01-20T10:30:00Z"
}

// cis.dag.task.claimed
{
    "run_id": "dag-run-abc123",
    "task_id": "analyze",
    "claimed_by": "node-b",
    "timestamp": "2025-01-20T10:30:05Z"
}
```

---

## 5. 实现建议

### 5.1 文件结构

```
cis-core/src/agent/cluster/
├── matrix_broadcast.rs      # 新增: Matrix 广播适配器
├── matrix_events.rs         # 新增: DAG 事件定义
└── ...

cis-core/src/matrix/events/
├── event_types.rs           # 已有: 添加 CisDag* 事件类型
└── mod.rs
```

### 5.2 配置选项

```toml
# config.toml
[matrix.dag_broadcast]
enabled = true
room_suffix = "dag-status"      # Room ID: !dag-status:{node_id}
federate = true                 # 是否联邦广播
batch_interval_ms = 500         # 批量发送间隔

[matrix.node_heartbeat]
enabled = true
room_suffix = "node-heartbeat"
interval_secs = 30              # 心跳间隔
include_resources = true        # 包含资源使用情况
```

### 5.3 性能优化

| 优化点 | 方案 |
|--------|------|
| 批量发送 | 500ms 内的事件合并为一批发送 |
| 大输出分离 | 任务输出 > 10KB 时只发送摘要，完整输出走 ContextStore |
| 本地优先 | 本地订阅者通过 tokio::sync::broadcast 接收，不走 Matrix |
| 智能联邦 | 只向活跃节点广播，离线节点使用 SyncQueue 延迟同步 |

---

## 6. 使用场景示例

### 6.1 监控 Dashboard

```rust
// GUI 或 Web Dashboard 订阅 DAG 状态 Room
let mut events = matrix_client.sync_room("!dag-status-node-abc:cis.local").await?;

while let Some(event) = events.next().await {
    match event.event_type {
        MatrixEventType::CisDagTaskStatus => {
            update_task_progress(event.content);
        }
        MatrixEventType::CisDagSessionBlocked => {
            show_alert("Task blocked, needs intervention");
        }
        _ => {}
    }
}
```

### 6.2 跨节点任务接管

```rust
// Node B 发现 Node A 离线
if node_a_offline {
    // 查询 Room 中 Node A 的活跃任务
    let active_tasks = matrix_client
        .query_room_events(
            "!dag-status-node-a:cis.local",
            EventFilter::by_type(MatrixEventType::CisDagTaskStatus)
                .with_status(DagNodeStatus::Running)
        )
        .await?;
    
    // 接管任务
    for task in active_tasks {
        executor.takeover_task(task.run_id, task.task_id).await?;
    }
}
```

### 6.3 CLI 实时监控

```bash
# 订阅 DAG 状态 Room 并实时显示
$ cis matrix subscribe !dag-status-node-abc:cis.local --format table

TIME        RUN_ID          TASK       STATUS    PROGRESS
10:30:01    dag-run-123     analyze    running   1/6
10:30:15    dag-run-123     analyze    completed 2/6
10:30:16    dag-run-123     plan       running   2/6
10:35:20    dag-run-123     plan       blocked   ⚠️
```

---

## 7. 风险评估

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| Matrix 连接中断 | 状态广播中断 | 本地事件队列 + 断线重连后批量同步 |
| 事件风暴 | 性能下降 | 批量发送 + 采样率控制 |
| 敏感信息泄露 | 安全风险 | 输出脱敏 + Room 加密（未来） |
| 时间不同步 | 事件顺序混乱 | 使用 origin_server_ts + 向量时钟 |

---

## 8. 结论与建议

### 8.1 核心结论

1. **Matrix Room 完全适合** DAG 状态广播和节点状态同步
2. **现有基础设施完善**，只需添加 DAG 相关的事件类型和广播逻辑
3. **联邦能力天然支持** 跨节点 DAG 分布式执行
4. **Element 客户端可直接查看** DAG 执行状态（人机友好）

### 8.2 实施建议

**Phase 1** (1-2 天): 
- 添加 `CisDag*` 事件类型到 `event_types.rs`
- 实现 `DagMatrixBroadcaster` 基础结构
- 集成到 `AgentClusterExecutor` 关键节点

**Phase 2** (2-3 天):
- 实现节点心跳广播
- 添加 CLI `matrix subscribe` 命令
- 实现跨节点任务接管

**Phase 3** (3-5 天):
- GUI Dashboard 实时展示
- 性能优化（批量、采样）
- 事件历史查询 API

### 8.3 代码示例

完整的 `matrix_broadcast.rs` 实现已包含在设计文档中，可直接参考实施。
