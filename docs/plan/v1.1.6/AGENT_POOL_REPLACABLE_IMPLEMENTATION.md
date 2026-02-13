# CIS v1.1.6 Agent Pool 可替换接口实现

> **实现日期**: 2026-02-12
> **目标**: 提供完整的 Agent Executor 接口实现示例
> **核心特性**: Agent 与任务解耦、可替换、可扩展

---

## 1. Agent Executor 实现

### 1.1 Claude Executor

```rust
use cis_core::agent::runtime::{AgentExecutor, AgentRuntime, TaskContext, TaskOutput};
use async_trait::async_trait;
use anyhow::Result;

/// Claude Agent 配置
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub max_tokens: usize,
}

/// Claude Agent 实现
pub struct ClaudeAgent {
    config: ClaudeConfig,
    client: reqwest::Client,
}

impl ClaudeAgent {
    pub fn new(config: ClaudeConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: reqwest::Client::new(),
        })
    }

    /// 准备 API 客户端
    async fn prepare_client(&self) -> Result<reqwest::Client> {
        Ok(self.client.clone())
    }
}

#[async_trait]
impl AgentExecutor for ClaudeAgent {
    /// 执行任务
    async fn execute(&self, ctx: TaskContext) -> Result<TaskOutput> {
        // 1. 准备客户端
        let client = self.prepare_client().await?;

        // 2. 渲染 prompt（由 TaskContext 提供）
        let prompt = ctx.render_prompt()?;

        // 3. 构建请求
        let request = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "messages": [{
                "role": "user",
                "content": prompt,
            }]
        });

        // 4. 发送请求
        let response = tokio::time::timeout(
            Duration::from_secs(ctx.timeout_secs.unwrap_or(300)),
            client
                .post(&self.config.base_url)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&request)
                .send()
        ).await??;

        // 5. 解析响应
        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("Claude API error: {}", error_text));
        }

        let response_json: serde_json::Value = response.json().await?;
        let content = response_json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage = response_json["usage"]
            .as_object()
            .map(|obj| obj.get("total_tokens").and_then(|v| v.as_u64()))
            .unwrap_or(0);

        // 6. 返回输出
        Ok(TaskOutput {
            task_id: ctx.id.clone(),
            content,
            tokens_used: usage,
            metadata: serde_json::json!({
                "model": self.config.model,
                "finish_reason": "stop"
            }),
        })
    }

    /// 流式对话（支持交互式任务）
    async fn chat_stream(&self, ctx: TaskContext) -> Result<Pin<Box<dyn Stream<Item = String> + Send>>> {
        let prompt = ctx.render_prompt()?;

        let client = self.prepare_client().await?;

        let request = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "stream": true,
            "messages": [{
                "role": "user",
                "content": prompt,
            }]
        });

        let response = client
            .post(&self.config.base_url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?;

        // 解析流式响应
        let stream = response.bytes_stream();
        let stream = async_stream::stream! {
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                // 解析 SSE 格式
                let lines = String::from_utf8_lossy(&chunk);
                for line in lines.lines() {
                    if line.starts_with("data: ") {
                        let data = &line[6..];
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(data) {
                            if let Some(content) = json["delta"].as_str() {
                                yield content.to_string();
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    /// 健康检查
    async fn health_check(&self) -> Result<HealthStatus> {
        let client = self.prepare_client().await?;

        let response = tokio::time::timeout(
            Duration::from_secs(5),
            client.get(&format!("{}/models", self.config.base_url))
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .send()
        ).await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                Ok(HealthStatus::Healthy {
                    status: "ok".to_string(),
                    message: "Claude API is accessible".to_string(),
                })
            }
            Ok(_) => Ok(HealthStatus::Unhealthy {
                status: "error".to_string(),
                message: "Claude API returned error".to_string(),
            }),
            Err(e) => Ok(HealthStatus::Unhealthy {
                status: "timeout".to_string(),
                message: format!("Claude API health check timeout: {}", e),
            }),
        }
    }

    /// 能力列表
    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::CodeReview,
            Capability::ModuleRefactoring,
            Capability::Documentation,
        ]
    }
}
```

