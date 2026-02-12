# CIS CLI 使用指南 - Agent 优化版

> **版本**: v1.1.6
> **适用对象**: Claude Code CLI, Claude Desktop, Claude API
> **最后更新**: 2026-02-12
> **核心原则**: CLI/GUI/远程 API 统一使用 Server API

---

## 🎯 核心架构理解

### CIS 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                   应用层 (Application Layer)              │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐       │
│  │   CLI     │  │   GUI    │  │  Web API    │       │
│  └─────┬────┘  └─────┬────┘  └──────┬───────┘       │
│        │             │              │                    │
│        └─────────────┴──────────────┘                    │
│                      ▼                                 │
├─────────────────────────────────────────────────────────────┤
│                   服务层 (Service Layer)                  │
│  ┌───────────────────────────────────────────────┐       │
│  │         CIS Server (Unified API)             │       │
│  │  - 认证/授权                               │       │
│  │  - 请求路由                                │       │
│  │  - 响应格式化                              │       │
│  └───────────────────┬───────────────────────────┘       │
│                      │                                    │
│        ┌─────────────┼─────────────┐                    │
│        ▼             ▼             ▼                    │
│  ┌─────────┐  ┌─────────┐  ┌─────────────┐          │
│  │ DAG     │  │ Memory  │  │   P2P      │          │
│  │ Service │  │ Service │  │  Service    │          │
│  └─────────┘  └─────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

### 🚨 重要架构原则

**CLI 不是基础层，只是 Server API 的一种访问方式**

| 访问方式 | 特点 | 使用场景 |
|---------|------|----------|
| **CLI** | 命令行交互，本地 Agent | 本地开发、脚本自动化 |
| **GUI** | 图形界面，用户交互 | 桌面用户、可视化操作 |
| **Web API** | HTTP/WebSocket 接口 | 远程 Agent、小程序、移动端 |

**所有三种方式都调用相同的 Server API**，确保行为一致性。

---

## 🚀 Claude Agent 执行效率优化

### 效率原则

1. **Server API 优先** - CLI/GUI 绝不直接实现业务逻辑
2. **异步非阻塞** - 所有 Server API 调用都是异步的
3. **批量操作** - 合并多个相关操作，减少往返次数
4. **智能缓存** - 利用 CIS 的缓存层，避免重复计算

### Claude 使用 CIS 的最佳流程

```rust
// ❌ 低效方式：直接实现业务逻辑
// 这样做会绕过 Server API，导致：
// - 代码重复（CLI/GUI/远程都要实现一遍）
// - 行为不一致
// - 缺少统一的错误处理和日志

use std::fs;
fn init_cis_bad() -> Result<()> {
    fs::create_dir_all("~/.cis")?;
    fs::write("~/.cis/config.toml", config)?;
    // ...
}

// ✅ 高效方式：调用 Server API
// 好处：
// - 一次实现，处处使用
// - 统一的错误处理
// - 自动记录日志
// - 支持所有访问方式

use cis_core::server::ServerApi;

async fn init_cis_good(server: Arc<dyn ServerApi>) -> Result<()> {
    let request = InitProjectRequest {
        path: PathBuf::from("~"),
        name: "default".to_string(),
        force: false,
    };

    let response = server.handle(Box::new(request)).await?;
    match response.status_code() {
        200 => Ok(()),
        _ => Err("Init failed".into()),
    }
}
```

### Agent 效率提升技巧

#### 1. 并行操作

```rust
// ❌ 串行：慢
let mem1 = memory.get("key1").await?;
let mem2 = memory.get("key2").await?;
let mem3 = memory.get("key3").await?;

// ✅ 并行：快 3 倍
use futures::join;
let (f1, f2, f3) = join!(
    memory.get("key1"),
    memory.get("key2"),
    memory.get("key3")
);
let (mem1, mem2, mem3) = (f1?, f2?, f3?);
```

