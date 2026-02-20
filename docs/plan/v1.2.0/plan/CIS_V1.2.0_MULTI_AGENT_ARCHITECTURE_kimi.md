# CIS v1.2.0 多 Agent 分工架构计划

> **版本**: v1.0
> **日期**: 2026-02-20
> **整合基于**:
> - GLM: `zeroclaw_agent_isolation_analysis.md`
> - GLM: `cis_multi_agent_architecture.md`
> - Kimi: `zeroclaw_agent_architecture_analysis_kimi.md`
> **核心目标**: 发挥 CIS 特色（四级决策、DAG 编排、P2P 跨设备）

---

## 执行摘要

### 架构定位

CIS v1.2.0 采用**真多 Agent 架构**，与 ZeroClaw 的单 Agent + Delegate Tool 有本质区别：

| 维度 | ZeroClaw | CIS v1.2.0 |
|------|----------|------------|
| **Agent 模式** | 单 Agent + Delegate Tool | 多 Agent 实例常驻 |
| **任务拆分** | Tool 级别委派 | Agent 级别分工 + DAG 编排 |
| **跨设备** | ❌ 不支持 | ✅ P2P 跨设备调用 |
| **记忆隔离** | session_id | Agent 命名空间 + Task ID + Device ID |
| **决策机制** | 无 | 四级决策（Mechanical → Arbitrated）|

### 核心架构

```
┌─────────────────────────────────────────────────────────────────┐
│                    CIS 多 Agent 生态系统                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  Receptionist Agent（前台接待）                           │  │
│  │  ├─ IM 接入（Matrix/Telegram/Discord）                    │  │
│  │  ├─ 任务分类 → 四级决策路由                                │  │
│  │  ├─ 轻量级模型（快速响应）                                │  │
│  │  └─ 记忆命名空间: "receptionist/"                        │  │
│  └────────────────┬─────────────────────────────────────────┘  │
│                   │ 委派任务                                     │
│      ┌────────────┼────────────┬──────────────┬─────────────┐   │
│      ▼            ▼            ▼              ▼             ▼   │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐   ┌──────────────┐      │
│  │Coder    │ │Doc      │ │Debugger │   │Remote Agent  │      │
│  │Agent    │ │Agent    │ │Agent    │   │(跨设备 P2P)   │      │
│  │         │ │         │ │         │   │              │      │
│  │Claude   │ │OpenCode │ │Kimi     │   │Remote Device │      │
│  │Sonnet   │ │GLM-4    │ │DeepSeek │   │Worker        │      │
│  └─────────┘ └─────────┘ └─────────┘   └──────────────┘      │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  CIS 核心服务（特色能力）                                  │  │
│  │  ├─ cis-scheduler: DAG 编排 + 四级决策                     │  │
│  │  ├─ cis-memory:    分组记忆 + 来源追踪                     │  │
│  │  ├─ cis-p2p:       跨设备 Agent 发现/调用                  │  │
│  │  └─ cis-identity:  DID 身份 + 联邦协调                     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## 1. Agent 角色定义

### 1.1 Receptionist Agent（主 Agent）

**定位**: 前台接待、IM 交互入口、任务分类与四级决策路由

**核心职责**:
1. **IM 接入**: Matrix, Telegram, Discord, Slack
2. **任务分类**: 使用轻量级 LLM 快速分类
3. **四级决策路由**:
   ```
   ┌─────────────────────────────────────────────────────────┐
   │  Mechanical (自动执行) → 直接委派 Worker Agent           │
   │  Recommended (建议执行) → 倒计时确认后委派               │
   │  Confirmed (需确认) → 人工确认后委派                     │
   │  Arbitrated (需仲裁) → 多方投票后委派                    │
   └─────────────────────────────────────────────────────────┘
   ```
4. **快速响应**: Claude Haiku / GPT-4o-mini，延迟 < 2s

**记忆管理**:
- 命名空间: `receptionist/`
- 存储: 用户偏好、常见问答、任务历史

**配置**:
```toml
[agents.receptionist]
name = "receptionist"
runtime = "claude"
model = "claude-haiku-3.5"
temperature = 0.7
system_prompt = "You are the receptionist for CIS..."

[agents.receptionist.memory]
namespace = "receptionist"
categories = ["conversation", "user_preferences"]
max_context_entries = 5

[agents.receptionist.channels]
matrix = { enabled = true, room_id = "!abc123:matrix.org" }
telegram = { enabled = true, bot_token = "${TELEGRAM_BOT_TOKEN}" }
```

### 1.2 Worker Agents（工作 Agent）

#### Coder Agent
```toml
[agents.coder]
name = "coder"
runtime = "claude"
model = "claude-sonnet-4-20250514"
temperature = 0.2
system_prompt = "You are an expert programmer agent..."

