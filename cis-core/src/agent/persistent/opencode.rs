//! # OpenCode Persistent Agent
//!
//! 基于 HTTP 协议实现的 OpenCode 持久化 Agent。
//!
//! ## 功能特性
//! - 通过 `opencode serve` 启动本地 HTTP 服务器
//! - 支持连接到已有的 OpenCode Server
//! - 完整的生命周期管理（启动、停止、连接、attach）
//! - HTTP Basic Auth 认证支持
//!
//! ## 使用示例
//! ```rust,no_run
//! use cis_core::agent::persistent::opencode::{OpenCodePersistentAgent, OpenCodeRuntime};
//! use cis_core::agent::persistent::{AgentConfig, AgentRuntime};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 启动新的 OpenCode Agent
//! let config = AgentConfig::new("my-agent", std::path::PathBuf::from("/work"));
//! let runtime = OpenCodeRuntime;
//! let agent = runtime.create_agent(config).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rand::Rng;
use serde_json::json;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::agent::persistent::{
    AgentConfig, AgentInfo, AgentRuntime, AgentStatus, PersistentAgent, RuntimeType, TaskRequest,
    TaskResult,
};
use crate::error::{CisError, Result};

/// OpenCode 持久化 Agent 实现
///
/// 通过 HTTP 协议与 OpenCode Server 通信，支持本地启动或远程连接。
pub struct OpenCodePersistentAgent {
    /// Agent 唯一标识
    agent_id: String,
    /// HTTP 客户端
    http_client: reqwest::Client,
    /// OpenCode Server 基础 URL
    base_url: String,
    /// 认证密码（如果有）
    password: Option<String>,
    /// 本地 serve 进程（如果是本地启动）
    process: Arc<RwLock<Option<Child>>>,
    /// Agent 状态
    state: Arc<RwLock<AgentState>>,
}

/// Agent 内部状态
#[derive(Debug, Clone)]
struct AgentState {
    /// 当前状态
    status: AgentStatus,
    /// 最后活动时间
    last_activity: DateTime<Utc>,
    /// 总请求数
    total_requests: u64,
    /// 当前任务 ID
    current_task: Option<String>,
}

impl OpenCodePersistentAgent {
    /// 启动本地 OpenCode Server 并创建 Agent
    ///
    /// # Arguments
    /// * `config` - Agent 配置
    ///
    /// # Returns
    /// 新创建的 OpenCodePersistentAgent 实例
    ///
    /// # Errors
    /// - 如果找不到 opencode 命令
    /// - 如果端口被占用
    /// - 如果服务器启动超时
    pub async fn start(config: AgentConfig) -> Result<Self> {
        info!("Starting OpenCode persistent agent: {}", config.name);

        // 1. 寻找可用端口
        let port = find_free_port().await?;
        debug!("Found free port: {}", port);

        // 2. 生成随机密码
        let password = generate_random_password();
        debug!("Generated random password for authentication");

        // 3. 检查 opencode 命令是否可用
        match which::which("opencode") {
            Ok(path) => debug!("Found opencode at: {:?}", path),
            Err(_) => {
                return Err(CisError::execution(
                    "opencode command not found. Please install OpenCode first.",
                ));
            }
        }

        // 4. 启动 opencode serve 进程
        let mut cmd = Command::new("opencode");
        cmd.args(&[
            "serve",
            "--port",
            &port.to_string(),
            "--hostname",
            "127.0.0.1",
            "--print-logs",
        ])
        .env("OPENCODE_SERVER_PASSWORD", &password)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&config.work_dir);

        // 设置环境变量
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(|e| {
            CisError::execution(format!("Failed to spawn opencode serve process: {}", e))
        })?;

        info!("OpenCode server process started with PID: {:?}", child.id());

        // 5. 等待 HTTP 服务就绪
        let base_url = format!("http://127.0.0.1:{}", port);
        let startup_timeout = Duration::from_secs(config.default_timeout_secs.min(30));

        match wait_for_server(&base_url, &password, startup_timeout).await {
            Ok(()) => info!("OpenCode server is ready at {}", base_url),
            Err(e) => {
                // 启动失败，尝试终止进程
                warn!("OpenCode server failed to start, terminating process");
                let mut child = child;
                let _ = child.kill().await;
                return Err(e);
            }
        }

        // 6. 创建 HTTP 客户端
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .map_err(|e| CisError::execution(format!("Failed to create HTTP client: {}", e)))?;

        let agent_id = format!("opencode-{}", uuid::Uuid::new_v4());

