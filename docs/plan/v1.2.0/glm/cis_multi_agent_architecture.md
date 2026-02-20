# CIS 多 Agent 分工架构设计

> **设计日期**: 2026-02-20
> **版本**: v1.2.0
> **设计人**: GLM
> **核心理念**: 主 Agent 接待 + Worker Agent 执行 + 跨设备 P2P 协作

---

## 1. 执行摘要

### 1.1 设计目标

CIS v1.2.0 需要**真正的多 Agent 分工架构**，不同于 ZeroClaw 的单 Agent + Delegate Tool 模式：

| 需求 | CIS 解决方案 | ZeroClaw 对比 |
|-----|-------------|--------------|
| **主 Agent** | Receptionist Agent（前台接待） | ❌ 单 Agent 处理所有任务 |
| **工作 Agent** | Worker Agents（专门任务） | ✅ Delegate Tool 类似 |
| **跨设备调用** | P2P 网络调用远程 Agent | ❌ 不支持跨设备 |
| **记忆分组** | 按 Agent/任务/设备隔离 | ⚠️ 仅 session_id 隔离 |
| **任务编排** | DAG 编排多 Agent 协作 | ❌ 单 Agent 顺序执行 |
| **降低幻觉** | 相关性过滤 + 来源追踪 | ✅ 类似机制 |

### 1.2 核心架构

```
┌────────────────────────────────────────────────────────────────┐
│                     CIS 多 Agent 生态系统                        │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │  Receptionist Agent（主 Agent）                           │ │
│  │  ├─ IM 接入（Matrix、Telegram、Discord）                   │ │
│  │  ├─ 任务分类和路由                                        │ │
│  │  ├─ 轻量级模型（Claude Haiku / GPT-4o-mini）              │ │
│  │  └─ 记忆命名空间: "receptionist/"                        │ │
│  └────────────────┬─────────────────────────────────────────┘ │
│                   │ 委派任务                                     │
│      ┌────────────┼────────────┬──────────────┬─────────────┐│
│      ▼            ▼            ▼              ▼             ▼│
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   ┌──────────────┐  │
│  │Coder    │ │Doc      │ │Debugger │   │Remote Agent │  │
│  │Agent    │ │Agent    │ │Agent    │   │(P2P)        │  │
│  │         │ │         │ │         │   │              │  │
│  │Claude   │ │OpenCode │ │Kimi     │   │Remote Device│  │
│  │Sonnet   │ │GLM-4    │ │DeepSeek │   │Agent        │  │
│  └─────────┘ └─────────┘ └─────────┘   └──────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐ │
│  │  Shared Services                                          │ │
│  │  ├─ cis-memory: 分组记忆（按 Agent/任务/设备）            │ │
│  │  ├─ cis-scheduler: DAG 编排（多 Agent 协作）              │ │
│  │  ├─ cis-p2p: 跨设备 Agent 调用                            │ │
│  │  └─ cis-identity: DID 身份验证                            │ │
│  └──────────────────────────────────────────────────────────┘ │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

---

## 2. Agent 角色定义

### 2.1 Receptionist Agent（主 Agent）

**定位**: 前台接待、任务路由、轻量级处理

**职责**:
1. **IM 交互接入**
   - Matrix (Element、Riot)
   - Telegram
   - Discord
   - Slack

2. **任务分类和路由**
   - 简单问答 → 直接回答
   - 代码任务 → 委派 Coder Agent
   - 文档任务 → 委派 Doc Agent
   - 调试任务 → 委派 Debugger Agent
   - 复杂任务 → DAG 编排多 Agent

3. **快速响应**
   - 使用轻量级模型（Claude Haiku / GPT-4o-mini）
   - 延迟 < 2秒

4. **记忆管理**
   - 记忆命名空间: `receptionist/`
   - 存储用户偏好、常见问答

**配置**:
```toml
[agents.receptionist]
name = "receptionist"
runtime = "claude"
model = "claude-haiku-3.5"  # 轻量级、快速
temperature = 0.7
system_prompt = """
You are the receptionist for CIS, a multi-agent system.

Your responsibilities:
1. Greet users and classify their requests
2. Answer simple questions directly
3. Delegate complex tasks to appropriate worker agents
4. Keep responses concise and friendly

Task types:
- Code: "write code", "implement", "debug", "fix bug" → Coder Agent
- Documentation: "write docs", "explain", "document" → Doc Agent
- Debugging: "debug", "error", "not working" → Debugger Agent
- Complex: Multi-step tasks → DAG orchestration
"""

# 记忆配置
[agents.receptionist.memory]
namespace = "receptionist"
categories = ["conversation", "user_preferences"]
max_context_entries = 5

# IM 接入配置
[agents.receptionist.channels]
matrix = { enabled = true, room_id = "!abc123:matrix.org" }
telegram = { enabled = true, bot_token = "${TELEGRAM_BOT_TOKEN}" }
discord = { enabled = false }
```

**实现**:
```rust
// cis-core/src/agent/receptionist.rs
use cis_traits::{Agent, AgentRuntime};
use cis_memory::MemoryService;
use cis_p2p::P2PNetwork;

