# Agent Teams 设计对 CIS 体系的提升

## 1. 核心提升概览

```
┌─────────────────────────────────────────────────────────────────┐
│                     提升对比图                                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  现有体系                           Agent Teams 体系              │
│  ─────────                          ────────────────             │
│                                                                 │
│  ┌──────────────┐                   ┌──────────────┐           │
│  │ 单次执行      │                   │ 持久化 Agent │           │
│  │ 执行完即销毁  │        ───►       │ 可复用、保持状态│          │
│  └──────────────┘                   └──────────────┘           │
│                                                                 │
│  ┌──────────────┐                   ┌──────────────┐           │
│  │ 单一 Agent   │                   │ 多 Agent 协作 │           │
│  │ DAG 只能一种  │        ───►       │ 不同 Task 不同│           │
│  └──────────────┘                   └──────────────┘           │
│                                                                 │
│  ┌──────────────┐                   ┌──────────────┐           │
│  │ 前后台分离   │                   │ 灵活切换      │           │
│  │ 无法 attach  │        ───►       │ -p 模式随时切 │           │
│  └──────────────┘                   └──────────────┘           │
│                                                                 │
│  ┌──────────────┐                   ┌──────────────┐           │
│  │ 单机限制     │                   │ 分布式协作    │           │
│  │ 无法跨节点   │        ───►       │ Matrix 联邦   │           │
│  └──────────────┘                   └──────────────┘           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## 2. 具体提升点

### 2.1 Agent 持久化与复用

**现有问题：**
```rust
// 现在：每次 Task 都创建新 Session
for task in tasks {
    let session = create_session(agent_type);  // 新建
    session.start();
    session.execute(task);
    session.destroy();  // 立即销毁 ❌
}
// 问题：Context 丢失、Warm-up 成本高
```

**提升后：**
```rust
// Agent Teams：复用已有 Agent
let agent = pool.acquire(AgentConfig {
    reuse_agent: Some("code-agent"),  // 复用！
    keep_agent: true,                  // 保持运行
});

agent.execute(task1);  // 执行第一个任务
agent.execute(task2);  // 复用同一个 Agent，Context 保持
agent.execute(task3);  // 历史对话都在
```

**价值：**
- ✅ 保留对话历史（Context 连续性）
- ✅ 减少 Warm-up 时间（不用重新加载模型）
- ✅ 支持多轮协作（Agent 记住之前的任务）

---

### 2.2 多 Agent 并行协作

**现有问题：**
```rust
// 现在：整个 DAG 只能用一个 Agent
pub struct AgentClusterConfig {
    pub default_agent: AgentType,  // 全局一个！
    pub max_workers: usize,
}

// 所有 Task 都用 Claude
// 无法让 Task A 用 Claude，Task B 用 OpenCode
```

**提升后：**
```yaml
# DAG 定义：每个 Task 自由选择 Agent
tasks:
  - task_id: "设计架构"
    agent_runtime: "claude"        # Claude 擅长设计
    agent_config:
      system_prompt: "你是架构师"
      
  - task_id: "写代码"
    agent_runtime: "opencode"      # OpenCode 成本低
    agent_config:
      model: "opencode/glm-4.7-free"
      
  - task_id: "安全审计"
    agent_runtime: "claude"        # Claude 更谨慎
    agent_config:
      system_prompt: "你是安全专家"
      
  - task_id: "代码审查"
    reuse_agent: "写代码"          # 复用之前的 Agent
```

**价值：**
- ✅ 按能力选 Agent（Claude 设计强、OpenCode 便宜）
- ✅ 成本优化（简单任务用便宜模型）
- ✅ 专业化分工（不同 Agent 不同角色）

---

### 2.3 前后台灵活切换（OpenCode -p 模式）

**现有问题：**
```rust
// 现在：Agent 要么前台交互，要么后台运行
// 无法切换

// 前台：阻塞终端
$ opencode run "任务"