[agents.coder.memory]
namespace = "coder"
categories = ["code_snippets", "project_conventions", "bug_fixes"]
max_context_entries = 10

[agents.coder.tools]
file_read = true
file_write = true
shell = true
git = true
```

#### Doc Agent
```toml
[agents.doc]
name = "doc"
runtime = "opencode"
model = "glm-4.7-free"
temperature = 0.5
system_prompt = "You are a technical writer agent..."

[agents.doc.memory]
namespace = "doc"
categories = ["documentation", "api_specs", "tutorials"]
```

#### Debugger Agent
```toml
[agents.debugger]
name = "debugger"
runtime = "kimi"
model = "kimi-latest"
temperature = 0.3
system_prompt = "You are a debugging expert agent..."

[agents.debugger.memory]
namespace = "debugger"
categories = ["error_logs", "solutions", "root_causes"]
max_context_entries = 15
```

### 1.3 Remote Agent（跨设备 Agent）

**场景**:
- 资源互补（设备 A 有 GPU，设备 B 有大内存）
- 地理分布
- 负载均衡
- 容错备份

**架构**:
```
Device A (主设备)          P2P Connection          Device B (远程设备)
├─ Receptionist Agent  ◄──────────────────────►  ├─ Doc Agent
├─ Coder Agent (本地)                            ├─ Debugger Agent
└─ P2P Network                                   └─ P2P Network
```

---

## 2. 四级决策 + DAG 编排

### 2.1 四级决策机制

```rust
// cis-scheduler/src/decision.rs
pub enum TaskLevel {
    /// 自动执行（Mechanical）
    Mechanical { retry: usize },
    
    /// 建议执行（Recommended）
    Recommended { 
        default_action: bool,
        timeout_secs: u64,
    },
    
    /// 需确认（Confirmed）
    Confirmed,
    
    /// 需仲裁（Arbitrated）
    Arbitrated { 
        stakeholders: Vec<String>,
    },
}
```

**决策流程**:
```
用户请求
    │
    ▼
Receptionist Agent 分类
    │
    ├── 简单问答 ──────────► Mechanical ──► 直接回答
    │
    ├── 代码生成 ──────────► Mechanical ──► Coder Agent
    │
    ├── 部署操作 ──────────► Recommended ──► 倒计时确认 ──► 执行
    │
    ├── 数据库删除 ────────► Confirmed ──► 人工确认 ──► 执行
    │
    └── 生产发布 ──────────► Arbitrated ──► DevOps + Manager 投票 ──► 执行
```

### 2.2 DAG 编排多 Agent 协作

**场景**: CI/CD Pipeline

```
[1] 代码审查 (Coder Agent) ──┐
                              ▼
                    [2] 运行测试 (Debugger Agent)
                              │
            ┌─────────────────┴─────────────────┐
            ▼                                   ▼
    [2a] 单元测试                        [2b] 集成测试
            │                                   │
            └─────────────────┬─────────────────┘
                              ▼
                    [3] 生成文档 (Doc Agent)
                              │
                              ▼
                    [4] 部署 (Remote Agent - 需仲裁)
```

**DAG 定义**:
```rust
// cis-scheduler/src/dag/multi_agent.rs
let mut dag = TaskDag::new();

// Task 1: 代码审查（Mechanical）
dag.add_node(Task {
    id: "code_review".to_string(),
    agent: "coder".to_string(),
    level: TaskLevel::Mechanical { retry: 3 },
    ..Default::default()
})?;

// Task 2: 单元测试（Recommended）
dag.add_node(Task {
    id: "unit_test".to_string(),
    agent: "debugger".to_string(),
    dependencies: vec!["code_review"],
    level: TaskLevel::Recommended {
        default_action: true,
        timeout_secs: 300,
    },
    ..Default::default()
})?;

// Task 5: 部署（Arbitrated）
dag.add_node(Task {
    id: "deploy".to_string(),
    agent: "remote_deployer".to_string(),
    dependencies: vec!["generate_docs"],
    level: TaskLevel::Arbitrated {
        stakeholders: vec!["devops".to_string(), "manager".to_string()],
    },
    ..Default::default()
})?;
```

---

## 3. 记忆分组与幻觉降低

### 3.1 三级记忆隔离

```
Level 1: Agent 级隔离
├─ receptionist/  (Receptionist Agent 专用)
├─ coder/         (Coder Agent 专用)
├─ doc/           (Doc Agent 专用)
└─ debugger/      (Debugger Agent 专用)

Level 2: Task 级隔离
├─ task_001/      (具体任务 ID)
└─ task_002/

Level 3: Device 级隔离
├─ device_local/      (本地设备)
├─ device_remote_A/   (远程设备 A)
└─ device_remote_B/   (远程设备 B)
```

### 3.2 记忆键命名规范

```rust
// 格式: {agent_namespace}/{task_id}/{device_id}/{key}