### 1.2 OpenCode Executor（GLM 内核）

```rust
/// OpenCode Agent 配置
#[derive(Debug, Clone)]
pub struct OpenCodeConfig {
    pub glm_api_key: String,
    pub endpoint: String,
    pub model: String,
    pub max_tokens: usize,
}

/// OpenCode Agent 实现（GLM 内核）
pub struct OpenCodeAgent {
    config: OpenCodeConfig,
    client: reqwest::Client,
}

impl OpenCodeAgent {
    pub fn new(config: OpenCodeConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl AgentExecutor for OpenCodeAgent {
    async fn execute(&self, ctx: TaskContext) -> Result<TaskOutput> {
        let prompt = ctx.render_prompt()?;

        // GLM API 请求格式
        let request = serde_json::json!({
            "model": self.config.model,
            "max_tokens": self.config.max_tokens,
            "messages": [{
                "role": "user",
                "content": prompt,
            }]
        });

        let response = tokio::time::timeout(
            Duration::from_secs(ctx.timeout_secs.unwrap_or(300)),
            self.client
                .post(&self.config.endpoint)
                .header("Authorization", format!("Bearer {}", self.config.glm_api_key))
                .json(&request)
                .send()
        ).await??;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!("OpenCode API error: {}", error_text));
        }

        let response_json: serde_json::Value = response.json().await?;
        let content = response_json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let usage = response_json["usage"]
            .as_object()
            .and_then(|obj| obj.get("total_tokens").and_then(|v| v.as_u64()))
            .unwrap_or(0);

        Ok(TaskOutput {
            task_id: ctx.id.clone(),
            content,
            tokens_used: usage,
            metadata: serde_json::json!({
                "model": self.config.model,
                "engine": "glm"
            }),
        })
    }

    async fn chat_stream(&self, ctx: TaskContext) -> Result<Pin<Box<dyn Stream<Item = String> + Send>>> {
        // OpenCode 流式实现（类似 Claude）
        // ...
        todo!("OpenCode stream implementation")
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // OpenCode 健康检查
        // ...
        Ok(HealthStatus::Healthy {
            status: "ok".to_string(),
            message: "OpenCode API is accessible".to_string(),
        })
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::ModuleRefactoring,
            Capability::EngineCodeInjection,
            Capability::PerformanceOptimization,
        ]
    }
}
```

### 1.3 Kimi Executor

```rust
/// Kimi Agent 配置
#[derive(Debug, Clone)]
pub struct KimiConfig {
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
}

/// Kimi Agent 实现
pub struct KimiAgent {
    config: KimiConfig,
    client: reqwest::Client,
}

#[async_trait]
impl AgentExecutor for KimiAgent {
    async fn execute(&self, ctx: TaskContext) -> Result<TaskOutput> {
        let prompt = ctx.render_prompt()?;

        // Kimi API 请求格式（月之暗面）
        let request = serde_json::json!({
            "model": self.config.model,
            "prompt": prompt,
            "temperature": 0.7,
        });

        let response = tokio::time::timeout(
            Duration::from_secs(ctx.timeout_secs.unwrap_or(300)),
            self.client
                .post(&self.config.endpoint)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .json(&request)
                .send()
        ).await??;

        // 解析响应
        let response_json: serde_json::Value = response.json().await?;
        let content = response_json["result"]
            .as_str()
            .unwrap_or("")
            .to_string();

        Ok(TaskOutput {
            task_id: ctx.id.clone(),
            content,
            tokens_used: 0, // Kimi 可能不返回 token 使用
            metadata: serde_json::json!({
                "model": self.config.model,
            }),
        })
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Kimi 健康检查
        Ok(HealthStatus::Healthy {
            status: "ok".to_string(),
            message: "Kimi API is accessible".to_string(),
        })
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::ModuleRefactoring,
            Capability::EngineCodeInjection,
        ]
    }
}
```

