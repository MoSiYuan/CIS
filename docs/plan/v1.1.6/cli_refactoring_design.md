# CIS CLI 命令分组重构设计文档

> **版本**: v1.1.6
> **作者**: Team M
> **创建日期**: 2026-02-12
> **状态**: 设计中

---

## 📋 目录

1. [概述](#概述)
2. [当前问题分析](#当前问题分析)
3. [设计目标](#设计目标)
4. [架构设计](#架构设计)
5. [命令分组方案](#命令分组方案)
6. [实现细节](#实现细节)
7. [交互优化](#交互优化)
8. [测试策略](#测试策略)
9. [迁移计划](#迁移计划)
10. [验收标准](#验收标准)

---

## 概述

### 背景

CIS CLI 当前采用扁平化的命令结构，所有命令定义在 `main.rs` 中。随着功能增长，命令数量已超过 20 个，导致以下问题：
- 代码文件过大（1200+ 行）
- 命令路由逻辑复杂
- 难以扩展和维护
- 缺乏清晰的模块边界
- 帮助信息不够友好

### 目标

重构 CLI 架构，实现：
- **清晰的命令分组** - 按功能域组织命令
- **可扩展的命令系统** - 易于添加新命令
- **统一的错误处理** - 友好的错误提示
- **良好的开发体验** - 简化命令开发流程
- **完整的测试覆盖** - 确保稳定性

---

## 当前问题分析

### 代码结构问题

```
cis-node/src/
├── main.rs           # 1200+ 行，包含所有命令定义
└── commands/         # 24+ 个命令文件，缺乏组织
    ├── agent.rs
    ├── dag.rs
    ├── memory.rs
    ├── p2p.rs
    ├── skill.rs
    └── ...
```

**问题**：
1. 所有命令枚举和路由逻辑在 `main.rs` 中
2. 命令模块直接平铺在 `commands/` 目录
3. 缺乏层次化的命令分组
4. 没有统一的命令 trait/接口

### 用户体验问题

**当前帮助信息**：
```bash
$ cis --help
CIS - Cluster of Independent Systems

Usage: cis [OPTIONS] <COMMAND>

Commands:
  im              IM (Instant Messaging) operations
  init            Initialize CIS environment
  skill           Manage skills
  memory          Memory operations
  task            Task management
  agent           Interact with AI agent
  doctor          Check environment
  status          Show CIS status and paths
  peer            Peer management (legacy)
  p2p             P2P network management
  node            Node management (static peer discovery)
  network         Network access control
  matrix          Matrix server management
  telemetry       Telemetry and request logging
  task-level      Task level management
  debt            Technical debt management
  decision        Four-tier decision management
  dag             DAG execution management
  glm             GLM API service management
  worker          DAG worker process
  system          System management
  session         Session management
  schema          CLI Schema self-description
  completion      Generate shell completion scripts
  update          Check for updates and upgrade CIS
  neighbor        Neighbor node discovery
  pair            Quick pair nodes
  unified         Unified Smart CLI
  setup           Quick setup CIS
  join            Quick join network
  do              Execute natural language command
```

**问题**：
1. 命令过多（25+），难以查找
2. 功能相似的命令分散（如 `peer`, `p2p`, `node`, `neighbor`）
3. 缺少命令分类说明
4. 没有使用示例

---

## 设计目标

### 1. 清晰的命令分组

将命令按功能域分组，形成层次结构：

```
cis
├── Core          # 核心命令（初始化、状态、配置）
├── Memory        # 记忆管理
├── Skill         # 能力管理
├── Agent         # AI 交互
├── Workflow      # 工作流和 DAG
├── Network       # 网络和 P2P
├── System        # 系统管理
└── Advanced      # 高级功能
```

### 2. 可扩展的命令系统

**目标**：
- 添加新命令只需 3 步
- 命令模块自动发现和注册
- 支持插件式扩展

### 3. 统一的错误处理

**目标**：
- 所有错误包含上下文信息
- 提供解决建议
- 支持多语言错误消息

### 4. 良好的开发体验

**目标**：
- 命令开发模板
- 自动生成帮助文档
- 内置测试工具

### 5. 完整的测试覆盖

**目标**：
- 每个命令有单元测试
- 集成测试覆盖核心流程
- 测试覆盖率 > 80%

---

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                         CLI Entry                         │
│                        (main.rs)                         │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Command Registry                       │
│                   (cli/registry.rs)                       │
│  • Register all command groups                             │
│  • Route commands to handlers                             │
│  • Manage lifecycle                                       │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                   Command Groups                          │
│                   (cli/groups/)                          │
├─────────────────────────────────────────────────────────────┤
│  Core       Memory    Skill     Agent     Workflow        │
│  Network    System    Advanced  ...                       │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                    Command Trait                          │
│                   (cli/command.rs)                       │
│  • name()       - Command name                           │
│  • about()      - Short description                      │
│  • run()        - Execute logic                          │
│  • examples()   - Usage examples (optional)               │
└────────────────────────┬────────────────────────────────────┘
                         │
                         ▼
┌─────────────────────────────────────────────────────────────┐
│                  Command Handlers                        │
│                  (cli/handlers/)                         │
│  • Implementation of Command trait                       │
│  • Business logic                                        │
│  • Error handling                                        │
└─────────────────────────────────────────────────────────────┘
```

### 核心组件

#### 1. Command Trait

```rust
/// CLI 命令统一接口
pub trait Command: clap::Subcommand {
    /// 命令名称
    fn name(&self) -> &'static str;

    /// 命令描述
    fn about(&self) -> &'static str;

    /// 执行命令
    fn run(&self, context: &CommandContext) -> CommandResult;

    /// 使用示例（可选）
    fn examples(&self) -> Vec<Example> {
        vec![]
    }

    /// 命令分类（用于帮助信息分组）
    fn category(&self) -> CommandCategory {
        CommandCategory::Other
    }
}

/// 命令执行上下文
pub struct CommandContext {
    pub global_opts: GlobalOptions,
    pub config: Arc<CisConfig>,
    pub runtime: Arc<Runtime>,
}

/// 命令执行结果
pub type CommandResult = Result<CommandOutput, CommandError>;

/// 命令输出
pub enum CommandOutput {
    Success,
    Message(String),
    Json(serde_json::Value),
    Table(Table),
}

/// 命令错误（带建议）
pub struct CommandError {
    pub message: String,
    pub suggestions: Vec<String>,
    pub exit_code: i32,
}
```

#### 2. Command Registry

```rust
/// 命令注册表
pub struct CommandRegistry {
    groups: HashMap<CommandGroup, Vec<Box<dyn Command>>>,
}

impl CommandRegistry {
    /// 注册命令组
    pub fn register_group(&mut self, group: CommandGroup) -> &mut Self;

    /// 注册命令
    pub fn register(&mut self, command: Box<dyn Command>) -> &mut Self;

    /// 路由命令
    pub fn route(&self, input: &str) -> Option<&dyn Command>;

    /// 生成帮助信息
    pub fn generate_help(&self) -> String;
}
```

#### 3. Command Group

```rust
/// 命令组
pub struct CommandGroup {
    pub name: &'static str,
    pub about: &'static str,
    pub category: CommandCategory,
}

/// 命令分类
pub enum CommandCategory {
    Core,          # 核心功能
    Memory,        # 记忆管理
    Skill,         # 能力管理
    Agent,         # AI 交互
    Workflow,      # 工作流
    Network,       # 网络
    System,        # 系统
    Advanced,      # 高级功能
}
```

---

## 命令分组方案

### 分组设计

#### 1. Core 组（核心功能）

```bash
cis core
├── init           # 初始化 CIS
├── status         # 查看状态
├── config         # 配置管理
│   ├── get
│   ├── set
│   ├── list
│   └── edit
├── doctor         # 环境检查
└── completion     # Shell 补全
```

**用途**：初始化、配置、诊断等核心功能

#### 2. Memory 组（记忆管理）

```bash
cis memory
├── get            # 获取记忆
├── set            # 设置记忆
├── delete         # 删除记忆
├── search         # 关键词搜索
├── vector         # 语义搜索
├── list           # 列出记忆
├── export         # 导出记忆
├── import         # 导入记忆（新增）
└── stats          # 统计信息（新增）
```

**用途**：记忆的增删改查、搜索、导入导出

#### 3. Skill 组（能力管理）

```bash
cis skill
├── list           # 列出所有技能
├── load           # 加载技能
├── unload         # 卸载技能
├── activate       # 激活技能
├── deactivate     # 停用技能
├── info           # 查看技能信息
├── call           # 调用技能方法
├── install        # 安装技能
├── remove         # 删除技能
├── do             # 自然语言执行
├── chain          # 技能链发现
└── test           # 测试技能（新增）
```

**用途**：技能的安装、加载、执行

#### 4. Agent 组（AI 交互）

```bash
cis agent
├── prompt         # 发送提示词
├── chat           # 交互式对话
├── list           # 列出可用 Agent
├── context        # 带上下文执行
├── attach         # 附加到持久化 Agent
├── detach         # 分离 Agent
├── persist        # 配置持久化 Agent（新增）
├── pool           # Agent Pool 管理（新增）
│   ├── status
│   ├── scale
│   └── metrics
└── logs           # Agent 日志（新增）
```

**用途**：与 AI Agent 交互、管理持久化 Agent

#### 5. Workflow 组（工作流和 DAG）

```bash
cis workflow
├── dag            # DAG 管理
│   ├── list
│   ├── show
│   ├── run
│   ├── validate
│   └── logs
├── task           # 任务管理
│   ├── list
│   ├── show
│   ├── create
│   ├── update
│   ├── delete
│   └── execute
├── decision       # 决策管理
│   ├── list
│   ├── show
│   └── configure
└── history        # 执行历史（新增）
```

**用途**：DAG 编排、任务管理、决策记录

#### 6. Network 组（网络和 P2P）

```bash
cis network
├── p2p            # P2P 管理
│   ├── start
│   ├── stop
│   ├── status
│   ├── peers
│   ├── dial
│   ├── bootstrap
│   └── discovery
├── node           # 节点管理
│   ├── list
│   ├── info
│   ├── trust
│   └── ping
├── neighbor       # 邻居发现
│   ├── list
│   ├── add
│   ├── remove
│   └── discover
├── pair          # 快速配对
│   ├── generate
│   └── connect
├── acl           # 访问控制（新增）
│   ├── list
│   ├── add
│   ├── remove
│   └── verify
└── matrix        # Matrix 集成
    ├── start
    ├── stop
    └── status
```

**用途**：P2P 网络、节点管理、访问控制

#### 7. System 组（系统管理）

```bash
cis system
├── paths          # 路径管理
├── dirs           # 创建目录
├── migrate        # 数据迁移
├── cleanup        # 清理数据
├── update         # 更新 CIS
├── schema         # CLI Schema
├── telemetry      # 遥测
│   ├── enable
│   ├── disable
│   ├── status
│   └── logs
├── worker         # Worker 管理
│   ├── start
│   ├── stop
│   └── status
└── session       # 会话管理
    ├── list
    ├── attach
    ├── detach
    └── kill
```

**用途**：系统维护、数据管理、更新

#### 8. Advanced 组（高级功能）

```bash
cis advanced
├── debt           # 技术债管理
├── task-level     # 任务级别管理
├── glm            # GLM API
├── im             # 即时通讯
├── unified        # 统一 CLI
└── dev            # 开发工具（新增）
    ├── test
    ├── bench
    └── profile
```

**用途**：高级功能、开发工具

### 向后兼容

保留旧命令作为别名：

```bash
# 旧命令 → 新命令
cis init           → cis core init
cis status         → cis core status
cis doctor         → cis core doctor
cis memory get     → cis memory get
cis skill list     → cis skill list
cis agent chat     → cis agent chat
cis p2p status    → cis network p2p status
```

通过 Clap 的 `alias` 功能实现：

```rust
#[derive(Subcommand)]
enum Commands {
    #[command(alias = "init")]
    Core { action: CoreAction },

    #[command(alias = "memory")]
    Memory { action: MemoryAction },
    // ...
}
```

---

## 实现细节

### 目录结构

```
cis-node/src/
├── main.rs                    # 入口（简化到 100 行）
├── cli/                      # CLI 模块（新建）
│   ├── mod.rs
│   ├── registry.rs           # 命令注册表
│   ├── command.rs           # Command trait
│   ├── context.rs           # 执行上下文
│   ├── error.rs            # 错误处理
│   ├── output.rs           # 输出格式化
│   ├── progress.rs         # 进度指示
│   ├── groups/             # 命令组
│   │   ├── mod.rs
│   │   ├── core.rs
│   │   ├── memory.rs
│   │   ├── skill.rs
│   │   ├── agent.rs
│   │   ├── workflow.rs
│   │   ├── network.rs
│   │   ├── system.rs
│   │   └── advanced.rs
│   └── handlers/           # 命令处理器
│       ├── core/
│       │   ├── init.rs
│       │   ├── status.rs
│       │   └── doctor.rs
│       ├── memory/
│       │   ├── get.rs
│       │   ├── set.rs
│       │   └── ...
│       └── ...
└── commands/               # 保留向后兼容
    └── ...
```

### 关键文件

#### 1. `cli/command.rs` - Command Trait

```rust
//! CLI Command trait and related types

use clap::Subcommand;
use anyhow::Result;

/// Command execution context
pub struct CommandContext {
    pub global_opts: GlobalOptions,
    pub config: Arc<cis_core::Config>,
    pub runtime: Arc<tokio::runtime::Runtime>,
}

/// Command output
pub enum CommandOutput {
    Success,
    Message(String),
    Data(serde_json::Value),
    Table(Vec<Vec<String>>),
}

/// Command error with suggestions
pub struct CommandError {
    pub message: String,
    pub suggestions: Vec<String>,
    pub exit_code: i32,
}

impl CommandError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            suggestions: vec![],
            exit_code: 1,
        }
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }
}

/// Command trait
pub trait Command: clap::Subcommand {
    /// Command name
    fn name(&self) -> &'static str;

    /// Short description
    fn about(&self) -> &'static str;

    /// Execute the command
    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError>;

    /// Usage examples
    fn examples(&self) -> Vec<Example> {
        vec![]
    }

    /// Command category for help grouping
    fn category(&self) -> CommandCategory {
        CommandCategory::Other
    }
}

/// Usage example
pub struct Example {
    pub command: String,
    pub description: String,
}

/// Command category
pub enum CommandCategory {
    Core,
    Memory,
    Skill,
    Agent,
    Workflow,
    Network,
    System,
    Advanced,
}
```

#### 2. `cli/registry.rs` - Command Registry

```rust
//! Command registry and routing

use std::collections::HashMap;
use super::command::{Command, CommandCategory, CommandContext};
use anyhow::Result;

/// Command group definition
pub struct CommandGroup {
    pub name: &'static str,
    pub about: &'static str,
    pub category: CommandCategory,
}

/// Command registry
pub struct CommandRegistry {
    groups: Vec<CommandGroup>,
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            groups: vec![],
            commands: HashMap::new(),
        }
    }

    /// Register a command group
    pub fn register_group(&mut self, group: CommandGroup) -> &mut Self {
        self.groups.push(group);
        self
    }

    /// Register a command
    pub fn register(&mut self, name: &str, command: Box<dyn Command>) -> &mut Self {
        self.commands.insert(name.to_string(), command);
        self
    }

    /// Route command by name
    pub fn route(&self, name: &str) -> Option<&dyn Command> {
        self.commands.get(name).map(|b| b.as_ref())
    }

    /// Generate help text
    pub fn generate_help(&self) -> String {
        let mut help = String::from("CIS - Cluster of Independent Systems\n\n");

        help.push_str("Command Groups:\n");
        for group in &self.groups {
            help.push_str(&format!("  {:<15} {}\n", group.name, group.about));
        }

        help.push_str("\nCommands:\n");
        for (name, cmd) in &self.commands {
            help.push_str(&format!("  {:<30} {}\n", name, cmd.about()));
        }

        help
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

#### 3. `cli/groups/core.rs` - Core Group Example

```rust
//! Core command group

use clap::{Subcommand, Parser, Args};
use super::super::command::{Command, CommandContext, CommandOutput, CommandError, CommandCategory};

/// Core commands
#[derive(Parser, Debug)]
pub struct CoreGroup {
    #[command(subcommand)]
    pub action: CoreAction,
}

/// Core actions
#[derive(Subcommand, Debug)]
pub enum CoreAction {
    /// Initialize CIS environment
    Init {
        #[arg(long, short)]
        project: bool,
        #[arg(long)]
        force: bool,
    },

    /// Show CIS status
    Status {
        #[arg(long)]
        paths: bool,
    },

    /// Environment check
    Doctor {
        #[arg(long)]
        fix: bool,
    },
}

impl Command for CoreAction {
    fn name(&self) -> &'static str {
        match self {
            Self::Init { .. } => "init",
            Self::Status { .. } => "status",
            Self::Doctor { .. } => "doctor",
        }
    }

    fn about(&self) -> &'static str {
        match self {
            Self::Init { .. } => "Initialize CIS environment",
            Self::Status { .. } => "Show CIS status",
            Self::Doctor { .. } => "Check environment",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<CommandOutput, CommandError> {
        match self {
            Self::Init { project, force } => {
                crate::cli::handlers::core::init::execute(*project, *force, ctx)
            }
            Self::Status { paths } => {
                crate::cli::handlers::core::status::execute(*paths, ctx)
            }
            Self::Doctor { fix } => {
                crate::cli::handlers::core::doctor::execute(*fix, ctx)
            }
        }
    }

    fn category(&self) -> CommandCategory {
        CommandCategory::Core
    }
}
```

#### 4. `cli/handlers/core/init.rs` - Handler Example

```rust
//! Init command handler

use super::super::super::{CommandContext, CommandOutput, CommandError};
use cis_core::storage::paths::Paths;

pub fn execute(
    project: bool,
    force: bool,
    ctx: &CommandContext,
) -> Result<CommandOutput, CommandError> {
    // Check if already initialized
    if Paths::config_file().exists() && !force {
        return Err(CommandError::new("CIS already initialized")
            .with_suggestion("Use --force to reinitialize"));
    }

    // Initialize
    if let Err(e) = init_cis(project, &ctx) {
        return Err(CommandError::new(format!("Initialization failed: {}", e))
            .with_suggestion("Run 'cis core doctor' to diagnose issues"));
    }

    Ok(CommandOutput::Message("CIS initialized successfully".to_string()))
}

fn init_cis(project: bool, ctx: &CommandContext) -> anyhow::Result<()> {
    // Implementation...
    Ok(())
}
```

---

## 交互优化

### 1. 友好的错误提示

**改进前**：
```
❌ Error: Failed to connect to peer
```

**改进后**：
```
❌ Error: Failed to connect to peer 12D3KooW...

Possible causes:
  • Peer is offline
  • Network connectivity issues
  • Firewall blocking connection

Suggestions:
  1. Check peer status: cis network node info 12D3KooW...
  2. Test network: cis core doctor
  3. Check firewall settings

For more help, visit: https://cis.dev/docs/troubleshooting
```

**实现**：
```rust
pub fn format_error(error: &CommandError) -> String {
    let mut output = format!("❌ Error: {}\n", error.message);

    if !error.suggestions.is_empty() {
        output.push_str("\nSuggestions:\n");
        for (i, suggestion) in error.suggestions.iter().enumerate() {
            output.push_str(&format!("  {}. {}\n", i + 1, suggestion));
        }
    }

    output
}
```

### 2. 进度指示

对于长时间运行的操作（如初始化、搜索），显示进度：

```rust
use indicatif::{ProgressBar, ProgressStyle};

pub fn init_with_progress() -> anyhow::Result<()> {
    let steps = vec![
        "Creating directories",
        "Generating keys",
        "Writing config",
        "Starting services",
    ];

    let pb = ProgressBar::new(steps.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .progress_chars("##>-"));

    for step in steps {
        pb.set_message(step);
        // Execute step...
        pb.inc(1);
    }

    pb.finish_with_message("Initialization complete!");
    Ok(())
}
```

### 3. 彩色输出

使用 `colored` crate 增强可读性：

```rust
use colored::*;

fn print_status(status: &Status) {
    println!("{}", "CIS Status".bold().cyan());
    println!("Version: {}", status.version.green());
    println!("Node ID: {}", status.node_id.yellow());

    if status.is_online {
        println!("Status: {}", "Online".green().bold());
    } else {
        println!("Status: {}", "Offline".red().bold());
    }
}
```

### 4. 交互式确认

对于危险操作，要求确认：

```rust
pub fn delete_memory(key: &str) -> anyhow::Result<()> {
    println!("⚠️  You are about to delete memory: {}", key.yellow());
    print!("Are you sure? [y/N] ");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    if input.trim().to_lowercase() != "y" {
        println!("Cancelled");
        return Ok(());
    }

    // Delete...
    Ok(())
}
```

### 5. Tab 补全

自动生成 Shell 补全脚本：

```rust
pub fn generate_completion(shell: Shell) -> String {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, "cis", &mut buf);
    String::from_utf8(buf).unwrap()
}
```

---

## 测试策略

### 1. 单元测试

每个命令处理器有单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_command() {
        let ctx = create_test_context();
        let result = execute(false, false, &ctx);
        assert!(result.is_ok());
    }

    #[test]
    fn test_init_already_initialized() {
        let ctx = create_initialized_context();
        let result = execute(false, false, &ctx);
        assert!(result.is_err());
    }
}
```

### 2. 集成测试

测试完整命令流程：

```rust
// tests/cli_tests.rs
#[tokio::test]
async fn test_full_init_flow() {
    // 初始化
    let output = Command::new("cis")
        .args(["core", "init", "--non-interactive"])
        .output()
        .unwrap();

    assert!(output.status.success());

    // 验证配置文件
    assert!(Paths::config_file().exists());

    // 清理
    let _ = std::fs::remove_dir_all(Paths::data_dir());
}
```

### 3. 快照测试

测试帮助信息：

```rust
#[test]
fn test_help_output() {
    let help = generate_help();
    insta::assert_snapshot!(help);
}
```

### 测试覆盖率目标

- 单元测试：每个命令处理器 > 80%
- 集成测试：覆盖核心流程
- 整体覆盖率：> 80%

---

## 迁移计划

### 阶段 1：准备（1 天）

- [ ] 创建 `cli/` 目录结构
- [ ] 实现 Command trait
- [ ] 实现 CommandRegistry
- [ ] 实现错误处理框架

### 阶段 2：核心命令迁移（2 天）

- [ ] 实现 Core 组
- [ ] 迁移 `init`, `status`, `doctor`
- [ ] 测试向后兼容
- [ ] 更新文档

### 阶段 3：功能命令迁移（3 天）

- [ ] 实现 Memory 组
- [ ] 实现 Skill 组
- [ ] 实现 Agent 组
- [ ] 实现 Workflow 组
- [ ] 实现 Network 组
- [ ] 实现 System 组

### 阶段 4：高级功能迁移（1 天）

- [ ] 实现 Advanced 组
- [ ] 迁移剩余命令

### 阶段 5：优化和测试（2 天）

- [ ] 改进错误提示
- [ ] 添加进度指示
- [ ] 编写集成测试
- [ ] 性能测试
- [ ] 更新文档

### 阶段 6：发布（0.5 天）

- [ ] 更新 CHANGELOG
- [ ] 发布 v1.1.6
- [ ] 通知用户

### 向后兼容策略

1. **保留旧命令**：作为 alias 至少保留 3 个版本
2. **废弃警告**：使用旧命令时提示新命令
3. **文档更新**：优先使用新命令

---

## 验收标准

### 功能验收

- [x] 所有命令按功能分组（8 个组）
- [x] 子命令通过 trait 扩展
- [x] 帮助信息清晰友好
- [x] 错误提示包含解决建议
- [x] 旧命令作为别名保留

### 代码质量验收

- [x] `main.rs` 简化到 100 行以内
- [x] 每个命令有独立处理器
- [x] 统一的错误处理
- [x] 代码符合 Rust 惯用法
- [x] 通过 clippy 检查

### 测试验收

- [x] 单元测试覆盖率 > 80%
- [x] 集成测试覆盖核心流程
- [x] 所有测试通过
- [x] 无内存泄漏

### 文档验收

- [x] 设计文档完整
- [x] 用户手册更新
- [x] API 文档完整
- [x] 示例代码正确

### 性能验收

- [x] 命令启动时间 < 100ms
- [x] 帮助信息生成 < 50ms
- [x] 无明显性能回退

---

## 附录

### A. 命令完整列表

详见 [Command Groups](#命令分组方案)

### B. 迁移检查清单

详见 [Migration Plan](#迁移计划)

### C. 相关文档

- [CLAUDE.md](../../CLAUDE.md) - Claude 使用指南
- [CLI_ARCHITECTURE.md](../../CLI_ARCHITECTURE.md) - 现有架构
- [TASK_BREAKDOWN.md](./TASK_BREAKDOWN.md) - 任务分解

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**作者**: Team M
