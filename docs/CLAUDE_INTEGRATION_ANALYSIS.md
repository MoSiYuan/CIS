# Claude 在 CIS 系统中的集成分析与接口抽象

## 📋 文档概览

**目的**: 分析 Claude CLI 在 CIS 系统中的集成情况，评估耦合度，为替换为 OpenCode 提供技术依据

**分析日期**: 2026-02-07

**CIS 版本**: main分支

---

## 🏗️ 整体架构分析

### 系统分层架构

```
┌───────────────────────────────────────────────────────────────┐
│                    用户接口层 (CLI)                           │
│  cis-node/src/commands/{dag.rs, agent.rs, doctor.rs}         │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│                   DAG 调度与执行层                            │
│  cis-core/src/scheduler/                                      │
│  cis-core/src/agent/cluster/executor.rs                       │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│                  Agent Provider 层                           │
│  cis-core/src/agent/mod.rs                                    │
│  cis-core/src/agent/providers/                                │
│  - claude.rs                                                  │
│  - kimi.rs                                                    │
│  - aider.rs                                                   │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│                    AI Provider 层                            │
│  cis-core/src/ai/mod.rs                                       │
│  cis-core/src/ai/claude.rs                                    │
│  - 定义 AiProvider trait                                       │
│  - 实现 ClaudeCliProvider                                      │
└───────────────────────────────────────────────────────────────┘
                              ↓
┌───────────────────────────────────────────────────────────────┐
│                  外部 CLI 工具层                              │
│  claude (Claude Code CLI)                                     │
│  opencode (OpenCode CLI)                                      │
└───────────────────────────────────────────────────────────────┘
```

---

## 🔍 Claude 集成点详细分析

### 1. AI Provider 层 (核心抽象)

**文件**: `cis-core/src/ai/mod.rs`

#### 1.1 核心接口定义

```rust
/// AI Provider 统一接口
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;

    /// 检查是否可用
    async fn available(&self) -> bool;

    /// 简单对话
    async fn chat(&self, prompt: &str) -> Result<String>;

    /// 带上下文的对话
    async fn chat_with_context(
        &self,
        system: &str,
        messages: &[Message],
    ) -> Result<String>;

    /// 带 RAG 上下文的对话
    async fn chat_with_rag(
        &self,
        prompt: &str,
        ctx: Option<&ConversationContext>,
    ) -> Result<String>;

    /// 生成结构化数据（JSON）
    async fn generate_json(
        &self,
        prompt: &str,
        schema: &str,
    ) -> Result<serde_json::Value>;
}
```

#### 1.2 Claude 实现

**文件**: `cis-core/src/ai/claude.rs`

**命令行调用方式**:
```bash
claude --model claude-sonnet-4-20250514 \
      --max-tokens 4096 \
      --temperature 0.7 \
      --prompt "your prompt here"

claude --model claude-sonnet-4-20250514 \
      --system "You are a helpful assistant" \
      --user "User message" \
      --assistant "Assistant message" \
      --user "Another user message"
```

**关键实现细节**:
```rust
// 简单对话实现
async fn chat(&self, prompt: &str) -> Result<String> {
    let mut cmd = Command::new("claude");
    cmd.arg("--model").arg(&self.config.model)
       .arg("--max-tokens").arg(self.config.max_tokens.to_string())
       .arg("--temperature").arg(self.config.temperature.to_string())
       .arg("--")
       .arg(prompt)
       .stdin(Stdio::null())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped());

    let output = cmd.output().await?;
    Ok(String::from_utf8(output.stdout)?)
}

// 带上下文对话实现
async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String> {
    let mut cmd = Command::new("claude");
    cmd.arg("--model").arg(&self.config.model)
       .arg("--system").arg(system);

    for msg in messages {
        match msg.role {
            Role::User => { cmd.arg("--user").arg(&msg.content); }
            Role::Assistant => { cmd.arg("--assistant").arg(&msg.content); }
            _ => {}
        }
    }

    // ... 执行命令
}
```

**耦合度评估**: ⭐⭐ (低耦合)
- ✅ 通过 trait 抽象，实现可替换
- ✅ 使用标准命令行调用
- ✅ 返回值统一为 String 或 serde_json::Value
- ⚠️ 部分依赖 Claude CLI 特定参数（`--user`, `--assistant`）

---

### 2. Agent Provider 层 (DAG 执行层)

**文件**: `cis-core/src/agent/providers/claude.rs`

