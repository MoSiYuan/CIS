# Claude 与 CIS 的耦合度分析及抽象接口复用

## 📋 文档概览

**目的**: 分析 Claude 与 CIS 的耦合程度，评估可复用的抽象接口，为 OpenCode 集成提供技术依据

**分析日期**: 2026-02-07

**CIS 版本**: main分支

---

## 🏗️ CIS 抽象接口体系

CIS 采用**两层抽象接口**设计，有效降低了与具体 AI 工具的耦合：

```
┌─────────────────────────────────────────────────────────┐
│              应用层 (DAG/CLI/GUI)                       │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│          AgentProvider trait (Agent 抽象层)            │
│  - 用于 DAG 执行、Agent Cluster                         │
│  - 支持流式输出、工作目录、会话管理                       │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│           AiProvider trait (AI 抽象层)                 │
│  - 用于简单的 AI 调用                                    │
│  - 支持 RAG 增强、结构化输出                             │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│          具体实现 (Claude/Kimi/Aider/OpenCode)          │
└─────────────────────────────────────────────────────────┘
```

---

## 📦 抽象接口详解

### 1. AiProvider Trait

**文件**: `cis-core/src/ai/mod.rs`

#### 1.1 接口定义

```rust
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;

    /// 检查是否可用（CLI 工具是否安装）
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

```rust
pub struct ClaudeCliProvider {
    config: ClaudeConfig,
}

#[async_trait]
impl AiProvider for ClaudeCliProvider {
    fn name(&self) -> &str { "claude-cli" }

    async fn available(&self) -> bool {
        Command::new("claude").arg("--version").output().await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn chat(&self, prompt: &str) -> Result<String> {
        let mut cmd = Command::new("claude");
        cmd.arg("--model").arg(&self.config.model)
           .arg("--max-tokens").arg(self.config.max_tokens.to_string())
           .arg("--temperature").arg(self.config.temperature.to_string())
           .arg("--").arg(prompt);
        // ... 执行并返回
    }

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
        // ... 执行并返回
    }

    async fn generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value> {
        // 构建增强 Prompt
        let full_prompt = format!(
            "{}\n\nPlease respond with valid JSON matching this schema:\n{}\n\nRespond ONLY with the JSON object, no markdown formatting.",
            prompt, schema
        );

        let response = self.chat(&full_prompt).await?;

        // 提取 JSON
        // ...
    }

    async fn chat_with_rag(&self, prompt: &str, ctx: Option<&ConversationContext>) -> Result<String> {
        let enhanced_prompt = if let Some(context) = ctx {
            context.prepare_ai_prompt(prompt).await?
        } else {
            prompt.to_string()
        };
        self.chat(&enhanced_prompt).await
    }
}
```

#### 1.3 接口复用性评估

| 方法 | Claude 实现 | OpenCode 复用难度 | 说明 |
|------|------------|------------------|------|
| `name()` | 返回 "claude-cli" | ✅ 极低 | 返回字符串即可 |
| `available()` | `claude --version` | ✅ 极低 | 改为 `opencode --version` |
| `chat()` | `claude --model -- prompt` | ⭐ 低 | 改为 `opencode run --format json` |
| `chat_with_context()` | `claude --system --user --assistant` | ⭐⭐ 中 | OpenCode 不直接支持，需 prompt 注入 |
| `generate_json()` | 文本解析提取 | ⭐ 低 | 同样逻辑 |
| `chat_with_rag()` | `prepare_ai_prompt()` + `chat()` | ✅ 极低 | 完全复用！ |

**复用评分**: ⭐⭐⭐⭐ (4/5) - **高度可复用**

---

### 2. AgentProvider Trait

**文件**: `cis-core/src/agent/mod.rs`

#### 2.1 接口定义

```rust
#[async_trait]
pub trait AgentProvider: Send + Sync {
    /// Provider 名称
    fn name(&self) -> &str;