pub struct ReceptionistAgent {
    runtime: AgentRuntime,
    memory: Arc<MemoryService>,
    p2p: Arc<P2PNetwork>,
    worker_agents: HashMap<String, WorkerAgent>,
    model: String,
}

impl ReceptionistAgent {
    pub async fn handle_message(&self, message: &IMMessage) -> anyhow::Result<IMMessage> {
        // 1. 分类任务
        let task_type = self.classify_task(&message.content).await?;

        match task_type {
            TaskType::Simple => {
                // 直接回答
                self.answer_directly(&message.content).await
            }

            TaskType::Code => {
                // 委派给 Coder Agent
                self.delegate_to_worker("coder", &message.content).await
            }

            TaskType::Documentation => {
                // 委派给 Doc Agent
                self.delegate_to_worker("doc", &message.content).await
            }

            TaskType::Debugging => {
                // 委派给 Debugger Agent
                self.delegate_to_worker("debugger", &message.content).await
            }

            TaskType::Complex => {
                // DAG 编排
                self.orchestrate_dag(&message.content).await
            }
        }
    }

    async fn classify_task(&self, message: &str) -> anyhow::Result<TaskType> {
        // 使用轻量级 LLM 分类
        let classification_prompt = format!(
            "Classify this task into one of: simple, code, documentation, debugging, complex.\n\nTask: {}",
            message
        );

        let response = self.runtime.chat(&classification_prompt, &self.model).await?;
        // 解析分类结果
        parse_task_type(&response)
    }

    async fn delegate_to_worker(&self, worker_name: &str, task: &str) -> anyhow::Result<IMMessage> {
        let worker = self.worker_agents.get(worker_name)
            .ok_or_else(|| anyhow!("Worker agent not found: {}", worker_name))?;

        // 委派任务
        let result = worker.execute(task).await?;

        Ok(IMMessage {
            content: result.output,
            ..Default::default()
        })
    }

    async fn orchestrate_dag(&self, task: &str) -> anyhow::Result<IMMessage> {
        // 创建 DAG
        let mut dag = TaskDag::new();

        // 添加任务节点
        dag.add_node("task1", vec![], TaskLevel::Mechanical { retry: 3 })?;
        dag.add_node("task2", vec!["task1"], TaskLevel::Recommended { .. })?;
        dag.add_node("task3", vec!["task1"], TaskLevel::Confirmed)?;

        // 执行 DAG
        let scheduler = DagScheduler::new();
        let result = scheduler.execute_dag(dag).await?;

        Ok(IMMessage {
            content: format!("DAG execution completed: {:?}", result),
            ..Default::default()
        })
    }
}
```

### 2.2 Worker Agents（工作 Agent）

**定位**: 专门任务执行、可独立运行、按需启动

#### Coder Agent

**职责**: 代码生成、代码审查、bug 修复

**配置**:
```toml
[agents.coder]
name = "coder"
runtime = "claude"
model = "claude-sonnet-4-20250514"  # 强代码能力
temperature = 0.2  # 低温度，确定性输出
system_prompt = """
You are an expert programmer agent.

Your responsibilities:
1. Write clean, efficient, well-documented code
2. Review code for bugs and best practices
3. Fix bugs and refactor code
4. Follow the project's coding conventions

Always:
- Include error handling
- Add comments for complex logic
- Follow language idioms
- Consider edge cases
"""

# 记忆配置
[agents.coder.memory]
namespace = "coder"
categories = ["code_snippets", "project_conventions", "bug_fixes"]
max_context_entries = 10

# 工具配置
[agents.coder.tools]
file_read = true
file_write = true
shell = true
git = true
browser = false
```

#### Doc Agent

**职责**: 文档编写、技术解释、API 文档

**配置**:
```toml
[agents.doc]
name = "doc"
runtime = "opencode"
model = "glm-4.7-free"  # 成本低、文档能力强
temperature = 0.5
system_prompt = """
You are a technical writer agent.

Your responsibilities:
1. Write clear, concise documentation
2. Explain technical concepts
3. Generate API documentation
4. Create tutorials and guides

Always:
- Use simple language
- Include examples
- Structure with headings and lists
- Define jargon
"""

# 记忆配置
[agents.doc.memory]
namespace = "doc"
categories = ["documentation", "api_specs", "tutorials"]
max_context_entries = 8
```

#### Debugger Agent

**职责**: 调试、错误分析、根因分析

**配置**:
```toml
[agents.debugger]
name = "debugger"
runtime = "kimi"
model = "kimi-latest"  # 推理能力强
temperature = 0.3
system_prompt = """
You are a debugging expert agent.

Your responsibilities:
1. Analyze error messages
2. Identify root causes
3. Suggest fixes
4. Verify solutions

Debugging process:
1. Gather information (logs, stack traces, code)
2. Analyze symptoms
3. Form hypotheses
4. Test hypotheses
5. Propose solutions
"""