#### 2. 批量 API

```rust
// ❌ 单次调用：效率低
for key in keys {
    memory.get(key).await?;  // N 次网络往返
}

// ✅ 批量调用：快 N 倍
let items = memory.get_batch(&keys).await?;  // 1 次网络往返
```

#### 3. 优先使用缓存

```rust
// CIS MemoryService 内置 LRU 缓存
// 热数据自动缓存，命中率 > 70%

// ✅ 直接使用语义搜索（利用缓存）
let results = memory.semantic_search("项目配置", 10, 0.7).await?;

// ❌ 绕过缓存，直接查数据库（不推荐）
// let results = memory.db_query("SELECT * FROM ...").await?;
```

#### 4. 事件驱动而非轮询

```rust
// ❌ 轮询：浪费 CPU 和网络
loop {
    let status = server.get_status().await?;
    if status == "completed" { break; }
    tokio::time::sleep(Duration::from_secs(1)).await;
}

// ✅ 事件驱动：高效
let mut receiver = event_bus.subscribe("task.completed").await;
while let Ok(event) = receiver.recv().await {
    // 只在有事件时才被唤醒
    break;
}
```

---

## 📖 CLI 命令快速参考

### Server API 调用模式

所有 CLI 命令都遵循统一模式：

```bash
# 基本格式
cis <command> [subcommand] [options] [arguments]

# 示例
cis memory set "user/theme" "dark"
cis project init --name "my-project"
cis agent start "default-worker"
```

### 核心命令分类

#### 1. 记忆管理

```bash
# 存储记忆
cis memory set <key> <value> [--domain <public|private>] [--category <type>]

# 获取记忆
cis memory get <key>

# 搜索记忆
cis memory search <query> [--limit <n>] [--threshold <score>]

# 语义搜索（推荐）
cis memory semantic "用户的主题偏好设置" --limit 5

# 列出键
cis memory list [--domain <public|private>]

# 删除记忆
cis memory delete <key>
```

#### 2. DAG 编排

```bash
# 执行 DAG
cis dag run <dag-name> [--project <path>]

# 查看 DAG 状态
cis dag status <execution-id>

# 查看执行日志
cis dag logs <execution-id> [--tail]

# 重试失败任务
cis dag retry <execution-id>

# 验证 DAG 定义
cis dag validate <dag-file>
```

#### 3. P2P 网络

```bash
# 查看网络状态
cis p2p status

# 查看连接的节点
cis p2p peers

# 手动连接节点
cis p2p connect <node-id>

# 触发同步
cis p2p sync [--force]

# 查看发现的节点
cis p2p discover
```

#### 4. Agent 管理

```bash
# 启动持久化 Agent
cis agent start <agent-name>

# 查看 Agent 状态
cis agent status [--all]

# 附加到 Agent（交互式）
cis agent attach <agent-name>

# 停止 Agent
cis agent stop <agent-name>

# 强制杀死 Agent
cis agent kill <agent-name>

# 查看 Agent 日志
cis agent logs <agent-name> [--tail]
```

#### 5. 项目管理

```bash
# 初始化项目
cis project init [--name <name>] [--path <path>]

# 验证项目
cis project validate [--path <path>]

# 查看项目信息
cis project info [--path <path>]

# 设置项目配置
cis project config set <key> <value>

# 获取项目配置
cis project config get <key>
```

---

## 🔧 CLI 开发指南

### CLI Handler 模板

所有 CLI handler 都应该遵循以下模式：

