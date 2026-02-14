# CIS (Cluster of Independent Systems) - Claude 使用指南

> **版本**: v1.1.5  
> **适用对象**: Claude Code CLI, Claude Desktop, Claude API  
> **最后更新**: 2026-02-11

---

## 快速开始

当你作为 Claude 被用户使用 CIS 时，请遵循以下引导：

```
┌─────────────────────────────────────────────────────────────┐
│  用户任务 → 识别需求 → 选择 CIS 能力 → 执行 → 记忆归档       │
└─────────────────────────────────────────────────────────────┘
```

### 1. 识别 CIS 能力需求

| 用户场景 | CIS 能力 | 调用方式 |
|---------|---------|---------|
| "记住这个偏好" / "以后提醒我" | **记忆存储** | `memory.set` |
| "查找之前的配置" / "我设置过什么" | **记忆搜索** | `memory.search` |
| "执行这个 workflow" / "按步骤执行" | **DAG 编排** | `dag.execute` |
| "与其他设备同步" / "分享给团队" | **P2P 网络** | `p2p.sync` |
| "用 Element 登录" / "Matrix 消息" | **联邦网关** | `matrix.*` |
| "接入我的项目" / "管理这个项目" | **项目接入** | `project.init` |
| "保持 Agent 运行" / "后台 Agent" | **持久化 Agent** | `agent.persist` |

---

## 客户项目接入指南

### 什么是 CIS 项目接入

CIS 项目接入允许将 CIS 的能力集成到客户项目中，提供：
- **项目级配置**: 项目特定的 AI 配置、记忆命名空间
- **本地 Skill**: 项目专属的 Skill 管理
- **双向绑定**: CIS ↔ Agent 的无缝集成
- **记忆隔离**: 项目记忆与全局记忆分离

### 项目目录结构

```
客户项目/
├── .cis/
│   ├── project.toml          # 项目配置
│   ├── skills/               # 本地 Skill 目录
│   │   ├── my-linter/
│   │   └── custom-deploy/
│   └── data/                 # 项目级数据
├── src/
└── ...
```

### 项目配置模板 (project.toml)

```toml
[project]
name = "my-awesome-project"
id = "proj-abc-123"

[ai]
# 项目级 AI 配置
guide = """
You are working on my-awesome-project.
Tech stack: Rust + React + PostgreSQL
Coding standards: Follow the project's CONVENTIONS.md
"""
provider = "claude"
model = "claude-3-sonnet"

# 本地 Skills
[[skills]]
name = "custom-linter"
path = "./skills/custom-linter"
auto_load = true  # 项目启动时自动加载

[[skills]]
name = "project-deploy"
path = "./skills/project-deploy"
auto_load = false  # 按需手动加载

[memory]
# 记忆命名空间 (默认: project/{project-name})
namespace = "project/my-awesome-project"
# 共享记忆键 (跨项目访问)
shared_keys = ["conventions", "architecture", "api-contracts"]
```

### Claude 如何帮助客户接入项目

#### 场景 1: 新项目接入 CIS

**用户**: "帮我将这个项目接入 CIS"

**Claude 应该**:
```bash
# 1. 进入项目目录
cd /path/to/user-project

# 2. 初始化 CIS 项目配置
cis project init --name "user-project"

# 3. 或手动创建配置文件
mkdir -p .cis/skills
cat > .cis/project.toml << 'EOF'
[project]
name = "user-project"
id = "proj-$(uuidgen | head -c 8)"

[ai]
provider = "claude"
guide = """
You are working on the user-project.
Please follow the project's coding conventions and architecture decisions.
Check CIS memory for project context before making changes.
"""

[memory]
namespace = "project/user-project"
shared_keys = ["conventions", "architecture"]
EOF
```

**回复**:
"已为你初始化 CIS 项目配置：
- 配置文件: `.cis/project.toml`
- 记忆命名空间: `project/user-project`
- 本地 Skill 目录: `.cis/skills/`