    /// Provider 版本
    fn version(&self) -> &str {
        "0.1.0"
    }

    /// 检查 Agent 是否可用
    async fn available(&self) -> bool;

    /// 执行指令（同步返回）
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse>;

    /// 流式执行
    async fn execute_stream(
        &self,
        req: AgentRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse>;

    /// 初始化（可选）
    async fn init(&mut self, _context: AgentContext) -> Result<()> {
        Ok(())
    }

    /// 获取 Agent 能力描述
    fn capabilities(&self) -> AgentCapabilities;
}
```

#### 2.2 关键数据结构

```rust
/// Agent 请求
pub struct AgentRequest {
    /// 主指令/Prompt
    pub prompt: String,
    /// 上下文信息
    pub context: AgentContext,
    /// 允许使用的 Skill 列表
    pub skills: Vec<String>,
    /// 系统提示词（覆盖默认）
    pub system_prompt: Option<String>,
    /// 会话历史
    pub history: Vec<AgentMessage>,
}

/// Agent 上下文
pub struct AgentContext {
    /// 工作目录
    pub work_dir: Option<PathBuf>,
    /// 允许访问的记忆前缀
    pub memory_access: Vec<String>,
    /// 项目配置
    pub project_config: Option<ProjectConfig>,
    /// 额外上下文数据
    pub extra: HashMap<String, serde_json::Value>,
}

/// Agent 响应
pub struct AgentResponse {
    /// 响应内容
    pub content: String,
    /// 使用的 Token 数（如果可用）
    pub token_usage: Option<TokenUsage>,
    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Agent 能力描述
pub struct AgentCapabilities {
    /// 是否支持流式输出
    pub streaming: bool,
    /// 是否支持工具调用
    pub tool_calling: bool,
    /// 是否支持多模态
    pub multimodal: bool,
    /// 最大上下文长度
    pub max_context_length: Option<usize>,
    /// 支持的模型列表
    pub supported_models: Vec<String>,
}
```

#### 2.3 Claude 实现

**文件**: `cis-core/src/agent/providers/claude.rs`

```rust
pub struct ClaudeProvider {
    config: AgentConfig,
}

impl ClaudeProvider {
    fn build_command(&self, req: &AgentRequest) -> Command {
        let mut cmd = Command::new("claude");

        // 设置工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // 基础参数
        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        // 系统提示词
        if let Some(ref system) = req.system_prompt {
            cmd.arg("--system").arg(system);
        }

        cmd
    }
}

#[async_trait]
impl AgentProvider for ClaudeProvider {
    fn name(&self) -> &str { "claude" }

