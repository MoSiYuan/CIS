# CIS Skill 迁移交接文档

## 完成内容

### 1. AI Executor Skill (`skills/ai-executor/`)

**功能**: 抽象 AI Agent 执行层，兼容多种 AI 工具

**支持的 Agent**:
- `claude` - Claude Code (Anthropic)
- `claude-cli` - Claude CLI (旧版)
- `kimi` - Kimi Code (Moonshot)
- `kimi-cli` - Kimi CLI
- `aider` - Aider (多模型)
- `codex` - OpenAI Codex

**特点**:
- 直接执行，不检查环境可用性
- 信任用户已配置好 AI 环境
- 纯同步执行（WASM 友好）

**使用**:
```rust
let executor = AiExecutor::new();
let resp = executor.execute(ExecuteRequest {
    agent: AgentType::ClaudeCode,
    prompt: "Hello".to_string(),
    work_dir: None,
})?;
```

### 2. Init Wizard Skill (`skills/init-wizard/`)

**功能**: 初始化引导，帮助用户配置 AI 环境

**检查项**:
- Claude Code 安装状态
- Kimi Code 安装状态
- Aider 安装状态
- OpenAI Codex 安装状态

**输出**:
- 生成 `~/.cis/config.toml` 配置建议
- 显示安装命令

**使用**:
```bash
cis skill run init-wizard
```

### 3. Memory Organizer Skill (`skills/memory-organizer/`)

**功能**: 记忆整理（从 AgentFlow 迁移）

**迁移来源**:
- `agentflow-core/src/memory/mod.rs` - 冲突解决逻辑
- `agentflow-core/src/db/migration.rs` - 迁移框架

**功能**:
- 提取关键词
- 生成摘要
- 自动分类

**WASM 接口**:
- `skill_init()` - 初始化
- `skill_on_memory_write()` - 处理记忆写入
- `skill_parse_keywords()` - 解析关键词

### 4. Push Client Skill (`skills/push-client/`)

**功能**: 推送客户端（从 AgentFlow push 迁移）

**迁移来源**:
- `agentflow-core/src/push/client.rs`
- `agentflow-core/src/push/mod.rs`

**注意**: CIS 核心禁止云端同步，此 skill 仅用于:
- Webhook 通知
- P2P 节点通信（元数据）
- 日志记录

**目标类型**:
- `Webhook` - HTTP webhook
- `P2PNode` - CIS P2P 节点
- `Log` - 仅日志

## 代码迁移清单

| AgentFlow 文件 | CIS Skill 目标 | 状态 |
|----------------|----------------|------|
| `db/migration.rs` | `memory-organizer/src/lib.rs` (migration mod) | ✅ 迁移 |
| `db/data_migration.rs` | `memory-organizer/src/lib.rs` (recovery mod) | ✅ 迁移 |
| `memory/mod.rs` | `memory-organizer/src/lib.rs` | ✅ 简化迁移 |
| `push/client.rs` | `push-client/src/lib.rs` | ✅ 改造迁移 |
| `push/mod.rs` | `push-client/src/lib.rs` (types mod) | ✅ 迁移 |
| `project.rs` | 废弃（无 project_id 概念） | ❌ 废弃 |

## 目录结构

```
CIS/
├── skills/
│   ├── ai-executor/          # AI 执行层（抽象）
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   │
│   ├── init-wizard/          # 初始化引导
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   │
│   ├── memory-organizer/     # 记忆整理（AgentFlow 迁移）
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   │
│   ├── push-client/          # 推送客户端（AgentFlow 迁移）
│   │   ├── src/lib.rs
│   │   └── Cargo.toml
│   │
│   └── ai-provider-core/     # AI Provider 参考实现（Native）
│       └── src/
│           ├── mod.rs
│           ├── claude.rs
│           └── kimi.rs
│
├── cis-core/
│   └── src/ai/               # AI 模块（已存在）
│       ├── mod.rs
│       ├── claude.rs
│       └── kimi.rs
│
└── docs/
    ├── AI_PROVIDER_SKILL.md  # AI Provider 设计文档
    ├── CORE_MIGRATION_PLAN.md # 核心迁移计划
    └── HANDOVER.md           # 本文档
```

