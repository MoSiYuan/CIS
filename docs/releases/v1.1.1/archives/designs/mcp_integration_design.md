# CIS MCP 服务化方案

## 1. 概述

将CIS（Cluster of Independent Systems）的多种API命令通过MCP（Model Context Protocol）协议暴露给AI Agent，支持Claude CLI和VS Code。

### 1.1 目标

- 将现有的cis-core功能通过MCP协议暴露
- 支持Claude CLI和VS Code集成
- 保持CLI命令的完全兼容性
- 提供类型安全的API接口
- 支持异步操作和长时运行任务

### 1.2 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│                        AI Agent (Claude)                        │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               │ MCP Protocol (JSON-RPC over stdio)
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│                    MCP Server (cis-mcp)                         │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Tool Registry & Router                       │  │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐            │  │
│  │  │ DAG Tools  │ │Skill Tools │ │Agent Tools │ ...        │  │
│  │  └────────────┘ └────────────┘ └────────────┘            │  │
│  └──────────────────────────────────────────────────────────┘  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Resource Management                          │  │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐            │  │
│  │  │ DAG Runs   │ │Sessions    │ │Workers     │            │  │
│  │  └────────────┘ └────────────┘ └────────────┘            │  │
│  └──────────────────────────────────────────────────────────┘  │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               │ Internal API (Rust function calls)
                               │
┌──────────────────────────────▼──────────────────────────────────┐
│                       cis-core Library                          │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐ ┌────────────┐  │
│  │ DAG        │ │Skill       │ │Agent       │ │Storage     │  │
│  │ Scheduler  │ │Manager     │ │Cluster     │ │Manager     │  │
│  └────────────┘ └────────────┘ └────────────┘ └────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

## 2. MCP工具设计

### 2.1 DAG管理工具

#### 2.1.1 `dag_create_run`
创建并启动新的DAG运行

```rust
tool dag_create_run {
    name: "dag_create_run"
    description: "创建并启动一个新的DAG运行实例"
    input_schema: {
        "type": "object",
        "properties": {
            "dag_file": {
                "type": "string",
                "description": "DAG定义文件路径（支持.toml和.json格式）"
            },
            "run_id": {
                "type": "string",
                "description": "自定义运行ID（可选，自动生成）"
            },
            "paused": {
                "type": "boolean",
                "description": "是否以暂停模式启动（用于检查后再执行）",
                "default": false
            }
        },
        "required": ["dag_file"]
    }
    output: {
        "run_id": "string",
        "status": "string",
        "dag_name": "string",
        "task_count": "number",
        "created_at": "string"
    }
}
```

#### 2.1.2 `dag_get_status`
获取DAG运行状态

```rust
tool dag_get_status {
    name: "dag_get_status"
    description: "获取DAG运行的详细状态信息"
    input_schema: {
        "type": "object",
        "properties": {
            "run_id": {
                "type": "string",
                "description": "DAG运行ID（不提供则使用活动运行）"
            },
            "verbose": {
                "type": "boolean",
                "description": "显示详细的任务列表",
                "default": false
            }
        }
    }
    output: {
        "run_id": "string",
        "status": "running|paused|completed|failed",
        "created_at": "string",
        "tasks": {
            "total": "number",
            "completed": "number",
            "running": "number",
            "pending": "number",
            "failed": "number"
        },
        "progress": "number",  // 0-100
        "tasks_detail": [...]  // verbose=true时
    }
}
```

#### 2.1.3 `dag_control`
控制DAG运行（暂停/恢复/中止）

```rust
tool dag_control {
    name: "dag_control"
    description: "控制DAG运行的执行状态"
    input_schema: {
        "type": "object",
        "properties": {
            "run_id": {
                "type": "string",
                "description": "DAG运行ID"
            },
            "action": {
                "type": "string",
                "enum": ["pause", "resume", "abort"],
                "description": "控制动作"
            },
            "force": {
                "type": "boolean",
                "description": "强制执行（不提示确认）",
                "default": false
            }
        },
        "required": ["run_id", "action"]
    }
    output: {
        "success": "boolean",
        "message": "string",
        "new_status": "string"
    }
}
```