        Ok(Self {
            agent_id,
            http_client,
            base_url,
            password: Some(password),
            process: Arc::new(RwLock::new(Some(child))),
            state: Arc::new(RwLock::new(AgentState {
                status: AgentStatus::Idle,
                last_activity: Utc::now(),
                total_requests: 0,
                current_task: None,
            })),
        })
    }

    /// 连接到已有的 OpenCode Server
    ///
    /// # Arguments
    /// * `url` - OpenCode Server URL（例如：http://localhost:8080）
    /// * `password` - 认证密码（如果有）
    ///
    /// # Returns
    /// 新创建的 OpenCodePersistentAgent 实例
    ///
    /// # Errors
    /// - 如果连接失败
    /// - 如果服务器返回错误
    pub async fn connect(url: &str, password: Option<&str>) -> Result<Self> {
        info!("Connecting to OpenCode server at: {}", url);

        // 标准化 URL
        let base_url = url.trim_end_matches('/').to_string();

        // 创建 HTTP 客户端
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| CisError::execution(format!("Failed to create HTTP client: {}", e)))?;

        // 验证连接 - 尝试访问根路径
        let resp = http_client
            .get(&format!("{}/", base_url))
            .send()
            .await
            .map_err(|e| {
                CisError::execution(format!("Failed to connect to OpenCode server at {}: {}", url, e))
            })?;

        if !resp.status().is_success() {
            // 可能需要认证
            if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                if let Some(pwd) = password {
                    let auth = base64_auth(pwd);
                    let resp = http_client
                        .get(&format!("{}/", base_url))
                        .header("Authorization", format!("Basic {}", auth))
                        .send()
                        .await
                        .map_err(|e| {
                            CisError::execution(format!(
                                "Failed to connect to OpenCode server at {}: {}",
                                url, e
                            ))
                        })?;

                    if !resp.status().is_success() {
                        return Err(CisError::execution(format!(
                            "Failed to authenticate to OpenCode server at {}: {}",
                            url,
                            resp.status()
                        )));
                    }
                } else {
                    return Err(CisError::execution(format!(
                        "OpenCode server at {} requires authentication",
                        url
                    )));
                }
            } else {
                return Err(CisError::execution(format!(
                    "OpenCode server at {} returned error: {}",
                    url,
                    resp.status()
                )));
            }
        }

        info!("Successfully connected to OpenCode server at {}", url);

        let agent_id = format!("opencode-{}", uuid::Uuid::new_v4());

        Ok(Self {
            agent_id,
            http_client,
            base_url,
            password: password.map(String::from),
            process: Arc::new(RwLock::new(None)),
            state: Arc::new(RwLock::new(AgentState {
                status: AgentStatus::Idle,
                last_activity: Utc::now(),
                total_requests: 0,
                current_task: None,
            })),
        })
    }

    /// 获取 Agent 统计信息
    pub async fn stats(&self) -> (u64, DateTime<Utc>) {
        let state = self.state.read().await;
        (state.total_requests, state.last_activity)
    }

    /// 获取 base URL
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 检查是否为本地管理的进程
    pub async fn is_local(&self) -> bool {
        self.process.read().await.is_some()
    }
}