## 后续开发任务

### 高优先级

1. **WASM Runtime 集成**
   - 在 `cis-core` 中集成 `wasmer` 或 `wasmtime`
   - 实现 skill 加载器
   - 定义 Host API 接口

2. **Host API 定义**
   ```rust
   // Host 提供给 WASM skill 的 API
   pub mod host_api {
       pub fn memory_get(key: &str) -> Option<Vec<u8>>;
       pub fn memory_set(key: &str, value: &[u8]);
       pub fn ai_chat(prompt: &str) -> Result<String>;
       pub fn log_info(msg: &str);
       pub fn http_post(url: &str, body: &[u8]) -> Result<Vec<u8>>;
   }
   ```

3. **Skill Registry**
   - 实现 skill 发现机制
   - 签名验证
   - 权限管理

### 中优先级

4. **AI Executor 增强**
   - 添加更多 AI agent 支持（Copilot, Codeium 等）
   - 流式输出支持
   - 重试和超时机制

5. **Init Wizard 完善**
   - 交互式配置向导
   - 自动下载安装脚本
   - 配置验证

6. **Memory Organizer 增强**
   - 更智能的分类算法
   - 关联记忆发现
   - 记忆压缩

### 低优先级

7. **Push Client 扩展**
   - 更多目标类型（Slack, Discord, Email）
   - 批量推送
   - 推送队列

## 技术决策记录

### 决策 1: 直接执行 vs 可用性检查

**选择**: 直接执行，不检查可用性

**理由**:
- 信任用户已配置好环境
- 减少启动时间
- 失败时通过 stderr 提示

### 决策 2: WASM vs Native

**选择**: WASM 作为默认，Native 作为性能关键

**理由**:
- WASM 沙箱安全
- 跨平台
- 依赖隔离

### 决策 3: project_id 废弃

**选择**: 完全废弃 project_id

**理由**:
- CIS 单节点无多项目概念
- 减少复杂度
- 使用 `domain` 替代（纯组织）

## 已知问题

1. **WASM 接口未完整实现**
   - 当前只有骨架代码
   - 需要 Host API 配合

2. **AI Executor 同步阻塞**
   - 当前使用 `std::process::Command`
   - WASM 环境需要改为异步回调

3. **错误处理简化**
   - 当前直接返回错误码
   - 需要完善的错误类型

## 联系信息

- 迁移执行: AgentFlow → CIS
- 执行日期: 2026-02-02
- 分支: feature/0.4.0-refactor

---

## SDK 设计 (`cis-skill-sdk`)

### 目录结构

```
cis-skill-sdk/
├── Cargo.toml
├── src/
│   ├── lib.rs          # 主入口，导出核心类型
│   ├── skill.rs        # Skill trait, SkillContext, NativeSkill, WasmSkill
│   ├── types.rs        # SkillMeta, Event, MemoryEntry, HttpRequest, etc.
│   ├── host.rs         # Host API (Native/WASM 双模式)
│   ├── im.rs           # IM 专用接口（Claude 开发用）
│   ├── ai.rs           # AI 调用封装
│   └── error.rs        # Error, Result, ErrorCode
├── cis-skill-sdk-derive/  # Proc-macro crate
│   └── src/lib.rs      # #[skill], #[derive(Skill)]
├── examples/
│   └── hello_skill.rs  # 示例
└── README.md
```

### 核心设计

1. **双模式支持**
   - `native` feature: 完整功能，支持异步
   - `wasm` feature: `no_std` 兼容，FFI 导入 Host API

2. **Skill Trait**
   ```rust
   pub trait Skill: Send + Sync {
       fn name(&self) -> &str;
       fn version(&self) -> &str;
       fn init(&mut self, config: SkillConfig) -> Result<()>;
       fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()>;
   }
   ```

3. **SkillContext 接口**
   ```rust
   pub trait SkillContext: Send + Sync {
       fn log(&self, level: LogLevel, message: &str);
       fn memory_get(&self, key: &str) -> Option<Vec<u8>>;
       fn memory_set(&self, key: &str, value: &[u8]) -> Result<()>;
       fn ai_chat(&self, prompt: &str) -> Result<String>;
       fn http_request(&self, request: HttpRequest) -> Result<HttpResponse>;
       // ...
   }
   ```