现在你可以：
1. 创建本地 Skill: `mkdir .cis/skills/my-skill`
2. 存储项目记忆: `cis memory set project/user-project/conventions "..."`
3. 运行项目级 DAG: `cis dag run --project .`
"

#### 场景 2: 使用项目级记忆

**用户**: "记住这个项目的架构决策"

**Claude 应该**:
```rust
// 1. 检测当前是否在项目中
let project = ProjectManager::find_project(std::env::current_dir()?);

// 2. 使用项目命名空间存储记忆
match project {
    Some(proj) => {
        let key = proj.memory_key("architecture/microservices");
        service.set_with_embedding(
            &key,
            b"采用微服务架构，服务间通过 gRPC 通信...",
            MemoryDomain::Public,
            MemoryCategory::Context
        ).await?;
        
        // 同时添加到共享键，方便其他项目引用
        proj.config.memory.shared_keys.push("architecture/microservices".to_string());
        proj.save()?;
    }
    None => {
        // 不在项目中，使用全局记忆
        service.set_with_embedding(
            "global/architecture/microservices",
            b"...",
            MemoryDomain::Public,
            MemoryCategory::Context
        ).await?;
    }
}
```

#### 场景 3: 项目级 Skill 自动加载

**用户**: "每次进入项目时自动加载自定义 linter"

**Claude 应该**:
```toml
# 编辑 .cis/project.toml
[[skills]]
name = "custom-linter"
path = "./skills/custom-linter"
auto_load = true  # ← 启用自动加载
```

```bash
# 创建本地 Skill 目录结构
mkdir -p .cis/skills/custom-linter
cat > .cis/skills/custom-linter/skill.toml << 'EOF'
[skill]
name = "custom-linter"
version = "1.0.0"
type = "native"
description = "项目特定的代码检查规则"

[permissions]
filesystem = true
command = true
EOF

# Skill 可执行文件
cat > .cis/skills/custom-linter/run.sh << 'EOF'
#!/bin/bash
# 自定义检查逻辑
echo "Running project-specific linting..."
EOF
chmod +x .cis/skills/custom-linter/run.sh
```

### 项目会话启动流程

```rust
use cis_core::project::{ProjectManager, ProjectSession};

// 1. 查找并加载项目
let mut manager = ProjectManager::new();
if let Some(project) = ProjectManager::find_project(&current_dir) {
    // 2. 创建项目会话
    let session = ProjectSession::new(project);
    
    // 3. 启动会话 (自动完成以下操作)
    session.start().await?;
    // - 加载项目配置
    // - 设置记忆命名空间
    // - 加载 auto_load = true 的 Skills
    // - 建立 Agent 双向绑定
    
    // 4. 获取 AI 引导上下文
    let ai_guide = session.project.build_ai_guide();
    // 包含项目信息、共享记忆、可用 Skills
}
```

---

## 自启动 Agent 指南

### 什么是持久化 Agent

CIS 支持持久化运行的 AI Agent，特点：
- **前后台切换**: 随时 attach/detach
- **自动重启**: 崩溃后自动恢复 (配置 `auto_restart`)
- **状态保持**: 长期保持运行状态
- **多 Runtime**: 支持 Claude / OpenCode / Kimi / Aider

### Agent 生命周期

```
┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│  创建    │───▶│  运行    │───▶│  忙碌    │───▶│  空闲    │
│ Created │    │ Running │    │  Busy   │    │  Idle   │
└─────────┘    └─────────┘    └─────────┘    └────┬────┘
     ▲                                             │
     └─────────────────────────────────────────────┘
                    │ 错误/关闭
                    ▼
              ┌─────────┐
              │  关闭    │
              │Shutdown │
              └─────────┘
```

### 配置自启动 Agent

#### 方式 1: 配置文件