#### 2.1.4 `dag_list`
列出DAG运行

```rust
tool dag_list {
    name: "dag_list"
    description: "列出DAG运行，支持过滤"
    input_schema: {
        "type": "object",
        "properties": {
            "all": {
                "type": "boolean",
                "description": "显示所有运行包括已完成",
                "default": false
            },
            "status": {
                "type": "string",
                "enum": ["running", "paused", "completed", "failed"],
                "description": "按状态过滤"
            },
            "scope": {
                "type": "string",
                "description": "按作用域过滤"
            },
            "limit": {
                "type": "number",
                "description": "限制结果数量",
                "default": 50
            }
        }
    }
    output: {
        "runs": [{
            "run_id": "string",
            "dag_id": "string",
            "status": "string",
            "tasks": { "completed": "number", "total": "number" },
            "created_at": "string"
        }]
    }
}
```

#### 2.1.5 `dag_execute`
执行DAG任务

```rust
tool dag_execute {
    name: "dag_execute"
    description: "执行DAG运行中的任务"
    input_schema: {
        "type": "object",
        "properties": {
            "run_id": {
                "type": "string",
                "description": "DAG运行ID"
            },
            "use_agent": {
                "type": "boolean",
                "description": "使用Agent集群执行",
                "default": false
            },
            "max_workers": {
                "type": "number",
                "description": "最大并发Agent数",
                "default": 4
            }
        }
    }
    output: {
        "status": "string",
        "duration_secs": "number",
        "completed": "number",
        "failed": "number",
        "skipped": "number",
        "outputs": [{
            "task_id": "string",
            "success": "boolean",
            "output": "string"
        }]
    }
}
```

#### 2.1.6 `dag_session_manage`
Agent会话管理

```rust
tool dag_session_manage {
    name: "dag_session_manage"
    description: "管理DAG执行的Agent会话"
    input_schema: {
        "type": "object",
        "properties": {
            "action": {
                "type": "string",
                "enum": ["list", "attach", "kill", "unblock", "logs"],
                "description": "会话操作"
            },
            "session_id": {
                "type": "string",
                "description": "会话ID"
            },
            "run_id": {
                "type": "string",
                "description": "DAG运行ID"
            },
            "task_id": {
                "type": "string",
                "description": "任务ID"
            },
            "tail": {
                "type": "number",
                "description": "显示最后N行日志",
                "default": 50
            },
            "follow": {
                "type": "boolean",
                "description": "实时跟踪输出",
                "default": false
            }
        },
        "required": ["action"]
    }
    output: {
        // 根据action不同返回不同结构
        "sessions": [...] | "session_info": {...} | "logs": "string"
    }
}
```

### 2.2 Skill管理工具

#### 2.2.1 `skill_list`
列出可用的skills

```rust
tool skill_list {
    name: "skill_list"
    description: "列出所有可用的skills"
    input_schema: {
        "type": "object",
        "properties": {
            "scope": {
                "type": "string",
                "description": "按作用域过滤（global/project/user）"
            },
            "type": {
                "type": "string",
                "description": "按类型过滤（dag/executor/etc）"
            }
        }
    }
    output: {
        "skills": [{
            "name": "string",
            "version": "string",
            "type": "string",
            "description": "string",
            "scope": "string",
            "loaded": "boolean"
        }]
    }
}
```

#### 2.2.2 `skill_invoke`
自然语言调用skill