    async fn available(&self) -> bool {
        Command::new("claude").arg("--version").output().await
            .map(|o| o.status.success())
            .unwrap_or(false)
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

    async fn execute_stream(&self, req: AgentRequest, tx: mpsc::Sender<String>) -> Result<AgentResponse> {
        let mut cmd = self.build_command(&req);
        cmd.arg("--stream").arg("--").arg(&req.prompt);

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        // 逐行读取并发送
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if tx.send(line).await.is_err() {
                break;
            }
        }

        let status = child.wait().await?;

        Ok(AgentResponse {
            content: String::new(),
            token_usage: None,
            metadata: [("exit_code".to_string(), serde_json::json!(status.code()))]
                .into_iter()
                .collect(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streaming: true,
            tool_calling: false,
            multimodal: true,
            max_context_length: Some(200_000),
            supported_models: vec![
                "claude-sonnet-4-20250514".to_string(),
                "claude-opus-4-20250514".to_string(),
            ],
        }
    }
}
```

#### 2.4 接口复用性评估

| 方法/结构 | Claude 实现 | OpenCode 复用难度 | 说明 |
|----------|------------|------------------|------|
| `name()` | 返回 "claude" | ✅ 极低 | 返回 "opencode" |
| `available()` | `claude --version` | ✅ 极低 | 改为 `opencode --version` |
| `execute()` | `claude --model --system -- prompt` | ⭐ 低 | 改为 `opencode run --format json --` |
| `execute_stream()` | `claude --stream` | ⭐⭐ 中 | 改为 `opencode run --format json`，解析 JSON 流 |
| `AgentRequest` | 结构体 | ✅ 完全复用 | 无需改动 |
| `AgentContext` | 工作目录等 | ✅ 完全复用 | 无需改动 |
| `AgentResponse` | 响应结构 | ✅ 完全复用 | 无需改动 |
| `AgentCapabilities` | 能力描述 | ✅ 完全复用 | 调整参数即可 |

**复用评分**: ⭐⭐⭐⭐⭐ (5/5) - **完全可复用**

---

## 🔍 耦合度分析

### 层级耦合度矩阵

| 层级 | 耦合度 | Claude 特定依赖 | 可复用抽象 |
|------|--------|----------------|-----------|
| **AI Provider 层** | ⭐⭐ 低 | `--user`, `--assistant` 参数 | `AiProvider` trait |
| **Agent Provider 层** | ⭐⭐⭐ 中 | `--system`, `--stream` 参数 | `AgentProvider` trait |
| **Agent Session 层** | ⭐⭐⭐⭐ 高 | 命令名硬编码、`--dangerously-skip-permissions` | ❌ 无抽象，需扩展 |
| **DAG 命令层** | ⭐ 极低 | 无 | 完全解耦 |

### 关键发现

#### 1. **良好的抽象设计** ✅

CIS 的两层抽象接口设计非常优秀：

- **AiProvider trait**: 简单 AI 调用场景
- **AgentProvider trait**: 复杂 Agent 执行场景

这两个 trait **完全解耦**了具体实现，OpenCode 可以直接复用！

#### 2. **Claude 特定依赖** ⚠️

但在实现细节上，有一些 Claude 特定依赖：

**AiProvider 层**:
```rust
// Claude 支持 --user 和 --assistant 参数
cmd.arg("--user").arg(&msg.content);
cmd.arg("--assistant").arg(&msg.content);

// OpenCode 需要通过 prompt 注入模拟
let enhanced = format!("User: {}\nAssistant: {}", user_msg, assistant_msg);
```

**AgentProvider 层**:
```rust
// Claude 支持 --system 参数
cmd.arg("--system").arg(system);

// OpenCode 需要通过 prompt 注入
let enhanced = format!("System: {}\n\n{}", system, prompt);
```

**Agent Session 层**:
```rust
// 硬编码命令名
let cmd_name = match self.agent_type {
    AgentType::Claude => "claude",
    // ...
};

// Claude 特定标志
cmd.arg("--dangerously-skip-permissions");
```

---

## 🎯 OpenCode 可复用的抽象接口

### 完全可复用 (无需改动)

#### 1. 数据结构 (100% 复用)

```rust
// ✅ 完全复用
pub struct AgentRequest {
    pub prompt: String,
    pub context: AgentContext,
    pub skills: Vec<String>,
    pub system_prompt: Option<String>,
    pub history: Vec<AgentMessage>,
}

pub struct AgentContext {
    pub work_dir: Option<PathBuf>,
    pub memory_access: Vec<String>,
    pub project_config: Option<ProjectConfig>,
    pub extra: HashMap<String, serde_json::Value>,
}

pub struct AgentResponse {
    pub content: String,
    pub token_usage: Option<TokenUsage>,
    pub metadata: HashMap<String, serde_json::Value>,
}

pub struct AgentCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub multimodal: bool,
    pub max_context_length: Option<usize>,
    pub supported_models: Vec<String>,
}
```

#### 2. AgentProvider trait 方法签名 (100% 复用)

```rust
#[async_trait]
pub trait AgentProvider: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    async fn available(&self) -> bool;
    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse>;
    async fn execute_stream(&self, req: AgentRequest, tx: mpsc::Sender<String>) -> Result<AgentResponse>;
    async fn init(&mut self, _context: AgentContext) -> Result<()>;
    fn capabilities(&self) -> AgentCapabilities;
}
```

#### 3. AiProvider trait 核心方法 (80% 复用)

```rust
#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn available(&self) -> bool;
    async fn chat(&self, prompt: &str) -> Result<String>;

