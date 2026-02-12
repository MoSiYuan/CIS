# CIS CLI 用户手册

> **版本**: v1.1.6
> **最后更新**: 2026-02-12

---

## 目录

1. [快速开始](#快速开始)
2. [命令分组](#命令分组)
3. [核心命令](#核心命令)
4. [记忆管理](#记忆管理)
5. [能力管理](#能力管理)
6. [AI 交互](#ai-交互)
7. [工作流管理](#工作流管理)
8. [网络管理](#网络管理)
9. [系统管理](#系统管理)
10. [高级功能](#高级功能)
11. [故障排查](#故障排查)

---

## 快速开始

### 安装

```bash
# 从源码构建
cargo install cis-node

# 或下载预编译二进制
curl -sSL https://cis.dev/install.sh | sh
```

### 初始化

```bash
# 交互式初始化
cis core init

# 快速初始化（使用默认配置）
cis core init --non-interactive

# 项目级初始化
cis core init --project
```

### 查看状态

```bash
cis core status
```

---

## 命令分组

CIS CLI 按功能分为 8 个命令组：

| 组名 | 描述 | 示例 |
|------|------|------|
| **Core** | 核心功能（初始化、配置、诊断） | `cis core init` |
| **Memory** | 记忆存储和检索 | `cis memory get` |
| **Skill** | 能力管理和执行 | `cis skill list` |
| **Agent** | AI 交互 | `cis agent chat` |
| **Workflow** | DAG 和任务工作流 | `cis workflow dag run` |
| **Network** | P2P 和网络管理 | `cis network p2p status` |
| **System** | 系统维护和工具 | `cis system update` |
| **Advanced** | 高级和实验功能 | `cis advanced dev test` |

### 查看帮助

```bash
# 全局帮助
cis --help

# 命令组帮助
cis core --help
cis memory --help

# 具体命令帮助
cis core init --help
```

---

## 核心命令

### 初始化

#### 初始化全局 CIS

```bash
cis core init
```

**选项**：
- `--project` - 初始化为项目（非全局）
- `--force` - 强制覆盖现有配置
- `--non-interactive` - 非交互模式（使用默认配置）
- `--skip-checks` - 跳过环境检查
- `--provider <PROVIDER>` - 指定 AI provider

#### 初始化项目

```bash
# 在项目目录下
cd /path/to/project
cis core init --project
```

这将创建 `.cis/project.toml` 配置文件。

### 状态

#### 查看 CIS 状态

```bash
cis core status
```

**输出示例**：
```
CIS Status
━━━━━━━━━━━━━━━━━━━━━━━━━━━━
Version:    1.1.6
Node ID:    12D3KooW...
Status:     Online
Config:     ~/.cis/config.toml
Data:       ~/.cis/data
```

#### 查看路径信息

```bash
cis core status --paths
```

### 配置管理

#### 获取配置值

```bash
cis core config get user.name
cis core config get ai.provider
```

#### 设置配置值

```bash
cis core config set user.name "John Doe"
cis core config set ai.provider claude
```

#### 列出所有配置

```bash
cis core config list
```

#### 编辑配置文件

```bash
cis core config edit
```

这将打开默认编辑器（`$EDITOR`）。

### 诊断

#### 运行诊断

```bash
cis core doctor
```

**检查项**：
- 配置文件完整性
- 数据目录权限
- 网络连接
- 依赖项版本

#### 自动修复问题

```bash
cis core doctor --fix
```

### Shell 补全

#### 生成补全脚本

```bash
# Bash
cis core completion bash > /tmp/cis-completion.bash
source /tmp/cis-completion.bash

# Zsh
cis core completion zsh > ~/.zfunc/_cis
echo "fpath=(~/.zfunc $fpath)" >> ~/.zshrc
autoload -U compinit && compinit

# Fish
cis core completion fish > ~/.config/fish/completions/cis.fish
```

#### 启用永久补全

**Bash**:
```bash
# 添加到 ~/.bashrc
eval "$(cis core completion bash)"
```

**Zsh**:
```bash
# 添加到 ~/.zshrc
eval "$(cis core completion zsh)"
```

---

## 记忆管理

### 获取记忆

```bash
cis memory get user.preference.theme
cis memory get project/architecture/microservices
```

### 设置记忆

```bash
# 基本存储
cis memory set user.preference.theme dark

# 指定域和类别
cis memory set user.api.key "sk-..." --domain private --category context

# 创建语义索引（支持自然语言搜索）
cis memory set project/conventions "Use Rust for all services" --index
```

**域 (Domain)**:
- `public` - 公开记忆（可 P2P 同步）
- `private` - 私有记忆（仅本地）

**类别 (Category)**:
- `context` - 上下文信息
- `execution` - 执行记录
- `result` - 执行结果
- `error` - 错误日志
- `skill` - 技能相关

### 搜索记忆

#### 关键词搜索

```bash
cis memory search "theme"
cis memory search "architecture" --limit 10
```

#### 语义搜索

```bash
cis memory vector "what theme did i prefer"
cis memory vector "how to handle errors" --limit 5 --threshold 0.7
```

**输出格式**：
```bash
# Plain text
cis memory vector "query" --format plain

# JSON
cis memory vector "query" --format json

# Table
cis memory vector "query" --format table
```

### 列出记忆

```bash
# 所有记忆
cis memory list

# 前缀过滤
cis memory list --prefix user

# 域过滤
cis memory list --domain public

# JSON 格式
cis memory list --format json
```

### 删除记忆

```bash
cis memory delete user.preference.old
```

### 导出/导入

#### 导出记忆

```bash
# 导出到文件
cis memory export --output memories.json

# 导出最近修改的记忆
cis memory export --since 1704067200 --output recent.json
```

#### 导入记忆

```bash
cis memory import --input memories.json

# 合并模式（不覆盖现有数据）
cis memory import --input backup.json --merge
```

### 统计信息

```bash
cis memory stats

# 按域分组
cis memory stats --by-domain

# 按类别分组
cis memory stats --by-category
```

---

## 能力管理

### 列出技能

```bash
cis skill list
```

### 安装技能

```bash
# 从路径安装
cis skill install ./skills/my-skill

# 从 URL 安装
cis skill install https://github.com/user/skill
```

### 加载/卸载技能

```bash
# 加载并激活
cis skill load my-skill --activate

# 仅加载
cis skill load my-skill

# 卸载
cis skill unload my-skill
```

### 激活/停用技能

```bash
cis skill activate my-skill
cis skill deactivate my-skill
```

### 查看技能信息

```bash
cis skill info my-skill
```

### 调用技能方法

```bash
cis skill call my-skill --method process --args '{"input": "data"}'
```

### 自然语言执行

```bash
# 语义匹配并执行技能
cis skill do "format my code"

# 显示候选技能
cis skill do "lint" --candidates

# 在项目上下文中执行
cis skill do "run tests" --project /path/to/project
```

### 技能链

```bash
# 发现并执行技能链
cis skill chain "review, test, and deploy my code"

# 预览模式（不实际执行）
cis skill chain "deploy" --preview

# 详细模式
cis skill chain "full workflow" --verbose
```

### 测试技能

```bash
cis skill test my-skill
```

---

## AI 交互

### 交互式对话

```bash
cis agent chat
```

**快捷键**：
- `Ctrl+C` - 发送消息
- `Ctrl+D` - 退出

### 发送提示词

```bash
cis agent prompt "Explain this code"
cis agent prompt "Help me debug this issue" --session my-session
```

### 带上下文执行

```bash
cis agent context "Continue from where we left off" --session my-session
```

### 查看可用 Agent

```bash
cis agent list
```

### 持久化 Agent

#### 配置持久化 Agent

编辑 `~/.cis/config.toml`:

```toml
[persistent_agent]
auto_start = true

[[persistent_agent.agents]]
name = "my-assistant"
runtime = "claude"
work_dir = "~/projects"
auto_restart = true
system_prompt = """
You are a helpful coding assistant.
Always check project memory before making changes.
"""
```

#### 启动 Agent

```bash
cis advanced persist start my-assistant
```

#### 附加到 Agent

```bash
cis agent attach my-assistant
```

#### 分离 Agent

在 Agent 会话中按 `Ctrl+D`。

#### Agent Pool 状态

```bash
cis agent pool status
```

---

## 工作流管理

### DAG 管理

#### 列出 DAG

```bash
cis workflow dag list
```

#### 查看 DAG 详情

```bash
cis workflow dag show my-workflow
```

#### 运行 DAG

```bash
cis workflow dag run my-workflow
```

#### 验证 DAG

```bash
cis workflow dag validate my-workflow
```

#### 查看 DAG 日志

```bash
cis workflow dag logs <execution-id>
```

### 任务管理

#### 创建任务

```bash
cis workflow task create --title "Review PR #123" \
  --description "Review and approve the changes" \
  --priority high
```

#### 列出任务

```bash
cis workflow task list

# 按状态过滤
cis workflow task list --status pending
cis workflow task list --status running
```

#### 更新任务

```bash
cis workflow task update <task-id> --status completed
```

#### 删除任务

```bash
cis workflow task delete <task-id>
```

### 决策管理

#### 配置决策级别

```bash
cis workflow decision configure mechanical --retry 3
cis workflow decision configure confirmed --timeout 300
```

### 执行历史

```bash
cis workflow history
cis workflow history --limit 50
```

---

## 网络管理

### P2P 管理

#### 启动 P2P

```bash
cis network p2p start
```

#### 查看 P2P 状态

```bash
cis network p2p status
```

#### 列出已连接的 Peer

```bash
cis network p2p peers
```

#### 连接到 Peer

```bash
cis network p2p dial /ip4/192.168.1.100/tcp/7677/p2p/12D3KooW...
```

### 节点管理

#### 列出节点

```bash
cis network node list
```

#### 查看节点信息

```bash
cis network node info <node-id>
```

#### 设置信任级别

```bash
cis network node trust <node-id> --level write
```

#### Ping 节点

```bash
cis network node ping <node-id>
```

### 快速配对

#### 生成配对码

```bash
cis network pair generate
```

**输出示例**：
```
Pairing code: 123-456

Share this code with the other node.
Valid for 5 minutes.
```

#### 使用配对码连接

```bash
cis network pair connect
Enter pairing code: 123-456
```

### 访问控制

#### 列出 ACL 规则

```bash
cis network acl list
```

#### 添加 ACL 规则

```bash
cis network acl add --peer <node-id> --trust write --expires 3600
```

#### 验证 ACL

```bash
cis network acl verify <node-id>
```

### Matrix 集成

#### 启动 Matrix 服务器

```bash
cis network matrix start
```

#### 查看 Matrix 状态

```bash
cis network matrix status
```

---

## 系统管理

### 路径管理

```bash
cis system paths
```

**输出**：
```
Config File:  ~/.cis/config.toml
Data Dir:     ~/.cis/data
Skills Dir:   ~/.cis/skills
Logs Dir:     ~/.cis/logs
```

### 创建目录

```bash
cis system dirs
```

### 数据迁移

```bash
cis system migrate --from v1.1.5 --to v1.1.6
```

### 清理数据

```bash
cis system cleanup --older-than 30d
```

### 更新 CIS

```bash
cis system update
cis system update --pre-release
```

### Worker 管理

#### 启动 Worker

```bash
cis system worker start
```

#### 查看 Worker 状态

```bash
cis system worker status
```

#### 停止 Worker

```bash
cis system worker stop
```

### 会话管理

#### 列出会话

```bash
cis system session list
```

#### 附加到会话

```bash
cis system session attach <session-id>
```

#### 终止会话

```bash
cis system session kill <session-id>
```

---

## 高级功能

### 技术债管理

```bash
cis advanced debt list
cis advanced debt add --description "Refactor this module"
```

### 任务级别管理

```bash
cis advanced task-level list
cis advanced task-level show mechanical
```

### GLM API

```bash
cis advanced glm start
cis advanced glm status
```

### 即时通讯

```bash
cis advanced im send --to <node-id> "Hello"
cis advanced im list
```

### 开发工具

#### 运行测试

```bash
cis advanced dev test
cis advanced dev test --package cis-core
```

#### 性能基准测试

```bash
cis advanced dev bench
cis advanced dev bench --filter vector_search
```

#### 性能分析

```bash
cis advanced dev profile --flamegraph
```

---

## 故障排查

### 常见错误

#### 1. "CIS not initialized"

**解决**：
```bash
cis core init
```

#### 2. "Permission denied"

**解决**：
```bash
# 检查权限
cis core doctor

# 修复权限
chmod 600 ~/.cis/config.toml
chmod 700 ~/.cis/data
```

#### 3. "Failed to connect to peer"

**解决**：
```bash
# 检查网络
cis network p2p status

# 检查防火墙
cis core doctor

# 尝试手动连接
cis network p2p dial <peer-address>
```

#### 4. "Memory not found"

**解决**：
```bash
# 搜索相似的键
cis memory search "partial-key"

# 列出所有键
cis memory list --prefix expected-prefix
```

### 调试模式

```bash
# 启用详细日志
RUST_LOG=debug cis <command>

# 保存日志到文件
RUST_LOG=debug cis <command> 2> cis.log
```

### 获取帮助

```bash
# 全局帮助
cis --help

# 命令组帮助
cis <group> --help

# 具体命令帮助
cis <group> <command> --help

# 在线文档
https://cis.dev/docs
```

### 报告问题

```bash
# 收集诊断信息
cis core doctor --verbose > diagnostics.txt

# 提交 Issue
https://github.com/cis-project/CIS/issues
```

---

## 附录

### 环境变量

| 变量 | 描述 |
|------|------|
| `CIS_CONFIG` | 配置文件路径 |
| `CIS_DATA` | 数据目录路径 |
| `CIS_HOME` | CIS 主目录（覆盖配置和数据路径） |
| `EDITOR` | 默认编辑器 |
| `RUST_LOG` | 日志级别（trace, debug, info, warn, error） |

### 配置文件

#### 全局配置

`~/.cis/config.toml`

```toml
[node]
name = "my-node"

[ai]
provider = "claude"
model = "claude-3-sonnet"

[p2p]
enabled = true
listen_port = 7677
```

#### 项目配置

`.cis/project.toml`

```toml
[project]
name = "my-project"

[ai]
guide = "You are working on my-project"

[memory]
namespace = "project/my-project"
```

### 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+C` | 中断当前操作 |
| `Ctrl+D` | 退出/分离 |
| `Ctrl+L` | 清屏 |
| `Ctrl+R` | 搜索历史 |
| `↑/↓` | 浏览历史 |

---

**文档版本**: 1.0
**最后更新**: 2026-02-12
**维护者**: CIS 团队