```toml
# ~/.cis/config.toml

[agent]
# 默认 Agent Provider
default_agent = "claude"

# 持久化 Agent 配置
[persistent_agent]
# 是否启用自启动
auto_start = true

# 自动启动的 Agents
[[persistent_agent.agents]]
name = "default-worker"
runtime = "claude"
work_dir = "~"
auto_restart = true
max_concurrent_tasks = 3
default_timeout_secs = 300

# 系统提示词
system_prompt = """
You are a persistent CIS Agent.
Always check project memory before making decisions.
Use available skills when appropriate.
"""

[[persistent_agent.agents]]
name = "project-helper"
runtime = "opencode"
work_dir = "~/projects"
auto_restart = true
model = "opencode/glm-4.7-free"
```

#### 方式 2: 编程方式

```rust
use cis_core::agent::persistent::{
    AgentConfig, RuntimeType, 
    ClaudePersistentAgent, ClaudeRuntime
};

// 1. 创建 Agent 配置
let config = AgentConfig::new(
    "my-persistent-agent",
    PathBuf::from("~/workspace")
)
.with_runtime(RuntimeType::Claude)
.with_model("claude-3-sonnet")
.with_system_prompt("You are a helpful coding assistant.")
.with_max_concurrent(2)
.with_timeout(600)
.with_auto_restart();  // ← 启用自动重启

// 2. 创建 Runtime
let runtime = ClaudeRuntime::new().await?;

// 3. 创建 Agent
let agent = runtime.create_agent(config).await?;

// 4. 启动 Agent (后台运行)
agent.start().await?;

// 5. 后续操作...
// 查看状态
let status = agent.status().await;
println!("Agent status: {}", status);

// 分配任务
let task = TaskRequest::new("task-1", "Review this code: ...")
    .with_timeout(300);
let result = agent.execute(task).await?;

// 前台交互 (attach)
agent.attach().await?;  // 进入交互式模式，直到用户按 Ctrl+D detach

// 优雅关闭
agent.shutdown().await?;
```

### Agent Pool 管理

```rust
use cis_core::agent::persistent::pool::{AgentPool, PoolConfig};

// 1. 配置 Pool
let pool_config = PoolConfig {
    min_agents: 2,      // 最少保持 2 个 Agent
    max_agents: 10,     // 最多 10 个 Agent
    idle_timeout_secs: 300,  // 空闲 5 分钟后回收
    auto_scale: true,   // 自动扩缩容
};

// 2. 创建 Pool
let pool = AgentPool::new(pool_config);

// 3. 获取 Agent (自动分配或创建)
let agent = pool.acquire(AgentAcquireConfig {
    runtime_type: RuntimeType::Claude,
    timeout_secs: 60,
}).await?;

// 4. 使用 Agent
let result = agent.execute(task).await?;

// 5. 归还 Agent (自动放回 Pool)
drop(agent);
```

### 四级决策与 Agent 集成

```rust
// DAG 任务中的 Agent 调用
use cis_core::types::TaskLevel;

async fn execute_with_agent(level: TaskLevel, prompt: &str) -> Result<()> {
    match level {
        TaskLevel::Mechanical { retry } => {
            // 机械级：Agent 自动执行，失败重试
            for attempt in 0..retry {
                match agent.execute(prompt).await {
                    Ok(result) => return Ok(result),
                    Err(e) if attempt < retry - 1 => {
                        tracing::warn!("Attempt {} failed, retrying...", attempt + 1);
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    Err(e) => return Err(e),
                }
            }
        }
        
        TaskLevel::Recommended { timeout, default_action } => {
            // 推荐级：Agent 执行，用户可撤销
            println!("Recommended action: {}", prompt);
            println!("Will execute in 5 seconds (Ctrl+C to cancel)...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            agent.execute(prompt).await?;
        }
        
        TaskLevel::Confirmed => {
            // 确认级：需要人工确认
            println!("Please confirm: {}", prompt);
            print!("Execute? [y/N] ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            
            if input.trim().to_lowercase() == "y" {
                agent.execute(prompt).await?;
            } else {
                return Err("User cancelled".into());
            }
        }
        
        TaskLevel::Arbitrated { stakeholders } => {
            // 仲裁级：多方投票
            // ... 投票逻辑
        }
    }
    
    Ok(())
}
```