# 记忆配置
[agents.debugger.memory]
namespace = "debugger"
categories = ["error_logs", "solutions", "root_causes"]
max_context_entries = 15
```

### 2.3 Remote Agent（跨设备 Agent）

**定位**: 通过 P2P 网络调用其他设备的 Agent

**场景**:
1. **资源互补**: 设备 A 有 GPU，设备 B 有大内存
2. **地理分布**: 在不同地区执行任务
3. **负载均衡**: 分散任务到多个设备
4. **容错备份**: 主设备故障时切换到备用设备

**架构**:
```
┌─────────────────────────────────────────────────────────────┐
│  Device A (主设备)                                            │
│  ├─ Receptionist Agent                                       │
│  ├─ Coder Agent (本地)                                       │
│  └─ P2P Network ←───┐                                       │
└──────────────────────┼──────────────────────────────────────┘
                       │ P2P Connection
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  Device B (远程设备)                                          │
│  ├─ Doc Agent (远程)                                         │
│  ├─ Debugger Agent (远程)                                    │
│  └─ P2P Network                                              │
└─────────────────────────────────────────────────────────────┘
```

**实现**:
```rust
// cis-core/src/agent/remote.rs
use cis_p2p::{P2PNetwork, PeerId};
use cis_traits::{Agent, AgentRuntime};

pub struct RemoteAgent {
    local_runtime: AgentRuntime,
    p2p: Arc<P2PNetwork>,
    remote_peer_id: PeerId,
    remote_agent_name: String,
}

impl RemoteAgent {
    pub async fn execute(&self, task: &str) -> anyhow::Result<TaskResult> {
        // 1. 构建远程调用消息
        let message = P2PMessage {
            type: "agent_execute",
            payload: json!({
                "agent": self.remote_agent_name,
                "task": task,
            }),
        };

        // 2. 通过 P2P 网络发送
        let response = self.p2p.send_request(&self.remote_peer_id, &message).await?;

        // 3. 等待远程 Agent 执行完成
        let result: TaskResult = serde_json::from_str(&response)?;

        Ok(result)
    }
}

// 在 Receptionist Agent 中使用
impl ReceptionistAgent {
    async fn delegate_to_remote(&self, device_id: &str, agent_name: &str, task: &str) -> anyhow::Result<IMMessage> {
        // 1. 通过 DID 查找远程设备
        let peer_id = self.p2p.resolve_did(device_id).await?;

        // 2. 创建远程 Agent 代理
        let remote_agent = RemoteAgent::new(peer_id, agent_name);

        // 3. 委派任务
        let result = remote_agent.execute(task).await?;

        Ok(IMMessage {
            content: result.output,
            ..Default::default()
        })
    }
}
```

---

## 3. 记忆分组策略

### 3.1 三级隔离

```
┌─────────────────────────────────────────────────────────────┐
│  记忆隔离层次结构                                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Level 1: Agent 级隔离                                        │
│  ├─ receptionist/  (Receptionist Agent 专用)                │
│  ├─ coder/          (Coder Agent 专用)                      │
│  ├─ doc/            (Doc Agent 专用)                        │
│  └─ debugger/       (Debugger Agent 专用)                   │
│                                                             │
│  Level 2: Task 级隔离                                         │
│  ├─ task_001/       (具体任务 ID)                           │
│  ├─ task_002/       (具体任务 ID)                           │
│  └─ ...                                                      │
│                                                             │
│  Level 3: Device 级隔离                                      │
│  ├─ device_local/   (本地设备)                              │
│  ├─ device_remote_A/  (远程设备 A)                          │
│  └─ device_remote_B/  (远程设备 B)                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 记忆键命名规范

```rust
// 格式: {agent_namespace}/{task_id}/{device_id}/{key}

// 示例 1: Receptionist Agent 的记忆
"receptionist/user_preferences/theme"                    // 用户偏好
"receptionist/conversations/2025-02-20_001"             // 对话记录

// 示例 2: Coder Agent 的记忆
"coder/task_001/device_local/code_snippet_1"            // 本地代码片段
"coder/task_001/device_remote_A/code_review"            // 远程设备代码审查

// 示例 3: Doc Agent 的记忆
"doc/task_002/device_local/api_spec"                    // API 规范
```

### 3.3 记忆分组实现

