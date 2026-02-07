# Agent Session 架构改进方案

## 📋 改进目标

将 Agent Session 层调整为支持 **Claude** 和 **OpenCode** 并列，让用户可以选择使用哪个 Agent。

---

## 🏗️ 当前架构问题

### 问题 1: 命令名硬编码

```rust
// ❌ 当前代码 (cis-core/src/agent/cluster/session.rs:266-294)
fn build_agent_command(&self) -> Result<CommandBuilder> {
    let cmd_name = match self.agent_type {
        AgentType::Claude => "claude",
        AgentType::Kimi => "kimi",
        AgentType::Aider => "aider",
        // ⚠️ 缺少 OpenCode！
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
            cmd.arg("--dangerously-skip-permissions");
        }
        _ => {}
    }

    Ok(cmd)
}
```

### 问题 2: 缺少用户配置选项

- 无法在配置文件中指定默认 Agent
- 无法在不同 DAG 中使用不同 Agent
- Agent 特定参数硬编码

---

## 🔧 改进方案

### 方案概览

```
用户配置 (config.toml)
    ↓
AgentType 枚举 (扩展)
    ↓
AgentCommandConfig (新增)
    ↓
build_agent_command (重构)
    ↓
Claude Session / OpenCode Session
```

---

## 📝 实施步骤

### 步骤 1: 扩展 AgentType 枚举

**文件**: `cis-core/src/agent/mod.rs`

```rust
/// Agent 类型
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentType {
    Claude,
    Kimi,
    Aider,
    OpenCode,  // ← 新增
    Custom,
}

impl AgentType {
    /// 获取命令名称
    pub fn command_name(&self) -> Option<&'static str> {
        match self {
            AgentType::Claude => Some("claude"),
            AgentType::Kimi => Some("kimi"),
            AgentType::Aider => Some("aider"),
            AgentType::OpenCode => Some("opencode"),  // ← 新增
            AgentType::Custom => None,
        }
    }

    /// 获取显示名称
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::Claude => "Claude Code",
            AgentType::Kimi => "Kimi Code",
            AgentType::Aider => "Aider",
            AgentType::OpenCode => "OpenCode",  // ← 新增
            AgentType::Custom => "Custom",
        }
    }

    /// 是否支持 PTY 交互
    pub fn supports_pty(&self) -> bool {
        match self {
            AgentType::Claude | AgentType::Kimi | AgentType::Aider => true,
            AgentType::OpenCode => true,  // ← 新增
            AgentType::Custom => false,
        }
    }
}
```

---

### 步骤 2: 创建 AgentCommandConfig 结构

**文件**: `cis-core/src/agent/config.rs` (新建)

```rust
//! Agent 命令配置
//!
//! 定义不同 Agent 的命令行参数配置

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Agent 命令配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCommandConfig {
    /// 命令名称
    pub command: String,
    /// 基础参数
    pub base_args: Vec<String>,
    /// 环境变量
    pub env_vars: HashMap<String, String>,
    /// 是否需要 PTY
    pub requires_pty: bool,
    /// 是否支持流式输出
    pub supports_streaming: bool,
}

impl AgentCommandConfig {
    /// 创建 Claude 配置
    pub fn claude() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "claude".to_string());

        Self {
            command: "claude".to_string(),
            base_args: vec![
                "--dangerously-skip-permissions".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
        }
    }

    /// 创建 Kimi 配置
    pub fn kimi() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "kimi".to_string());

        Self {
            command: "kimi".to_string(),
            base_args: vec![
                "--dangerously-skip-permissions".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
        }
    }

    /// 创建 Aider 配置
    pub fn aider() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "aider".to_string());

        Self {
            command: "aider".to_string(),
            base_args: vec![],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
        }
    }

    /// 创建 OpenCode 配置
    pub fn opencode() -> Self {
        let mut env_vars = HashMap::new();
        env_vars.insert("CIS_AGENT".to_string(), "opencode".to_string());

        Self {
            command: "opencode".to_string(),
            base_args: vec![
                "run".to_string(),
                "--format".to_string(),
                "json".to_string(),
            ],
            env_vars,
            requires_pty: true,
            supports_streaming: true,
        }
    }

    /// 从 AgentType 创建配置
    pub fn from_agent_type(agent_type: crate::agent::AgentType) -> Option<Self> {
        match agent_type {
            crate::agent::AgentType::Claude => Some(Self::claude()),
            crate::agent::AgentType::Kimi => Some(Self::kimi()),
            crate::agent::AgentType::Aider => Some(Self::aider()),
            crate::agent::AgentType::OpenCode => Some(Self::opencode()),  // ← 新增
            crate::agent::AgentType::Custom => None,
        }
    }
}

impl Default for AgentCommandConfig {
    fn default() -> Self {
        Self::claude()  // 默认使用 Claude
    }
}
```