```rust
tool skill_invoke {
    name: "skill_invoke"
    description: "使用自然语言描述调用合适的skill"
    input_schema: {
        "type": "object",
        "properties": {
            "description": {
                "type": "string",
                "description": "任务的自然语言描述"
            },
            "project_path": {
                "type": "string",
                "description": "项目路径"
            },
            "show_candidates": {
                "type": "boolean",
                "description": "显示候选skill列表",
                "default": false
            }
        },
        "required": ["description"]
    }
    output: {
        "skill": {
            "name": "string",
            "confidence": "number"
        },
        "result": "any",
        "execution_time_ms": "number"
    }
}
```

#### 2.2.3 `skill_chain`
Skill链执行

```rust
tool skill_chain {
    name: "skill_chain"
    description: "自动发现并执行skill链"
    input_schema: {
        "type": "object",
        "properties": {
            "description": {
                "type": "string",
                "description": "任务的复杂描述"
            },
            "preview": {
                "type": "boolean",
                "description": "仅预览不执行",
                "default": false
            },
            "verbose": {
                "type": "boolean",
                "description": "显示详细信息",
                "default": false
            }
        },
        "required": ["description"]
    }
    output: {
        "chain": [{
            "skill": "string",
            "order": "number",
            "reason": "string"
        }],
        "results": [...]
    }
}
```

### 2.3 Agent工具

#### 2.3.1 `agent_execute`
执行Agent任务

```rust
tool agent_execute {
    name: "agent_execute"
    description: "执行AI Agent任务"
    input_schema: {
        "type": "object",
        "properties": {
            "prompt": {
                "type": "string",
                "description": "任务提示词"
            },
            "skills": {
                "type": "array",
                "items": { "type": "string" },
                "description": "启用的技能列表"
            },
            "context": {
                "type": "object",
                "description": "上下文信息（工作目录等）"
            }
        },
        "required": ["prompt"]
    }
    output: {
        "content": "string",
        "token_usage": {
            "total": "number",
            "prompt": "number",
            "completion": "number"
        },
        "execution_time_ms": "number"
    }
}
```

### 2.4 内存管理工具

#### 2.4.1 `memory_store`
存储内存

```rust
tool memory_store {
    name: "memory_store"
    description: "存储记忆到向量数据库"
    input_schema: {
        "type": "object",
        "properties": {
            "content": {
                "type": "string",
                "description": "记忆内容"
            },
            "metadata": {
                "type": "object",
                "description": "元数据标签"
            },
            "scope": {
                "type": "string",
                "description": "作用域"
            }
        },
        "required": ["content"]
    }
    output: {
        "memory_id": "string",
        "stored": "boolean"
    }
}
```

#### 2.4.2 `memory_search`
搜索记忆

```rust
tool memory_search {
    name: "memory_search"
    description: "向量搜索相关记忆"
    input_schema: {
        "type": "object",
        "properties": {
            "query": {
                "type": "string",
                "description": "搜索查询"
            },
            "limit": {
                "type": "number",
                "description": "返回结果数",
                "default": 10
            },
            "scope": {
                "type": "string",
                "description": "搜索作用域"
            }
        },
        "required": ["query"]
    }
    output: {
        "results": [{
            "content": "string",
            "similarity": "number",
            "metadata": "object"
        }]
    }
}
```

### 2.5 Worker管理工具

#### 2.5.1 `worker_list`
列出活动workers

```rust
tool worker_list {
    name: "worker_list"
    description: "列出所有活动的DAG worker进程"
    input_schema: {
        "type": "object",
        "properties": {
            "scope": {
                "type": "string",
                "description": "按作用域过滤"
            }
        }
    }
    output: {
        "workers": [{
            "id": "string",
            "scope": "string",
            "status": "string",
            "pid": "number",
            "uptime_secs": "number"
        }]
    }
}
```

## 3. 资源（Resources）设计

MCP Resources允许Agent订阅和监控变化。

### 3.1 DAG运行状态资源

```typescript
resource://cis/dag/runs/{run_id}
{
    "uri": "resource://cis/dag/runs/{run_id}",
    "name": "DAG Run Status",
    "description": "实时DAG运行状态",
    "mimeType": "application/json"
}
```

### 3.2 Agent会话资源