```rust
// cis-types/src/memory.rs
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,      // Private | Public
    pub category: MemoryCategory,  // Context | Skill | Result | Error
    pub timestamp: DateTime<Utc>,
    pub ttl: Option<u64>,

    // 新增字段
    pub agent_namespace: Option<String>,  // Agent 命名空间
    pub task_id: Option<String>,          // 任务 ID
    pub device_id: Option<String>,        // 设备 ID
    pub tags: Vec<String>,                // 标签
}

// cis-memory/src/grouped_memory.rs
use cis_traits::Memory;

pub struct GroupedMemoryService {
    storage: Arc<dyn StorageService>,
}

impl GroupedMemoryService {
    /// 按 Agent 命名空间存储
    pub async fn set_for_agent(
        &self,
        agent_name: &str,
        key: &str,
        value: &[u8],
        domain: MemoryDomain,
        category: MemoryCategory,
    ) -> anyhow::Result<()> {
        let full_key = format!("{}/{}", agent_name, key);
        self.storage.set(&full_key, value).await
    }

    /// 按 Agent 命名空间检索
    pub async fn get_for_agent(
        &self,
        agent_name: &str,
        key: &str,
    ) -> anyhow::Result<Option<MemoryEntry>> {
        let full_key = format!("{}/{}", agent_name, key);
        self.storage.get(&full_key).await
    }

    /// 按 Agent + Task 检索
    pub async fn search_in_agent_task(
        &self,
        agent_name: &str,
        task_id: &str,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryEntry>> {
        let prefix = format!("{}/{}/", agent_name, task_id);
        self.storage.search_prefix(&prefix, query, limit).await
    }

    /// 跨设备同步记忆（公域）
    pub async fn sync_to_device(
        &self,
        key: &str,
        target_device_id: &str,
    ) -> anyhow::Result<()> {
        let entry = self.storage.get(key).await?.ok_or_else(|| anyhow!("Key not found"))?;

        // 只同步公域记忆
        if matches!(entry.domain, MemoryDomain::Private) {
            return Ok(());
        }

        // 通过 P2P 网络同步
        self.p2p.sync_memory(&entry, target_device_id).await
    }
}
```

---

## 4. DAG 编排多 Agent 协作

### 4.1 DAG 场景

**场景**: 完整的 CI/CD 流程

```
┌─────────────────────────────────────────────────────────────┐
│  DAG: CI/CD Pipeline                                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  [1] 代码审查 (Coder Agent)                                 │
│       │                                                     │
│       ▼                                                     │
│  [2] 运行测试 (Debugger Agent)                              │
│       │                                                     │
│       ├─ [2a] 单元测试                                       │
│       └─ [2b] 集成测试                                       │
│                                                             │
│       ▼                                                     │
│  [3] 生成文档 (Doc Agent)                                    │
│       │                                                     │
│       ▼                                                     │
│  [4] 部署 (Remote Agent - 部署服务器)                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 DAG 定义

```rust
// cis-scheduler/src/dag/multi_agent.rs
use cis_types::{Task, TaskLevel};
use cis_traits::DagScheduler;

pub struct MultiAgentDag {
    scheduler: DagScheduler,
    agents: HashMap<String, Box<dyn Agent>>,
}

impl MultiAgentDag {
    pub async fn create_cicd_pipeline(&self) -> anyhow::Result<TaskDag> {
        let mut dag = TaskDag::new();

        // Task 1: 代码审查（Mechanical 级，自动执行）
        dag.add_node(Task {
            id: "code_review".to_string(),
            title: "Review code changes".to_string(),
            agent: "coder".to_string(),  // ← 指定 Agent
            level: TaskLevel::Mechanical { retry: 3 },
            ..Default::default()
        })?;

        // Task 2: 单元测试（Recommended 级，可撤销）
        dag.add_node(Task {
            id: "unit_test".to_string(),
            title: "Run unit tests".to_string(),
            agent: "debugger".to_string(),  // ← 指定 Agent
            dependencies: vec!["code_review".to_string()],
            level: TaskLevel::Recommended {
                default_action: true,
                timeout_secs: 300,
            },
            ..Default::default()
        })?;

        // Task 3: 集成测试（Confirmed 级，需确认）
        dag.add_node(Task {
            id: "integration_test".to_string(),
            title: "Run integration tests".to_string(),
            agent: "debugger".to_string(),
            dependencies: vec!["code_review".to_string()],
            level: TaskLevel::Confirmed,
            ..Default::default()
        })?;

        // Task 4: 生成文档（Mechanical 级）
        dag.add_node(Task {
            id: "generate_docs".to_string(),
            title: "Generate API documentation".to_string(),
            agent: "doc".to_string(),  // ← 指定 Agent
            dependencies: vec!["unit_test".to_string(), "integration_test".to_string()],
            level: TaskLevel::Mechanical { retry: 2 },
            ..Default::.default()
        })?;

        // Task 5: 部署（Arbitrated 级，需要多方投票）
        dag.add_node(Task {
            id: "deploy".to_string(),
            title: "Deploy to production".to_string(),
            agent: "remote_deployer".to_string(),  // ← 远程 Agent
            dependencies: vec!["generate_docs".to_string()],
            level: TaskLevel::Arbitrated {
                stakeholders: vec!["devops".to_string(), "manager".to_string()],
            },
            ..Default::default()
        })?;

        Ok(dag)
    }
}
```

### 4.3 DAG 执行

```rust
// cis-scheduler/src/executor/multi_agent_executor.rs
use cis_traits::{Agent, TaskExecutor};

pub struct MultiAgentExecutor {
    agents: HashMap<String, Box<dyn Agent>>,
}