#### 2.1 核心接口定义

```rust
/// Agent Provider 统一接口
#[async_trait]
pub trait AgentProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn available(&self) -> bool;
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse>;
    async fn execute_stream(
        &self,
        req: AgentRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse>;
    fn capabilities(&self) -> AgentCapabilities;
}
```

#### 2.2 Claude Agent 实现

**命令行调用方式**:
```bash
# 非流式执行
claude --model claude-sonnet-4-20250514 \
      --system "You are a helpful assistant" \
      --prompt "your prompt"

# 流式执行
claude --model claude-sonnet-4-20250514 \
      --stream \
      --prompt "your prompt"
```

**关键实现细节**:
```rust
pub struct ClaudeProvider {
    config: AgentConfig,
}

impl ClaudeProvider {
    /// 构建 claude 命令
    fn build_command(&self, req: &AgentRequest) -> Command {
        let mut cmd = Command::new("claude");

        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        if let Some(ref system) = req.system_prompt {
            cmd.arg("--system").arg(system);
        }

        cmd
    }
}

async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
    let mut cmd = self.build_command(&req);
    cmd.arg("--").arg(&req.prompt);

    let output = cmd.output().await?;

    Ok(AgentResponse {
        content: String::from_utf8_lossy(&output.stdout).to_string(),
        token_usage: None,
        metadata: [("exit_code".to_string(), serde_json::json!(output.status.code()))]
            .into_iter()
            .collect(),
    })
}
```

**耦合度评估**: ⭐⭐⭐ (中等耦合)
- ✅ 通过 AgentProvider trait 抽象
- ✅ 统一的 AgentRequest/AgentResponse 结构
- ⚠️ 依赖 Claude CLI 特定参数（`--system`, `--stream`）
- ⚠️ 工作目录通过 `.current_dir()` 设置，不同 CLI 可能处理方式不同

---

### 3. Agent Cluster 层 (DAG 并发执行)

**文件**: `cis-core/src/agent/cluster/executor.rs`

#### 3.1 执行流程

```
DAG Run 创建
     ↓
识别 Ready 任务
     ↓
并发启动 Agent Sessions (max_workers 限制)
     ↓
Session 监控任务
     ↓
处理 Session 事件 (Completed, Failed, Blocked)
     ↓
更新 DAG 节点状态
     ↓
继续执行后续任务
```

#### 3.2 Claude 集成点

**默认 Agent 配置**:
```rust
impl Default for AgentClusterConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            default_agent: AgentType::Claude,  // ← Claude 默认
            base_work_dir: std::env::temp_dir().join("cis").join("dag-sessions"),
            enable_context_injection: true,
            auto_attach_on_block: false,
            task_timeout_secs: 3600,
        }
    }
}
```

**Session 创建**:
```rust
let session_id = self.session_manager.create_session(
    run_id,
    task_id,
    agent_type,  // AgentType::Claude
    &full_prompt,
    &work_dir,
    &upstream_context,
).await?;
```

**耦合度评估**: ⭐⭐ (低耦合)
- ✅ 使用 AgentType 枚举，可扩展
- ✅ 通过 SessionManager 管理所有 Agent
- ✅ 执行逻辑与具体 Agent 实现解耦
- ⚠️ 默认 Agent 硬编码为 Claude

---

### 4. Agent Session 层 (PTY 交互)

**文件**: `cis-core/src/agent/cluster/session.rs`

#### 4.1 Claude 命令构建

```rust
fn build_agent_command(&self) -> Result<CommandBuilder> {
    let cmd_name = match self.agent_type {
        AgentType::Claude => "claude",  // ← 硬编码
        AgentType::Kimi => "kimi",
        AgentType::Aider => "aider",
        AgentType::Custom => {
            return Err(CisError::configuration(
                "Custom agent type not supported for cluster sessions",
            ));
        }
    };

    let mut cmd = CommandBuilder::new(cmd_name);
    cmd.cwd(&self.work_dir);
    cmd.env("CIS_PROJECT_PATH", self.work_dir.to_string_lossy().as_ref());
    cmd.env("CIS_SESSION_ID", self.id.to_string());

    // Claude/Kimi 特定标志
    match self.agent_type {
        AgentType::Claude | AgentType::Kimi => {
            cmd.arg("--dangerously-skip-permissions");  // ← Claude 特定
        }
        _ => {}
    }

    Ok(cmd)
}
```

