# CIS 集成层代码审阅报告

> **审阅日期**: 2026-02-15
> **审阅模块**: cis-mcp-adapter + skills
> **Agent ID**: adb698b
> **版本**: v1.1.5

---

## 1. 概述

### 1.1 模块职责

集成层是 CIS 与外部系统和内置功能连接的桥梁，包含两个关键部分：

| 模块 | 职责 | 关键特性 |
|------|------|---------|
| **cis-mcp-adapter** | MCP (Model Context Protocol) 适配器 | JSON-RPC 2.0、工具/资源管理、提示词管理 |
| **skills/** | 各类内置 Skill 实现 | Shell/Builtin/WASM 类型、进程隔离、事件驱动 |

这两个部分为 CIS 提供了：
- **外部协议适配**: 通过 MCP 与 AI Agents (Claude, OpenCode 等) 集成
- **内置功能扩展**: 11 个内置 Skill 提供 DAG 执行、AI 交互、通讯等功能
- **统一入口**: CapabilityLayer 整合所有 Skill
- **标准化接口**: Skill SDK 统一抽象

### 1.2 技术栈

- **协议**: JSON-RPC 2.0, Model Context Protocol (2024-11-05)
- **传输**: stdio (标准输入输出)
- **异步**: Tokio async/await
- **序列化**: serde + serde_json
- **进程管理**: tokio::process (Worker 隔离)

---

## 2. 架构分析

### 2.1 文件结构

```
crates/cis-mcp-adapter/          # MCP 适配器 (2,185 行)
├── src/
│   ├── main.rs                  # 54 行 - 入口
│   ├── server.rs                # 930 行 - MCP 服务器核心
│   ├── mcp_protocol.rs          # 222 行 - 协议类型定义
│   ├── prompts.rs               # 412 行 - 提示词管理
│   └── resources.rs             # 567 行 - 资源管理
└── tests/
    ├── mcp_protocol_tests.rs    # 协议测试
    └── test_mcp.sh              # Shell 测试

skills/                          # 内置 Skills (1,765 行)
├── dag-executor/                # 450 行 - DAG 执行器
│   └── src/lib.rs
├── matrix-register-skill/       # 375 行 - Matrix 注册
│   └── src/lib.rs
├── im/                          # 258 行 - 即时消息
│   └── src/lib.rs
├── push-client/                 # 191 行 - 推送客户端
│   └── src/lib.rs
├── init-wizard/                 # 192 行 - 初始化向导
│   └── src/lib.rs
├── memory-organizer/            # 181 行 - 记忆组织器
│   └── src/lib.rs
├── ai-executor/                 # 83 行 - AI 执行器
│   └── src/lib.rs
└── llm-memory-organizer/        # 35 行 - LLM 记忆组织
    └── src/lib.rs
```

### 2.2 模块组织

#### 2.2.1 MCP Adapter 架构

```
┌─────────────────────────────────────────────────────────┐
│                    CisMcpServer                          │
├─────────────────────────────────────────────────────────┤
│  Protocol Layer (mcp_protocol.rs)                       │
│  - McpRequest/Response (JSON-RPC 2.0)                   │
│  - Tool/Resource/Prompt 定义                             │
├─────────────────────────────────────────────────────────┤
│  Server Layer (server.rs)                               │
│  - Request routing & handling                           │
│  - stdio transport                                      │
│  - Error handling                                       │
├─────────────────────────────────────────────────────────┤
│  Managers (prompts.rs, resources.rs)                    │
│  - PromptStore: 提示词管理                              │
│  - ResourceManager: 资源管理                            │
├─────────────────────────────────────────────────────────┤
│  Integration (CapabilityLayer)                          │
│  - Tool execution via cis_capability                    │
│  - Resource access                                      │
└─────────────────────────────────────────────────────────┘
```

**架构优势**:
- ✅ **分层清晰**: 协议层、服务层、管理层分离
- ✅ **标准遵循**: 正确实现 JSON-RPC 2.0 和 MCP 规范
- ✅ **可扩展**: 易于添加新工具/资源/提示词
- ✅ **错误处理**: 标准错误码和错误传播

**架构问题**:
- ⚠️ **硬编码 schema**: 工具定义使用内联 JSON schema (server.rs:120-400)
- ⚠️ **缺少验证**: 参数验证不完整
- ⚠️ **状态管理**: 无订阅状态跟踪 (subscribe: false)

#### 2.2.2 Skills 架构

```
┌─────────────────────────────────────────────────────────┐
│                    Skill Types                          │
├─────────────────────────────────────────────────────────┤
│  Builtin (编译集成)                                      │
│  - DagExecutorSkill: 进程隔离 + Matrix 通信             │
│  - AiExecutor: 直接命令执行                             │
│  - MemoryOrganizer: LLM 增强                            │
├─────────────────────────────────────────────────────────┤
│  Shell (脚本调用)                                        │
│  - IM, PushClient, InitWizard                           │
├─────────────────────────────────────────────────────────┤
│  WASM (沙箱执行)                                         │
│  - LLM Memory Organizer                                 │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│              Skill Lifecycle                            │
├─────────────────────────────────────────────────────────┤
│  1. Discovery (CapabilityLayer)                         │
│  2. Load (Skill SDK)                                    │
│  3. Execute (Event/Context)                             │
│  4. Cleanup (Drop)                                      │
└─────────────────────────────────────────────────────────┘
```

**架构优势**:
- ✅ **统一接口**: Skill trait 统一抽象
- ✅ **类型多样**: 支持 Builtin/Shell/WASM
- ✅ **进程隔离**: DAG Worker 独立进程
- ✅ **事件驱动**: 基于事件触发

**架构问题**:
- ⚠️ **生命周期管理**: 无统一注册中心
- ⚠️ **版本管理**: 缺少版本控制
- ⚠️ **依赖管理**: 无依赖解析
- ⚠️ **权限控制**: 运行时无限制

---

## 3. 代码质量评估

### 3.1 优点

| 维度 | 说明 | 示例 |
|------|------|------|
| **协议实现正确** | 完全符合 JSON-RPC 2.0 和 MCP 规范 | `mcp_protocol.rs` 类型定义完整 |
| **错误处理完善** | 标准错误码、anyhow 上下文传播 | `server.rs:46-57` 错误响应 |
| **类型安全** | Rust 强类型、枚举模式匹配 | `McpRequest` 枚举分发 |
| **抽象良好** | Skill trait 统一、Provider 抽象 | `AiExecutor` 抽象多种 AI |
| **异步处理** | 全面使用 async/await | Tokio 运行时集成 |
| **日志记录** | tracing 结构化日志 | `debug!`, `info!`, `warn!`, `error!` |

### 3.2 问题汇总表

| 级别 | 问题描述 | 文件位置 | 影响 | 建议 |
|-----|---------|---------|------|------|
| **严重** | MCP 协议实现不完整 | `server.rs:106-107` | 🔴 功能缺失 | 实现资源订阅机制 |
| **严重** | 权限控制缺失 | `server.rs:78-88` | 🔴 安全风险 | 添加命令验证和权限检查 |
| **严重** | Worker 进程管理不当 | `dag-executor/lib.rs:169-190` | 🔴 资源泄漏 | 实现进程监控和清理 |
| **严重** | 命令注入风险 | `ai-executor/lib.rs:56` | 🔴 安全漏洞 | 参数验证和清理 |
| **重要** | 硬编码 JSON schema | `server.rs:120-400` | 🟠 可维护性 | 使用 schema 生成工具 |
| **重要** | 技能版本管理缺失 | Skills | 🟠 功能缺失 | 实现版本控制机制 |
| **重要** | 测试覆盖不足 | 集成层 | 🟠 质量保证 | 增加单元和集成测试 |
| **重要** | 技能生命周期不完整 | `CapabilityLayer` | 🟠 功能缺失 | 实现完整生命周期管理 |
| **重要** | 错误处理不统一 | 多处 | 🟠 可维护性 | 统一错误类型定义 |
| **一般** | 代码重复 | `server.rs` | 🟡 可维护性 | 提取公共代码 |
| **一般** | 函数过长 | `server.rs:120-400` | 🟡 可读性 | 拆分长函数 |
| **一般** | 文档不完整 | API 文档 | 🟡 可用性 | 补充文档注释 |
| **一般** | 性能监控缺失 | 整体 | 🟡 可观测性 | 添加指标收集 |

---

## 4. 功能完整性

### 4.1 已实现功能

#### MCP Adapter

✅ **协议基础**:
- `initialize` - 握手和版本协商
- `ping` - 连接检测

✅ **工具管理**:
- `tools/list` - 列出所有可用工具
- `tools/call` - 调用工具执行
- 15+ 工具定义 (DAG、记忆、AI、项目等)

✅ **资源管理**:
- `resources/list` - 列出资源
- `resources/read` - 读取资源内容

✅ **提示词管理**:
- `prompts/list` - 列出提示词
- `prompts/get` - 获取提示词
- `prompts/render` - 渲染提示词

#### Skills

✅ **DAG 执行器** (450 行):
- 进程隔离 (Global/Project/User/Type)
- Matrix Room 通信
- Worker 生命周期管理
- 重试机制

✅ **AI 执行器** (83 行):
- 多 AI 支持 (Claude/Kimi/Aider/Codex)
- 命令执行封装
- 工作目录支持

✅ **记忆组织器** (181 行):
- LLM 增强
- 自动分类

✅ **通讯功能**:
- Matrix 注册 (375 行)
- IM 集成 (258 行)
- 推送客户端 (191 行)

✅ **初始化向导** (192 行):
- 引导式配置

### 4.2 缺失/不完整功能

#### 协议层面

❌ **资源订阅**:
```
缺失: resources/subscribe
缺失: resources/unsubscribe
缺失: 订阅状态管理
缺失: 变更通知推送
```

❌ **动态注册**:
```
缺失: 运行时工具注册
缺失: 运行时资源注册
缺失: 热重载支持
```

❌ **批量操作**:
```
缺失: 批量工具调用
缺失: 批量资源读取
```

#### Skill 层面

❌ **生命周期管理**:
```
缺失: 技能注册中心
缺失: 技能依赖解析
缺失: 技能安装/卸载
缺失: 技能启用/禁用
```

❌ **版本管理**:
```
缺失: 版本号定义
缺失: 版本冲突检测
缺失: 多版本共存
```

❌ **权限控制**:
```
缺失: 文件系统权限限制
缺失: 网络权限限制
缺失: 命令执行权限验证
```

❌ **监控和调试**:
```
缺失: 执行统计
缺失: 性能指标
缺失: 错误追踪
```

---

## 5. 安全性审查

### 5.1 已有安全措施

✅ **协议安全**:
- JSON-RPC 2.0 标准错误处理
- 类型安全的序列化/反序列化
- 输入验证 (基础)

✅ **文件系统安全**:
- 使用 `dirs::data_dir()` 标准路径
- 相对路径限制

✅ **日志记录**:
- tracing 审计日志
- 错误追踪

### 5.2 潜在风险

#### 5.2.1 命令注入 (严重)

**位置**: `ai-executor/lib.rs:56`

```rust
cmd.arg(&req.prompt)  // 直接使用用户输入
```

**风险**:
- 恶意 prompt 可注入 shell 命令
- 例如: `prompt && rm -rf /`

**修复建议**:
```rust
// 1. 参数验证
fn validate_prompt(prompt: &str) -> Result<(), String> {
    // 禁止 shell 特殊字符
    if prompt.contains('|') || prompt.contains('&') || prompt.contains(';') {
        return Err("Invalid characters in prompt".to_string());
    }
    Ok(())
}

// 2. 参数分离
cmd.arg("--prompt").arg(&req.prompt);

// 3. 白名单模式
let allowed_prefixes = vec!["review:", "fix:", "explain:"];
if !allowed_prefixes.iter().any(|p| req.prompt.starts_with(p)) {
    return Err("Invalid prompt prefix".to_string());
}
```

#### 5.2.2 文件系统权限过大 (严重)

**位置**: `dag-executor/lib.rs:169-190`

```rust
let mut child = tokio::process::Command::new(&worker_binary)
    .args(&worker_args)
    .spawn()?;
```

**风险**:
- Worker 进程无文件系统限制
- 可访问任意文件

**修复建议**:
```rust
// 1. 使用 chroot 限制
.use_chroot(true)
.chroot_dir("/var/empty/cis-worker")

// 2. 使用 Landlock (Linux)
#[cfg(target_os = "linux")]
{
    let rules = landlock::Ruleset::new()
        .allow_path(PathBuf::from("/tmp/cis-work"))
        .create()?;
    rules.apply().await?;
}

// 3. 使用沙箱 (BSD/macOS)
#[cfg(target_os = "macos")]
{
    let sandbox = sandbox::Sandbox::new()
        .with_profile("strict")
        .with_exception("/tmp/cis-work");
    sandbox.apply()?;
}
```

#### 5.2.3 网络操作无限制 (中等)

**位置**: `push-client/lib.rs`, `im/lib.rs`

**风险**:
- Push Client 可连接任意服务器
- IM Skill 无认证机制

**修复建议**:
```rust
// 1. 网络权限检查
pub struct NetworkPermission {
    allowed_hosts: Vec<String>,
    allow_private: bool,
    allow_loopback: bool,
}

impl NetworkPermission {
    pub fn check(&self, url: &str) -> Result<(), Error> {
        let parsed = Url::parse(url)?;
        let host = parsed.host_str().ok_or(Error::InvalidHost)?;

        // 检查白名单
        if !self.allowed_hosts.contains(&host.to_string()) {
            return Err(Error::HostNotAllowed);
        }

        Ok(())
    }
}

// 2. 使用前检查
permission.check("https://example.com")?;
```

#### 5.2.4 缺乏认证 (中等)

**位置**: Matrix 集成

**风险**:
- Matrix 连接无认证
- 任何人可发送 DAG 执行请求

**修复建议**:
```rust
// 1. Token 认证
pub struct AuthToken {
    token: String,
    expires_at: DateTime<Utc>,
}

impl AuthToken {
    pub fn verify(&self, token: &str) -> Result<(), Error> {
        if self.token != token {
            return Err(Error::InvalidToken);
        }
        if self.expires_at < Utc::now() {
            return Err(Error::TokenExpired);
        }
        Ok(())
    }
}

// 2. 签名验证
use ed25519_dalek::{Keypair, Signature, Signer};

pub fn verify_request(
    public_key: &PublicKey,
    request: &[u8],
    signature: &Signature
) -> Result<(), Error> {
    public_key.verify(request, signature)
        .map_err(|_| Error::InvalidSignature)
}
```

#### 5.2.5 无审计日志 (低)

**位置**: 整体

**风险**:
- 无法追溯操作历史
- 难以进行安全审计

**修复建议**:
```rust
pub struct AuditLogger {
    log_file: Arc<Mutex<File>>,
}

impl AuditLogger {
    pub async fn log_operation(&self, op: AuditOperation) {
        let entry = json!({
            "timestamp": Utc::now().to_rfc3339(),
            "operation": op.operation,
            "user": op.user,
            "resource": op.resource,
            "result": op.result,
        });

        let mut log = self.log_file.lock().await;
        writeln!(log, "{}", entry).await;
    }
}
```

---

## 6. 性能分析

### 6.1 性能优点

✅ **进程隔离**:
- DAG Worker 独立进程，故障隔离
- 避免内存泄漏累积

✅ **异步处理**:
- 全面使用 async/await
- 非阻塞 I/O

✅ **连接复用**:
- Matrix Room 长连接
- 避免重复建立连接

### 6.2 性能问题

#### 6.2.1 缺少 Worker 监控 (中等)

**位置**: `dag-executor/lib.rs:169-190`

**问题**:
- 启动 Worker 后无监控
- 僵尸进程可能累积

**影响**:
- 资源浪费
- 内存泄漏

**优化建议**:
```rust
impl WorkerManager {
    pub async fn monitor_workers(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            self.cleanup_inactive_workers().await;
        }
    }

    async fn cleanup_inactive_workers(&self) {
        let mut workers = self.workers.write().await;
        workers.retain(|id, worker| {
            match worker.try_wait() {
                Ok(Some(status)) => {
                    warn!("Worker {} exited: {:?}", id, status);
                    false
                }
                Ok(None) => true, // Still running
                Err(e) => {
                    error!("Error checking worker {}: {}", id, e);
                    false
                }
            }
        });
    }
}
```

#### 6.2.2 数据库连接池管理不完善 (低)

**位置**: `im/lib.rs`

**问题**:
- 可能每次查询创建新连接
- 无连接复用

**优化建议**:
```rust
use sqlx::postgres::PgPoolOptions;

pub struct ImSkill {
    pool: PgPool,
}

impl ImSkill {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }
}
```

#### 6.2.3 缺少性能指标 (低)

**位置**: 整体

**问题**:
- 无执行时间统计
- 无吞吐量监控

**优化建议**:
```rust
use prometheus::{Counter, Histogram, IntGauge};

pub struct Metrics {
    requests_total: Counter,
    request_duration: Histogram,
    active_workers: IntGauge,
}

impl Metrics {
    pub fn record_request(&self, duration: Duration) {
        self.requests_total.inc();
        self.request_duration.observe(duration.as_secs_f64());
    }
}
```

#### 6.2.4 同步等待阻塞 (低)

**位置**: `dag-executor/lib.rs:178`

```rust
tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
```

**问题**:
- 固定延迟，不合理
- Worker 启动慢则失败

**优化建议**:
```rust
// 1. 轮询检查
for _ in 0..10 {
    match child.try_wait() {
        Ok(None) => {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }
        Ok(Some(status)) => {
            return Err(DagExecutorError::SpawnFailed(...));
        }
        Err(e) => {
            return Err(...);
        }
    }
}

// 2. 事件通知
// Worker 启动后发送就绪事件
```

---

## 7. 文档和测试

### 7.1 文档覆盖

#### 已有文档

✅ **代码注释**:
- 模块级文档注释 (`//!`)
- 函数级文档 (`///`)
- 示例代码 (部分)

❌ **缺失文档**:
- API 参考文档 (公开 API)
- 部署指南 (MCP Server 部署)
- 故障排查文档
- Skill 开发指南 (详细)
- 集成测试文档

### 7.2 测试覆盖

#### 现有测试

✅ **协议测试**:
- `mcp_protocol_tests.rs` - MCP 协议测试
- `test_mcp.sh` - Shell 测试

❌ **缺失测试**:

| 类型 | 覆盖率 | 优先级 |
|------|--------|--------|
| 单元测试 | < 30% | 高 |
| 集成测试 | 几乎没有 | 高 |
| 错误场景测试 | 缺失 | 中 |
| 性能测试 | 缺失 | 中 |
| 安全测试 | 缺失 | 高 |

**建议增加的测试**:

```rust
// 1. 单元测试示例
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let server = create_test_server().await;
        let response = server.handle_initialize(None, &json!({})).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_tool_call_with_invalid_params() {
        let server = create_test_server().await;
        let request = json!({
            "method": "tools/call",
            "params": { "name": "dag_create_run" }
        });
        let response = server.handle_tool_call(None, &request).await;
        assert!(response.is_err());
    }
}

// 2. 集成测试示例
#[tokio::test]
async fn test_dag_execution_e2e() {
    // 1. 启动 MCP Server
    // 2. 发送 DAG 创建请求
    // 3. 等待执行完成
    // 4. 验证结果
}

// 3. 安全测试示例
#[tokio::test]
async fn test_command_injection_prevented() {
    let executor = AiExecutor::new();
    let req = ExecuteRequest {
        agent: AgentType::ClaudeCode,
        prompt: "rm -rf / && echo bad".to_string(),
        work_dir: None,
    };
    let result = executor.execute(req);
    assert!(result.is_err());
}
```

---

## 8. 改进建议

### 8.1 立即修复 (严重级别, 1-2 周)

#### 1. 完善 MCP 协议实现

**优先级**: 🔴 最高

**当前状态**:
```rust
// server.rs:106-107
resources: Some(ResourcesCapability {
    subscribe: false,  // ❌ 未实现
    list_changed: false,
}),
```

**修复方案**:

```rust
// 1. 添加订阅状态管理
pub struct CisMcpServer {
    capability: Arc<CapabilityLayer>,
    prompts: Arc<PromptStore>,
    resources: Arc<ResourceManager>,
    subscribed_resources: Arc<Mutex<HashSet<String>>>,  // 新增
}

// 2. 实现订阅处理
async fn handle_resources_subscribe(
    &self,
    id: Option<Value>,
    request: &Value
) -> anyhow::Result<McpResponse> {
    let params: ResourceSubscribeParams = serde_json::from_value(
        request.get("params").cloned().unwrap()
    )?;

    let uri = params.uri;

    // 验证资源存在
    if !self.resources.exists(&uri).await {
        return Ok(McpResponse::error(
            id,
            error_codes::INVALID_PARAMS,
            format!("Resource not found: {}", uri),
        ));
    }

    // 添加订阅
    self.subscribed_resources.lock().await.insert(uri.clone());

    Ok(McpResponse::success(id, json!({ "uri": uri })))
}

async fn handle_resources_unsubscribe(
    &self,
    id: Option<Value>,
    request: &Value
) -> anyhow::Result<McpResponse> {
    let params: ResourceUnsubscribeParams = serde_json::from_value(
        request.get("params").cloned().unwrap()
    )?;

    self.subscribed_resources.lock().await.remove(&params.uri);

    Ok(McpResponse::success(id, json!({ "uri": params.uri })))
}

// 3. 资源变更通知
async fn notify_resource_change(&self, uri: &str, content: Value) {
    if self.subscribed_resources.lock().await.contains(uri) {
        // 发送通知到客户端
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/resources/updated",
            "params": {
                "uri": uri,
                "content": content
            }
        });
        // ... 发送逻辑
    }
}
```

#### 2. 添加权限控制

**优先级**: 🔴 最高

**当前状态**:
```rust
// server.rs:78-88
match method {
    "tools/call" => self.handle_tool_call(id, &request).await,
    // ❌ 无权限检查
}
```

**修复方案**:

```rust
// 1. 定义权限
#[derive(Debug, Clone, PartialEq)]
pub enum Permission {
    FileSystem { read: bool, write: bool, paths: Vec<String> },
    Network { allow_hosts: Vec<String> },
    Command { allow_list: Vec<String> },
    Process { spawn: bool },
}

// 2. 权限检查
impl CisMcpServer {
    async fn handle_tool_call_with_permission(
        &self,
        id: Option<Value>,
        request: &Value,
        caller: &CallerContext,
    ) -> anyhow::Result<McpResponse> {
        let params: ToolCallParams = serde_json::from_value(
            request.get("params").cloned().unwrap()
        )?;

        // 检查工具调用权限
        let required_perm = self.get_required_permission(&params.name)?;
        if !self.check_permission(caller, &required_perm)? {
            return Ok(McpResponse::error(
                id,
                error_codes::PERMISSION_DENIED,
                format!("Permission denied for tool: {}", params.name),
            ));
        }

        self.handle_tool_call_impl(id, &params).await
    }

    fn get_required_permission(&self, tool_name: &str) -> Result<Permission, Error> {
        match tool_name {
            "dag_create_run" => Ok(Permission::Process { spawn: true }),
            "memory_set" => Ok(Permission::FileSystem {
                read: true,
                write: true,
                paths: vec!["~/.cis/data".to_string()],
            }),
            "network_request" => Ok(Permission::Network {
                allow_hosts: vec!["api.example.com".to_string()],
            }),
            _ => Err(Error::UnknownTool),
        }
    }

    fn check_permission(
        &self,
        caller: &CallerContext,
        required: &Permission,
    ) -> Result<bool, Error> {
        match required {
            Permission::Process { spawn: true } => {
                Ok(caller.permissions.allow_process_spawn)
            }
            Permission::FileSystem { paths, .. } => {
                Ok(paths.iter().all(|p| {
                    caller.permissions.allowed_paths.iter().any(|ap| ap.starts_with(p))
                }))
            }
            _ => Ok(false),
        }
    }
}
```

#### 3. 实现 Worker 进程监控

**优先级**: 🔴 最高

**修复方案**:

```rust
// dag-executor/lib.rs
impl DagExecutorSkill {
    pub fn new(node_id: String, worker_binary: String) -> Self {
        let skill = Self {
            name: "dag-executor".to_string(),
            worker_manager: WorkerManager::new(),
            nucleus: Mutex::new(None),
            node_id,
            worker_binary,
            retry_config: RetryConfig::default(),
        };

        // 启动监控任务
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                skill.worker_manager.cleanup_inactive_workers().await;
            }
        });

        skill
    }
}

impl WorkerManager {
    pub async fn cleanup_inactive_workers(&self) {
        let mut workers = self.workers.write().await;
        let mut to_remove = Vec::new();

        for (id, worker) in workers.iter() {
            match worker.child.try_wait() {
                Ok(Some(status)) => {
                    warn!("Worker {} exited: {:?}", id, status);
                    to_remove.push(id.clone());
                }
                Ok(None) => {
                    // 仍在运行，检查活跃度
                    if worker.last_activity.elapsed() > Duration::from_secs(300) {
                        warn!("Worker {} inactive for 5 minutes", id);
                        // 发送健康检查
                        to_remove.push(id.clone());
                    }
                }
                Err(e) => {
                    error!("Error checking worker {}: {}", id, e);
                    to_remove.push(id.clone());
                }
            }
        }

        for id in to_remove {
            workers.remove(&id);
        }
    }
}
```

### 8.2 高优先级 (重要级别, 2-4 周)

#### 1. 使用 Schema 生成工具

**当前问题**:
```rust
// server.rs:120-148 - 硬编码 JSON schema
Tool {
    name: "dag_create_run".to_string(),
    description: "Create a new DAG run".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "dag_file": { "type": "string", ... },
            // ... 30+ 行
        }
    }),
}
```

**改进方案**:

```rust
// 1. 使用 schemars
use schemars::{JsonSchema, schema_for};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct DagCreateRunParams {
    #[schemars(description = "Path to DAG definition file")]
    pub dag_file: String,

    #[schemars(description = "Optional custom run ID")]
    #[serde(default)]
    pub run_id: Option<String>,

    #[schemars(description = "Execution scope")]
    #[serde(default)]
    pub scope: Option<String>,
}

// 2. 自动生成 schema
fn tool_schema<T: JsonSchema>() -> Value {
    let schema = schema_for!(T);
    serde_json::to_value(schema).unwrap()
}

Tool {
    name: "dag_create_run".to_string(),
    description: "Create a new DAG run".to_string(),
    input_schema: tool_schema::<DagCreateRunParams>(),
}
```

#### 2. 实现技能注册中心

**设计方案**:

```rust
pub struct SkillRegistry {
    skills: Arc<RwLock<HashMap<String, SkillMetadata>>>,
    repository: SkillRepository,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillMetadata {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub dependencies: Vec<String>,
    pub capabilities: Vec<String>,
    pub installed_at: DateTime<Utc>,
    pub hash: String,  // 内容哈希
}

impl SkillRegistry {
    pub async fn install(&self, spec: &SkillSpec) -> Result<(), Error> {
        // 1. 验证依赖
        self.validate_dependencies(&spec.dependencies).await?;

        // 2. 下载/复制 Skill
        let skill_path = self.repository.fetch(spec).await?;

        // 3. 验证签名
        self.verify_signature(&skill_path, &spec.signature)?;

        // 4. 注册
        let metadata = SkillMetadata {
            name: spec.name.clone(),
            version: spec.version.clone(),
            hash: self.compute_hash(&skill_path)?,
            installed_at: Utc::now(),
            ..Default::default()
        };

        self.skills.write().await.insert(spec.name.clone(), metadata);

        Ok(())
    }

    pub async fn resolve_conflicts(&self) -> Result<Vec<String>, Error> {
        // 检测版本冲突
        let mut conflicts = Vec::new();
        let skills = self.skills.read().await;

        for (name, meta) in skills.iter() {
            for dep in &meta.dependencies {
                if let Some(dep_meta) = skills.get(dep) {
                    if !self.is_compatible(meta, dep_meta) {
                        conflicts.push(format!("{} depends on {} but version incompatible",
                            name, dep));
                    }
                }
            }
        }

        Ok(conflicts)
    }
}
```

#### 3. 增加测试覆盖

**目标**: 单元测试覆盖率 > 70%, 集成测试覆盖主要流程

```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_mcp_server_full_workflow() {
    // 1. 启动服务器
    let server = start_test_server().await;

    // 2. 初始化
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {}
        }
    });
    let init_resp = server.send_request(init_req).await;
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "cis-mcp");

    // 3. 列出工具
    let list_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list"
    });
    let list_resp = server.send_request(list_req).await;
    assert!(list_resp["result"]["tools"].as_array().unwrap().len() > 0);

    // 4. 调用工具
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "ping",
            "arguments": {}
        }
    });
    let call_resp = server.send_request(call_req).await;
    assert!(call_resp.get("error").is_none());
}
```

### 8.3 中优先级 (1-2 个月)

#### 1. 添加技能版本管理

```rust
pub struct SkillVersionManager {
    versions: Arc<RwLock<HashMap<String, Vec<SkillVersion>>>>,
}

#[derive(Debug, Clone)]
pub struct SkillVersion {
    pub version: SemanticVersion,
    pub path: PathBuf,
    pub active: bool,
}

impl SkillVersionManager {
    pub async fn install_version(&self, name: &str, version: &SemanticVersion)
        -> Result<(), Error> {
        let versions = self.versions.read().await;
        let existing = versions.get(name);

        if let Some(existing_versions) = existing {
            // 检查是否已存在
            if existing_versions.iter().any(|v| &v.version == version) {
                return Err(Error::VersionAlreadyInstalled);
            }

            // 检查依赖冲突
            for dep_version in existing_versions {
                if self.has_conflicts(version, dep_version) {
                    return Err(Error::VersionConflict);
                }
            }
        }

        // 安装新版本
        // ...
    }

    pub async fn activate_version(&self, name: &str, version: &SemanticVersion)
        -> Result<(), Error> {
        let mut versions = self.versions.write().await;
        if let Some(skill_versions) = versions.get_mut(name) {
            for v in skill_versions.iter_mut() {
                v.active = &v.version == version;
            }
        }
        Ok(())
    }
}
```

#### 2. 实现技能依赖管理

```rust
pub struct DependencyResolver {
    registry: Arc<SkillRegistry>,
}

impl DependencyResolver {
    pub async fn resolve(&self, root: &SkillSpec) -> Result<Vec<SkillSpec>, Error> {
        let mut resolved = Vec::new();
        let mut queue = vec![root.clone()];

        while let Some(spec) = queue.pop() {
            // 检查循环依赖
            if resolved.iter().any(|s| s.name == spec.name) {
                continue;
            }

            // 解析依赖
            for dep_name in &spec.dependencies {
                let dep_spec = self.registry.find_latest(dep_name).await?;
                queue.push(dep_spec);
            }

            resolved.push(spec);
        }

        // 拓扑排序
        self.topological_sort(resolved)
    }

    fn topological_sort(&self, specs: Vec<SkillSpec>) -> Result<Vec<SkillSpec>, Error> {
        // Kahn's algorithm
        // ...
    }
}
```

#### 3. 添加性能监控

```rust
use prometheus::{Counter, Histogram, IntGauge, Registry};

pub struct SkillMetrics {
    executions_total: Counter,
    execution_duration: Histogram,
    active_workers: IntGauge,
}

impl SkillMetrics {
    pub fn new() -> Self {
        Self {
            executions_total: Counter::new("cis_skill_executions_total", "Total skill executions").unwrap(),
            execution_duration: Histogram::with_opts(
                HistogramOpts::new("cis_skill_execution_duration_seconds", "Skill execution duration")
                    .buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0])
            ).unwrap(),
            active_workers: IntGauge::new("cis_active_workers", "Number of active workers").unwrap(),
        }
    }

    pub fn register(&self, registry: &Registry) -> Result<(), Error> {
        registry.register(Box::new(self.executions_total.clone()))?;
        registry.register(Box::new(self.execution_duration.clone()))?;
        registry.register(Box::new(self.active_workers.clone()))?;
        Ok(())
    }
}

// 使用
impl Skill {
    async fn execute_with_metrics(&self, ctx: &SkillContext, event: Event) -> Result<()> {
        let timer = metrics.execution_duration.start_timer();
        metrics.executions_total.inc();

        let result = self.execute(ctx, event).await;

        timer.observe_duration();
        result
    }
}
```

### 8.4 长期优化 (3-6 个月)

#### 1. 技能市场

```rust
pub struct SkillMarket {
    registry_url: String,
    cache: Arc<RwLock<HashMap<String, SkillManifest>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub latest_version: String,
    pub description: String,
    pub downloads: u64,
    pub rating: f32,
    pub tags: Vec<String>,
}

impl SkillMarket {
    pub async fn search(&self, query: &str) -> Result<Vec<SkillManifest>, Error> {
        let url = format!("{}/api/v1/skills/search?q={}", self.registry_url, query);
        let response = reqwest::get(&url).await?;
        Ok(response.json().await?)
    }

    pub async fn download(&self, name: &str, version: &str) -> Result<PathBuf, Error> {
        let url = format!("{}/api/v1/skills/{}/{}", self.registry_url, name, version);
        let response = reqwest::get(&url).await?;

        let temp_dir = std::env::temp_dir();
        let skill_path = temp_dir.join(format!("{}-{}.tar.gz", name, version));

        let mut file = File::create(&skill_path)?;
        file.write_all(&response.bytes().await?)?;

        Ok(skill_path)
    }
}
```

#### 2. 技能推荐系统

```rust
pub struct SkillRecommender {
    usage_stats: Arc<RwLock<HashMap<String, UsageStats>>>,
}

#[derive(Debug, Clone)]
pub struct UsageStats {
    pub call_count: u64,
    pub last_used: DateTime<Utc>,
    pub success_rate: f32,
}

impl SkillRecommender {
    pub async fn recommend(&self, context: &TaskContext) -> Vec<String> {
        let mut scores = Vec::new();

        for (name, stats) in self.usage_stats.read().await.iter() {
            let score = self.calculate_score(context, stats);
            scores.push((name.clone(), score));
        }

        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.into_iter()
            .take(5)
            .map(|(name, _)| name)
            .collect()
    }

    fn calculate_score(&self, context: &TaskContext, stats: &UsageStats) -> f32 {
        let recency_score = self.recency_score(stats.last_used);
        let frequency_score = stats.call_count as f32 / 1000.0;
        let success_score = stats.success_rate;

        recency_score * 0.4 + frequency_score * 0.3 + success_score * 0.3
    }

    fn recency_score(&self, last_used: DateTime<Utc>) -> f32 {
        let elapsed = Utc::now().signed_duration_since(last_used);
        let days = elapsed.num_days();
        1.0 / (1.0 + days as f32 / 30.0)  // 30 天半衰期
    }
}
```

#### 3. 完善文档

```markdown
# CIS Skill 开发指南

## 快速开始

### 创建新 Skill

\`\`\`bash
cis skill create my-skill --type builtin
\`\`\`

### Skill 结构

\`\`\`
my-skill/
├── skill.toml       # Skill 配置
├── Cargo.toml       # Rust 依赖
└── src/
    └── lib.rs       # 实现
\`\`\`

### 实现示例

\`\`\`rust
use cis_core::skill::{Skill, SkillContext, Event, Result};

pub struct MySkill;

#[async_trait]
impl Skill for MySkill {
    fn name(&self) -> &str { "my-skill" }

    async fn execute(&self, ctx: &SkillContext, event: Event) -> Result<()> {
        // 处理事件
        Ok(())
    }
}
\`\`\`

## API 参考

### Skill Trait

### Event Types

### Context APIs

## 最佳实践

### 错误处理

### 日志记录

### 测试
\`\`\`

---

## 9. 总结

### 9.1 整体评分

| 维度 | 评分 | 说明 |
|------|------|------|
| **架构设计** | ⭐⭐⭐⭐☆ (4/5) | 分层清晰、协议正确、可扩展性好 |
| **代码质量** | ⭐⭐⭐☆☆ (3.5/5) | 类型安全、错误处理完善，但有硬编码 |
| **功能完整性** | ⭐⭐⭐☆☆ (3/5) | 基础功能完整，高级功能缺失 |
| **安全性** | ⭐⭐☆☆☆ (2/5) | 多处严重安全漏洞 |
| **性能** | ⭐⭐⭐☆☆ (3/5) | 异步处理好，但缺少监控 |
| **测试** | ⭐⭐☆☆☆ (2/5) | 覆盖率不足 |
| **文档** | ⭐⭐⭐☆☆ (3/5) | 有注释，缺详细文档 |

### **整体评分: ⭐⭐⭐☆☆ (3.5/5)**

### 9.2 主要优点

1. **架构设计先进**
   - MCP 协议实现正确
   - 分层清晰，职责明确
   - 可扩展性强

2. **协议实现标准**
   - 完全符合 JSON-RPC 2.0
   - MCP 规范遵循
   - 错误处理规范

3. **多协议支持**
   - Matrix 通讯
   - 多种 AI 集成
   - 进程隔离

4. **类型安全**
   - Rust 强类型
   - 枚举模式匹配
   - 编译时检查

### 9.3 主要问题

1. **MCP 协议不完整**
   - 缺少资源订阅机制
   - 缺少动态注册
   - 缺少批量操作

2. **安全机制薄弱**
   - 权限控制缺失
   - 命令注入风险
   - 文件系统无限制
   - 缺少认证

3. **生命周期管理不完整**
   - 无技能注册中心
   - 缺少版本管理
   - 依赖处理缺失

4. **测试覆盖不足**
   - 单元测试少
   - 集成测试缺失
   - 安全测试缺失

### 9.4 优先修复项

#### 立即修复 (1-2 周)

1. **实现资源订阅机制** (🔴 严重)
   - 添加 `resources/subscribe` 和 `unsubscribe`
   - 实现订阅状态管理
   - 添加变更通知

2. **添加权限控制** (🔴 严重)
   - 工具调用权限检查
   - 文件系统访问限制
   - 网络操作限制

3. **实现 Worker 进程监控** (🔴 严重)
   - 定期清理僵尸进程
   - 健康检查
   - 资源限制

4. **修复命令注入漏洞** (🔴 严重)
   - 参数验证
   - 参数分离
   - 白名单模式

#### 高优先级 (2-4 周)

5. **实现技能注册中心** (🟠 重要)
   - 动态发现
   - 安装/卸载
   - 版本管理

6. **使用 Schema 生成工具** (🟠 重要)
   - 避免硬编码
   - 自动验证
   - 文档生成

7. **增加测试覆盖** (🟠 重要)
   - 单元测试 > 70%
   - 集成测试
   - 安全测试

8. **完善错误处理** (🟠 重要)
   - 统一错误类型
   - 上下文传播
   - 用户友好消息

#### 中优先级 (1-2 个月)

9. **添加性能监控** (🟡 一般)
   - 指标收集
   - 性能追踪
   - 告警机制

10. **完善文档** (🟡 一般)
    - API 参考
    - 开发指南
    - 部署文档

---

## 10. 附录

### 10.1 文件清单

| 文件 | 行数 | 功能 | 优先级 |
|------|------|------|--------|
| `main.rs` | 54 | 入口 | 低 |
| `server.rs` | 930 | MCP 服务器 | 高 |
| `mcp_protocol.rs` | 222 | 协议定义 | 高 |
| `prompts.rs` | 412 | 提示词管理 | 中 |
| `resources.rs` | 567 | 资源管理 | 中 |
| `dag-executor/lib.rs` | 450 | DAG 执行器 | 高 |
| `matrix-register-skill/lib.rs` | 375 | Matrix 注册 | 中 |
| `im/lib.rs` | 258 | IM 集成 | 中 |
| `push-client/lib.rs` | 191 | 推送客户端 | 低 |
| `init-wizard/lib.rs` | 192 | 初始化向导 | 低 |
| `memory-organizer/lib.rs` | 181 | 记忆组织 | 中 |
| `ai-executor/lib.rs` | 83 | AI 执行器 | 高 |

### 10.2 技术债务清单

#### 高优先级技术债务

1. **WASM 沙箱** - 安全漏洞多，需要全面重构
2. **权限控制** - 运行时无限制，严重安全风险
3. **硬编码 Schema** - 维护性差，需要重构
4. **Worker 管理** - 资源泄漏风险，需要监控
5. **测试覆盖** - 覆盖率不足，质量保证缺失

#### 中优先级技术债务

1. **版本管理** - 缺少版本控制机制
2. **依赖管理** - 无依赖解析
3. **错误处理** - 不统一，需要标准化
4. **性能监控** - 缺少可观测性
5. **文档** - API 文档不完整

### 10.3 参考

- [MCP 规范](https://modelcontextprotocol.io/)
- [JSON-RPC 2.0 规范](https://www.jsonrpc.org/specification)
- [CIS 架构文档](./ARCHITECTURE.md)
- [CIS Skill 开发指南](./SKILL_DEVELOPMENT.md)

---

**报告生成**: 2026-02-15
**审阅者**: Agent adb698b
**下次审阅**: v1.2.0 或 3 个月后