impl TaskExecutor for MultiAgentExecutor {
    async fn execute_task(&self, task: &Task) -> anyhow::Result<TaskResult> {
        // 1. 获取指定的 Agent
        let agent = self.agents.get(&task.agent)
            .ok_or_else(|| anyhow!("Agent not found: {}", task.agent))?;

        // 2. 执行任务
        let result = agent.execute(&task.prompt).await?;

        // 3. 保存结果到记忆（按 Agent 分组）
        agent.memory().set_for_agent(
            &task.agent,
            &format!("task_result_{}", task.id),
            serde_json::to_vec(&result)?.as_slice(),
            MemoryDomain::Public,
            MemoryCategory::Result,
        ).await?;

        Ok(result)
    }
}
```

---

## 5. 跨设备 P2P 调用

### 5.1 P2P 网络拓扑

```
┌─────────────────────────────────────────────────────────────┐
│  CIS P2P 网络                                               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐         ┌──────────────┐                │
│  │ Device A     │         │ Device B     │                │
│  │ (主设备)      │◄────────┤(远程设备)     │                │
│  │              │  P2P    │              │                │
│  │ ├─ Reception │         │ ├─ Doc Agent │                │
│  │ ├─ Coder     │         │ ├─ Debugger  │                │
│  │ └─ Debugger  │         │ └─ Deploy    │                │
│  └──────────────┘         └──────────────┘                │
│         │                         │                         │
│         └─────────────────────────┼─────────────────────┐ │
│                                   ▼                     │ │
│                          ┌──────────────┐               │ │
│                          │ Device C     │               │ │
│                          │(GPU 服务器)   │               │ │
│                          │ ├─ Training  │               │ │
│                          │ └─ Inference │               │ │
│                          └──────────────┘               │ │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 设备发现

```rust
// cis-p2p/src/discovery/agent_discovery.rs
use cis_p2p::{P2PNetwork, PeerId};
use cis_identity::Did;

pub struct AgentDiscovery {
    p2p: Arc<P2PNetwork>,
}

impl AgentDiscovery {
    /// 查找提供特定 Agent 的远程设备
    pub async fn find_devices_with_agent(
        &self,
        agent_name: &str,
    ) -> anyhow::Result<Vec<DeviceAgentInfo>> {
        // 1. 通过 mDNS/DHT 发现设备
        let peers = self.p2p.discover_peers().await?;

        // 2. 筛选提供该 Agent 的设备
        let mut results = Vec::new();
        for peer in peers {
            // 查询设备提供的 Agents
            let agents = self.query_device_agents(&peer.id).await?;
            if agents.contains(&agent_name.to_string()) {
                results.push(DeviceAgentInfo {
                    device_id: peer.did.to_string(),
                    peer_id: peer.id.clone(),
                    agent_name: agent_name.to_string(),
                    capabilities: self.get_device_capabilities(&peer.id).await?,
                });
            }
        }

        Ok(results)
    }

    /// 查询设备提供的 Agents
    async fn query_device_agents(&self, peer_id: &PeerId) -> anyhow::Result<Vec<String>> {
        let message = P2PMessage {
            type: "list_agents",
            payload: json!({}),
        };

        let response = self.p2p.send_request(peer_id, &message).await?;
        let info: DeviceInfo = serde_json::from_str(&response)?;
        Ok(info.agents)
    }
}
```

### 5.3 远程 Agent 调用流程

```rust
// cis-core/src/agent/remote_call.rs
use cis_p2p::P2PNetwork;

pub struct RemoteAgentCaller {
    p2p: Arc<P2PNetwork>,
    discovery: AgentDiscovery,
}

impl RemoteAgentCaller {
    /// 跨设备调用 Agent
    pub async fn call_remote_agent(
        &self,
        agent_name: &str,
        task: &str,
        preferences: DevicePreference,
    ) -> anyhow::Result<TaskResult> {
        // 1. 查找提供该 Agent 的设备
        let devices = self.discovery.find_devices_with_agent(agent_name).await?;

        if devices.is_empty() {
            return Err(anyhow!("No devices found with agent: {}", agent_name));
        }

        // 2. 根据偏好选择设备
        let selected_device = self.select_device(devices, preferences).await?;

        // 3. 调用远程 Agent
        let message = P2PMessage {
            type: "agent_execute",
            payload: json!({
                "agent": agent_name,
                "task": task,
            }),
        };

        let response = self.p2p.send_request(&selected_device.peer_id, &message).await?;
        let result: TaskResult = serde_json::from_str(&response)?;

        // 4. 同步记忆（如果需要）
        if result.sync_memory {
            self.sync_result_memory(&result, &selected_device.device_id).await?;
        }

        Ok(result)
    }

    /// 根据偏好选择设备
    async fn select_device(
        &self,
        devices: Vec<DeviceAgentInfo>,
        preference: DevicePreference,
    ) -> anyhow::Result<DeviceAgentInfo> {
        match preference {
            DevicePreference::Local => {
                // 优先选择本地设备
                devices.into_iter()
                    .find(|d| d.device_id == "local")
                    .ok_or_else(|| anyhow!("Local device not found"))
            }

            DevicePreference::LowLatency => {
                // 选择延迟最低的设备
                let mut best = None;
                let mut best_latency = u64::MAX;

                for device in devices {
                    let latency = self.p2p.ping(&device.peer_id).await?;
                    if latency < best_latency {
                        best_latency = latency;
                        best = Some(device);
                    }
                }

                best.ok_or_else(|| anyhow!("No devices available"))
            }

            DevicePreference::HighPerformance => {
                // 选择性能最好的设备（GPU、大内存）
                devices.into_iter()
                    .max_by_key(|d| d.capabilities.gpu_score + d.capabilities.memory_score)
                    .ok_or_else(|| anyhow!("No devices available"))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DevicePreference {
    Local,          // 本地优先
    LowLatency,     // 低延迟优先
    HighPerformance,// 高性能优先
}
```