---

## 记忆系统使用指南

### 核心概念

CIS 记忆分为 **私域 (Private)** 和 **公域 (Public)**：

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS 记忆架构                              │
├────────────────────────────┬────────────────────────────────┤
│        私域记忆             │          公域记忆               │
│        (Private)           │          (Public)              │
├────────────────────────────┼────────────────────────────────┤
│ • 本地加密存储              │ • 明文存储                      │
│ • 永不同步                  │ • 可 P2P 同步                   │
│ • 敏感信息                  │ • 共享配置                      │
│ • API Keys, 个人偏好        │ • 项目设置, 团队约定             │
└────────────────────────────┴────────────────────────────────┘
```

### Claude 应该何时使用记忆

**✅ 应该存储到 CIS 记忆：**
- 用户明确说"记住"、"保存"、"记下来"
- 用户的偏好设置 (主题、语言、默认行为)
- 项目特定的配置约定
- 需要跨会话保持的上下文

**❌ 不应该存储：**
- 临时计算结果
- 敏感凭证 (使用系统 keychain 或私域记忆)
- 一次性查询结果

### 记忆操作模板

#### 存储记忆

```rust
// 当用户说："记住我喜欢深色主题"
// 你应该调用 CIS 记忆 API:

use cis_core::memory::{MemoryService, MemoryDomain, MemoryCategory};

// 存储到公域 (可同步给其他设备)
service.set(
    "user/preference/theme", 
    b"dark", 
    MemoryDomain::Public,      // 或 Private
    MemoryCategory::Context
).await?;

// 语义索引 (支持自然语言搜索)
service.set_with_embedding(
    "user/preference/theme",
    b"用户偏好使用深色主题界面",
    MemoryDomain::Public,
    MemoryCategory::Context
).await?;
```

#### 搜索记忆

```rust
// 当用户说："我之前设置过什么主题？"
// 使用语义搜索:

let results = service.semantic_search(
    "用户主题偏好设置",  // 自然语言查询
    5,                    // 返回数量
    0.7                   // 相似度阈值
).await?;

// 或精确查找
if let Some(item) = service.get("user/preference/theme").await? {
    println!("主题: {:?}", String::from_utf8_lossy(&item.value));
}
```

### 记忆键命名规范

```
{domain}/{category}/{identifier}

示例：
• user/preference/language          → 用户语言偏好
• project/{id}/database-config      → 项目数据库配置  
• workflow/{name}/last-run          → 工作流上次运行
• device/{hostname}/settings        → 设备特定设置
```

---

## DAG 编排使用指南

### 什么是 DAG

DAG (有向无环图) 用于编排多步骤任务，支持：
- **依赖管理**: 任务按依赖顺序执行
- **并行执行**: 无依赖的任务并行运行
- **故障恢复**: 支持重试和回滚
- **多级决策**: Mechanical → Recommended → Confirmed → Arbitrated

### Claude 应该何时使用 DAG

**✅ 适合使用 DAG 的场景：**
- 多步骤 workflow (代码审查 → 测试 → 部署)
- 需要按顺序执行的任务链
- 可以并行化的独立子任务
- 需要持久化和追踪的复杂操作

**❌ 不适合 DAG 的场景：**
- 简单的单步命令
- 需要实时交互的操作
- 一次性临时任务

### DAG 定义模板

```toml
# 保存为: .cis/dags/my-workflow.toml

[skill]
name = "code-review-and-deploy"
version = "1.0.0"
type = "dag"
description = "代码审查并部署"

[dag]
policy = "all_success"  # 所有任务必须成功

