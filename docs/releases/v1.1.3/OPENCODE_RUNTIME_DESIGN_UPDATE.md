# OpenCode Runtime 设计更新

**日期**: 2026-02-09  
**更新原因**: 调研发现 OpenCode 使用 HTTP 模式而非 `-p` 参数

---

## 调研关键发现

### OpenCode 实际机制

| 预期 | 实际 |
|------|------|
| `opencode run -p` (Unix Socket) | `opencode serve` (HTTP Server) |
| `-p` 后台参数 | `-p` 仅在 `attach` 中表示 `--password` |
| Socket 文件通信 | HTTP/WebSocket/SSE |

### 核心命令

```bash
# 启动 HTTP Server（真正的"后台模式"）
opencode serve --host 127.0.0.1 --port 8080

# 连接运行中的 Server
opencode attach http://127.0.0.1:8080

# 单次命令模式（使用已有 server）
opencode run --attach http://127.0.0.1:8080 "任务"
```

### 通信协议

- **协议**: HTTP/WebSocket/SSE
- **认证**: `OPENCODE_SERVER_PASSWORD` 环境变量
- **接口**: RESTful API + SSE 流式输出

---

## 更新后的设计

### OpenCodePersistentAgent 架构

```rust
pub struct OpenCodePersistentAgent {
    agent_id: String,
    http_client: reqwest::Client,
    base_url: String,
    password: Option<String>,
    process: Option<Child>,  // serve 进程（如果是本地启动）
}

impl OpenCodePersistentAgent {
    /// 连接到已有的 OpenCode Server
    pub async fn connect(url: &str, password: Option<&str>) -> Result<Self> {
        // HTTP 客户端连接
    }
    
    /// 启动新的本地 Server
    pub async fn start_local(port: u16) -> Result<Self> {
        // spawn opencode serve
        // 等待 HTTP 就绪
        // 返回连接
    }
}

#[async_trait]
impl PersistentAgent for OpenCodePersistentAgent {
    async fn execute(&self, task: TaskRequest) -> Result<TaskResult> {
        // POST /v1/chat
        // 或 WebSocket 连接
    }
    
    async fn attach(&self) -> Result<()> {
        // opencode attach <url>
        // 或者直接在代码中实现交互式终端
    }
}
```

### HTTP API 封装

```rust
pub struct OpenCodeClient {
    client: reqwest::Client,
    base_url: String,
    password: Option<String>,
}

impl OpenCodeClient {
    /// 发送聊天请求
    pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse> {
        let url = format!("{}/v1/chat", self.base_url);
        let resp = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.password.as_deref().unwrap_or("")))
            .json(&json!({ "messages": messages }))
            .send()
            .await?;
        
        Ok(resp.json().await?)
    }
    
    /// SSE 流式输出
    pub async fn chat_stream(&self, messages: Vec<Message>) -> Result<impl Stream<Item = Result<String>>> {
        // SSE 实现
    }
}
```

---

## 实现调整

### 方案 A: 本地启动模式（推荐）

适用于 CIS 管理 OpenCode 进程：

```rust
impl OpenCodePersistentAgent {
    pub async fn start(config: AgentConfig) -> Result<Self> {
        // 1. 寻找可用端口
        let port = find_free_port().await?;
        
        // 2. 启动 opencode serve
        let mut child = Command::new("opencode")
            .args(&["serve", "--port", &port.to_string()])
            .env("OPENCODE_SERVER_PASSWORD", &generate_password())
            .spawn()?;
        
        // 3. 等待 HTTP 就绪
        wait_for_server(port).await?;
        
        // 4. 创建 HTTP 客户端
        let client = OpenCodeClient::new(
            format!("http://127.0.0.1:{}", port),
            Some(password),
        );
        
        Ok(Self { ... })
    }
}
```

### 方案 B: 外部连接模式

适用于连接用户已启动的 OpenCode Server：

```rust
impl OpenCodePersistentAgent {
    pub async fn connect(url: &str, password: Option<&str>) -> Result<Self> {
        let client = OpenCodeClient::new(url.to_string(), password.map(String::from));
        
        // 验证连接
        client.health_check().await?;
        
        Ok(Self { ... })
    }
}
```

---

## 与 Claude 的差异

| 特性 | ClaudePersistentAgent | OpenCodePersistentAgent |
|------|----------------------|------------------------|
| 通信 | PTY (stdin/stdout) | HTTP/WebSocket |
| 进程 | AgentSession + PTY | opencode serve |
| attach | SessionManager.attach | opencode attach URL |
| 适用 | 交互式任务 | API 调用任务 |

---

## 实施调整

### 任务 2.2 更新

原任务：实现 OpenCodePersistentAgent（Unix Socket）
更新为：实现 OpenCodePersistentAgent（HTTP Client）

**关键修改**:
1. 使用 `reqwest` 作为 HTTP 客户端
2. 管理 `opencode serve` 子进程
3. 实现 HTTP API 封装
4. 支持 WebSocket/SSE 流式输出

### 依赖更新

```toml
[dependencies]
reqwest = { version = "0.11", features = ["json", "stream"] }
tokio-tungstenite = "0.20"  # WebSocket（如果需要）
eventsource = "0.5"         # SSE（如果需要）
```

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| HTTP 延迟高于 Socket | 本地 127.0.0.1，延迟 < 1ms |
| 进程管理复杂 | 使用进程守护（如果 serve 崩溃自动重启）|
| 端口冲突 | 动态端口分配 |
| 认证安全 | 随机密码，仅本地监听 |

---

**设计更新完成**