---

## 6. 降低幻觉机制

### 6.1 多重过滤策略

```rust
// cis-memory/src/loader/anti_hallucination.rs
use cis_traits::Memory;

pub struct AntiHallucinationLoader {
    memory: Arc<dyn Memory>,
    min_relevance: f64,
    require_source: bool,
}

impl AntiHallucinationLoader {
    pub async fn load_context_safe(&self, query: &str) -> anyhow::Result<MemoryContext> {
        // 1. 检索相关记忆
        let entries = self.memory.hybrid_search(query, 10, None, None).await?;

        // 2. Layer 1: 相关性过滤
        let relevant: Vec<_> = entries
            .into_iter()
            .filter(|e| e.final_score >= self.min_relevance)
            .collect();

        if relevant.is_empty() {
            warn!("No relevant memories found for query: {}", query);
            return Ok(MemoryContext::empty());
        }

        // 3. Layer 2: 不可信记忆过滤
        let trusted: Vec<_> = relevant
            .into_iter()
            .filter(|e| !is_untrusted_memory(&e.key))
            .collect();

        // 4. Layer 3: 来源验证
        if self.require_source {
            let verified: Vec<_> = trusted
                .into_iter()
                .filter(|e| e.source.is_some())  // 必须有来源
                .collect();

            Ok(MemoryContext::from_entries(verified))
        } else {
            Ok(MemoryContext::from_entries(trusted))
        }
    }
}

/// 判断是否为不可信记忆
fn is_untrusted_memory(key: &str) -> bool {
    let key_lower = key.to_lowercase();

    // AI 生成的总结
    if key_lower.contains("ai_summary_") ||
       key_lower.contains("assistant_resp_") ||
       key_lower.contains("llm_generated_") {
        return true;
    }

    // 未验证的外部数据
    if key_lower.starts_with("external_") && !key_lower.contains("_verified_") {
        return true;
    }

    // 用户标记的不可信
    if key_lower.starts_with("untrusted_") {
        return true;
    }

    false
}
```

### 6.2 引用追踪

```rust
// cis-core/src/agent/citation_tracker.rs
pub struct CitationTracker {
    extracted_citations: HashMap<String, Vec<String>>,
}

impl CitationTracker {
    /// 从 LLM 输出中提取引用
    pub fn extract_citations(&self, output: &str) -> Vec<String> {
        let re = Regex::new(r"\[(memory|source):([^\]]+)\]").unwrap();
        let mut citations = Vec::new();

        for cap in re.captures_iter(output) {
            if let Some(source) = cap.get(2) {
                citations.push(source.as_str().to_string());
            }
        }

        citations
    }

    /// 验证引用是否在上下文中
    pub async fn verify_citations(
        &self,
        citations: &[String],
        context_entries: &[MemoryEntry],
    ) -> Vec<CitationValidationResult> {
        let mut results = Vec::new();

        for citation in citations {
            let exists = context_entries.iter()
                .any(|e| &e.key == citation);

            results.push(CitationValidationResult {
                citation: citation.clone(),
                valid: exists,
                entry: if exists {
                    context_entries.iter().find(|e| &e.key == citation).cloned()
                } else {
                    None
                },
            });
        }

        results
    }
}

#[derive(Debug, Clone)]
pub struct CitationValidationResult {
    pub citation: String,
    pub valid: bool,
    pub entry: Option<MemoryEntry>,
}
```

### 6.3 Prompt 工程降低幻觉

```rust
// cis-core/src/agent/prompts/anti_hallucination.rs
pub const ANTI_HALLUCINATION_SYSTEM_PROMPT: &str = r#"
You are a helpful assistant with access to a memory system.

**CRITICAL RULES TO AVOID HALLUCINATION:**

1. **Only use facts from the provided memory context**
   - If the answer is not in the context, say "I don't have enough information"
   - Do NOT make up facts or guess

2. **Cite your sources**
   - Use format: `[memory:key_name]` when referencing memory
   - Example: "According to [memory:project_config], the database URL is..."

3. **Mark uncertainty**
   - If you're uncertain, mark with [UNCERTAIN]
   - Example: "The database URL is postgres://localhost:5432/mydb [UNCERTAIN]"

4. **Verify before stating**
   - Check if the fact is in the memory context
   - If not, explicitly state you don't know

**Example of GOOD response:**
"Based on [memory:user_preferences], you prefer dark mode. The database URL according to [memory:project_config] is postgres://localhost:5432/mydb [UNCERTAIN - may have changed]."

**Example of BAD response:**
"You prefer dark mode. The database URL is postgres://localhost:5432/mydb (wrong - not verified)"

**Memory Context:**
{memory_context}

**User Question:**
{user_question}
"#;
```