[[dag.tasks]]
id = "1"
name = "获取代码变更"
skill = "git-diff"
level = { type = "mechanical", retry = 3 }  # 机械级，自动重试

[[dag.tasks]]
id = "2"
name = "AI 代码审查"
skill = "ai-code-review"
deps = ["1"]  # 依赖任务 1
level = { type = "confirmed" }  # 确认级，需要人工确认
agent = "claude"  # 指定使用 Claude

[[dag.tasks]]
id = "3"
name = "运行测试"
skill = "cargo-test"
deps = ["2"]
level = { type = "mechanical", retry = 2 }

[[dag.tasks]]
id = "4"
name = "部署"
skill = "deploy"
deps = ["3"]
level = { type = "recommended", timeout = 300, default_action = "execute" }
```

### 四级决策级别

| 级别 | 适用场景 | 行为 | 示例 |
|-----|---------|------|------|
| **Mechanical** | 低风险、可自动化 | 自动重试，无需确认 | 代码格式化、静态检查 |
| **Recommended** | 中等风险、有默认 | 执行但通知，可撤销 | 测试运行、文档生成 |
| **Confirmed** | 高风险、需批准 | 等待人工确认 | 代码提交、配置变更 |
| **Arbitrated** | 关键决策、多方参与 | 需要多方投票 | 架构变更、发布决策 |

### 执行 DAG

```bash
# 通过 CLI 执行
cis skill run code-review-and-deploy

# 或在 Rust 代码中:
use cis_core::scheduler::{DagScheduler, TaskDag};

let dag = TaskDag::from_file(".cis/dags/my-workflow.toml").await?;
let scheduler = DagScheduler::new();
let result = scheduler.execute(dag).await?;
```

---

## 网络依赖使用指南

### P2P 组网模式

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS 组网模式                             │
├──────────────┬──────────────┬──────────────┬────────────────┤
│   单机模式    │  局域网 mDNS  │   P2P 公网    │    混合模式     │
├──────────────┼──────────────┼──────────────┼────────────────┤
│ 个人使用      │ 团队内网      │ 分布式团队    │ 企业部署        │
│ 无网络依赖    │ 自动发现      │ NAT穿透       │ 云端+本地       │
│ ✅ 可用       │ ✅ 可用       │ ✅ 可用       │ ⚠️ 部分可用     │
└──────────────┴──────────────┴──────────────┴────────────────┘
```

### Claude 应该何时使用网络功能

**✅ 使用 P2P 网络：**
- 用户有多个 CIS 节点需要同步
- 需要与团队成员共享记忆/配置
- 跨设备状态同步

**❌ 不使用网络：**
- 单机使用场景
- 敏感数据 (使用私域记忆)

### 网络配置模板

```toml
# ~/.cis/config.toml

[p2p]
enabled = true
listen_port = 7677
bootstrap_nodes = [
    "/dns/cis-bootstrap.example.com/tcp/7677/p2p/12D3KooW..."
]

[p2p.discovery]
enable_mdns = true        # 局域网自动发现
enable_dht = true         # DHT 公网发现

[p2p.sync]
auto_sync = true          # 自动同步公域记忆
sync_interval_secs = 300  # 每5分钟同步
```

### 公域记忆同步

```rust
// 当用户说："把这个配置同步到其他设备"
// 公域记忆会自动同步，无需手动操作

// 手动触发同步
let pending = memory_service.get_pending_sync(100).await?;
for marker in pending {
    p2p.sync_memory(&marker.key, &marker.sync_peers).await?;
    memory_service.mark_synced(&marker.key).await?;
}
```

---

## 完整对话示例

### 示例 1: 记忆用户偏好

**用户**: "记住我喜欢用深色主题"