    // ⚠️ chat_with_context 需要 prompt 注入适配
    async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String>;

    // ✅ 完全复用 - RAG 增强
    async fn chat_with_rag(&self, prompt: &str, ctx: Option<&ConversationContext>) -> Result<String>;

    async fn generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value>;
}
```

---

## 🔧 OpenCode 实现方案

### 方案 1: 实现 AgentProvider trait (推荐)

**文件**: `cis-core/src/agent/providers/opencode.rs`

```rust
//! OpenCode Agent Provider
//!
//! 实现了 AgentProvider trait，完全复用 CIS 的抽象接口

use async_trait::async_trait;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::agent::{
    AgentCapabilities, AgentConfig, AgentRequest, AgentResponse, AgentProvider,
};
use crate::error::Result;

pub struct OpenCodeAgentProvider {
    config: AgentConfig,
}

impl OpenCodeAgentProvider {
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    /// 构建命令（适配 OpenCode 的参数格式）
    fn build_command(&self, req: &AgentRequest) -> Command {
        let mut cmd = Command::new("opencode");
        cmd.arg("run");

        // 工作目录
        if let Some(ref work_dir) = req.context.work_dir {
            cmd.current_dir(work_dir);
        }

        // 模型选择
        if let Some(ref model) = self.config.model {
            cmd.arg("--model").arg(model);
        }

        // JSON 输出格式
        cmd.arg("--format").arg("json");

        cmd
    }

    /// 将多轮消息转换为 OpenCode 格式
    fn format_messages(&self, system: Option<&str>, messages: &[crate::ai::Message]) -> String {
        let mut parts = Vec::new();

        // System prompt
        if let Some(sys) = system {
            parts.push(format!("System: {}", sys));
        }

        // 历史消息
        for msg in messages {
            match msg.role {
                crate::ai::Role::System => {
                    parts.push(format!("System: {}", msg.content));
                }
                crate::ai::Role::User => {
                    parts.push(format!("User: {}", msg.content));
                }
                crate::ai::Role::Assistant => {
                    parts.push(format!("Assistant: {}", msg.content));
                }
            }
        }

        parts.join("\n\n")
    }
}

#[async_trait]
impl AgentProvider for OpenCodeAgentProvider {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn available(&self) -> bool {
        Command::new("opencode")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse> {
        let mut cmd = self.build_command(&req);

        // 添加 prompt
        cmd.arg("--").arg(&req.prompt);

        let output = cmd.output().await?;

        // 解析 JSON 输出
        let content = Self::parse_json_output(&output.stdout)?;

        Ok(AgentResponse {
            content,
            token_usage: None,
            metadata: [("exit_code".to_string(), serde_json::json!(output.status.code()))]
                .into_iter()
                .collect(),
        })
    }

    async fn execute_stream(
        &self,
        req: AgentRequest,
        tx: mpsc::Sender<String>,
    ) -> Result<AgentResponse> {
        let mut cmd = self.build_command(&req);
        cmd.arg("--").arg(&req.prompt);

        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("Failed to capture stdout");

        // 逐行解析 JSON 事件流
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            // 解析 JSON 事件
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
                    let _ = tx.send(content.to_string()).await;
                }
            }
        }

        let status = child.wait().await?;