---

## 7. 与 ZeroClaw 的对比

### 7.1 架构对比

| 维度 | ZeroClaw | CIS v1.2.0 (本文设计) |
|-----|----------|---------------------|
| **Agent 模式** | 单 Agent + Delegate Tool | 主 Agent + Worker Agents + Remote Agents |
| **任务执行** | 顺序执行 | DAG 并行执行 |
| **记忆隔离** | session_id | Agent/Task/Device 三级隔离 |
| **跨设备** | ❌ 不支持 | ✅ P2P 网络调用 |
| **降低幻觉** | 相关性过滤 | 相关性 + 来源追踪 + 引用验证 |
| **任务编排** | 简单委派 | 四级决策 + 联邦协调 |

### 7.2 CIS 独特优势

1. **真正的多 Agent 协作**
   - ZeroClaw: Delegate Tool 只是临时创建子上下文
   - CIS: 每个 Worker Agent 是独立进程，可并行执行

2. **DAG 编排能力**
   - ZeroClaw: 顺序执行，无并行
   - CIS: DAG 支持 并行、依赖管理、四级决策

3. **P2P 跨设备调用**
   - ZeroClaw: 不支持
   - CIS: 通过 DID + P2P 调用远程 Agent

4. **联邦协调**
   - ZeroClaw: 单设备
   - CIS: 跨节点 DAG 协调，CRDT 同步

5. **记忆分组**
   - ZeroClaw: 仅 session_id
   - CIS: Agent/Task/Device 三级隔离 + 标签系统

---

## 8. 实施路线图

### Phase 1: 主 Agent + 本地 Worker（Week 1-3）

**目标**: 实现基本的本地多 Agent 架构

**Deliverables**:
- [ ] Receptionist Agent 实现
- [ ] Coder/Doc/Debugger Worker Agents
- [ ] Agent Pool 管理
- [ ] 记忆分组（按 Agent）
- [ ] 基本的任务委派

### Phase 2: DAG 编排（Week 4-6）

**目标**: 实现多 Agent DAG 协作

**Deliverables**:
- [ ] MultiAgentDag 定义
- [ ] MultiAgentExecutor 实现
- [ ] Agent 指定和路由
- [ ] 并行执行支持
- [ ] 四级决策集成

### Phase 3: P2P 跨设备调用（Week 7-9）

**目标**: 实现跨设备 Agent 调用

**Deliverables**:
- [ ] AgentDiscovery 实现
- [ ] RemoteAgentCaller 实现
- [ ] 设备选择策略
- [ ] 远程记忆同步
- [ ] DID 身份验证

### Phase 4: 降低幻觉机制（Week 10-11）

**目标**: 实现完整的防幻觉机制

**Deliverables**:
- [ ] AntiHallucinationLoader
- [ ] CitationTracker
- [ ] 引用验证
- [ ] Prompt 工程
- [ ] 事后验证

### Phase 5: 集成和测试（Week 12-13）

**目标**: 完整的端到端测试

**Deliverables**:
- [ ] 完整的 CI/CD DAG 示例
- [ ] 跨设备调用测试
- [ ] 幻觉检测测试
- [ ] 性能测试
- [ ] 文档完善

---

## 9. 配置示例

### 9.1 完整的多 Agent 配置

```toml
# ~/.cis/config.toml

[agents.receptionist]
name = "receptionist"
runtime = "claude"
model = "claude-haiku-3.5"
temperature = 0.7
enabled = true

[agents.receptionist.memory]
namespace = "receptionist"
max_context_entries = 5

[agents.receptionist.channels]
matrix = { enabled = true, room_id = "!abc123:matrix.org", username = "cis-bot" }
telegram = { enabled = true, bot_token = "${TELEGRAM_BOT_TOKEN}" }
discord = { enabled = false }

[agents.coder]
name = "coder"
runtime = "claude"
model = "claude-sonnet-4-20250514"
temperature = 0.2
enabled = true

[agents.coder.memory]
namespace = "coder"
max_context_entries = 10

[agents.coder.tools]
file_read = true
file_write = true
shell = true
git = true
browser = false

[agents.doc]
name = "doc"
runtime = "opencode"
model = "glm-4.7-free"
temperature = 0.5
enabled = true

[agents.doc.memory]
namespace = "doc"
max_context_entries = 8

[agents.debugger]
name = "debugger"
runtime = "kimi"
model = "kimi-latest"
temperature = 0.3
enabled = true

[agents.debugger.memory]
namespace = "debugger"
max_context_entries = 15

# 远程 Agent 配置
[agents.remote_deployer]
name = "remote_deployer"
runtime = "remote"
device_id = "deploy-server-1"
agent = "deployer"
enabled = true

# P2P 配置
[p2p]
enabled = true
listen_port = 7677
bootstrap_nodes = [
    "/dns/cis-bootstrap.example.com/tcp/7677/p2p/12D3KooW..."
]

[p2p.discovery]
enable_mdns = true
enable_dht = true

# DAG 编排配置
[scheduler]
max_parallel_tasks = 5
default_timeout_secs = 600
enable_federation = true

# 幻觉防护配置
[anti_hallucination]
min_relevance_score = 0.7
require_source = false
enable_citation_tracking = true
enable_post_verification = false
```