**Claude 应该**:
```rust
// 1. 识别意图: 存储用户偏好到 CIS 记忆
// 2. 选择域: Public (跨设备同步)
// 3. 执行存储

service.set_with_embedding(
    "user/preference/theme",
    b"dark",
    MemoryDomain::Public,
    MemoryCategory::Context
).await?;

// 4. 确认
"好的，我已记住你偏好深色主题。这个设置会在你所有 CIS 设备间同步。"
```

### 示例 2: 创建代码审查 DAG

**用户**: "帮我创建一个代码审查 workflow"

**Claude 应该**:
```toml
// 1. 创建 DAG 配置文件
// 2. 保存到 .cis/dags/code-review.toml
// 3. 使用四级决策: Mechanical → Confirmed
```

**回复**:
"已为你创建代码审查 workflow，包含以下步骤：
1. 获取代码变更 (Mechanical - 自动执行)
2. AI 代码审查 (Confirmed - 需要确认)
3. 运行测试 (Mechanical - 自动执行)

运行: `cis skill run code-review`
"

### 示例 3: 同步配置到团队

**用户**: "把这个项目配置分享给团队"

**Claude 应该**:
```rust
// 1. 确认是公域记忆 (会自动同步)
// 2. 如果当前是私域，询问是否转换
// 3. 手动触发同步

service.set(
    "project/xyz/config",
    config_json,
    MemoryDomain::Public,  // 公域 = 可同步
    MemoryCategory::Context
).await?;

// 4. 触发 P2P 同步
p2p.broadcast_public_memory().await?;
```

**回复**:
"已将项目配置保存到公域记忆，正在同步到已连接的团队节点。团队成员在下次同步后将能看到此配置。"

### 示例 4: 接入新项目

**用户**: "我想把当前项目接入 CIS 管理"

**Claude 应该**:
```bash
# 1. 检查当前目录
if [ ! -f ".cis/project.toml" ]; then
    # 2. 初始化项目配置
    cis project init
    
    # 3. 询问项目信息并配置
    echo "已初始化 CIS 项目配置"
fi

# 4. 建议创建项目记忆
echo "建议创建以下项目记忆："
echo "- 技术栈约定"
echo "- 架构决策记录"
echo "- API 契约"
echo "- 部署配置"
```

**回复**:
"已为你初始化 CIS 项目配置：
- `.cis/project.toml` - 项目配置
- `.cis/skills/` - 本地 Skill 目录

建议下一步：
1. 存储项目技术栈: `cis memory set project/{name}/tech-stack "..."`
2. 创建项目级 DAG: `mkdir .cis/dags/`
3. 配置自启动 Agent: 编辑 `.cis/project.toml` 添加 `[persistent_agent]`
"

### 示例 5: 配置自启动 Agent

**用户**: "我希望有一个一直在后台运行的 Agent"

**Claude 应该**:
```toml
# 编辑 ~/.cis/config.toml

[persistent_agent]
auto_start = true

[[persistent_agent.agents]]
name = "my-background-agent"
runtime = "claude"
work_dir = "~/projects"
auto_restart = true
system_prompt = """
You are a persistent CIS Agent always running in background.
When assigned tasks, execute them efficiently.
Report back results and any issues encountered.
"""
```

```bash
# 启动 Agent
cis agent start my-background-agent

# 查看状态
cis agent status

# 附加到 Agent 进行交互
cis agent attach my-background-agent

# 分离回后台 (在 Agent 中按 Ctrl+D)
```

**回复**:
"已配置自启动 Agent `my-background-agent`：
- Runtime: Claude
- 自动重启: 启用
- 工作目录: ~/projects

命令：
- 启动: `cis agent start my-background-agent`
- 查看状态: `cis agent status`
- 交互式连接: `cis agent attach my-background-agent`
- 停止: `cis agent stop my-background-agent`
"

---

## API 快速参考

### 记忆 API

```rust
// 存储
service.set(key, value, domain, category).await?;
service.set_with_embedding(key, value, domain, category).await?;

// 读取
let item = service.get(key).await?;
let items = service.search(query, options).await?;
let results = service.semantic_search(query, limit, threshold).await?;

// 删除
service.delete(key).await?;

// 列出
let keys = service.list_keys(Some(domain)).await?;
```

