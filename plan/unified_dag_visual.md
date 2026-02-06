# 统一 DAG 架构可视化

## 简化视图：两套方案如何整合

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           用户层 (CLI/GUI/API)                               │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  $ cis dag execute --hybrid --room !dag-status:cis.local dag.toml    │  │
│  └────────────────────────────────┬──────────────────────────────────────┘  │
└───────────────────────────────────┼─────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      UnifiedDagExecutor (统一调度器)                         │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  fn execute() {                                                     │   │
│   │      match mode {                                                   │   │
│   │          Local  => local_executor.execute(),      // 直接执行        │   │
│   │          Matrix => matrix_executor.listen_and_execute(), // 监听执行 │   │
│   │          Hybrid => {                                                │   │
│   │              local_executor.execute(),          // 本地执行         │   │
│   │              matrix_broadcaster.broadcast(),    // 广播状态         │   │
│   │          }                                                          │   │
│   │      }                                                              │   │
│   │  }                                                                  │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
└───────────────────────────────────┬─────────────────────────────────────────┘
                                    │
              ┌─────────────────────┼─────────────────────┐
              │                     │                     │
              ▼                     ▼                     ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│   LocalExecutor     │ │   MatrixExecutor    │ │  MatrixBroadcaster  │
│   (API 直连)         │ │   (Room 监听)        │ │  (状态广播)          │
│                     │ │                     │ │                     │
│ • create_session()  │ │ • join_room()       │ │ • broadcast_status()│
│ • kill_session()    │ │ • listen_events()   │ │ • broadcast_output()│
│ • attach_session()  │ │ • claim_task()      │ │ • subscribe()       │
└──────────┬──────────┘ └──────────┬──────────┘ └──────────┬──────────┘
           │                       │                       │
           └───────────────────────┼───────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                      SessionManager (全局单例 - 唯一)                        │
│                                                                              │
│   ┌─────────────────────────────────────────────────────────────────────┐   │
│   │  sessions: HashMap<SessionId, AgentSession>                        │   │
│   │                                                                     │   │
│   │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐          │   │
│   │  │Session-1 │  │Session-2 │  │Session-3 │  │Session-N │          │   │
│   │  │(Claude) │  │(Kimi)   │  │(Claude) │  │(Aider)  │          │   │
│   │  │Running  │  │Blocked  │  │Completed│  │Running  │          │   │
│   │  └──────────┘  └──────────┘  └──────────┘  └──────────┘          │   │
│   └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│   本地广播: tokio::sync::broadcast ──────► CLI/GUI 实时更新                │
│   Matrix广播: Option<MatrixBroadcaster> ───► Room !dag-status:cis.local    │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 数据流对比

### 场景 A: 纯本地执行 (Local Mode)

```
User → CLI → LocalExecutor → SessionManager → AgentSession(PTY)
                                    │
                                    └─► CLI 直接轮询/attach

特点: 快速、单机、无需 Matrix
```

### 场景 B: 纯 Matrix 执行 (Matrix Mode)

```
Coordinator Room                    Worker Node
│                                   │
├─ Broadcast: Task-1 → Node-A ─────►├─ Listen Room
├─ (waiting)                        │
├─ Broadcast: Task-1 Running ◄──────├─ SessionManager::create_session()
├─ (waiting)                        │
├─ Broadcast: Task-1 Complete ◄─────├─ 执行完成
│                                   │
├─ Broadcast: Task-2 → Node-B ─────►├─ (另一个节点)

特点: 纯事件驱动、分布式、去中心化
```

### 场景 C: 混合执行 (Hybrid Mode - 推荐) ⭐