// 后台：无法交互
$ opencode run "任务" &
// 出问题无法查看
```

**提升后：**
```rust
// OpenCode -p 模式
pub struct OpenCodePersistentAgent {
    pid: u32,
    socket_path: PathBuf,
}

// 1. 后台启动
let agent = OpenCodePersistentAgent::start().await;

// 2. 发送任务（后台执行）
agent.execute(task).await;

// 3. 需要调试？随时 attach
agent.attach().await;  // 进入交互式终端
// ... 查看输出、干预 ...
// Ctrl+B,D detach 返回后台

// 4. 继续后台运行
agent.execute(next_task).await;
```

**价值：**
- ✅ 后台执行不阻塞
- ✅ 出问题可 attach 调试
- ✅ 适合长时间运行的 Agent

---

### 2.4 跨节点 Agent 协作

**现有问题：**
```rust
// 现在：Agent 只能在单节点内运行
// Node A 的 Agent 无法和 Node B 通信

// 两个节点的 Agent 是完全独立的
Node A: AgentSession { ... }
Node B: AgentSession { ... }
// 没有连接 ❌
```

**提升后：**
```rust
// 通过 Matrix Federation 跨节点
pub struct FederatedAgent {
    local_agent: Box<dyn PersistentAgent>,
    room_id: String,  // Matrix Room
}

// Node A 的 Agent
let agent_a = FederatedAgent::new(
    ClaudePersistentAgent::new(),
    "!agent-team:cis.local"
);

// Node B 的 Agent
let agent_b = FederatedAgent::new(
    OpenCodePersistentAgent::new(),
    "!agent-team:cis.local"  // 同一个 Room！
);

// Agent A 发送消息给 Agent B
agent_a.send(AgentMessage {
    to: Some("agent-b@node-b"),
    payload: json!({"code": "..."}),
});

// 通过 Matrix Federation 同步
// 端口 7676，自动跨节点传输
```

**价值：**
- ✅ 分布式 Agent 团队
- ✅ 跨机器负载均衡
- ✅ 多节点协作完成大任务

---

## 3. 架构能力提升

### 3.1 调度能力

| 能力 | 现有 | 提升后 |
|------|------|--------|
| 并发数 | max_workers (固定) | 动态调度 + Agent 池 |
| Agent 选择 | 全局 default | 每 Task 可选 |
| 资源复用 | 无 | Agent Pool 复用 |
| 故障恢复 | 重新执行 | Agent 重启保留状态 |

### 3.2 通信能力

| 能力 | 现有 | 提升后 |
|------|------|--------|
| 节点内 | EventBus (内存) | EventBus + Mailbox |
| 跨节点 | 不支持 | Matrix Federation |
| 消息类型 | 仅状态同步 | 任意消息传递 |
| 持久化 | 内存 | SQLite + 联邦同步 |

### 3.3 运维能力

| 能力 | 现有 | 提升后 |
|------|------|--------|
| 查看 Agent | 无法查看 | `cis agent list` |
| 调试 | 日志 | attach 交互式 |
| 手动干预 | 杀死重试 | 发送消息干预 |
| 监控 | 无 | Agent 心跳检测 |

---

## 4. 业务场景提升

### 4.1 复杂开发工作流

**现有：**
```
单一 Agent 串行执行：
设计 -> 编码 -> 测试 -> 修复
(都用 Claude，成本高)
```

**提升后：**
```
专业化多 Agent 协作：

Claude(架构师)      OpenCode(码农)      Claude(审计)
     │                    │                  │
     ▼                    ▼                  ▼
 设计架构 ──────────► 写代码 ──────────► 安全审查
     │                    │                  │
     │                成本低              关键节点
     │                    │                  │
     └────────────────────┴──────────────────┘
                          ▼
                    自动集成测试
```

**收益：**
- 成本降低 60%（OpenCode 处理 80% 任务）
- 质量提升（专业 Agent 做专业事）
- 速度提升（并行执行）

### 4.2 24/7 智能值守

**现有：**
```
无法长期运行，没有后台模式
```

**提升后：**
```rust
// 创建长期运行的监控 Agent
let monitor = AgentPool::acquire(AgentConfig {
    runtime_type: RuntimeType::OpenCode,
    keep_agent: true,
    auto_recover: true,  // 故障自动重启
});