---

## 2. Task Context 实现

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// 任务上下文（Agent 不可知的完整数据结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    /// 任务 ID
    pub id: String,

    /// 任务名称
    pub name: String,

    /// 任务类型
    #[serde(rename = "type")]
    pub task_type: String,

    /// 优先级
    pub priority: String,

    /// Prompt 模板（包含占位符）
    pub prompt_template: String,

    /// 上下文变量（用于填充占位符）
    pub context_vars: HashMap<String, String>,

    /// 引擎代码上下文（如果需要引擎代码注入）
    pub engine_context: Option<EngineCodeContext>,

    /// 超时时间（秒）
    pub timeout_secs: Option<u64>,

    /// 依赖任务
    pub dependencies: Vec<String>,

    /// 元数据
    pub metadata: HashMap<String, serde_json::Value>,
}

impl TaskContext {
    /// 渲染 prompt（填充变量和引擎上下文）
    pub fn render_prompt(&self) -> Result<String> {
        let mut prompt = self.prompt_template.clone();

        // 1. 填充上下文变量
        for (key, value) in &self.context_vars {
            let placeholder = format!("{{{}}}", key);
            prompt = prompt.replace(&placeholder, value);
        }

        // 2. 添加引擎代码上下文（如果有）
        if let Some(engine_ctx) = &self.engine_context {
            prompt.push_str("\n\n== 引擎代码上下文 ==\n");
            prompt.push_str(&format!("引擎类型: {:?}\n", engine_ctx.engine_type));
            prompt.push_str("可修改源代码:\n");

            for file in &engine_ctx.injectable_files {
                let file_info = format!(
                    "- {}\n",
                    file.relative_path
                );
                prompt.push_str(&file_info);

                // 如果文件较小，直接包含内容
                if file.size < 10 * 1024 {
                    let content = String::from_utf8_lossy(&file.content);
                    prompt.push_str("```cpp\n");
                    prompt.push_str(&content);
                    prompt.push_str("\n```\n");
                } else {
                    prompt.push_str("(内容过大，仅提供路径)\n");
                }
            }

            prompt.push_str("只读参考:\n");
            for file in &engine_ctx.readonly_files {
                prompt.push_str(&format!("- {}\n", file.relative_path));
            }
        }

        // 3. 添加任务元数据
        if !self.metadata.is_empty() {
            prompt.push_str("\n== 任务元数据 ==\n");
            for (key, value) in &self.metadata {
                prompt.push_str(&format!("{}: {:?}\n", key, value));
            }
        }

        Ok(prompt)
    }
}