        Ok(AgentResponse {
            content: String::new(),
            token_usage: None,
            metadata: [("exit_code".to_string(), serde_json::json!(status.code()))]
                .into_iter()
                .collect(),
        })
    }

    fn capabilities(&self) -> AgentCapabilities {
        AgentCapabilities {
            streaming: true,
            tool_calling: false,
            multimodal: true,
            max_context_length: Some(200_000),
            supported_models: vec![
                "opencode/big-pickle".to_string(),
                "anthropic/claude-3-opus-20240229".to_string(),
                "openai/gpt-4".to_string(),
            ],
        }
    }
}

impl OpenCodeAgentProvider {
    /// 解析 OpenCode JSON 输出
    fn parse_json_output(stdout: &[u8]) -> Result<String> {
        let output = String::from_utf8(stdout)?;

        // OpenCode JSON 输出是事件流
        for line in output.lines() {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
                    return Ok(content.to_string());
                }
                if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        // 如果 JSON 解析失败，返回原始输出
        Ok(output)
    }
}
```

### 方案 2: 实现 AiProvider trait

**文件**: `cis-core/src/ai/opencode.rs`

```rust
//! OpenCode AI Provider
//!
//! 实现了 AiProvider trait，复用 CIS 的 RAG 增强功能

use super::{AiProvider, AiError, ConversationContext, Message, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeConfig {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    pub work_dir: Option<PathBuf>,
}

fn default_model() -> String { "opencode/big-pickle".to_string() }
fn default_max_tokens() -> usize { 4096 }
fn default_temperature() -> f32 { 0.7 }

impl Default for OpenCodeConfig {
    fn default() -> Self {
        Self {
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            work_dir: None,
        }
    }
}

pub struct OpenCodeProvider {
    config: OpenCodeConfig,
}

impl OpenCodeProvider {
    pub fn new(config: OpenCodeConfig) -> Self {
        Self { config }
    }
}

impl Default for OpenCodeProvider {
    fn default() -> Self {
        Self::new(OpenCodeConfig::default())
    }
}

#[async_trait]
impl AiProvider for OpenCodeProvider {
    fn name(&self) -> &str {
        "opencode"
    }

    async fn available(&self) -> bool {
        Command::new("opencode")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn chat(&self, prompt: &str) -> Result<String> {
        let mut cmd = Command::new("opencode");
        cmd.arg("run")
           .arg("--model").arg(&self.config.model)
           .arg("--format").arg("json")
           .arg("--")
           .arg(prompt)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());

        if let Some(ref work_dir) = self.config.work_dir {
            cmd.current_dir(work_dir);
        }

        let output = cmd.output().await.map_err(AiError::Io)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AiError::CliError(stderr.to_string()));
        }

        Self::parse_json_output(&output.stdout)
    }

    async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String> {
        // ⚠️ OpenCode 不直接支持 --user --assistant
        // 通过 prompt 注入模拟
        let mut full_prompt = String::new();

        if !system.is_empty() {
            full_prompt.push_str(&format!("System: {}\n\n", system));
        }

        for msg in messages {
            match msg.role {
                super::Role::System => {
                    full_prompt.push_str(&format!("System: {}\n", msg.content));
                }
                super::Role::User => {
                    full_prompt.push_str(&format!("User: {}\n", msg.content));
                }
                super::Role::Assistant => {
                    full_prompt.push_str(&format!("Assistant: {}\n", msg.content));
                }
            }
        }

        self.chat(&full_prompt).await
    }

    async fn generate_json(&self, prompt: &str, schema: &str) -> Result<serde_json::Value> {
        let full_prompt = format!(
            "{}\n\nPlease respond with valid JSON matching this schema:\n{}\n\nRespond ONLY with the JSON object, no markdown formatting.",
            prompt, schema
        );

        let response = self.chat(&full_prompt).await?;

        // 提取 JSON（与 Claude 实现相同）
        let trimmed = response.trim();
        let json_str = if trimmed.starts_with('{') && trimmed.ends_with('}') {
            trimmed
        } else if let Some(start) = trimmed.find("```json") {
            let after = &trimmed[start + 7..];
            if let Some(end) = after.find("```") {
                after[..end].trim()
            } else {
                return Err(AiError::InvalidResponse("Invalid JSON block".to_string()));
            }
        } else {
            return Err(AiError::InvalidResponse("No JSON found".to_string()));
        };

        serde_json::from_str(json_str)
            .map_err(|e| AiError::InvalidResponse(format!("JSON parse error: {}", e)))
    }

    async fn chat_with_rag(
        &self,
        prompt: &str,
        ctx: Option<&ConversationContext>,
    ) -> Result<String> {
        // ✅ 完全复用 RAG 逻辑
        let enhanced_prompt = if let Some(context) = ctx {
            match context.prepare_ai_prompt(prompt).await {
                Ok(enhanced) => enhanced,
                Err(e) => {
                    tracing::warn!("Failed to prepare AI prompt: {}, using original", e);
                    prompt.to_string()
                }
            }
        } else {
            prompt.to_string()
        };

        self.chat(&enhanced_prompt).await
    }
}

