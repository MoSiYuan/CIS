# AI Provider Skill 设计

## 架构

```
┌─────────────────────────────────────────┐
│           CIS Node                       │
│  ┌───────────────────────────────────┐  │
│  │      AI Provider Core (Native)    │  │
│  │  ┌─────────────┐ ┌─────────────┐  │  │
│  │  │ Claude CLI  │ │  Kimi Code  │  │  │
│  │  │  (Default)  │ │  (Optional) │  │  │
│  │  └──────┬──────┘ └──────┬──────┘  │  │
│  │         └───────────────┘         │  │
│  │              │                     │  │
│  │         ┌────┴────┐                │  │
│  │         │ 统一接口 │                │  │
│  │         │ AiProvider              │  │
│  │         └────┬────┘                │  │
│  │              │                     │  │
│  │  ┌───────────┼───────────┐        │  │
│  │  │           │           │        │  │
│  │  ▼           ▼           ▼        │  │
│  │ Other    Memory    External      │  │
│  │ Skills   Skill     Process       │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

## 核心设计原则

1. **Claude CLI 为核心** - 默认且必须可用
2. **统一接口** - 其他 Skill 不感知具体 AI 实现
3. **环境依赖隔离** - AI 调用通过 CLI 工具，不直接依赖 API
4. **本地优先** - 所有 AI 交互本地完成，不经过网络代理

## 接口定义

```rust
/// AI 提供者统一接口
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
        context: serde_json::Value,
    ) -> Result<String>;
    
    /// 流式响应（用于长文本）
    async fn chat_stream(
        &self,
        prompt: &str,
        tx: mpsc::Sender<String>,
    ) -> Result<()>;
}

pub struct Message {
    pub role: Role,
    pub content: String,
}

pub enum Role {
    System,
    User,
    Assistant,
}
```

## Claude CLI Provider (Native Skill)

### 实现

```rust
//! Claude CLI AI Provider
//! 
//! 通过本地 claude 命令行工具调用 AI
//! 要求: claude CLI 已安装且配置了 API key

use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, error, info};

pub struct ClaudeCliProvider {
    config: ClaudeConfig,
}

#[derive(Clone, Debug)]
pub struct ClaudeConfig {
    /// 模型名称
    pub model: String,  // 默认: claude-sonnet-4-20250514
    /// 最大 token
    pub max_tokens: usize,
    /// 温度
    pub temperature: f32,
    /// 工作目录（用于上下文）
    pub work_dir: Option<PathBuf>,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            temperature: 0.7,
            work_dir: None,
        }
    }
}

#[async_trait]
impl AiProvider for ClaudeCliProvider {
    fn name(&self) -> &str {
        "claude-cli"
    }
    
    async fn available(&self) -> bool {
        // 检查 claude CLI 是否安装
        match Command::new("claude")
            .arg("--version")
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }
    
    async fn chat(&self, prompt: &str) -> Result<String> {
        debug!("Claude CLI chat: prompt_len={}", prompt.len());
        
        let mut cmd = Command::new("claude");
        
        // 基础参数
        cmd.arg("--model").arg(&self.config.model)
           .arg("--max-tokens").arg(self.config.max_tokens.to_string());
        
        // 在工作目录执行（如果有）
        if let Some(ref work_dir) = self.config.work_dir {
            cmd.current_dir(work_dir);
        }
        
        // 非交互模式，直接传递 prompt
        cmd.arg("--")
           .arg(prompt)
           .stdin(Stdio::null())
           .stdout(Stdio::piped())
           .stderr(Stdio::piped());
        
        let output = cmd.output().await
            .context("Failed to execute claude CLI")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Claude CLI error: {}", stderr);
            return Err(anyhow!("Claude CLI failed: {}", stderr));
        }
        
        let response = String::from_utf8_lossy(&output.stdout);
        info!("Claude response: {} chars", response.len());
        
        Ok(response.to_string())
    }
    
    async fn chat_with_context(
        &self,
        system: &str,
        messages: &[Message],
        _context: serde_json::Value,
    ) -> Result<String> {
        // 构建对话历史文件
        let conversation = self.build_conversation(system, messages)?;
        
        let mut cmd = Command::new("claude");
        
        cmd.arg("--model").arg(&self.config.model)
           .arg("--system").arg(system);
        
        // 传递完整对话
        for msg in messages {
            match msg.role {
                Role::User => {
                    cmd.arg("--user").arg(&msg.content);
                }
                Role::Assistant => {
                    cmd.arg("--assistant").arg(&msg.content);
                }
                _ => {}
            }
        }
        
        let output = cmd.output().await?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    async fn chat_stream(
        &self,
        prompt: &str,
        tx: mpsc::Sender<String>,
    ) -> Result<()> {
        // 流式输出需要解析 claude --stream 输出
        let mut cmd = Command::new("claude");
        cmd.arg("--stream")
           .arg("--")
           .arg(prompt)
           .stdout(Stdio::piped());
        
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        while let Some(line) = lines.next_line().await? {
            tx.send(line).await?;
        }
        
        Ok(())
    }
}
```

## Kimi Code Provider (Optional)

```rust
//! Kimi Code AI Provider
//!
//! 通过 kimi CLI 调用

pub struct KimiCodeProvider {
    config: KimiConfig,
}

#[async_trait]
impl AiProvider for KimiCodeProvider {
    fn name(&self) -> &str {
        "kimi-code"
    }
    
    async fn available(&self) -> bool {
        Command::new("kimi")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    
    async fn chat(&self, prompt: &str) -> Result<String> {
        let output = Command::new("kimi")
            .arg("chat")
            .arg("--no-stream")
            .arg("--")
            .arg(prompt)
            .output()
            .await?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
```

## 配置

```toml
# ~/.cis/config.toml
[ai]
provider = "claude-cli"  # 或 "kimi-code"

[ai.claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

[ai.kimi]
model = "kimi-k2"
```

## 使用示例

```rust
// 在 Skill 中使用 AI
pub struct MemoryOrganizerSkill {
    ai: Arc<dyn AiProvider>,
}

#[async_trait]
impl Skill for MemoryOrganizerSkill {
    async fn handle_event(&self, event: Event) -> Result<()> {
        match event {
            Event::MemoryWrite { key, value } => {
                // 使用 AI 生成摘要
                let prompt = format!(
                    "Summarize this memory in 3 keywords:\n{}",
                    String::from_utf8_lossy(&value)
                );
                
                let summary = self.ai.chat(&prompt).await?;
                
                // 存储摘要
                sdk::memory_set(
                    &format!("_meta/{}/keywords", key),
                    summary.as_bytes()
                );
            }
            _ => {}
        }
        
        Ok(())
    }
}
```

## 作为 Native Skill 注册

```rust
// cis-node/src/skills/ai_provider/mod.rs
pub struct AiProviderSkill {
    provider: Arc<dyn AiProvider>,
}

impl NativeSkill for AiProviderSkill {
    fn name(&self) -> &str {
        "ai-provider"
    }
    
    async fn init(&mut self, ctx: SkillContext) -> Result<()> {
        // 默认使用 Claude CLI
        let provider = ClaudeCliProvider::new(ClaudeConfig::default());
        
        if !provider.available().await {
            panic!("Claude CLI not found. Please install: npm install -g @anthropic-ai/claude-cli");
        }
        
        self.provider = Arc::new(provider);
        
        // 注册到全局，供其他 Skill 使用
        ctx.register_ai_provider(self.provider.clone());
        
        Ok(())
    }
}

// 注册（编译时）
inventory::submit! {
    SkillRegistration::native::<AiProviderSkill>("ai-provider", "1.0.0")
}
```