/// 引擎代码上下文
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineCodeContext {
    /// 引擎类型
    pub engine_type: EngineType,

    /// 可注入文件（源代码，可修改）
    pub injectable_files: Vec<EngineFile>,

    /// 只读文件（引擎 API、文档等）
    pub readonly_files: Vec<EngineFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineFile {
    pub relative_path: String,
    pub content: Vec<u8>,
    pub size: usize,
}
```

---

## 3. Agent Registry 实现

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent 注册表
pub struct AgentRegistryImpl {
    agents: Arc<RwLock<HashMap<String, AgentDescriptor>>>,
}

impl AgentRegistryImpl {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册 Agent
    pub async fn register(&self, descriptor: AgentDescriptor) {
        let mut agents = self.agents.write().await;
        tracing::info!("注册 Agent: {} ({})", descriptor.display_name, descriptor.agent_type);
        agents.insert(descriptor.agent_type.clone(), descriptor);
    }

    /// 获取 Agent 描述符
    pub async fn get(&self, agent_type: &str) -> Option<AgentDescriptor> {
        let agents = self.agents.read().await;
        agents.get(agent_type).cloned()
    }

    /// 列出所有 Agents
    pub async fn list(&self) -> Vec<AgentDescriptor> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// 查询能力匹配的 Agents
    pub async fn find_by_capability(&self, capability: &str) -> Vec<AgentDescriptor> {
        let agents = self.agents.read().await;
        agents
            .values()
            .filter(|agent| agent.capabilities.contains(&capability.to_string()))
            .cloned()
            .collect()
    }
}

/// Agent 描述符
#[derive(Debug, Clone)]
pub struct AgentDescriptor {
    pub agent_type: String,
    pub display_name: String,
    pub capabilities: Vec<String>,
    pub factory: Arc<dyn Fn() -> Result<Box<dyn AgentExecutor>> + Send + Sync>,
}
```

---

## 4. 完整工作流程示例

### 4.1 创建 Agent Pool

```rust
use cis_core::agent::pool::{AgentPool, AgentRegistry};

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // 2. 创建 Agent Registry
    let registry = Arc::new(AgentRegistryImpl::new());

    // 3. 注册默认 Agents
    registry.register(AgentDescriptor {
        agent_type: "claude".to_string(),
        display_name: "Claude (Anthropic)".to_string(),
        capabilities: vec!["code_review".to_string(), "module_refactoring".to_string()],
        factory: Arc::new(|| Ok(Box::new(ClaudeAgent::new(ClaudeConfig {
            api_key: std::env::var("ANTHROPIC_API_KEY")?,
            model: "claude-3-sonnet-20250214".to_string(),
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
            max_tokens: 4096,
        })?)),
    });

    registry.register(AgentDescriptor {
        agent_type: "opencode".to_string(),
        display_name: "OpenCode (GLM)".to_string(),
        capabilities: vec!["module_refactoring".to_string(), "engine_injection".to_string()],
        factory: Arc::new(|| Ok(Box::new(OpenCodeAgent::new(OpenCodeConfig {
            glm_api_key: std::env::var("GLM_API_KEY")?,
            endpoint: "https://open.bigmodel.cn/api/paas/v4/chat/completions".to_string(),
            model: "glm-4-plus".to_string(),
            max_tokens: 8192,
        })?)),
    });

    // 4. 创建 Agent Pool
    let pool = AgentPool::new(registry);

    // 5. 启动 Pool
    pool.start().await?;

    Ok(())
}
```

### 4.2 创建和分配任务

```rust
use cis_core::agent::pool::TaskContext;

async fn create_and_assign_task() -> Result<()> {
    // 1. 创建任务上下文
    let task = TaskContext {
        id: "task-001".to_string(),
        name: "重构 scheduler 模块".to_string(),
        task_type: "module_refactoring".to_string(),
        priority: "p1".to_string(),
        prompt_template: "请重构以下模块...".to_string(),
        context_vars: {
            "module_name".to_string(): "scheduler".to_string(),
            "max_lines".to_string(): "500".to_string(),
        }.into_iter().collect(),
        engine_context: None,
        timeout_secs: Some(3600),
        dependencies: vec![],
        metadata: HashMap::new(),
    };

    // 2. 分配任务（自动选择合适的 Agent）
    let pool = AgentPool::instance();
    let output = pool.assign_task(task).await?;

    println!("任务执行结果: {:?}", output);

    Ok(())
}
```

---

## 5. 配置文件示例

### 5.1 Agent Pool 配置

```toml
# ~/.cis/agent-pool.toml

[pool]
# 任务队列大小
max_concurrent_tasks = 10

# Session 复用
enable_session_reuse = true
session_ttl_minutes = 30
max_sessions_per_runtime = 10

[registry]
# Agent 注册表路径
agent_descriptors_file = "~/.cis/agents.toml"

# 引擎代码扫描
enable_engine_scanning = true
max_engine_file_size_mb = 1
max_total_engine_size_mb = 10

[logging]
level = "info"
log_file = "~/.cis/logs/agent-pool.log"
```

### 5.2 Agents 定义

```toml
# ~/.cis/agents.toml

[[agent]]
type = "claude"
name = "Claude (Anthropic)"
enabled = true

[agent.capabilities]
code_review = true
module_refactoring = true
documentation = true

[agent.config]
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-3-sonnet-20250214"
base_url = "https://api.anthropic.com/v1/messages"
max_tokens = 4096

[[agent]]
type = "opencode"
name = "OpenCode (GLM)"
enabled = true

[agent.capabilities]
module_refactoring = true
engine_injection = true
performance_optimization = true

[agent.config]
glm_api_key_env = "GLM_API_KEY"
endpoint = "https://open.bigmodel.cn/api/paas/v4/chat/completions"
model = "glm-4-plus"
max_tokens = 8192

[[agent]]
type = "kimi"
name = "Kimi (月之暗面)"
enabled = false  # 可选启用

[agent.capabilities]
module_refactoring = true
engine_injection = true

[agent.config]
api_key_env = "KIMI_API_KEY"
endpoint = "https://api.moonshot.cn/v1"
model = "moonshot-v1-8k"
```

---

## 6. CLI 命令示例

```bash
# 启动 Agent Pool
cis agent-pool start --config ~/.cis/agent-pool.toml

# 查看状态
cis agent-pool status

# 列出 Agents
cis agent-pool list-agents

# 创建任务
cis agent-pool create-task --type module_refactoring --priority p1 \
    --prompt "重构 scheduler 模块" \
    --context module_name=scheduler,max_lines=500

# 分配任务
cis agent-pool assign-task --task-id task-001

# 查看任务队列
cis agent-pool list-tasks

# 查看执行历史
cis agent-pool history --limit 10

# 导出报告
cis agent-pool report --output report.json --format json
```

---

## 7. 测试示例

### 7.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[tokio::test]
    async fn test_claude_agent_execute() {
        let agent = ClaudeAgent::new(ClaudeConfig {
            api_key: "test-key".to_string(),
            model: "claude-3-sonnet-20240229".to_string(),
            base_url: "https://api.anthropic.com/v1/messages".to_string(),
            max_tokens: 100,
        }).unwrap();

        let task = TaskContext {
            id: "test-001".to_string(),
            name: "测试任务".to_string(),
            task_type: "code_review".to_string(),
            priority: "p2".to_string(),
            prompt_template: "请审查以下代码".to_string(),
            context_vars: HashMap::new(),
            engine_context: None,
            timeout_secs: Some(60),
            dependencies: vec![],
            metadata: HashMap::new(),
        };

        let result = agent.execute(task).await;

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(!output.content.is_empty());
    }

    #[tokio::test]
    async fn test_agent_registry() {
        let registry = AgentRegistryImpl::new();

        // 测试注册
        let descriptor = AgentDescriptor {
            agent_type: "test".to_string(),
            display_name: "Test Agent".to_string(),
            capabilities: vec!["test".to_string()],
            factory: Arc::new(|| Ok(Box::new(TestAgent))),
        };

        tokio::spawn(async move {
            registry.register(descriptor).await;
        }).await.unwrap();

        // 测试获取
        let retrieved = registry.get("test").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().agent_type, "test");
    }
}
```

### 7.2 集成测试

```rust
#[tokio::test]
async fn test_full_workflow() {
    // 1. 初始化 Registry
    let registry = Arc::new(AgentRegistryImpl::new());

    // 2. 注册 Agents
    registry.register(AgentDescriptor {
        agent_type: "claude".to_string(),
        display_name: "Claude Test".to_string(),
        capabilities: vec!["code_review".to_string()],
        factory: Arc::new(|| Ok(Box::new(MockClaudeAgent::new()?))),
    });

    // 3. 创建 Pool
    let pool = AgentPool::new(registry);

    // 4. 创建任务
    let task = TaskContext {
        id: "integration-001".to_string(),
        name: "集成测试任务".to_string(),
        task_type: "code_review".to_string(),
        priority: "p1".to_string(),
        prompt_template: "请审查代码: {{code}}".to_string(),
        context_vars: {
            "code".to_string(): "fn main() {}".to_string(),
        }.into_iter().collect(),
        engine_context: None,
        timeout_secs: Some(60),
        dependencies: vec![],
        metadata: HashMap::new(),
    };

    // 5. 分配并执行
    let output = pool.assign_task(task).await;

    // 6. 验证结果
    assert!(output.is_ok());
    let result = output.unwrap();
    assert_eq!(result.task_id, "integration-001");
}
```

---

## 8. 部署和运行

### 8.1 构建和安装

```bash
# 构建 CIS
cargo build --release