// 示例
"receptionist/user_preferences/theme"
"coder/task_001/device_local/code_snippet_1"
"coder/task_001/device_remote_A/code_review"
```

### 3.3 降低幻觉的四层过滤

```rust
// cis-memory/src/loader/anti_hallucination.rs
pub struct AntiHallucinationLoader {
    min_relevance: f64,      // Layer 1: 相关性过滤
    exclude_untrusted: bool, // Layer 2: 不可信记忆过滤
    require_source: bool,    // Layer 3: 来源验证
    max_entries: usize,      // Layer 4: 数量限制
}

impl AntiHallucinationLoader {
    pub async fn load_context_safe(&self, query: &str) -> Result<MemoryContext> {
        // 1. Hybrid search
        let entries = self.memory.hybrid_search(query, self.max_entries).await?;
        
        // 2. 相关性过滤
        let relevant: Vec<_> = entries
            .into_iter()
            .filter(|e| e.final_score >= self.min_relevance)
            .collect();
        
        // 3. 不可信记忆过滤
        let trusted: Vec<_> = relevant
            .into_iter()
            .filter(|e| !is_untrusted_memory(&e.key))
            .collect();
        
        // 4. 来源验证
        if self.require_source {
            trusted.into_iter()
                .filter(|e| e.source.is_some())
                .collect()
        } else {
            trusted
        }
    }
}
```

---

## 4. P2P 跨设备 Agent 调用

### 4.1 设备发现

```rust
// cis-p2p/src/discovery/agent_discovery.rs
pub struct AgentDiscovery {
    p2p: Arc<P2PNetwork>,
}

impl AgentDiscovery {
    /// 查找提供特定 Agent 的远程设备
    pub async fn find_devices_with_agent(
        &self,
        agent_name: &str,
    ) -> Result<Vec<DeviceAgentInfo>> {
        // 1. 通过 mDNS/DHT 发现设备
        let peers = self.p2p.discover_peers().await?;
        
        // 2. 筛选提供该 Agent 的设备
        let mut results = Vec::new();
        for peer in peers {
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
}
```

### 4.2 远程 Agent 调用

```rust
// cis-core/src/agent/remote_call.rs
pub struct RemoteAgentCaller {
    p2p: Arc<P2PNetwork>,
    discovery: AgentDiscovery,
}

impl RemoteAgentCaller {
    pub async fn call_remote_agent(
        &self,
        agent_name: &str,
        task: &str,
        preference: DevicePreference,
    ) -> Result<TaskResult> {
        // 1. 查找设备
        let devices = self.discovery.find_devices_with_agent(agent_name).await?;
        
        // 2. 根据偏好选择设备
        let selected = self.select_device(devices, preference).await?;
        
        // 3. 调用远程 Agent
        let message = P2PMessage {
            type: "agent_execute",
            payload: json!({
                "agent": agent_name,
                "task": task,
                "dag_context": ...,
            }),
        };
        
        let response = self.p2p.send_request(&selected.peer_id, &message).await?;
        let result: TaskResult = serde_json::from_str(&response)?;
        
        // 4. 同步记忆（公域）
        if result.sync_memory {
            self.sync_result_memory(&result, &selected.device_id).await?;
        }
        
        Ok(result)
    }
}
```

---

## 5. 实施计划

### Phase 1: 基础 Agent 架构（Week 1-3）

- [ ] Receptionist Agent 实现
- [ ] Worker Agent 基础框架
- [ ] Agent 间通信机制
- [ ] 四级决策路由

### Phase 2: 记忆分组（Week 4-5）

- [ ] 三级记忆隔离实现
- [ ] AntiHallucinationLoader
- [ ] 来源追踪机制
- [ ] 记忆同步（P2P）

### Phase 3: DAG 编排（Week 6-7）

- [ ] MultiAgentDag 实现
- [ ] 四级决策集成到 DAG
- [ ] 跨 Agent 任务依赖
- [ ] 联邦协调器

### Phase 4: P2P 跨设备（Week 8-9）

- [ ] Agent 发现服务
- [ ] 远程 Agent 调用
- [ ] 设备偏好路由
- [ ] 记忆跨设备同步

### Phase 5: 集成测试（Week 10-11）

- [ ] 端到端测试
- [ ] 性能基准测试
- [ ] 安全审计
- [ ] 文档完善

---

## 6. 参考文档

- [GLM: ZeroClaw Agent 隔离分析](../zeroclaw/zeroclaw_agent_isolation_analysis.md)
- [GLM: CIS 多 Agent 架构](../glm/cis_multi_agent_architecture.md)
- [Kimi: ZeroClaw Agent 架构分析](../zeroclaw/zeroclaw_agent_architecture_analysis_kimi.md)

---

**计划完成时间**: 2026-02-20
**整合者**: Kimi
**版本**: CIS v1.2.0