impl OpenCodeProvider {
    /// 解析 JSON 格式输出
    fn parse_json_output(stdout: &[u8]) -> Result<String> {
        let output = String::from_utf8(stdout)?;

        // OpenCode JSON 输出是事件流
        for line in output.lines() {
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
                    return Ok(content.to_string());
                }
                if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
                    return Ok(text.to_string());
                }
            }
        }

        Ok(output)
    }
}
```

---

## 📊 需要额外适配的部分

### 1. Agent Session 层 (硬编码问题)

**文件**: `cis-core/src/agent/cluster/session.rs`

**问题**: 命令名硬编码

```rust
// ❌ 当前代码（硬编码）
let cmd_name = match self.agent_type {
    AgentType::Claude => "claude",
    AgentType::Kimi => "kimi",
    AgentType::Aider => "aider",
    // 缺少 OpenCode！
};

// ⚠️ Claude 特定标志
match self.agent_type {
    AgentType::Claude | AgentType::Kimi => {
        cmd.arg("--dangerously-skip-permissions");
    }
    _ => {}
}
```

**解决方案**:

```rust
// ✅ 改进后（扩展支持）
let cmd_name = match self.agent_type {
    AgentType::Claude => "claude",
    AgentType::Kimi => "kimi",
    AgentType::Aider => "aider",
    AgentType::OpenCode => "opencode",  // ← 新增
    AgentType::Custom => {
        return Err(CisError::configuration(
            "Custom agent type not supported for cluster sessions",
        ));
    }
};

// ✅ 改进后（条件标志）
match self.agent_type {
    AgentType::Claude | AgentType::Kimi => {
        cmd.arg("--dangerously-skip-permissions");
    }
    AgentType::OpenCode => {
        cmd.arg("--format").arg("json");  // ← OpenCode 特定
    }
    _ => {}
}
```

### 2. 多轮对话处理 (prompt 注入)

**问题**: OpenCode 不支持 `--user --assistant` 参数

**Claude 方式**:
```bash
claude --system "You are helpful" \
       --user "First question" \
       --assistant "Answer 1" \
       --user "Second question"
```

**OpenCode 方式**:
```bash
opencode run --format json -- \
    "System: You are helpful\n
     User: First question\n
     Assistant: Answer 1\n
     User: Second question"
```

**解决方案**: 在 `chat_with_context()` 中实现 prompt 注入

```rust
async fn chat_with_context(&self, system: &str, messages: &[Message]) -> Result<String> {
    let mut parts = Vec::new();

    if !system.is_empty() {
        parts.push(format!("System: {}", system));
    }

    for msg in messages {
        match msg.role {
            Role::User => parts.push(format!("User: {}", msg.content)),
            Role::Assistant => parts.push(format!("Assistant: {}", msg.content)),
            Role::System => parts.push(format!("System: {}", msg.content)),
        }
    }

    let full_prompt = parts.join("\n\n");
    self.chat(&full_prompt).await
}
```

---

## 🎯 集成步骤

### 步骤 1: 扩展枚举 (5分钟)

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

// cis-core/src/ai/mod.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    Claude,
    Kimi,
    OpenCode,  // ← 新增
}
```