# 安装 Agent Pool
cargo install --path cis-agent-pool

# 启动服务
sudo systemctl start cis-agent-pool
# 或
cis-agent-pool --daemon --config ~/.cis/agent-pool.toml
```

### 8.2 系统服务配置

```ini
# /etc/systemd/system/cis-agent-pool.service

[Unit]
Description=CIS Agent Pool
After=network.target

[Service]
Type=simple
User=cis
ExecStart=/usr/bin/cis-agent-pool --config /home/cis/.cis/agent-pool.toml
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
```

---

## 9. 监控和日志

### 9.1 日志示例

```
[2026-02-12 10:23:45 INFO] 注册 Agent: Claude (Anthropic) (claude)
[2026-02-12 10:23:46 INFO] 注册 Agent: OpenCode (GLM) (opencode)
[2026-02-12 10:23:47 INFO] Agent Pool 已启动，监听端口 7678
[2026-02-12 10:24:01 INFO] 任务创建: task-001 (类型: module_refactoring)
[2026-02-12 10:24:01 INFO] 分配任务 task-001 到 Agent opencode (能力匹配)
[2026-02-12 10:24:02 INFO] 使用 Session session-abc123 (复用)
[2026-02-12 10:24:05 INFO] Agent opencode 开始执行任务 task-001
[2026-02-12 10:25:30 INFO] Agent opencode 完成任务 task-001
[2026-02-12 10:25:30 INFO] 归还 Session session-abc123 到复用队列
[2026-02-12 10:25:31 INFO] 任务完成，tokens_used: 1234
```

### 9.2 Prometheus 指标

```rust
/// Prometheus 指标收集
use prometheus::{Counter, Histogram, Gauge};