```rust
use cis_core::server::ServerApi;
use std::sync::Arc;

/// CLI 上下文（包含 Server API 引用）
pub struct CliContext {
    pub server: Arc<dyn ServerApi>,
    pub config: CliConfig,
}

/// 命令处理函数（异步）
pub async fn handle_command(ctx: &CliContext, args: CommandArgs) -> Result<()> {
    // 1. 构建请求对象
    let request = MyRequest {
        param1: args.value1,
        param2: args.value2,
    };

    // 2. 调用 Server API（关键！）
    let response = ctx.server
        .handle(Box::new(request))
        .await?;

    // 3. 处理响应
    match response.status_code() {
        200 => {
            println!("✓ Success");
            if let Some(data) = response.data() {
                println!("{:?}", data);
            }
        }
        404 => println!("✗ Not found"),
        500 => println!("✗ Internal error"),
        _ => println!("✗ Unknown error"),
    }

    Ok(())
}
```

### ❌ 常见错误模式

#### 错误 1：直接实现业务逻辑

```rust
// ❌ 错误：CLI handler 直接操作文件系统
pub async fn handle_init(ctx: &CliContext, args: InitArgs) -> Result<()> {
    use std::fs;
    fs::create_dir_all(&cis_dir)?;
    fs::write(&config_file, config)?;
    // 问题：
    // - 绕过了 Server API
    // - GUI/远程 API 无法复用此逻辑
    // - 错误处理不一致
}
```

#### 错误 2：重复实现

```rust
// ❌ 错误：CLI/GUI/远程 API 各实现一遍

// cis-node/src/cli/handlers/memory.rs
pub async fn handle_set_memory_cli(...) {
    // 完整的验证逻辑
    // 完整的错误处理
    // 完整的日志记录
}

// cis-gui/src/memory.rs
pub async fn handle_set_memory_gui(...) {
    // 相同的验证逻辑（重复！）
    // 相同的错误处理（重复！）
    // 相同的日志记录（重复！）
}

// ✅ 正确：调用统一的 Server API
pub async fn handle_set_memory(...) {
    let request = SetMemoryRequest { key, value };
    ctx.server.handle(Box::new(request)).await?;
}
```

### ✅ 正确模式

```rust
// ✅ 正确：所有客户端都调用 Server API

// cis-core/src/server/handlers/memory.rs
// 一次实现，所有客户端共享

pub async fn handle_set_memory_request(
    request: SetMemoryRequest,
    context: &mut ServerContext,
) -> Result<SetMemoryResponse> {
    // 完整的验证
    // 完整的错误处理
    // 完整的日志记录
    // 返回统一格式的响应
}

// cis-node/src/cli/handlers/memory.rs
pub async fn handle_set_memory_cli(ctx: &CliContext, args: SetArgs) -> Result<()> {
    let request = SetMemoryRequest::from_args(args);
    let response = ctx.server.handle(Box::new(request)).await?;
    println_result(response);
    Ok(())
}

// cis-gui/src/memory.rs
pub async fn handle_set_memory_gui(ctx: &GuiContext, args: SetArgs) -> Result<()> {
    let request = SetMemoryRequest::from_args(args);
    let response = ctx.server.handle(Box::new(request)).await?;
    update_ui(response);
    Ok(())
}

// crates/cis-mcp-adapter/src/memory.rs
pub async fn handle_set_memory_mcp(ctx: &McpContext, args: SetArgs) -> Result<()> {
    let request = SetMemoryRequest::from_args(args);
    let response = ctx.server.handle(Box::new(request)).await?;
    format_mcp_response(response);
    Ok(())
}
```

---

## 🎯 Claude Agent 执行场景

### 场景 1: 识别 CIS 能力

| 用户输入 | Claude 应该 | Server API |
|---------|------------|-------------|
| "记住这个偏好" | 存储记忆 | `server.handle(SetMemoryRequest)` |
| "查找之前的配置" | 搜索记忆 | `server.handle(SearchMemoryRequest)` |
| "执行这个 workflow" | 执行 DAG | `server.handle(ExecuteDagRequest)` |
| "与其他设备同步" | P2P 同步 | `server.handle(SyncP2pRequest)` |
| "接入我的项目" | 初始化项目 | `server.handle(InitProjectRequest)` |

### 场景 2: 项目级操作