#[async_trait]
impl PersistentAgent for OpenCodePersistentAgent {
    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::OpenCode
    }

    async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        let start_time = Instant::now();

        // 更新状态为忙碌
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Busy;
            state.current_task = Some(task.task_id.clone());
        }

        debug!("Executing task {} on OpenCode agent", task.task_id);

        // 构造请求体 - 使用 OpenCode HTTP API 格式
        let request_body = json!({
            "messages": [
                {
                    "role": "system",
                    "content": task.context.get("system_prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or("You are a helpful assistant.")
                },
                {
                    "role": "user",
                    "content": task.prompt
                }
            ],
            "stream": false,
            "model": task.context.get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
        });

        // 发送请求到 OpenCode Server
        let url = format!("{}/api/chat", self.base_url);
        let mut request = self.http_client.post(&url).json(&request_body);

        // 添加认证头（如果有密码）
        if let Some(ref password) = self.password {
            let auth = base64_auth(password);
            request = request.header("Authorization", format!("Basic {}", auth));
        }

        let resp = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                // 更新状态为错误
                let mut state = self.state.write().await;
                state.status = AgentStatus::Error;
                state.current_task = None;

                return Err(CisError::execution(format!(
                    "Failed to send request to OpenCode server: {}",
                    e
                )));
            }
        };

        let status = resp.status();
        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();

            // 更新状态为错误
            let mut state = self.state.write().await;
            state.status = AgentStatus::Error;
            state.current_task = None;

            return Err(CisError::execution(format!(
                "OpenCode API error ({}): {}",
                status,
                error_text
            )));
        }

        // 解析响应
        let response_body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                let mut state = self.state.write().await;
                state.status = AgentStatus::Error;
                state.current_task = None;

                return Err(CisError::execution(format!(
                    "Failed to parse OpenCode response: {}",
                    e
                )));
            }
        };

        // 提取内容 - 支持 OpenAI 兼容格式
        let content = response_body
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                // 尝试其他格式
                response_body
                    .get("response")
                    .and_then(|r| r.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "No response content".to_string());

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // 更新状态为空闲
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Idle;
            state.last_activity = Utc::now();
            state.total_requests += 1;
            state.current_task = None;
        }

        info!(
            "Task {} completed in {}ms on OpenCode agent",
            task.task_id, duration_ms
        );

        Ok(TaskResult {
            task_id: task.task_id,
            success: true,
            output: Some(content),
            error: None,
            duration_ms,
            completed_at: Utc::now(),
            metadata: {
                let mut meta = HashMap::new();
                meta.insert("agent_id".to_string(), json!(self.agent_id));
                meta.insert("runtime".to_string(), json!("opencode"));
                meta
            },
        })
    }

    async fn status(&self) -> AgentStatus {
        // 检查 HTTP 服务是否可用
        let check_url = format!("{}/", self.base_url);
        let mut request = self.http_client.get(&check_url).timeout(Duration::from_secs(5));

        if let Some(ref password) = self.password {
            let auth = base64_auth(password);
            request = request.header("Authorization", format!("Basic {}", auth));
        }

        match request.send().await {
            Ok(resp) if resp.status().is_success() => {
                let state = self.state.read().await;
                state.status.clone()
            }
            Ok(resp) => {
                warn!("OpenCode server returned error status: {}", resp.status());
                AgentStatus::Error
            }
            Err(e) => {
                warn!("OpenCode server health check failed: {}", e);
                AgentStatus::Error
            }
        }
    }

    async fn attach(&self) -> Result<()> {
        info!("Attaching to OpenCode server at {}", self.base_url);

        // 调用 opencode attach 命令
        let mut cmd = Command::new("opencode");
        cmd.arg("attach").arg(&self.base_url);

        // 如果有密码，添加密码参数
        if let Some(ref password) = self.password {
            cmd.arg("--password").arg(password);
        }

        let status = cmd.status().await.map_err(|e| {
            CisError::execution(format!("Failed to execute opencode attach: {}", e))
        })?;

        if !status.success() {
            return Err(CisError::execution(format!(
                "opencode attach failed with exit code: {:?}",
                status.code()
            )));
        }

        info!("Detached from OpenCode server");
        Ok(())
    }

    async fn detach(&self) -> Result<()> {
        // HTTP 模式下无需特殊处理
        // attach 命令会在用户退出时自动 detach
        debug!("Detach called for OpenCode agent - no action needed for HTTP mode");
        Ok(())
    }

    async fn shutdown(&self) -> Result<()> {
        info!("Shutting down OpenCode agent: {}", self.agent_id);

        // 更新状态
        {
            let mut state = self.state.write().await;
            state.status = AgentStatus::Shutdown;
        }

        // 如果有本地进程，优雅地终止它
        if let Some(mut child) = self.process.write().await.take() {
            info!("Terminating local OpenCode server process");

            // 尝试优雅终止
            let _ = child.kill().await;

            // 等待进程退出
            match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                Ok(Ok(status)) => {
                    info!("OpenCode server process exited with status: {:?}", status);
                }
                Ok(Err(e)) => {
                    warn!("Failed to wait for OpenCode process: {}", e);
                }
                Err(_) => {
                    warn!("Timeout waiting for OpenCode process to exit");
                }
            }
        }

        Ok(())
    }
}

impl Drop for OpenCodePersistentAgent {
    fn drop(&mut self) {
        // 尝试在 drop 时清理资源
        // 注意：这里不能使用 async，只能尝试同步关闭
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            let process = self.process.clone();
            rt.spawn(async move {
                if let Some(mut child) = process.write().await.take() {
                    let _ = child.kill().await;
                }
            });
        }
    }
}

/// OpenCode Runtime 实现
///
/// 负责创建和管理 OpenCode Persistent Agent。
pub struct OpenCodeRuntime;

impl OpenCodeRuntime {
    /// 创建新的 OpenCodeRuntime
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenCodeRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentRuntime for OpenCodeRuntime {
    fn runtime_type(&self) -> RuntimeType {
        RuntimeType::OpenCode
    }

    async fn create_agent(&self, config: AgentConfig) -> Result<Box<dyn PersistentAgent>> {
        let agent = OpenCodePersistentAgent::start(config).await?;
        Ok(Box::new(agent))
    }

    async fn list_agents(&self) -> Vec<AgentInfo> {
        // 这里可以通过扫描已知端口或进程来实现
        // 暂时返回空列表
        // TODO: 实现进程扫描或端口检测
        vec![]
    }
}

/// 寻找可用端口
async fn find_free_port() -> Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
        CisError::execution(format!("Failed to bind to find free port: {}", e))
    })?;

    let port = listener.local_addr()?.port();
    drop(listener);

    Ok(port)
}