lazy_static! {
    static ref AGENT_EXECUTIONS_TOTAL: Counter = register_counter!(
        "cis_agent_executions_total",
        "Agent executions total"
    );
    static ref AGENT_EXECUTION_DURATION: Histogram = register_histogram!(
        "cis_agent_execution_duration_seconds",
        "Agent execution duration"
    );
    static ref AGENT_EXECUTION_ERRORS: Counter = register_counter!(
        "cis_agent_execution_errors_total",
        "Agent execution errors total"
    );
    static ref ACTIVE_SESSIONS: Gauge = register_gauge!(
        "cis_active_sessions",
        "Active sessions"
    );
}

// 在执行任务时记录指标
AGENT_EXECUTIONS_TOTAL
    .with_label_values(&[agent_type])
    .inc();

AGENT_EXECUTION_DURATION
    .observe(duration_secs);

ACTIVE_SESSIONS.set(active_count);
```

---

## 总结

### 核心特性

1. **可替换的 Agent 接口**
   - AgentExecutor trait 定义统一的执行接口
   - 通过 AgentDescriptor 注册，工厂模式创建
   - 运行时动态加载，无需重新编译

2. **Agent 与任务解耦**
   - TaskContext 封装完整任务信息
   - Agent 不需要知道任务来源
   - 支持任意任务格式

3. **Session 复用**
   - SessionPool 统一管理不同 Runtime 的 Session
   - 序列化/反序列化支持跨 Agent 传递
   - TTL 自动清理过期 Session

4. **引擎代码注入**
   - EngineCodeScanner 扫描引擎代码
   - EngineCodeContext 传递给 Task
   - 自动准备代码上下文到 prompt

5. **可扩展性**
   - 新增 Agent：只需实现 trait 并注册
   - 新增能力：扩枚举即可
   - 无需修改核心代码

---

**文档版本**: 1.0
**实现完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**状态**: ✅ 可实现
