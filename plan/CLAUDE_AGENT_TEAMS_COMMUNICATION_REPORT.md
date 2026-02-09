# Claude Agent Teams 通信机制研究报告

**生成时间**: 2026-02-08
**研究目标**: 理解 Claude Agent Teams 中多个 Agent 之间的通信交互方式

---

## 目录

1. [概述](#概述)
2. [Agent Teams vs Subagents](#agent-teams-vs-subagents)
3. [Mailbox 通信系统](#mailbox-通信系统)
4. [共享任务列表](#共享任务列表)
5. [通信模式](#通信模式)
6. [技术架构](#技术架构)
7. [实际应用示例](#实际应用示例)

---

## 概述

Claude Agent Teams 是 Claude Code 的多智能体协作系统，允许多个 AI Agent 协同工作完成复杂任务。与传统的 Subagent 机制不同，Agent Teams 使用更先进的通信架构。

### 核心特性

- **独立 Agent 身份**: 每个 Agent 拥有独立的会话上下文
- **持久化通信**: 通过 Mailbox 系统进行消息传递
- **任务协调**: 共享任务列表实现协同工作
- **灵活路由**: 支持点对点和广播通信

---

## Agent Teams vs Subagents

| 特性 | Subagents | Agent Teams |
|------|-----------|-------------|
| **会话关系** | 父子关系 | 平等协作关系 |
| **通信方式** | 内部 JSONL 消息传递 | Mailbox 消息系统 |
| **上下文共享** | 继承父会话上下文 | 独立上下文，通过消息共享 |
| **持久化** | 临时创建，任务完成后销毁 | 持久化 Agent，可长期运行 |
| **通信模式** | 单向（父→子） | 双向（任意 Agent 之间） |
| **任务协调** | 父 Agent 调度 | 共享任务列表自动协调 |
| **适用场景** | 简单任务委托 | 复杂多步骤协作 |

### Subagent 通信格式

Subagent 使用 JSONL 格式进行消息持久化：

```json
{
  "agentId": "a4b0450",
  "sessionId": "d849b67c-ca52-4691-aa89-12c85f162bf1",
  "uuid": "unique-message-id",
  "parentUuid": "parent-message-id",
  "type": "user|assistant",
  "userType": "external",
  "isSidechain": true,
  "message": {
    "role": "user|assistant",
    "content": "任务描述..."
  }
}
```

**特点**：
- 消息存储在 `~/.claude/projects/*/subagents/agent-*.jsonl`
- 通过 `parentUuid` 维护消息链
- 只支持父会话发起的通信

---

## Mailbox 通信系统

Agent Teams 的核心通信机制是 Mailbox 系统，类似电子邮件的工作方式。

### 基本概念

- **Mailbox**: 每个 Agent 拥有一个独立的邮箱
- **Message**: Agent 之间传递的结构化消息
- **Address**: Agent 的唯一标识地址

### 消息类型

#### 1. 点对点消息 (Point-to-Point)

一个 Agent 直接向另一个 Agent 发送消息：

```
Agent A ──────message──────> Agent B
         └── Mailbox B ──┘
```

**使用场景**：
- 请求特定专家 Agent 的帮助
- 分配子任务给特定 Agent
- 获取另一个 Agent 的结果

**消息格式**：
```json
{
  "from": "agent-a@team",
  "to": "agent-b@team",
  "type": "request",
  "id": "msg-001",
  "timestamp": "2026-02-08T10:30:00Z",
  "payload": {
    "task": "分析数据...",
    "context": {...}
  }
}
```

#### 2. 广播消息 (Broadcast)

一个 Agent 向团队中所有其他 Agent 发送消息：

```
                ┌──> Agent B
Agent A ───────┼──> Agent C
                └──> Agent D
```

**使用场景**：
- 公告重要信息
- 请求团队协作
- 分享共享资源更新

**消息格式**：
```json
{
  "from": "coordinator@team",
  "to": "*",
  "type": "broadcast",
  "id": "msg-002",
  "timestamp": "2026-02-08T10:35:00Z",
  "payload": {
    "announcement": "项目里程碑完成",
    "metadata": {...}
  }
}
```

### Mailbox 操作

```python
# 伪代码示例
mailbox = get_mailbox("agent-b@team")

# 发送消息
mailbox.send({
  "to": "agent-c@team",
  "type": "request",
  "payload": {...}
})

# 接收消息
messages = mailbox.receive()
for msg in messages:
    process_message(msg)

# 标记消息已处理
mailbox.mark_processed(msg.id)
```

---

## 共享任务列表

Agent Teams 使用共享任务列表实现协同工作，类似 Kanban 看板。

### 任务状态流转

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│ Pending  │───>│ Assigned │───>│ Running  │───>│Completed │
└──────────┘    └──────────┘    └──────────┘    └──────────┘
     │                                │
     │                                │
     v                                v
┌──────────┐                    ┌──────────┐
│  Failed  │<───────────────────│ Blocked  │
└──────────┘                    └──────────┘
```

### 任务列表数据结构

```json
{
  "team": "development-team",
  "tasks": [
    {
      "id": "task-001",
      "title": "实现用户认证",
      "description": "设计并实现 JWT 认证系统",
      "status": "running",
      "assigned_to": "security-agent@team",
      "created_by": "coordinator@team",
      "created_at": "2026-02-08T09:00:00Z",
      "dependencies": [],
      "priority": "high",
      "progress": 60,
      "subtasks": [
        {"id": "sub-001", "title": "设计 Token 结构", "status": "completed"},
        {"id": "sub-002", "title": "实现中间件", "status": "running"},
        {"id": "sub-003", "title": "编写测试", "status": "pending"}
      ]
    },
    {
      "id": "task-002",
      "title": "设计数据库架构",
      "status": "pending",
      "assigned_to": null,
      "dependencies": ["task-001"]
    }
  ]
}
```

### 任务分配机制

1. **自动分配**: 系统根据 Agent 能力和负载自动分配
2. **手动分配**: Coordinator Agent 手动指定
3. **自助认领**: Agent 主动认领适合的任务

### 任务协调流程

```
┌─────────────┐
 │ Coordinator │
 │   Agent     │
 └──────┬──────┘
        │ 1. 创建任务
        ▼
 ┌─────────────────┐
 │ Shared Task List│
 └────────┬────────┘
          │
    ┌─────┴─────┬─────────────┐
    │           │             │
    ▼           ▼             ▼
┌───────┐  ┌─────────┐  ┌──────────┐
│ Dev   │  │ Security│  │   Test   │
│Agent  │  │  Agent  │  │  Agent   │
└───────┘  └─────────┘  └──────────┘
    │           │             │
    └───────────┴─────────────┘
                │ 2. 更新任务状态
                ▼
         ┌─────────────────┐
         │ Shared Task List│
         └─────────────────┘
```

---

## 通信模式

### 1. 请求-响应模式

最基础的通信模式：

```
Agent A                    Agent B
  │                          │
  │────── request ──────────>│
  │                          │
  │<───── response ──────────│
  │                          │
```

**场景**: Agent A 需要Agent B 提供特定服务

**示例**:
```json
// Request
{
  "from": "frontend-agent@team",
  "to": "api-agent@team",
  "type": "request",
  "action": "get_user_data",
  "params": {"user_id": 123}
}

// Response
{
  "from": "api-agent@team",
  "to": "frontend-agent@team",
  "type": "response",
  "request_id": "msg-001",
  "status": "success",
  "data": {
    "user_id": 123,
    "name": "张三",
    "email": "zhangsan@example.com"
  }
}
```

### 2. 发布-订阅模式

多个 Agent 订阅感兴趣的事件：

```
                    ┌── Agent A (订阅者)
Event Publisher ────┼── Agent B (订阅者)
                    └── Agent C (订阅者)
```

**场景**: 状态变更通知、事件广播

**示例**:
```json
// 订阅
{
  "from": "test-agent@team",
  "to": "event-bus",
  "type": "subscribe",
  "events": ["deploy_completed", "test_failed"]
}

// 发布事件
{
  "from": "deploy-agent@team",
  "to": "*",
  "type": "event",
  "event": "deploy_completed",
  "data": {
    "version": "1.2.3",
    "environment": "production"
  }
}
```

### 3. 任务流模式

多 Agent 协作完成复杂工作流：

```
┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│ Agent A │───>│ Agent B │───>│ Agent C │───>│ Agent D │
│(规划)   │    │(开发)   │    │(测试)   │    │(部署)   │
└─────────┘    └─────────┘    └─────────┘    └─────────┘
     │              │              │              │
     └──────────────┴──────────────┴──────────────┘
                        │
                 共享任务列表
```

**场景**: CI/CD 流水线、代码审查流程

### 4. 协商模式

多个 Agent 协商决策：

```
                    ┌─────────┐
                    │ Agent A │
                    └────┬────┘
                         │
        ┌────────────────┼────────────────┐
        │                │                │
    ┌───┴───┐        ┌───┴───┐        ┌───┴───┐
    │Agent B│        │Agent C │        │Agent D │
    └───┬───┘        └───┬───┘        └───┬───┘
        │                │                │
        └────────────────┼────────────────┘
                         │
                    ┌────▼────┐
                    │ Decision│
                    └─────────┘
```

**场景**: 架构设计决策、优先级排序

---

## 技术架构

### 系统架构图

```
┌─────────────────────────────────────────────────────────┐
│                   Claude Agent Teams                    │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐ │
│  │   Agent A    │  │   Agent B    │  │   Agent C    │ │
│  │              │  │              │  │              │ │
│  │ ┌──────────┐ │  │ ┌──────────┐ │  │ ┌──────────┐ │ │
│  │ │  Mailbox │◄├──┤│  Mailbox │◄├──┤│  Mailbox │ │ │
│  │ └──────────┘ │  │ └──────────┘ │  │ └──────────┘ │ │
│  │              │  │              │  │              │ │
│  └──────────────┘  └──────────────┘  └──────────────┘ │
│          │                  │                  │       │
│          └──────────────────┼──────────────────┘       │
│                             │                           │
│                    ┌────────▼────────┐                 │
│                    │  Message Router │                 │
│                    └────────┬────────┘                 │
│                             │                           │
│                    ┌────────▼────────┐                 │
│                    │ Shared Task List│                 │
│                    └────────┬────────┘                 │
├────────────────────────────┼───────────────────────────┤
│                         MCP                          │
│              (Model Context Protocol)                  │
└────────────────────────────┼───────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │  Claude API     │
                    └─────────────────┘
```

### MCP 协议 (Model Context Protocol)

Claude Agent Teams 使用 MCP 协议进行标准化通信：

**核心特性**:
- 标准化消息格式
- 工具调用协议
- 资源访问控制
- 状态同步机制

**MCP 消息示例**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "execute_task",
    "arguments": {
      "task_id": "task-001",
      "agent": "developer@team"
    }
  }
}
```

### 持久化存储

```
~/.claude/
├── teams/
│   ├── team-1/
│   │   ├── agents.json          # Agent 配置
│   │   ├── mailboxes/           # Mailbox 存储
│   │   │   ├── agent-a.mbox
│   │   │   └── agent-b.mbox
│   │   ├── tasks.json           # 任务列表
│   │   └── messages.jsonl       # 消息历史
```

---

## 实际应用示例

### 示例 1: CI/CD 流水线

**场景**: 自动化代码测试、构建、部署流程

**Agent 配置**:
- **Coordinator Agent**: 协调整个流程
- **Test Agent**: 运行测试套件
- **Build Agent**: 编译和打包
- **Deploy Agent**: 部署到生产环境

**通信流程**:

```
1. Coordinator 创建任务并添加到共享列表
2. Test Agent 认领测试任务
   └─> 运行测试
   └─> 发送结果给 Coordinator
3. Build Agent 认领构建任务
   └─> 编译代码
   └─> 发送构建产物
4. Deploy Agent 认领部署任务
   └─> 部署到生产
   └─> 通知所有 Agent
```

**消息示例**:
```json
// 1. Coordinator 创建任务
{
  "from": "coordinator@ci-team",
  "to": "task-list",
  "type": "create_task",
  "task": {
    "id": "test-001",
    "title": "运行单元测试",
    "assigned_to": "test-agent@ci-team"
  }
}

// 2. Test Agent 报告结果
{
  "from": "test-agent@ci-team",
  "to": "coordinator@ci-team",
  "type": "task_completed",
  "task_id": "test-001",
  "result": {
    "status": "passed",
    "tests_run": 156,
    "failed": 0
  }
}

// 3. 广播部署完成
{
  "from": "deploy-agent@ci-team",
  "to": "*",
  "type": "broadcast",
  "event": "deployment_completed",
  "data": {
    "version": "v2.3.1",
    "url": "https://app.example.com"
  }
}
```

### 示例 2: 代码审查团队

**场景**: 多 Agent 协作进行代码审查

**Agent 配置**:
- **Reviewer Agent A**: 安全性审查
- **Reviewer Agent B**: 性能审查
- **Reviewer Agent C**: 代码规范审查
- **Coordinator Agent**: 汇总审查意见

**工作流程**:

```
1. Coordinator 接收 PR 请求
2. 创建审查任务并分配给各 Reviewer
3. Reviewers 并行审查代码
4. 每个 Reviewer 通过 Mailbox 发送评论
5. Coordinator 汇总所有评论
6. 发送综合报告给开发者
```

**并行审查模式**:
```
          ┌── PR #123 ──┐
          │             │
    ┌─────┴─────┬────────┬──────────┐
    │           │        │          │
    ▼           ▼        ▼          ▼
┌─────────┐ ┌─────────┐┌─────────┐┌──────────┐
│ Security│ │Performance│Style│  Coordinator│
│ Reviewer│ │ Reviewer ││Reviewer││            │
└────┬────┘ └────┬────┘└────┬────┘└─────┬──────┘
     │            │           │            │
     └────────────┴───────────┴────────────┘
                        │
                  汇总审查意见
```

### 示例 3: 研究分析团队

**场景**: 多 Agent 协作进行市场研究

**Agent 配置**:
- **Research Agent**: 收集行业数据
- **Analysis Agent**: 数据分析和可视化
- **Writer Agent**: 生成研究报告
- **Fact-Checker Agent**: 事实核查

**协作模式**:

```
Research Agent ──[数据]──> Analysis Agent
                                 │
                              [分析]
                                 │
                                 ▼
                          Writer Agent
                                 │
                            [草稿]
                                 │
                                 ▼
                         Fact-Checker Agent
                                 │
                              [核查]
                                 │
                                 ▼
                          最终报告
```

---

## 关键技术要点

### 1. 消息可靠性

- **消息确认**: 接收方必须确认收到消息
- **重试机制**: 失败消息自动重试
- **持久化**: 消息持久化存储，防止丢失
- **幂等性**: 确保重复消息不会造成问题

### 2. 并发控制

- **任务锁**: 防止多个 Agent 同时处理同一任务
- **版本控制**: 任务列表版本控制，避免冲突
- **事务支持**: 关键操作支持事务

### 3. 安全性

- **身份验证**: Agent 身份验证和授权
- **消息加密**: 敏感消息加密传输
- **访问控制**: 基于 ACL 的权限管理
- **审计日志**: 完整的操作日志

### 4. 可扩展性

- **动态注册**: Agent 可以动态加入/离开团队
- **负载均衡**: 任务自动分配给空闲 Agent
- **水平扩展**: 支持跨机器部署

---

## 与 CIS 的集成可能

如果要在 CIS 系统中实现类似的 Agent Teams 机制，可以考虑：

### 1. CIS P2P 网络作为通信层

```
Agent Teams 通信层
       │
       ▼
    CIS P2P 网络
       │
       ▼
   联邦节点发现
```

### 2. 利用 CIS 的分布式特性

- **节点发现**: 使用 CIS 的联邦网络机制发现 Agent
- **消息路由**: 通过 CIS P2P 网络进行消息传递
- **状态同步**: 使用 CIS 的存储层同步任务状态
- **身份验证**: 利用 CIS 的节点密钥系统

### 3. 混合架构

```toml
[cis]
# 启用 Agent Teams
enable_agent_teams = true

[agent_teams]
# Agent 团队配置
team_name = "cis-development"
max_agents = 10

[agent_teams.communication]
# 使用 CIS P2P 网络
transport = "cis-p2p"
# 或使用传统 Mailbox
# transport = "mailbox"
```

---

## 总结

Claude Agent Teams 的通信机制相比 Subagent 机制有以下优势：

1. **更灵活的通信**: 支持任意 Agent 之间的双向通信
2. **更好的协作**: 共享任务列表实现高效协同
3. **更强的扩展性**: 支持动态增减 Agent
4. **更持久的状态**: Agent 和任务都是持久化的
5. **更丰富的模式**: 支持多种通信模式

**适用场景**:
- 复杂的多步骤任务
- 需要多个专业 Agent 协作
- 长期运行的 Agent 服务
- 需要任务持久化和恢复
- 分布式工作负载

**不适用场景**:
- 简单的一次性任务（使用 Subagent 更合适）
- 需要快速反馈的交互式操作
- 单 Agent 可以完成的任务

---

## 参考资料

- [Claude Code Agent Teams 官方文档](https://code.claude.com/docs/en/agent-teams)
- [MCP 协议规范](https://modelcontextprotocol.io/)
- [Claude Code Subagent 机制](https://code.claude.com/docs/en/tools)
- CIS 项目文档: [plan/ARCHITECTURE_DESIGN.md](ARCHITECTURE_DESIGN.md)

---

**报告结束**