---

### 步骤 3: 更新 session.rs

**文件**: `cis-core/src/agent/cluster/session.rs`

```rust
use crate::agent::AgentType;
use crate::agent::config::AgentCommandConfig;  // ← 新增

impl AgentSession {
    /// Build command for agent type (重构版本)
    fn build_agent_command(&self) -> Result<CommandBuilder> {
        // 获取 Agent 配置
        let config = AgentCommandConfig::from_agent_type(self.agent_type)
            .ok_or_else(|| CisError::configuration(
                format!("Agent type {:?} not supported for cluster sessions", self.agent_type)
            ))?;

        // 构建命令
        let mut cmd = CommandBuilder::new(&config.command);

        // 设置工作目录
        cmd.cwd(&self.work_dir);
        cmd.env("CIS_PROJECT_PATH", self.work_dir.to_string_lossy().as_ref());
        cmd.env("CIS_SESSION_ID", self.id.to_string());

        // 添加环境变量
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        // 添加基础参数
        for arg in &config.base_args {
            cmd.arg(arg);
        }

        debug!("Built agent command: {:?} with args {:?}", config.command, config.base_args);

        Ok(cmd)
    }

    /// Create session with specific agent type (静态工厂方法)
    pub fn with_agent_type(
        agent_type: AgentType,
        id: SessionId,
        work_dir: PathBuf,
        prompt: String,
        upstream_context: String,
        event_broadcaster: EventBroadcaster,
        max_buffer_lines: usize,
    ) -> Self {
        Self::new(
            id,
            agent_type,
            work_dir,
            prompt,
            upstream_context,
            event_broadcaster,
            max_buffer_lines,
        )
    }
}
```

---

### 步骤 4: 更新 executor.rs 配置

**文件**: `cis-core/src/agent/cluster/executor.rs`

```rust
/// Agent Cluster Executor configuration
#[derive(Debug, Clone)]
pub struct AgentClusterConfig {
    /// Maximum concurrent workers
    pub max_workers: usize,
    /// Default agent type (← 用户可配置)
    pub default_agent: AgentType,
    /// Base work directory for sessions
    pub base_work_dir: std::path::PathBuf,
    /// Enable upstream context injection
    pub enable_context_injection: bool,
    /// Auto-attach on blockage
    pub auto_attach_on_block: bool,
    /// Task timeout (seconds)
    pub task_timeout_secs: u64,
}

impl Default for AgentClusterConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            default_agent: AgentType::Claude,  // ← 可改为 OpenCode
            base_work_dir: std::env::temp_dir().join("cis").join("dag-sessions"),
            enable_context_injection: true,
            auto_attach_on_block: false,
            task_timeout_secs: 3600,
        }
    }
}

// 从配置创建
impl AgentClusterConfig {
    /// 从配置文件加载
    pub fn from_config(config: &CisConfig) -> Self {
        Self {
            max_workers: config.agent.max_workers.unwrap_or(4),
            default_agent: config.agent.default_agent
                .and_then(|s| s.parse().ok())
                .unwrap_or(AgentType::Claude),  // ← 从配置读取
            base_work_dir: std::env::temp_dir().join("cis").join("dag-sessions"),
            enable_context_injection: true,
            auto_attach_on_block: false,
            task_timeout_secs: 3600,
        }
    }
}
```

---

### 步骤 5: 添加配置文件支持

**文件**: `config.example.toml`

```toml
[agent]
# 默认 Agent 类型: claude, kimi, aider, opencode
default_agent = "claude"
# default_agent = "opencode"  # ← 切换到 OpenCode

# 最大并发 Worker 数
max_workers = 4

# Task 超时时间（秒）
task_timeout_secs = 3600

# Agent 特定配置
[agent.claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096

[agent.opencode]
model = "opencode/big-pickle"
# model = "anthropic/claude-3-opus-20240229"  # 使用 Claude 模型
max_tokens = 4096
```

---

### 步骤 6: 添加 CLI 命令支持

**文件**: `cis-node/src/commands/agent.rs`

