# Agent 联邦协议规范

> 版本: 1.0.0  
> 状态: Draft  
> 最后更新: 2026-02-08

## 目录

- [概述](#概述)
- [架构](#架构)
- [事件类型](#事件类型)
- [消息流程](#消息流程)
- [Room 分配策略](#room-分配策略)
- [路由机制](#路由机制)
- [错误处理](#错误处理)
- [安全考虑](#安全考虑)
- [参考实现](#参考实现)

## 概述

Agent 联邦协议（Agent Federation Protocol）是 CIS 系统中用于跨节点 Agent 通信的协议规范。它基于 Matrix Federation（端口 7676）构建，允许不同 CIS 节点上的 Agent 相互发现、通信和协调任务。

### 设计目标

- **透明性**: 对应用层屏蔽节点边界，像调用本地 Agent 一样调用远程 Agent
- **可靠性**: 支持重试、超时和故障恢复
- **可扩展性**: 支持动态添加/移除节点和 Agent
- **安全性**: 基于 Matrix Federation 的签名验证机制

### 核心概念

| 概念 | 说明 |
|------|------|
| Agent | 持久化的 AI 助手（Claude、OpenCode、Kimi、Aider） |
| Node | 运行 CIS 的物理或虚拟机器 |
| Federation | 节点间的 Matrix 协议通信 |
| Room | Matrix 中的聊天室，用于路由事件 |
| Address | Agent 的联邦地址（`agent-id@node-id`） |

## 架构

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CIS Cluster                                     │
│                                                                             │
│  ┌─────────────────────┐                    ┌─────────────────────┐        │
│  │     Node A          │                    │     Node B          │        │
│  │  ┌───────────────┐  │  Federation      │  ┌───────────────┐  │        │
│  │  │ Agent Pool    │  │◄────────────────►│  │ Agent Pool    │  │        │
│  │  │ ┌───────────┐ │  │   Port 7676      │  │ ┌───────────┐ │  │        │
│  │  │ │ Agent 1   │ │  │                  │  │ │ Agent 3   │ │  │        │
│  │  │ │ (Claude)  │ │  │                  │  │ │ (Kimi)    │ │  │        │
│  │  │ └───────────┘ │  │                  │  │ └───────────┘ │  │        │
│  │  │ ┌───────────┐ │  │                  │  │ ┌───────────┐ │  │        │
│  │  │ │ Agent 2   │ │  │                  │  │ │ Agent 4   │ │  │        │
│  │  │ │(OpenCode) │ │  │                  │  │ │ (Aider)   │ │  │        │
│  │  │ └───────────┘ │  │                  │  │ └───────────┘ │  │        │
│  │  └───────────────┘  │                    └───────────────┘  │        │
│  │  ┌───────────────┐  │                    ┌───────────────┐  │        │
│  │  │Routing Table  │  │◄────────────────►│  │Routing Table  │  │        │
│  │  │- Agent 3 → B  │  │  State Sync      │  │- Agent 1 → A  │  │        │
│  │  │- Agent 4 → B  │  │                  │  │- Agent 2 → A  │  │        │
│  │  └───────────────┘  │                    └───────────────┘  │        │
│  └─────────────────────┘                    └─────────────────────┘        │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                    !agent-federation-default:{node}                  │   │
│  │                         (Federation Room)                            │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 事件类型

所有事件通过 Matrix Federation 传输，使用 CIS 自定义事件类型命名空间 `io.cis.agent.*`。

### 事件类型列表

| 事件类型 | 方向 | 说明 |
|---------|------|------|
| `io.cis.agent.registered` | Broadcast | Agent 上线通知 |
| `io.cis.agent.unregistered` | Broadcast | Agent 下线通知 |
| `io.cis.agent.task.request` | P2P | 任务请求 |
| `io.cis.agent.task.response` | P2P | 任务响应 |
| `io.cis.agent.message` | P2P/Broadcast | 直接消息 |
| `io.cis.agent.heartbeat` | Broadcast | 心跳 |
| `io.cis.agent.status_update` | Broadcast | 状态更新 |

### Agent 注册事件

```json
{
  "event_type": "io.cis.agent.registered",
  "agent_id": "worker-1",
  "node_id": "kitchen.local",
  "runtime_type": "claude",
  "capabilities": ["coding", "review", "test"],
  "timestamp": "2026-02-08T10:30:00Z"
}
```

**说明**:
- Agent 启动时发送
- 所有节点收到后更新本地路由表
- 可选择发送当前所有 Agent 列表作为响应

### Agent 注销事件

```json
{
  "event_type": "io.cis.agent.unregistered",
  "agent_id": "worker-1",
  "node_id": "kitchen.local",
  "reason": "shutdown",
  "timestamp": "2026-02-08T18:00:00Z"
}
```

**说明**:
- Agent 关闭时发送
- 超时未收到心跳也会触发注销

### 任务请求事件

```json
{
  "event_type": "io.cis.agent.task.request",
  "request_id": "req-uuid-123",
  "from_agent": "coordinator@living.local",
  "to_agent": "worker-1@kitchen.local",
  "task": {
    "task_id": "task-456",
    "prompt": "Implement a function to...",
    "context": "Rust project in /workspace",
    "system_prompt": "You are a senior Rust developer",
    "model": "claude-3-opus",
    "metadata": {}
  },
  "timeout_secs": 300,
  "timestamp": "2026-02-08T10:35:00Z"
}
```

**说明**:
- `request_id` 用于关联响应
- `timeout_secs` 为可选，默认 300 秒
- 通过 Room 路由到目标节点

### 任务响应事件

```json
{
  "event_type": "io.cis.agent.task.response",
  "request_id": "req-uuid-123",
  "from_agent": "worker-1@kitchen.local",
  "to_agent": "coordinator@living.local",
  "result": {
    "success": true,
    "output": "Implementation completed...",
    "exit_code": 0,
    "metadata": {
      "tokens_used": 1500,
      "duration_ms": 45000
    }
  },
  "timestamp": "2026-02-08T10:35:45Z"
}
```

### 消息事件

```json
{
  "event_type": "io.cis.agent.message",
  "message_id": "msg-uuid-789",
  "from_agent": "worker-1@kitchen.local",
  "to_agent": "coordinator@living.local",
  "message_type": "progress",
  "payload": {
    "percent": 50,
    "status": "compiling"
  },
  "timestamp": "2026-02-08T10:35:20Z"
}
```

**广播消息** (`to_agent` 为 null):

```json
{
  "event_type": "io.cis.agent.message",
  "message_id": "msg-uuid-999",
  "from_agent": "coordinator@living.local",
  "to_agent": null,
  "message_type": "announcement",
  "payload": {
    "message": "New task available"
  },
  "timestamp": "2026-02-08T10:40:00Z"
}
```

### 心跳事件

```json
{
  "event_type": "io.cis.agent.heartbeat",
  "agent_id": "worker-1",
  "node_id": "kitchen.local",
  "status": "idle",
  "timestamp": "2026-02-08T10:36:00Z"
}
```

**说明**:
- 默认每 30 秒发送一次
- 超过 120 秒未收到心跳认为 Agent 离线

### 状态更新事件

```json
{
  "event_type": "io.cis.agent.status_update",
  "agent_id": "worker-1",
  "node_id": "kitchen.local",
  "status": "busy",
  "current_task": "task-456",
  "timestamp": "2026-02-08T10:35:05Z"
}
```

## 消息流程

### 1. Agent 发现流程

```
Node A (新节点)                              Node B (已有节点)
   │                                              │
   │  1. AgentRegistered (broadcast)              │
   │─────────────────────────────────────────────>│
   │                                              │
   │  2. AgentRegistered (响应自己的 Agent 列表)   │
   │<─────────────────────────────────────────────│
   │                                              │
   │  3. 双方更新路由表                            │
   │                                              │
```

### 2. 任务执行流程

```
Coordinator@A                                Worker@B
   │                                              │
   │  1. TaskRequest (via Federation Room)        │
   │─────────────────────────────────────────────>│
   │                                              │
   │                                              │──┐
   │                                              │  │ 2. 执行任务
   │                                              │<─┘
   │                                              │
   │  3. TaskResponse (via Federation Room)       │
   │<─────────────────────────────────────────────│
   │                                              │
```

### 3. 心跳和故障检测

```
Agent@A                                      Node B
   │                                          │
   │  1. Heartbeat (every 30s)               │
   │─────────────────────────────────────────>│
   │                                          │
   │  2. Heartbeat (every 30s)               │
   │─────────────────────────────────────────>│
   │  x (connection lost)                     │
   │                                          │
   │                                          │──┐
   │                                          │  │ 3. 检测超时 (>120s)
   │                                          │<─┘
   │                                          │
   │                                          │──┐
   │  4. AgentUnregistered (timeout)         │  │
   │<─────────────────────────────────────────│<─┘
```

## Room 分配策略

### Room 结构

| Room ID 格式 | 用途 | 联邦同步 |
|-------------|------|---------|
| `!agent-federation-{ns}:{node}` | Agent 联邦主要通信 | 是 |
| `!agent-broadcast:{node}` | 广播消息 | 是 |

### Room 创建规则

1. **默认命名空间**: `default`
   - Room ID: `!agent-federation-default:kitchen.local`
   
2. **广播 Room**: 
   - Room ID: `!agent-broadcast:kitchen.local`
   - 用于向所有节点广播消息

3. **自定义命名空间**:
   - 支持按团队/项目划分命名空间
   - 示例: `!agent-federation-team-alpha:kitchen.local`

### Room 成员管理

```rust
// Room 成员自动同步
pub struct AgentFederationRoom {
    pub room_id: String,
    pub federate: bool,
    pub members: Vec<String>, // node_ids
}
```

## 路由机制

### Agent 地址格式

```
agent-id@node-id

示例:
- worker-1@kitchen.local
- coordinator@living.local
- review-bot@cloud-seed.cis.dev
```

### 路由表结构

```rust
pub struct AgentRoutingTable {
    // 远程 Agent 路由表: agent-id -> node-id
    remote_agents: HashMap<String, String>,
    // 节点地址表: node-id -> federation URL
    node_urls: HashMap<String, String>,
}
```

### 路由决策流程

```
收到消息 -> 解析目标 Agent ID
    │
    ├── 是本地 Agent? ──Yes──► 直接处理
    │
    └── 查询路由表
         │
         ├── 找到远程节点? ──Yes──► 转发到对应节点
         │
         └── 未找到? ──► 返回 Unknown Agent 错误
```

## 错误处理

### 错误类型

| 错误码 | 说明 | 处理建议 |
|-------|------|---------|
| `AGENT_NOT_FOUND` | 目标 Agent 不存在 | 检查地址，或等待 Agent 上线 |
| `AGENT_BUSY` | Agent 正忙 | 稍后重试或使用其他 Agent |
| `TIMEOUT` | 任务超时 | 检查任务复杂度，增加超时时间 |
| `NODE_UNREACHABLE` | 节点不可达 | 检查网络连接 |
| `INVALID_PAYLOAD` | 消息格式错误 | 检查协议版本 |

### 错误响应示例

```json
{
  "event_type": "io.cis.agent.task.response",
  "request_id": "req-uuid-123",
  "from_agent": "worker-1@kitchen.local",
  "to_agent": "coordinator@living.local",
  "result": {
    "success": false,
    "output": "Agent is currently busy with another task",
    "exit_code": -1,
    "metadata": {
      "error_code": "AGENT_BUSY",
      "current_task": "task-123"
    }
  },
  "timestamp": "2026-02-08T10:35:05Z"
}
```

### 重试策略

```rust
pub const DEFAULT_RETRY_POLICY: RetryPolicy = RetryPolicy {
    max_retries: 3,
    base_delay_ms: 100,
    max_delay_ms: 5000,
    backoff_multiplier: 2.0,
};
```

## 安全考虑

### 1. 传输安全

- 使用 HTTPS/TLS 进行 Federation 通信
- 支持 mTLS 双向认证
- 端口 7676 建议限制在可信网络

### 2. 事件签名

```rust
pub struct CisMatrixEvent {
    // ... other fields
    pub signatures: Option<HashMap<String, HashMap<String, String>>>,
    pub hashes: Option<HashMap<String, String>>,
}
```

### 3. 访问控制

- 仅接受来自已知节点的 Federation 连接
- Agent 间通信基于 Matrix 的 Room 成员关系
- 敏感操作需要额外授权

### 4. 最佳实践

1. **网络隔离**: Federation 端口应限制在内网或 VPN
2. **证书管理**: 定期轮换 TLS 证书
3. **审计日志**: 记录所有跨节点操作
4. **限流**: 防止单个 Agent 过度使用资源

## 参考实现

### Rust 代码示例

```rust
use cis_core::agent::federation::{
    AgentAddress, AgentFederationEvent, AgentRoutingTable,
    TaskRequestPayload, TaskResultPayload,
};
use cis_core::agent::persistent::{RuntimeType, AgentStatus};

// 1. 解析 Agent 地址
let addr = AgentAddress::parse("worker-1@kitchen.local")?;

// 2. 创建并发送任务请求
let task = TaskRequestPayload::new("task-123", "Implement feature X")
    .with_context("Project: myapp")
    .with_timeout(300);

let event = AgentFederationEvent::task_request(
    "req-uuid",
    "coordinator@living.local",
    "worker-1@kitchen.local",
    task,
    Some(300),
);

// 3. 管理路由表
let mut routing = AgentRoutingTable::new();
routing.register_remote("worker-1", "kitchen.local");
routing.register_node_url("kitchen.local", "http://kitchen.local:7676");

// 4. 路由决策
match routing.route("worker-1", "living.local") {
    AgentRoute::Local => println!("Local agent"),
    AgentRoute::Remote { node_id } => println!("Remote agent at {}", node_id),
    AgentRoute::Unknown => println!("Unknown agent"),
}
```

### 配置文件示例

```toml
[agent_federation]
enabled = true
heartbeat_interval_secs = 30
agent_timeout_secs = 120
default_task_timeout_secs = 300

[agent_federation.room]
namespace = "default"
federate = true

[[agent_federation.peers]]
node_id = "kitchen.local"
url = "http://kitchen.local:7676"
trusted = true
```

## 附录

### A. 状态机

```
         ┌─────────────┐
         │   Initial   │
         └──────┬──────┘
                │ register
                ▼
         ┌─────────────┐
    ┌───►│   Idle      │◄────┐
    │    └──────┬──────┘     │ complete
    │ start     │            │
    │           ▼            │
    │    ┌─────────────┐     │
    └────┤    Busy     │─────┘
         └──────┬──────┘
                │ error
                ▼
         ┌─────────────┐
         │   Error     │
         └──────┬──────┘
                │ recover
                ▼
         ┌─────────────┐
         │  Shutdown   │
         └─────────────┘
```

### B. 性能指标

| 指标 | 目标值 | 说明 |
|------|-------|------|
| 事件传输延迟 | < 100ms | 同数据中心 |
| 心跳间隔 | 30s | 可配置 |
| 超时检测 | 120s | 2 个心跳周期 |
| 路由表大小 | 1000+ | 单节点支持 |

### C. 相关文档

- [Matrix Federation 规范](https://spec.matrix.org/v1.9/server-server-api/)
- [CIS Federation 架构](../matrix/federation/README.md)
- [Persistent Agent 设计](./claude_persistent_usage.md)

---

*本协议规范遵循 CIS 架构设计原则，如有变更请更新版本号并通知所有节点。*