```typescript
resource://cis/agent/sessions/{session_id}
{
    "uri": "resource://cis/agent/sessions/{session_id}",
    "name": "Agent Session",
    "description": "Agent会话状态和输出",
    "mimeType": "application/json"
}
```

### 3.3 系统状态资源

```typescript
resource://cis/system/status
{
    "uri": "resource://cis/system/status",
    "name": "System Status",
    "description": "CIS系统整体状态",
    "mimeType": "application/json"
}
```

## 4. 实现方案

### 4.1 项目结构

```
cis-mcp/
├── Cargo.toml
├── src/
│   ├── main.rs              # MCP服务器入口
│   ├── server.rs            # MCP服务器实现
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── dag.rs           # DAG工具
│   │   ├── skill.rs         # Skill工具
│   │   ├── agent.rs         # Agent工具
│   │   ├── memory.rs        # 内存工具
│   │   └── worker.rs        # Worker工具
│   ├── resources/
│   │   ├── mod.rs
│   │   ├── dag.rs           # DAG资源
│   │   ├── agent.rs         # Agent资源
│   │   └── system.rs        # 系统资源
│   ├── transport/
│   │   ├── mod.rs
│   │   ├── stdio.rs         # stdio传输层
│   │   └── sse.rs           # SSE传输层（用于Web）
│   └── types.rs             # MCP类型定义
└── tests/
    ├── integration_test.rs
    └── test_tools.rs
```

### 4.2 技术栈