/// 等待服务器就绪
async fn wait_for_server(url: &str, password: &str, timeout: Duration) -> Result<()> {
    let start = Instant::now();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| CisError::execution(format!("Failed to create health check client: {}", e)))?;

    let auth = base64_auth(password);

    while start.elapsed() < timeout {
        match client
            .get(&format!("{}/", url))
            .header("Authorization", format!("Basic {}", auth))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                debug!("OpenCode server is ready");
                return Ok(());
            }
            Ok(resp) => {
                debug!("OpenCode server returned status: {}, retrying...", resp.status());
            }
            Err(e) => {
                debug!("OpenCode server not ready yet: {}, retrying...", e);
            }
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    Err(CisError::execution(format!(
        "OpenCode server failed to start within {:?}",
        timeout
    )))
}

/// 生成随机密码
fn generate_random_password() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();

    (0..32)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect()
}

/// 生成 Basic Auth 的 base64 编码
fn base64_auth(password: &str) -> String {
    // OpenCode 使用空用户名，密码作为密码
    let credentials = format!(":{}", password);
    base64::encode(credentials)
}

// 由于 base64 crate 可能不可用，这里提供一个简单的实现
mod base64 {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    pub fn encode(input: impl AsRef<[u8]>) -> String {
        let input = input.as_ref();
        let mut result = String::with_capacity((input.len() + 2) / 3 * 4);

        for chunk in input.chunks(3) {
            let b = match chunk.len() {
                1 => [chunk[0], 0, 0],
                2 => [chunk[0], chunk[1], 0],
                3 => [chunk[0], chunk[1], chunk[2]],
                _ => unreachable!(),
            };

            let n = (b[0] as usize) << 16 | (b[1] as usize) << 8 | (b[2] as usize);

            result.push(ALPHABET[(n >> 18) & 63] as char);
            result.push(ALPHABET[(n >> 12) & 63] as char);

            if chunk.len() > 1 {
                result.push(ALPHABET[(n >> 6) & 63] as char);
            } else {
                result.push('=');
            }

            if chunk.len() > 2 {
                result.push(ALPHABET[n & 63] as char);
            } else {
                result.push('=');
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_password() {
        let pwd1 = generate_random_password();
        let pwd2 = generate_random_password();

        assert_eq!(pwd1.len(), 32);
        assert_eq!(pwd2.len(), 32);
        assert_ne!(pwd1, pwd2); // 几乎不可能相同
    }

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64::encode(""), "");
        assert_eq!(base64::encode("f"), "Zg==");
        assert_eq!(base64::encode("fo"), "Zm8=");
        assert_eq!(base64::encode("foo"), "Zm9v");
        assert_eq!(base64::encode("hello"), "aGVsbG8=");
    }

    #[test]
    fn test_base64_auth() {
        let auth = base64_auth("test-password");
        assert!(!auth.is_empty());
        // 解码验证 - :test-password 的 base64 编码
        assert_eq!(auth, base64::encode(":test-password"));
    }

    #[tokio::test]
    async fn test_find_free_port() {
        let port = find_free_port().await.unwrap();
        assert!(port > 0);
        assert!(port <= 65535);

        // 验证端口确实可用
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port)).await;
        assert!(listener.is_ok());
    }

    // 集成测试（需要 opencode 命令）
    #[tokio::test]
    #[ignore = "Requires opencode to be installed"]
    async fn test_opencode_agent_lifecycle() {
        let config = AgentConfig::new(
            "test-agent",
            std::env::temp_dir().join("opencode-test"),
        );

        // 启动 Agent
        let agent = OpenCodePersistentAgent::start(config).await.unwrap();
        assert_eq!(agent.runtime_type(), RuntimeType::OpenCode);
        assert!(agent.is_local().await);

        // 检查状态
        let status = agent.status().await;
        assert!(status.is_available());

        // 执行任务
        let task = TaskRequest::new("test-task-1", "Say 'Hello, World!'")
            .with_context("system_prompt", "You are a test assistant.");

        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
        assert!(result.output.is_some());

        // 检查统计
        let (total, _) = agent.stats().await;
        assert_eq!(total, 1);

        // 关闭 Agent
        agent.shutdown().await.unwrap();

        // 验证状态
        let status = agent.status().await;
        assert!(!status.is_available());
    }

    #[tokio::test]
    #[ignore = "Requires running OpenCode server"]
    async fn test_opencode_connect() {
        // 假设有一个运行在 8080 端口的 OpenCode server
        let agent = OpenCodePersistentAgent::connect("http://127.0.0.1:8080", Some("test-password"))
            .await
            .unwrap();

        assert!(!agent.is_local().await);

        // 执行任务
        let task = TaskRequest::new("test-task", "Hello");
        let result = agent.execute(task).await.unwrap();
        assert!(result.success);
    }
}