// 后台持续监控
monitor.execute("监控服务器日志，发现异常立即报警").await;

// 随时 attach 查看状态
$ cis agent attach monitor-agent
```

**收益：**
- 持续监控不间断
- 异常情况可人工介入
- 历史状态可追溯

### 4.3 分布式代码审查

**现有：**
```
单机 Agent，无法利用多台机器算力
```

**提升后：**
```
Node A (Mac)          Node B (Linux)        Node C (Windows)
   │                       │                      │
   ▼                       ▼                      ▼
Claude Agent         OpenCode Agent        Claude Agent
  (前端审查)            (后端审查)            (安全审查)
   │                       │                      │
   └───────────────────────┼──────────────────────┘
                           ▼
                    Matrix Federation
                           │
                    汇总审查报告
                           ▼
                    Node A (协调者)
```

**收益：**
- 并行审查提速 3 倍
- 利用异构环境（各平台测试）
- 单点故障不影响整体

---

## 5. 性能提升数据（预估）

### 5.1 执行效率

| 指标 | 现有 | 提升后 | 提升幅度 |
|------|------|--------|----------|
| Task 启动时间 | 3-5s | 0.1s (复用) | **30-50x** |
| Context 保留 | 0% | 100% | **无限** |
| 并行 Agent 数 | 1 类型 | N 类型 | **N 倍** |
| 跨节点协作 | 不支持 | 支持 | **新增** |

### 5.2 成本优化

```
场景：100 个 Task 的 DAG

现有方案（全用 Claude）：
- 100 tasks × $0.05 = $5.00

提升后方案（混合）：
- 20 设计 tasks × Claude × $0.05 = $1.00
- 70 编码 tasks × OpenCode × $0.001 = $0.07
- 10 审计 tasks × Claude × $0.05 = $0.50
- 总计: $1.57

成本降低: 68%
```

### 5.3 可靠性

| 指标 | 现有 | 提升后 |
|------|------|--------|
| 单点故障 | 影响全部 | 仅影响单个 Agent |
| 故障恢复 | 重试整个 DAG | 重启单个 Agent |
| 状态丢失 | 全部丢失 | 仅丢失当前 Task |
| 人工干预 | 无法干预 | 可 attach 干预 |

---

## 6. 与竞品对比

| 特性 | Claude Code | OpenCode | CIS + Agent Teams |
|------|-------------|----------|-------------------|
| 多 Agent | ✅ 原生支持 | ❌ 不支持 | ✅ 支持 |
| Agent 复用 | ✅ 支持 | ❌ 每次新建 | ✅ 支持 |
| 后台模式 | ❌ 不支持 | ✅ -p 模式 | ✅ 统一支持 |
| 跨节点 | ❌ 单机 | ❌ 单机 | ✅ 分布式 |
| 开放协议 | ❌ 封闭 | ❌ 封闭 | ✅ Matrix |
| 自托管 | ❌ 云端 | ❌ 云端 | ✅ 完全自主 |

---

## 7. 总结

### 核心价值

1. **效率提升**：Agent 复用减少 90% 启动时间
2. **成本优化**：智能选择 Agent 降低 60% 成本
3. **能力扩展**：从单机到分布式，支持复杂协作
4. **运维增强**：可调试、可监控、可干预

### 对 CIS 的意义

```
CIS 1.0: 单节点 DAG 执行器
    ↓
CIS 2.0: 分布式多 Agent 协作平台
```

**Agent Teams 让 CIS 从「工具」进化为「平台」：**
- 从执行预设流程 → 智能协作解决问题
- 从单机运行 → 分布式联邦
- 从一次性任务 → 持续化服务

这是 CIS 从「好用的 DAG 调度器」到「下一代 AI 基础设施」的关键一跃。

---

**提升分析完成**