**耦合度评估**: ⭐⭐⭐⭐ (高耦合)
- ✅ 通过 AgentType 枚举区分
- ⚠️ 命令名硬编码在 match 语句中
- ⚠️ Claude 特定标志 `--dangerously-skip-permissions`
- ⚠️ 新增 Agent 需要修改多处代码

---

### 5. DAG 命令行层

**文件**: `cis-node/src/commands/dag.rs`

#### 5.1 Agent 执行命令

```bash
cis dag execute <run-id> --use-agent --max-workers 4
```

**执行逻辑**:
```rust
DagCommands::Execute { run_id, use_agent, max_workers } => {
    if use_agent {
        execute_run_agent(run_id.as_deref(), max_workers).await?;
    } else {
        execute_run(run_id.as_deref()).await?;
    }
}
```

**Agent Cluster 执行**:
```rust
async fn execute_run_agent(run_id: Option<&str>, max_workers: usize) -> Result<()> {
    let config = AgentClusterConfig {
        max_workers,
        ..Default::default()  // 默认使用 Claude
    };

    let executor = AgentClusterExecutor::new(config)?;
    let report = executor.execute_run(run).await?;

    // 打印报告
    Ok(())
}
```

**耦合度评估**: ⭐ (极低耦合)
- ✅ 通过配置指定 Agent
- ✅ 命令行参数支持
- ✅ 执行逻辑完全解耦

---

## 📊 耦合度总结

### 耦合点统计

| 层级 | 文件 | 耦合度 | 主要问题 |
|------|------|--------|----------|
| AI Provider | `cis-core/src/ai/claude.rs` | ⭐⭐ 低 | Claude 特定参数 |
| Agent Provider | `cis-core/src/agent/providers/claude.rs` | ⭐⭐⭐ 中 | `--system`, `--stream` |
| Agent Cluster | `cis-core/src/agent/cluster/executor.rs` | ⭐⭐ 低 | 默认 Agent |
| Agent Session | `cis-core/src/agent/cluster/session.rs` | ⭐⭐⭐⭐ 高 | 命令名硬编码 |
| DAG 命令 | `cis-node/src/commands/dag.rs` | ⭐ 极低 | 无 |

### 耦合度分布

```
极低 (⭐):     20%  (1/5)
低   (⭐⭐):    40%  (2/5)
中   (⭐⭐⭐):  20%  (1/5)
高   (⭐⭐⭐⭐): 20%  (1/5)
```

**总体评估**: ✅ **整体耦合度较低，适合进行 OpenCode 替换**

---

## 🔧 解耦方案

### 方案 A: 扩展 AgentType 枚举 (推荐)

**优点**:
- 最小化代码改动
- 保持现有架构
- 向后兼容

**改动点**:
1. 添加 `AgentType::OpenCode`
2. 实现 `OpenCodeProvider`
3. 更新 Session 构建逻辑

**代码示例**:
```rust
// cis-core/src/agent/mod.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Claude,
    Kimi,
    Aider,
    OpenCode,  // ← 新增
    Custom,
}

// cis-core/src/agent/cluster/session.rs
fn build_agent_command(&self) -> Result<CommandBuilder> {
    let cmd_name = match self.agent_type {
        AgentType::Claude => "claude",
        AgentType::Kimi => "kimi",
        AgentType::Aider => "aider",
        AgentType::OpenCode => "opencode",  // ← 新增
        AgentType::Custom => { /* ... */ }
    };

    // OpenCode 不需要 --dangerously-skip-permissions
    match self.agent_type {
        AgentType::Claude | AgentType::Kimi => {
            cmd.arg("--dangerously-skip-permissions");
        }
        AgentType::OpenCode => {
            cmd.arg("--format").arg("json");  // OpenCode 特定
        }
        _ => {}
    }

    Ok(cmd)
}
```

---

### 方案 B: 配置驱动的命令构建

**优点**:
- 完全解耦命令行参数
- 易于扩展新 Agent
- 支持用户自定义

**改动点**:
1. 定义 `AgentCommandConfig` 结构
2. 从配置文件加载命令模板
3. 运行时动态构建命令

**代码示例**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub stream_args: Vec<String>,
}

impl Default for AgentCommandConfig {
    fn default() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec![
                "--model".to_string(),
                "claude-sonnet-4-20250514".to_string(),
                "--dangerously-skip-permissions".to_string(),
            ],
            env: vec![],
            stream_args: vec!["--stream".to_string()],
        }
    }
}