### 步骤 2: 实现 Provider (1-2小时)

```bash
# 创建文件
touch cis-core/src/ai/opencode.rs
touch cis-core/src/agent/providers/opencode.rs
```

### 步骤 3: 更新工厂方法 (10分钟)

```rust
// cis-core/src/ai/mod.rs
pub fn from_config(config: AiProviderConfig) -> Box<dyn AiProvider> {
    match config.provider_type {
        ProviderType::Claude => { /* ... */ }
        ProviderType::Kimi => { /* ... */ }
        ProviderType::OpenCode => {
            Box::new(OpenCodeProvider::new(config.opencode.unwrap_or_default()))
        }
    }
}

// cis-core/src/agent/mod.rs
pub fn create(config: &AgentConfig) -> Result<Box<dyn AgentProvider>> {
    match config.provider_type {
        AgentType::Claude => { /* ... */ }
        AgentType::Kimi => { /* ... */ }
        AgentType::Aider => { /* ... */ }
        AgentType::OpenCode => {
            Ok(Box::new(providers::OpenCodeAgentProvider::new(config.clone())))
        }
        AgentType::Custom => { /* ... */ }
    }
}
```

### 步骤 4: 更新 Session 构建 (15分钟)

```rust
// cis-core/src/agent/cluster/session.rs
let cmd_name = match self.agent_type {
    AgentType::Claude => "claude",
    AgentType::Kimi => "kimi",
    AgentType::Aider => "aider",
    AgentType::OpenCode => "opencode",  // ← 新增
    AgentType::Custom => { /* ... */ }
};
```

### 步骤 5: 更新配置文件 (5分钟)

```toml
# config.example.toml
[ai]
default_provider = "opencode"

[ai.opencode]
model = "opencode/big-pickle"
max_tokens = 4096
temperature = 0.7
```

---

## 📈 复用度总结

### 可直接复用 (100%)

- ✅ **AgentRequest** 数据结构
- ✅ **AgentContext** 数据结构
- ✅ **AgentResponse** 数据结构
- ✅ **AgentCapabilities** 数据结构
- ✅ **AgentProvider** trait 方法签名
- ✅ **chat_with_rag()** RAG 逻辑

### 需要适配 (80%)

- ⚠️ **chat_with_context()** - prompt 注入模拟
- ⚠️ **execute_stream()** - JSON 流解析
- ⚠️ **Agent Session** - 命令名扩展

### 完全不可复用 (0%)

- ❌ Agent Session 命令构建逻辑（需修改代码）

---

## 🎁 结论

### 核心发现

1. **CIS 的抽象接口设计非常优秀** ⭐⭐⭐⭐⭐
   - 两层抽象，职责清晰
   - 接口简洁，易于实现
   - 支持复杂场景（流式、RAG、会话管理）

2. **OpenCode 可以高度复用这些抽象** ⭐⭐⭐⭐
   - **AgentProvider trait**: 100% 复用接口签名
   - **AiProvider trait**: 80% 复用（需适配多轮对话）
   - **数据结构**: 100% 复用
   - **RAG 逻辑**: 100% 复用

3. **耦合主要集中在实现细节**
   - 命令行参数差异（可适配）
   - 会话管理硬编码（需扩展枚举）

### 推荐方案

**采用方案 1**: 实现 `AgentProvider` 和 `AiProvider` trait

**优势**:
- ✅ 最小化代码改动
- ✅ 保留全部 CIS 能力（RAG、向量检索、会话管理）
- ✅ 符合现有架构设计
- ✅ 易于维护和扩展

**实施时间**: 2-3 小时

---

**文档结束**