### DAG API

```rust
// 从文件加载
let dag = TaskDag::from_file("path/to/dag.toml").await?;

// 编程式构建
let mut dag = TaskDag::new();
dag.add_node(id, deps, level, rollback)?;
dag.validate()?;

// 执行
let scheduler = DagScheduler::new();
let result = scheduler.execute(dag).await?;
```

### P2P API

```rust
// 网络管理
let p2p = P2PNetwork::new(node_id, did, bind_addr).await?;
p2p.start().await?;

// 发现节点
let peers = p2p.get_connected_peers().await;

// 广播
p2p.broadcast(topic, data).await?;

// 同步记忆
p2p.sync_memory(key, peers).await?;
```

### 项目 API

```rust
// 查找项目
let project = ProjectManager::find_project(&current_dir);

// 初始化项目
let project = Project::init(&dir, "project-name")?;

// 创建会话
let session = ProjectSession::new(project);
session.start().await?;

// 获取 AI 引导
let guide = session.project.build_ai_guide();
```

### Agent API

```rust
// 创建配置
let config = AgentConfig::new("agent-name", work_dir)
    .with_auto_restart()
    .with_timeout(600);

// 创建 Agent
let runtime = ClaudeRuntime::new().await?;
let agent = runtime.create_agent(config).await?;

// 启动
agent.start().await?;

// 执行任务
let result = agent.execute(task).await?;

// 前台交互
agent.attach().await?;

// 优雅关闭
agent.shutdown().await?;
```

---

## 故障排查

### 记忆搜索不到

```bash
# 检查向量引擎状态
cis memory status

# 重建索引
cis memory rebuild-index

# 检查存储位置
ls -la ~/.cis/data/core/
```

### DAG 执行失败

```bash
# 查看执行日志
cis dag logs <execution-id>

# 检查任务状态
cis dag status <execution-id>

# 重试失败任务
cis dag retry <execution-id>
```

### P2P 连接问题

```bash
# 检查网络状态
cis p2p status

# 查看发现的节点
cis p2p peers

# 手动连接
cis p2p connect <node-id>
```

### 项目接入问题

```bash
# 检查项目配置
cat .cis/project.toml

# 验证项目结构
cis project validate

# 重新初始化
cis project init --force
```

### Agent 问题

```bash
# 查看 Agent 状态
cis agent status

# 查看 Agent 日志
cis agent logs <agent-name>

# 重启 Agent
cis agent restart <agent-name>

# 强制停止
cis agent kill <agent-name>
```

---

## 最佳实践

1. **记忆键命名**: 使用 `domain/category/identifier` 层次结构
2. **域选择**: 敏感信息用 Private，共享配置用 Public
3. **DAG 设计**: 机械任务自动执行，高风险任务需确认
4. **错误处理**: 所有记忆/DAG/网络操作都要处理错误
5. **索引优化**: 重要记忆使用 `set_with_embedding` 建立语义索引
6. **项目隔离**: 每个项目独立的命名空间，避免记忆冲突
7. **Agent 管理**: 使用 Pool 管理 Agent 生命周期，避免资源泄漏
8. **配置分层**: 全局配置 → 项目配置 → 任务配置，层层覆盖

---

## 相关文档

- [Agent 配置指南](./docs/AGENT_CONFIGURATION_GUIDE.md)
- [分布式 DAG 协调器](./docs/DISTRIBUTED_DAG_COORDINATOR.md)
- [组网指南](./docs/NETWORKING.md)
- [存储设计](./docs/STORAGE_DESIGN.md)
- [快速开始](./docs/getting-started/quickstart.md)

---

**提示**: 本文档自动注入到 Claude 的上下文中。当用户提及 CIS 相关功能时，请参考本文档的指导。