- **语言**: Rust
- **MCP协议**: [rust-mcp-sdk](https://github.com/juliancao/rust-mcp-sdk) 或自定义实现
- **序列化**: serde_json
- **异步运行时**: tokio
- **日志**: tracing
- **错误处理**: anyhow

### 4.3 核心依赖

```toml
[dependencies]
cis-core = { path = "../cis-core" }
tokio = { version = "1.40", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
async-trait = "0.1"

# MCP SDK (选择一个)
mcp-sdk = "0.1"  # 如果可用
# 或自己实现MCP协议
```

### 4.4 实现步骤

#### Phase 1: MCP服务器基础框架
1. 实现MCP协议基础（JSON-RPC over stdio）
2. 工具注册和路由系统
3. 错误处理和日志
4. 基本测试框架

#### Phase 2: DAG工具实现
1. `dag_create_run`
2. `dag_get_status`
3. `dag_control`
4. `dag_list`
5. `dag_execute`
6. `dag_session_manage`

#### Phase 3: Skill和Agent工具
1. `skill_list`
2. `skill_invoke`
3. `skill_chain`
4. `agent_execute`

#### Phase 4: 资源支持
1. DAG运行状态订阅
2. Agent会话订阅
3. 系统状态监控

#### Phase 5: 高级特性
1. SSE传输层（用于Web集成）
2. 批量操作
3. 长时运行任务支持
4. 性能优化

### 4.5 MCP Server实现示例

```rust
use cis_core::scheduler::DagScheduler;
use anyhow::Result;

struct McpServer {
    scheduler: DagScheduler,
    skill_manager: Arc<SkillManager>,
}

impl McpServer {
    async fn handle_call(&mut self, tool_name: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        match tool_name {
            "dag_create_run" => self.dag_create_run(args).await,
            "dag_get_status" => self.dag_get_status(args).await,
            "skill_invoke" => self.skill_invoke(args).await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        }
    }

    async fn dag_create_run(&self, args: serde_json::Value) -> Result<serde_json::Value> {
        let dag_file = args["dag_file"].as_str().ok_or_else(|| anyhow!("Missing dag_file"))?;
        let run_id = args["run_id"].as_str();
        let paused = args["paused"].as_bool().unwrap_or(false);

        // 调用cis-core函数
        let run_id = commands::create_run(dag_file, run_id.map(|s| s.to_string()), paused).await?;

        Ok(serde_json::json!({
            "run_id": run_id,
            "status": "created"
        }))
    }
}
```

### 4.6 集成配置

#### Claude Desktop配置
```json
{
  "mcpServers": {
    "cis": {
      "command": "/path/to/cis-mcp",
      "args": [],
      "env": {
        "CIS_DATA_DIR": "~/.cis"
      }
    }
  }
}
```

#### VS Code (Cline/Cursor)配置
```json
{
  "mcpServers": {
    "cis": {
      "command": "/path/to/cis-mcp",
      "args": ["--stdio"]
    }
  }
}
```

## 5. 使用示例

### 5.1 创建并执行DAG

```python
# Agent使用MCP工具
tools = [
    {
        "name": "dag_create_run",
        "arguments": {
            "dag_file": "./test_dag/hello.toml",
            "paused": true
        }
    }
]

# 返回: { "run_id": "550e8400-...", "status": "paused" }

tools = [
    {
        "name": "dag_get_status",
        "arguments": {
            "run_id": "550e8400-...",
            "verbose": true
        }
    }
]

tools = [
    {
        "name": "dag_control",
        "arguments": {
            "run_id": "550e8400-...",
            "action": "resume"
        }
    }
]
```

### 5.2 Skill链执行

```python
tools = [
    {
        "name": "skill_chain",
        "arguments": {
            "description": "部署应用到Kubernetes并配置监控",
            "preview": true
        }
    }
]

# 返回发现的skill链:
# [
#   { "skill": "k8s-deploy", "order": 1, "reason": "部署到K8s" },
#   { "skill": "prometheus-setup", "order": 2, "reason": "配置监控" }
# ]
```

### 5.3 Agent会话交互

```python
# 订阅Agent会话资源
resource = "resource://cis/agent/sessions/550e8400-task1"

# 实时获取Agent输出
# 资源会持续推送Agent的思考和执行过程
```

## 6. 优势和挑战

### 6.1 优势

1. **统一接口**: AI Agent通过标准MCP协议访问所有CIS功能
2. **类型安全**: Rust提供强类型保证
3. **高性能**: 异步执行，支持并发操作
4. **可扩展**: 易于添加新工具和资源
5. **安全性**: 通过MCP协议边界保护系统

### 6.2 挑战

1. **长时运行任务**: DAG执行可能需要很长时间，需要异步处理
2. **错误传播**: 需要将Rust错误正确映射到MCP错误格式
3. **资源限制**: 需要考虑内存和CPU使用
4. **并发控制**: 避免同时执行的DAG冲突

### 6.3 解决方案

1. **任务队列**: 为长时运行任务实现队列系统
2. **状态轮询**: Agent定期检查任务状态
3. **资源订阅**: 使用MCP Resources推送状态更新
4. **取消支持**: 实现任务取消机制

## 7. 测试策略

### 7.1 单元测试
- 每个工具的输入/输出验证
- 错误处理测试
- 边界条件测试

### 7.2 集成测试
- 完整的DAG执行流程
- Skill链执行
- Agent会话管理

### 7.3 MCP协议测试
- 符合MCP规范测试
- 与Claude Desktop的集成测试
- 与VS Code的集成测试

## 8. 文档

### 8.1 用户文档
- MCP工具使用指南
- 配置说明
- 故障排除

### 8.2 开发文档
- API文档
- 架构设计
- 贡献指南

### 8.3 示例
- 常见使用场景
- 完整工作流程
- 最佳实践

## 9. 发布计划

- **v0.1.0**: 基础MCP服务器 + DAG工具
- **v0.2.0**: Skill和Agent工具
- **v0.3.0**: 资源支持
- **v0.4.0**: Web传输层
- **v1.0.0**: 完整功能 + 文档

## 10. 相关资源

- [MCP协议规范](https://modelcontextprotocol.io/)
- [Claude Desktop集成](https://claude.ai/)
- [CIS项目文档](./README.md)
- [Rust异步编程](https://tokio.rs/)