4. **IM 专用接口** (`im` 模块)
   - `ImMessage`: 完整的消息结构
   - `ImMessageBuilder`: 流式构建消息
   - `ImContextExt`: 为 SkillContext 添加 IM 方法
   - 事件：`ImEvent::MessageReceived`, `ImEvent::UserOnline`, etc.

5. **AI 封装** (`ai` 模块)
   ```rust
   Ai::chat(prompt)?;
   Ai::summarize(text, max_length)?;
   Ai::extract_keywords(text, count)?;
   Ai::sentiment(text)?;
   Ai::classify(text, &["cat", "dog"])?;
   ```

### 使用示例

```rust
use cis_skill_sdk::{Skill, SkillContext, Event, Result};
use cis_skill_sdk::im::{ImContextExt, ImMessage, ImMessageBuilder};

pub struct EchoSkill;

impl Skill for EchoSkill {
    fn name(&self) -> &str { "echo" }
    
    fn handle_event(&self, ctx: &dyn SkillContext, event: Event) -> Result<()> {
        if let Event::Custom { name, data } = event {
            if name == "im:message" {
                let msg: ImMessage = serde_json::from_value(data)?;
                let reply = ImMessageBuilder::text("收到！")
                    .to(&msg.from)
                    .reply_to(&msg.id)
                    .build();
                ctx.im_send(&reply)?;
            }
        }
        Ok(())
    }
}
```

### 给 Claude（IM 开发）

Claude 正在开发 IM 相关功能，SDK 已提供：

1. **消息类型**: `ImMessage`, `MessageContent`, `MessageType`
2. **构建器**: `ImMessageBuilder` 提供流式 API
3. **上下文扩展**: `ImContextExt` trait 添加 `im_send`, `im_reply`, `im_get_history` 等方法
4. **事件**: `ImEvent` 枚举涵盖所有 IM 场景

**Claude 开发 IM Skill 时**:
1. 引入 `cis-skill-sdk` 依赖，启用 `native` feature
2. 实现 `Skill` trait
3. 使用 `im` 模块中的类型和 API
4. 通过 `ctx.im_send()` 等方法与 IM 系统交互

---

## 存储与热插拔设计 (`cis-core`)

### 跨平台目录结构

| 平台 | 数据目录 |
|------|---------|
| macOS | `~/Library/Application Support/CIS` |
| Linux | `~/.local/share/cis` |
| Windows | `%LOCALAPPDATA%\CIS` |

```
$CIS_DATA_DIR/
├── config.toml              # 主配置
├── node.key                 # 节点私钥
├── core/
│   ├── core.db              # 核心数据库（任务、配置、索引）
│   └── backup/              # 自动备份
├── skills/
│   ├── registry.json        # Skill 注册表
│   ├── installed/           # Skill 代码
│   │   ├── native/          # Native Skills
│   │   └── wasm/            # WASM Skills
│   └── data/                # Skill 独立数据库
│       ├── ai-executor/data.db
│       ├── im/data.db       # IM 独立数据库
│       └── memory-organizer/data.db
├── logs/
├── cache/                   # 可安全删除
└── runtime/                 # 重启清空
```

### 数据库隔离

#### 核心数据库 (`core/core.db`)

```rust
pub struct CoreDb;
impl CoreDb {
    pub fn open() -> Result<Self>;
    pub fn set_config(&self, key: &str, value: &[u8]) -> Result<()>;
    pub fn register_memory_index(&self, key: &str, skill: Option<&str>) -> Result<()>;
    // 不存储 Skill 数据，只存索引
}
```

#### Skill 数据库 (`skills/data/{name}/data.db`)

```rust
pub struct SkillDb {
    name: String,
    conn: Connection,
}

// 每个 Skill 完全独立
// 热插拔：独立加载/卸载，不影响核心
```

### 热插拔支持

```rust
use cis_core::skill::{SkillManager, LoadOptions};

// 1. 创建管理器
let manager = SkillManager::new(db_manager)?;

// 2. 加载 Skill（热插拔）
manager.load("im", LoadOptions::default())?;
manager.activate("im")?;

// 3. 卸载 Skill（热插拔）
manager.unload("im")?;
```