```
User → CLI → HybridExecutor
                │
                ├─► LocalExecutor.execute() ──► SessionManager
                │       │                           │
                │       │                           ├─► AgentSession
                │       │                           │
                │       └─ SessionEvent ────────────┤
                │               │                   │
                │               ▼                   │
                └─► MatrixBroadcaster ◄─────────────┘
                        │
                        ├─► Room !dag-status:cis.local
                        │       ├─► GUI Dashboard (subscribe)
                        │       ├─► Web UI (subscribe)
                        │       └─► Other Nodes (subscribe)
                        │
                        └─► CLI: cis matrix subscribe !dag-status

特点: 本地执行性能好 + Matrix 广播可观测
```

## 关键代码整合

```rust
// 1. 创建 session（所有模式共用）
impl SessionManager {
    pub async fn create_session(...) -> Result<SessionId> {
        let session = AgentSession::new(...);
        session.start().await?;
        
        // 本地广播
        self.local_broadcaster.send(SessionEvent::Created { ... });
        
        // Matrix 广播（如果启用）
        if let Some(ref matrix) = self.matrix_broadcaster {
            matrix.broadcast_session_created(&session_id).await?;
        }
        
        Ok(session_id)
    }
}

// 2. 状态变更（所有模式共用）
impl AgentSession {
    pub async fn mark_blocked(&self, reason: &str) {
        // 更新本地状态
        self.state = SessionState::Blocked { reason };
        
        // 本地广播
        self.local_broadcaster.send(SessionEvent::Blocked { ... });
        
        // Matrix 广播（如果启用）
        if let Some(ref matrix) = self.matrix_broadcaster {
            matrix.broadcast_blocked(&self.id, reason).await;
        }
    }
}

// 3. CLI 统一入口
pub async fn execute_dag(
    dag_file: &str,
    mode: ExecutionMode,
) -> Result<()> {
    let executor = UnifiedDagExecutor::new(mode);
    
    match mode {
        ExecutionMode::Local => {
            // 纯本地
            executor.local.execute_run(&mut run).await
        }
        ExecutionMode::Matrix { room_id } => {
            // 纯 Matrix
            executor.matrix.join_room(&room_id).await?;
            executor.matrix.listen_and_execute().await
        }
        ExecutionMode::Hybrid { room_id } => {
            // 混合：本地执行 + Matrix 广播
            let handle = executor.matrix.start_broadcasting(&room_id).await?;
            let result = executor.local.execute_run(&mut run).await?;
            handle.stop().await;
            Ok(result)
        }
    }
}
```

## 部署场景

### 场景 1: 单机开发

```yaml
# docker-compose.yml (单机)
services:
  cis-node:
    image: cis:latest
    command: ["cis", "dag", "execute", "--local", "dag.toml"]
    # 无需 Matrix 依赖
```

### 场景 2: 多机协作

```yaml
# docker-compose.yml (多机)
services:
  cis-coordinator:
    image: cis:latest
    command: ["cis", "dag", "execute", "--hybrid", "--room", "!dag-project:cis.local", "dag.toml"]
    environment:
      - CIS_NODE_ID=coordinator-1
      - CIS_MATRIX_ENABLED=true
      
  cis-worker-1:
    image: cis:latest
    command: ["cis", "node", "worker", "--accept-remote-tasks", "--room", "!dag-project:cis.local"]
    environment:
      - CIS_NODE_ID=worker-1
      - CIS_MATRIX_ENABLED=true
```

## 总结

| 问题 | 答案 |
|------|------|
| 两套方案冲突吗？ | **不冲突**，共享 SessionManager |
| 需要二选一吗？ | **不需要**，可以同时使用 |
| 推荐默认？ | **Hybrid 模式** = 本地执行 + Matrix 广播 |
| 何时用 Local？ | 单机、快速、无网络 |
| 何时用 Matrix？ | 纯分布式、无中心节点 |
| 何时用 Hybrid？ | **推荐默认**，兼顾性能和可观测性 |

**核心洞察**:
- SessionManager 是唯一的（已有）
- Matrix 是可选的增强（新增）
- 本地性能 + 联邦可观测性 = Hybrid（推荐）