**用户**: "把当前项目接入 CIS"

**Claude 的处理流程**：

```rust
// 1. 检测是否在项目中
let project_dir = std::env::current_dir()?;
let is_in_project = project_dir.join(".cis/project.toml").exists();

if is_in_project {
    println!("当前已在 CIS 项目中");
    return Ok(());
}

// 2. 调用 Server API 初始化项目
let request = InitProjectRequest {
    path: project_dir.clone(),
    name: project_name.to_string(),
    force: false,
};

let response = ctx.server
    .handle(Box::new(request))
    .await?;

// 3. 处理响应
match response.status_code() {
    200 => {
        println!("✓ 项目已初始化");
        println!("配置文件: .cis/project.toml");
        println!("记忆命名空间: project/{}", project_name);
    }
    409 => {
        println!("✗ 项目已存在，使用 --force 覆盖");
    }
    _ => {
        println!("✗ 初始化失败");
    }
}
```

### 场景 3: 智能记忆操作

**用户**: "记住这个项目的数据库配置"

**Claude 的处理流程**：

```rust
// 1. 识别项目上下文
let project = ProjectManager::find_project(std::env::current_dir()?)
    .ok_or("Not in a project")?;

// 2. 构建记忆键（使用项目命名空间）
let key = format!("project/{}/database/config", project.config.name);

// 3. 存储记忆（带语义索引）
let request = SetMemoryRequest {
    key: key.clone(),
    value: serde_json::to_vec(&db_config)?,
    domain: MemoryDomain::Public,
    category: MemoryCategory::Context,
    enable_embedding: true,  // 启用语义索引
};

let response = ctx.server.handle(Box::new(request)).await?;

// 4. 添加到项目共享键（可选）
let update_config_request = UpdateProjectConfigRequest {
    project_path: project.path,
    update: ProjectConfigUpdate::AddSharedKey(key.clone()),
};

ctx.server.handle(Box::new(update_config_request)).await?;
```

### 场景 4: 高效 DAG 执行

**用户**: "运行项目的测试和部署 workflow"

**Claude 的处理流程**：

```rust
// 1. 检测项目
let project = ProjectManager::find_project(std::env::current_dir()?)?;

// 2. 查找项目级 DAG
let dag_path = project.path.join(".cis/dags/test-deploy.toml");
if !dag_path.exists() {
    println!("✗ DAG 文件不存在: {}", dag_path.display());
    return Ok(());
}

// 3. 加载并验证 DAG
let load_request = LoadDagRequest {
    file_path: dag_path.clone(),
    validate: true,
};

let load_response = ctx.server.handle(Box::new(load_request)).await?;
let dag = load_response.dag.ok_or("Failed to load DAG")?;

// 4. 执行 DAG（使用项目命名空间）
let execute_request = ExecuteDagRequest {
    dag,
    project_namespace: Some(project.config.name),
    timeout: Some(1800),  // 30 分钟
};

let execution_response = ctx.server.handle(Box::new(execute_request)).await?;

// 5. 监听执行事件（事件驱动）
let event_bus = ctx.event_bus();
let mut receiver = event_bus.subscribe("dag.task.completed").await;

println!("🚀 开始执行 DAG...");

let mut completed_tasks = 0;
let total_tasks = dag.tasks.len();

while let Ok(event) = receiver.recv().await {
    if let Some(task_event) = event.downcast_ref::<TaskCompletedEvent>() {
        completed_tasks += 1;
        println!("✓ [{}/{}] {}", completed_tasks, total_tasks, task_event.task_name);

        if completed_tasks == total_tasks {
            println!("🎉 DAG 执行完成");
            break;
        }
    }
}
```

---

## 🚨 常见问题和解决方案

### 问题 1: CLI 命令不响应

**症状**: 执行 `cis xxx` 后无响应

**原因**: Server API 未启动或连接失败