---

## 10. 使用示例

### 10.1 简单问答

```bash
# 用户在 Matrix 中发送消息
User: "What's the weather today?"

# Receptionist Agent 直接回答（不委派）
Receptionist: "I don't have real-time weather access. Would you like me to check weather.com?"
```

### 10.2 代码任务

```bash
# 用户在 Telegram 中发送消息
User: "Write a Rust function to parse JSON"

# Receptionist Agent 委派给 Coder Agent
Receptionist: "I'll delegate this to our code expert."

# Coder Agent 执行
Coder Agent: "Here's a Rust function to parse JSON:

```rust
use serde_json::Value;

fn parse_json(input: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(input)
}
```

This function uses the `serde_json` crate for parsing."
```

### 10.3 复杂 DAG 流程

```bash
# 用户在 Discord 中发送消息
User: "Run CI/CD for my PR"

# Receptionist Agent 创建 DAG
Receptionist: "Starting CI/CD pipeline for your PR. This involves:
1. Code review
2. Unit tests
3. Integration tests
4. Generate docs
5. Deploy (requires approval)

I'll keep you updated on progress."

# DAG 执行（多 Agent 并行）
- Coder Agent: Reviewing code...
- Debugger Agent: Running unit tests...
- Debugger Agent: Running integration tests...
- Doc Agent: Generating API docs...

# Task 5 需要批准
Receptionist: "CI/CD pipeline completed tasks 1-4 successfully.
Task 5 (Deploy) requires approval from devops and manager.
@devops @manager Please approve."

# 批准后执行
Remote Deploy Agent: "Deploying to production... Done! Deployment successful."
```

### 10.4 跨设备调用

```bash
# 用户在本地设备上
User: "Train a model on this dataset (large dataset, needs GPU)"

# Receptionist Agent 发现本地没有 GPU
Receptionist: "This task requires a GPU. I found 3 devices with GPUs:
- Device C (GPU Server) - 4x RTX 4090
- Device D (Training Server) - 8x A100
- Device E (Colab) - 1x T4

Which one should I use?"

# 用户选择
User: "Use Device D"

# 调用远程 Agent
Remote Agent (Device D): "Starting training on 8x A100...
Progress: [################----] 80%
Estimated time remaining: 2 hours
```

---

## 11. 总结

### 11.1 核心设计原则

1. **主 Agent 接待** - Receptionist Agent 处理所有 IM 交互
2. **Worker Agent 执行** - 专门 Agent 处理专门任务
3. **P2P 跨设备** - 通过 P2P 网络调用远程 Agent
4. **DAG 编排** - 复杂任务用 DAG 协调多 Agent
5. **记忆分组** - 按 Agent/Task/Device 隔离记忆
6. **降低幻觉** - 多重过滤 + 引用追踪

### 11.2 与 ZeroClaw 的关键区别

| 特性 | ZeroClaw | CIS v1.2.0 |
|-----|----------|-----------|
| Agent 模式 | 单 Agent | 多 Agent（主 + Worker + Remote） |
| 任务执行 | 顺序 | 并行（DAG） |
| 跨设备 | ❌ | ✅ P2P |
| 记忆隔离 | session_id | 三级隔离 |
| 降低幻觉 | 基础过滤 | 引用追踪 + 验证 |

### 11.3 CIS 独特优势

1. ✅ **真正的多 Agent 协作** - 每个 Agent 是独立进程
2. ✅ **P2P 跨设备调用** - 调用远程设备的 Agent
3. ✅ **DAG 编排能力** - 并行执行、依赖管理、四级决策
4. ✅ **联邦协调** - 跨节点 DAG 协调
5. ✅ **记忆分组** - Agent/Task/Device 三级隔离
6. ✅ **降低幻觉** - 引用追踪、来源验证

---

**设计完成时间**: 2026-02-20
**版本**: v1.2.0
**设计师**: GLM
**相关文档**:
- [CIS v1.2.0 Final Plan](../task/CIS_V1.2.0_FINAL_PLAN.md)
- [ZeroClaw Agent 分析](../zeroclaw/zeroclaw_agent_isolation_analysis.md)
- [ZeroClaw Agent 分析 (Kimi)](../zeroclaw/zeroclaw_agent_architecture_analysis_kimi.md)