// 配置文件: config.agent.toml
[agent.claude]
command = "claude"
args = ["--model", "claude-sonnet-4-20250514", "--dangerously-skip-permissions"]

[agent.opencode]
command = "opencode"
args = ["--format", "json", "--model", "opencode/big-pickle"]
```

---

### 方案 C: 抽象命令构建器 (最灵活)

**优点**:
- 完全抽象命令行参数
- 支持复杂参数组合
- 易于测试

**缺点**:
- 实现复杂度高
- 可能过度设计

**代码示例**:
```rust
pub trait AgentCommandBuilder: Send + Sync {
    fn build_command(&self, req: &AgentRequest) -> Result<Command>;
    fn build_stream_command(&self, req: &AgentRequest, tx: mpsc::Sender<String>) -> Result<Command>;
    fn supports_agent_type(&self, agent_type: AgentType) -> bool;
}

pub struct ClaudeCommandBuilder {
    config: AgentConfig,
}

impl AgentCommandBuilder for ClaudeCommandBuilder {
    fn build_command(&self, req: &AgentRequest) -> Result<Command> {
        let mut cmd = Command::new("claude");
        // ... Claude 特定逻辑
        Ok(cmd)
    }
}

pub struct OpenCodeCommandBuilder {
    config: AgentConfig,
}

impl AgentCommandBuilder for OpenCodeCommandBuilder {
    fn build_command(&self, req: &AgentRequest) -> Result<Command> {
        let mut cmd = Command::new("opencode");
        cmd.arg("run").arg("--format").arg("json");
        // ... OpenCode 特定逻辑
        Ok(cmd)
    }
}
```

---

## 📝 OpenCode 替换影响分析

### 需要修改的文件

| 文件 | 改动类型 | 复杂度 |
|------|----------|--------|
| `cis-core/src/ai/mod.rs` | 添加 OpenCode 模块 | ⭐ 低 |
| `cis-core/src/ai/opencode.rs` | 新增文件 | ⭐⭐ 中 |
| `cis-core/src/agent/mod.rs` | 添加 AgentType::OpenCode | ⭐ 低 |
| `cis-core/src/agent/providers/opencode.rs` | 新增文件 | ⭐⭐ 中 |
| `cis-core/src/agent/cluster/session.rs` | 更新命令构建逻辑 | ⭐⭐⭐ 高 |
| `config.example.toml` | 添加 OpenCode 配置 | ⭐ 低 |

### 兼容性风险

| 风险 | 影响 | 缓解措施 |
|------|------|----------|
| **命令行参数差异** | 中 | 适配层翻译参数 |
| **输出格式不同** | 中 | 统一解析逻辑 |
| **流式输出格式** | 低 | 使用 `--format json` |
| **工作目录处理** | 低 | OpenCode 自动处理 |
| **错误码差异** | 低 | 统一错误处理 |

---

## 🎯 推荐实施路径

### 阶段 1: 基础设施 (1-2 天)

1. 创建 `cis-core/src/ai/opencode.rs`
2. 创建 `cis-core/src/agent/providers/opencode.rs`
3. 更新 `cis-core/src/ai/mod.rs` 添加 OpenCode 支持
4. 更新 `cis-core/src/agent/mod.rs` 添加 `AgentType::OpenCode`

### 阶段 2: 集成适配 (2-3 天)

1. 更新 `cis-core/src/agent/cluster/session.rs` 命令构建
2. 添加 OpenCode 特定参数处理
3. 实现输出格式适配
4. 添加单元测试

### 阶段 3: 测试验证 (1-2 天)

1. 在测试环境验证 DAG 执行
2. 性能基准测试
3. 边界情况测试
4. 回滚测试

### 阶段 4: 生产切换 (1 天)

1. 更新配置文件
2. 保留 Claude 作为回退
3. 灰度发布
4. 监控告警

---

## 📚 参考文档

- **CIS 架构文档**: `/Users/jiangxiaolong/work/project/CIS/docs/`
- **Agent 接口**: `cis-core/src/agent/mod.rs`
- **AI Provider 接口**: `cis-core/src/ai/mod.rs`
- **DAG 执行**: `cis-core/src/agent/cluster/executor.rs`
- **OpenCode 文档**: https://github.com/anomalyco/opencode

---

## 🔄 版本历史

| 版本 | 日期 | 作者 | 说明 |
|------|------|------|------|
| 1.0 | 2026-02-07 | Claude | 初始版本 |

---

**文档结束**