**解决**:
```bash
# 检查 Server 状态
cis server status

# 查看日志
tail -f ~/.cis/logs/server.log
```

### 问题 2: Agent 执行缓慢

**症状**: Claude Agent 响应很慢

**原因**:
1. 未使用缓存
2. 串行操作
3. 轮询而非事件驱动

**解决**:
```rust
// ✅ 使用批量 API
let items = memory.get_batch(&keys).await?;

// ✅ 使用并行操作
let (r1, r2, r3) = join!(op1(), op2(), op3());

// ✅ 使用事件驱动
let mut receiver = event_bus.subscribe("event.type").await;
while let Ok(event) = receiver.recv().await {
    // 处理事件
}
```

### 问题 3: 项目配置未生效

**症状**: 修改 `.cis/project.toml` 后无变化

**原因**: Server API 缓存了配置

**解决**:
```bash
# 重启 Server（清除缓存）
cis server restart

# 或强制重新加载配置
cis project reload --path .
```

---

## 📚 完整 API 参考

### 请求/响应模式

所有 Server API 都遵循统一模式：

```rust
/// 请求 trait
pub trait Request: Send + Sync {
    fn request_type(&self) -> &'static str;

    fn validate(&self) -> Result<()> {
        Ok(())  // 默认实现
    }
}

/// 响应 trait
pub trait Response: Send + Sync {
    fn status_code(&self) -> u16;

    fn data(&self) -> Option<&serde_json::Value> {
        None
    }
}
```

### 核心 Server API 列表

| API 端点 | 请求类型 | 响应类型 | 说明 |
|-----------|---------|-----------|------|
| `/memory/set` | SetMemoryRequest | SetMemoryResponse | 存储记忆 |
| `/memory/get` | GetMemoryRequest | GetMemoryResponse | 获取记忆 |
| `/memory/search` | SearchMemoryRequest | SearchMemoryResponse | 搜索记忆 |
| `/dag/execute` | ExecuteDagRequest | ExecuteDagResponse | 执行 DAG |
| `/dag/status` | DagStatusRequest | DagStatusResponse | DAG 状态 |
| `/agent/start` | StartAgentRequest | StartAgentResponse | 启动 Agent |
| `/agent/execute` | ExecuteTaskRequest | ExecuteTaskResponse | 执行任务 |
| `/project/init` | InitProjectRequest | InitProjectResponse | 初始化项目 |
| `/p2p/connect` | ConnectPeerRequest | ConnectPeerResponse | 连接节点 |
| `/p2p/sync` | SyncP2pRequest | SyncP2pResponse | P2P 同步 |

---

## 🎓 最佳实践总结

### Claude 使用 CIS 的黄金法则

1. **Server API 优先**
   - CLI/GUI/远程 API 都调用相同的 Server 接口
   - 绝不在客户端实现业务逻辑

2. **异步非阻塞**
   - 所有 Server API 调用都是异步的
   - 使用 `join!` 并行执行独立操作

3. **利用缓存**
   - 优先使用 `semantic_search`（内置缓存）
   - 使用批量 API 减少往返

4. **事件驱动**
   - 订阅事件而非轮询状态
   - 使用 `event_bus` 监听变化

5. **错误处理**
   - 所有 API 调用都要检查 `status_code`
   - 提供清晰的错误信息给用户

6. **项目上下文**
   - 自动检测项目目录
   - 使用项目命名空间隔离记忆

---

**文档版本**: 1.1.6
**优化日期**: 2026-02-12
**维护者**: CIS Team

---

## 相关文档

- [CIS 架构设计](../ARCHITECTURE.md)
- [Server API 文档](../api/SERVER_API.md)
- [代码复用结构设计](../plan/v1.1.6/CODE_REUSE_STRUCTURE_DESIGN.md)
- [原有模块拆分分析](../plan/v1.1.6/MONOLITHIC_MODULES_ANALYSIS.md)
