# ClaudePersistentAgent 使用指南

## 概述

`ClaudePersistentAgent` 是基于 `SessionManager` 和 PTY 会话实现的 Claude Code 持久化 Agent。它支持：

- 持久化运行（后台保持运行）
- 前后台切换（attach/detach）
- 任务执行和状态监控
- 事件驱动的任务完成检测

## 快速开始

### 1. 使用 Runtime 创建 Agent

```rust
use cis_core::agent::persistent::{
    claude::ClaudeRuntime,
    AgentConfig, AgentRuntime,
};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建 Runtime
    let runtime = ClaudeRuntime::new();
    
    // 配置 Agent
    let config = AgentConfig::new(
        "my-claude-agent",
        PathBuf::from("/path/to/work/dir"),
    )
    .with_system_prompt("You are a helpful coding assistant.")
    .with_timeout(300);
    
    // 创建 Agent
    let agent = runtime.create_agent(config).await?;
    
    println!("Agent created: {}", agent.agent_id());
    
    Ok(())
}
```

### 2. 执行任务

```rust
use cis_core::agent::persistent::TaskRequest;

// 创建任务
let task = TaskRequest::new(
    "task-1",
    "Write a function to calculate fibonacci numbers"
)
.with_context("language", "rust")
.with_timeout(120);

// 执行任务
let result = agent.execute(task).await?;

if result.success {
    println!("Output: {:?}", result.output);
    println!("Duration: {}ms", result.duration_ms);
}
```

### 3. 直接创建 Agent（更底层）

```rust
use cis_core::agent::persistent::claude::ClaudePersistentAgent;
use cis_core::agent::cluster::SessionManager;

let session_manager = SessionManager::global();
let config = AgentConfig::new("agent-name", work_dir);

let agent = ClaudePersistentAgent::start(session_manager, config).await?;
```

### 4. 连接到已有的 Session

```rust
use cis_core::agent::cluster::SessionId;

let session_id = SessionId::new("persistent", "agent-name-uuid");
let agent = ClaudePersistentAgent::attach_to_session(
    session_manager,
    session_id
).await?;
```

### 5. Attach/Detach 交互式会话

```rust
// 进入前台交互模式
agent.attach().await?;

// 用户可以在终端与 Claude 交互
// ...

// 返回后台运行
agent.detach().await?;
```

### 6. 关闭 Agent

```rust
// 优雅关闭
agent.shutdown().await?;
```

### 7. 获取统计信息

```rust
// 直接创建时可以直接访问 stats()
let agent = ClaudePersistentAgent::start(session_manager, config).await?;
let stats = agent.stats().await;
println!("Completed tasks: {}", stats.completed_tasks);
```

### 8. 列出所有 Claude Agents

```rust
let runtime = ClaudeRuntime::new();
let agents = runtime.list_agents().await;

for agent in agents {
    println!("{}: {:?}", agent.id, agent.status);
}
```

## 架构

```
┌─────────────────────────────────────┐
│        ClaudePersistentAgent        │
├─────────────────────────────────────┤
│  - session_id: SessionId            │
│  - session_manager: SessionManager  │
│  - state: ClaudeAgentState          │
├─────────────────────────────────────┤
│  + start()                          │
│  + attach_to_session()              │
│  + execute()                        │
│  + attach() / detach()              │
│  + shutdown()                       │
│  + stats()                          │
└─────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│          SessionManager             │
├─────────────────────────────────────┤
│  - sessions: HashMap<SessionId,     │
│              AgentSession>          │
├─────────────────────────────────────┤
│  + create_session()                 │
│  + get_session()                    │
│  + send_input()                     │
│  + attach_session()                 │
│  + kill_session()                   │
└─────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────┐
│           AgentSession              │
├─────────────────────────────────────┤
│  - pty_master: MasterPty            │
│  - process_handle: Child            │
│  - state: SessionState              │
│  - persistent: bool                 │
├─────────────────────────────────────┤
│  + send_input()                     │
│  + mark_idle()                      │
│  + shutdown()                       │
└─────────────────────────────────────┘
```

## 与 OpenCode 的区别

| 特性 | ClaudePersistentAgent | OpenCodePersistentAgent |
|------|----------------------|-------------------------|
| 通信方式 | PTY | HTTP |
| 启动方式 | claude CLI | opencode serve |
| 任务完成检测 | 事件 + 静默期检测 | HTTP 响应 |
| attach 方式 | PTY 直接连接 | opencode attach 命令 |

## 注意事项

1. **需要安装 Claude Code**: 确保 `claude` 命令在 PATH 中可用
2. **PTY 交互**: Claude 使用 PTY 进行交互，与 HTTP 方式不同
3. **任务完成检测**: 通过检测输出静默期来判断任务完成
4. **持久化模式**: 会话在任务完成后保持运行，不会自动销毁

## 错误处理

```rust
match agent.execute(task).await {
    Ok(result) => { /* 处理成功 */ }
    Err(CisError::Execution(msg)) => { /* 执行错误 */ }
    Err(CisError::NotFound(msg)) => { /* Session 不存在 */ }
    Err(e) => { /* 其他错误 */ }
}
```