```rust
use cis_core::agent::AgentType;

/// Agent 管理命令
pub struct AgentCommands;

impl AgentCommands {
    /// 列出可用的 Agent
    pub async fn list() -> Result<()> {
        println!("可用的 Agent 类型:");
        println!("  claude    - Claude Code CLI");
        println!("  kimi      - Kimi Code CLI");
        println!("  aider     - Aider CLI");
        println!("  opencode  - OpenCode CLI (开源)");  // ← 新增
        Ok(())
    }

    /// 设置默认 Agent
    pub async fn set_default(agent_type: &str) -> Result<()> {
        let agent = agent_type.parse::<AgentType>()
            .map_err(|_| CisError::configuration(format!("Invalid agent type: {}", agent_type)))?;

        println!("设置默认 Agent 为: {}", agent.display_name());
        // TODO: 更新配置文件
        Ok(())
    }

    /// 测试 Agent 是否可用
    pub async fn check(agent_type: Option<&str>) -> Result<()> {
        let agent = if let Some(name) = agent_type {
            name.parse::<AgentType>()?
        } else {
            AgentType::Claude  // 默认检查 Claude
        };

        println!("检查 Agent: {}", agent.display_name());

        // 检查命令是否存在
        let cmd_name = agent.command_name()
            .ok_or_else(|| CisError::configuration("Custom agent not supported"))?;

        let output = tokio::process::Command::new(cmd_name)
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                println!("✅ {} 可用", agent.display_name());
                Ok(())
            }
            _ => {
                println!("❌ {} 不可用 (未安装或不在 PATH 中)", agent.display_name());
                Err(CisError::configuration(format!("{} not available", agent.display_name())))
            }
        }
    }
}
```

---

## 🎯 使用示例

### 示例 1: 在配置文件中设置默认 Agent

```toml
# config.toml
[agent]
default_agent = "opencode"  # 切换到 OpenCode
```

### 示例 2: 在 DAG 配置中指定 Agent

```toml
# example-dag.toml
[dag]
name = "我的 DAG"
default_agent = "opencode"  # ← 使用 OpenCode

[[dag.tasks]]
id = "task1"
command = "实现一个登录功能"
agent = "claude"  # ← 这个任务使用 Claude

[[dag.tasks]]
id = "task2"
command = "测试登录功能"
agent = "opencode"  # ← 这个任务使用 OpenCode
```

### 示例 3: 通过 CLI 命令切换

```bash
# 列出可用 Agent
cis agent list

# 检查 Agent 是否可用
cis agent check opencode

# 设置默认 Agent
cis agent set-default opencode

# 使用指定 Agent 执行 DAG
cis dag run example-dag.toml --agent opencode
```

---

## 📊 改进前后对比

| 功能 | 改进前 | 改进后 |
|------|--------|--------|
| **支持的 Agent** | Claude, Kimi, Aider | + OpenCode |
| **配置方式** | 硬编码 | 配置文件 |
| **DAG 级别选择** | ❌ 不支持 | ✅ 支持 |
| **任务级别选择** | ❌ 不支持 | ✅ 支持 |
| **运行时切换** | ❌ 不支持 | ✅ CLI 命令 |
| **扩展性** | 低（需修改代码） | 高（配置驱动） |

---

## 🔧 需要修改的文件清单

| 文件 | 改动类型 | 复杂度 |
|------|----------|--------|
| `cis-core/src/agent/mod.rs` | 修改 | ⭐ 扩展枚举 |
| `cis-core/src/agent/config.rs` | 新增 | ⭐⭐ 新增配置结构 |
| `cis-core/src/agent/cluster/session.rs` | 修改 | ⭐⭐ 重构命令构建 |
| `cis-core/src/agent/cluster/executor.rs` | 修改 | ⭐ 添加配置读取 |
| `cis-node/src/commands/agent.rs` | 新增/修改 | ⭐⭐ 添加 CLI 命令 |
| `config.example.toml` | 修改 | ⭐ 添加配置项 |

---

## ✅ 验证清单

- [ ] AgentType 枚举包含 OpenCode
- [ ] AgentCommandConfig 正确配置所有 Agent
- [ ] build_agent_command 支持所有 Agent 类型
- [ ] 配置文件可以指定默认 Agent
- [ ] CLI 命令可以检查和切换 Agent
- [ ] DAG 配置可以指定任务级别 Agent
- [ ] 所有测试通过

---

**文档结束**
