# CLI 重构快速参考

> **Team M | v1.1.6 | 2026-02-12**

## 新旧命令对照

| 旧命令 | 新命令 | 说明 |
|--------|--------|------|
| `cis init` | `cis core init` | 初始化 CIS |
| `cis status` | `cis core status` | 查看状态 |
| `cis doctor` | `cis core doctor` | 诊断 |
| `cis memory get <key>` | `cis memory get <key>` | 获取记忆 |
| `cis memory set <key> <val>` | `cis memory set <key> <val>` | 设置记忆 |
| `cis skill list` | `cis skill list` | 列出技能 |
| `cis agent chat` | `cis agent chat` | 交互对话 |
| `cis p2p status` | `cis network p2p status` | P2P 状态 |
| `cis dag run <name>` | `cis workflow dag run <name>` | 运行 DAG |

## 命令组概览

```
cis
├── core          # 核心: init, status, config, doctor
├── memory        # 记忆: get, set, search, vector
├── skill         # 技能: list, load, do, chain
├── agent         # AI: chat, prompt, attach
├── workflow      # 工作流: dag, task, decision
├── network       # 网络: p2p, node, pair
├── system        # 系统: paths, update, worker
└── advanced      # 高级: debt, dev, im
```

## 常用命令

### 初始化
```bash
cis core init --non-interactive
cis core init --project
```

### 配置
```bash
cis core config get user.name
cis core config set ai.provider claude
cis core config list
```

### 记忆
```bash
cis memory set user.preference.theme dark
cis memory get user.preference.theme
cis memory vector "what theme did i prefer"
```

### 技能
```bash
cis skill list
cis skill do "format my code"
cis skill chain "test and deploy"
```

### 网络
```bash
cis network p2p status
cis network pair generate
cis network node info <id>
```

## 错误处理示例

```bash
$ cis memory get nonexistent
❌ Error: Memory not found: nonexistent

Suggestions:
  1. Search for similar keys: cis memory search "nonexistent"
  2. List all keys: cis memory list
  3. Check for typos

Details: No memory entry with key "nonexistent"
```

## 输出格式

```bash
# Plain (默认)
cis memory list --format plain

# JSON
cis memory list --format json

# Table
cis memory list --format table
```

## 关键文件

```
cis-node/src/cli/
├── command.rs     # Command trait
├── registry.rs    # 命令注册
├── error.rs      # 错误处理
├── groups/       # 命令组定义
└── handlers/     # 命令处理器

docs/
├── plan/v1.1.6/cli_refactoring_design.md  # 完整设计
├── user/cli-usage.md                    # 用户手册
└── plan/v1.1.6/TEAM_M_COMPLETION_REPORT.md  # 完成报告
```

## 后续工作

1. 实现剩余命令处理器 (~3 天)
2. 编写集成测试 (~1 天)
3. 更新 main.rs (~0.5 天)
4. 性能测试 (~0.5 天)

**总预计剩余时间**: 5 天