**生命周期**: `Registered → Loaded → Active → Unloaded`

**关键特性**:
- 每个 Skill 独立 SQLite 连接
- 卸载时关闭连接，释放资源
- 不影响核心数据库和其他 Skill

---

## Agent 抽象层与双向集成 (`cis-core`)

### 架构设计

CIS 作为基础设施，记忆是基础设施里的数据。支持双向调用：

```
┌─────────────────────────────────────────────────────────────┐
│                     CIS 基础设施层                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │   Memory    │  │   Skills    │  │   Tasks     │          │
│  │   Store     │  │   Registry  │  │  Scheduler  │          │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘          │
│         └─────────────────┼─────────────────┘                │
│                           │                                  │
│                    ┌──────┴──────┐                          │
│                    │  CIS Core   │                          │
│                    │   Engine    │                          │
│                    └──────┬──────┘                          │
└───────────────────────────┼─────────────────────────────────┘
                            │
              ┌─────────────┴─────────────┐
              │                           │
              ▼                           ▼
    ┌──────────────────┐      ┌──────────────────┐
    │   CIS → Agent    │      │   Agent → CIS    │
    │   (调用 LLM)     │      │   (调用基础设施)  │
    └──────────────────┘      └──────────────────┘
```

### 1. CIS → Agent (CIS 调用 Agent)

```rust
use cis_core::agent::{AgentProvider, AgentRequest, AgentContext};

// 创建请求
let req = AgentRequest {
    prompt: "Review this code".to_string(),
    context: AgentContext::new()
        .with_work_dir(project_path),
    skills: vec!["memory-search".to_string()],
    system_prompt: None,
    history: vec![],
};

// 执行
let response = agent.execute(req).await?;
```

**支持的 Agents**: Claude, Kimi, Aider

### 2. Agent → CIS (Agent 调用 CIS)

Claude 通过 CLI 调用 CIS：

```bash
# 记忆操作
$ cis memory get <key>
$ cis memory set <key> <value>
$ cis memory search <query>

# 任务管理
$ cis task list
$ cis task create --title "..." --description "..."

# Skill 调用
$ cis skill call <name> --method <method>

# 导出上下文
$ cis context export
```

### 3. 项目级集成

项目配置文件 `.cis/project.toml`:

```toml
[project]
name = "my-project"
id = "uuid"

[ai]
guide = """
You are working on a Rust project with CIS integration.
Available skills: memory-search, task-manage, code-review
"""
provider = "claude"

[[skills]]
name = "custom-linter"
path = "./skills/custom-linter"
auto_load = true

[memory]
namespace = "project/my-project"
shared_keys = ["conventions", "architecture"]
```

**代码位置**:
- `cis-core/src/agent/` - Agent 抽象层和 Provider 实现
- `cis-core/src/project/` - 项目管理和会话

---

## 已完成工作总结

### SDK (`cis-skill-sdk`)
- ✅ 双模式支持 (Native/WASM)
- ✅ Skill trait 定义
- ✅ Host API 接口
- ✅ IM 专用接口 (`ImMessage`, `ImMessageBuilder`, `ImContextExt`)
- ✅ AI 调用封装
- ✅ Derive 宏

### Core (`cis-core`)
- ✅ 跨平台目录结构 (macOS/Linux/Windows)
- ✅ 数据库隔离 (核心/Split 分离)
- ✅ 热插拔支持
- ✅ Agent 抽象层 (Claude/Kimi/Aider)
- ✅ 双向集成 (CIS ↔ Agent)
- ✅ 项目级配置
- ✅ Skill 管理器

### Skills (非 IM)
- ✅ `ai-executor` - AI Agent 执行层
- ✅ `init-wizard` - 初始化引导
- ✅ `memory-organizer` - 记忆整理 (WASM)
- ✅ `push-client` - 推送客户端 (WASM)

---

**下一步**: 
1. **Claude 开发 IM Skill** - 使用 SDK，数据独立存储
2. **WASM Runtime 集成** - 在 core 中集成 wasmer/wasmtime
3. **CLI 实现** - 实现 `cis` 命令行工具
4. **发布准备** - macOS/Linux/Windows 构建脚本
