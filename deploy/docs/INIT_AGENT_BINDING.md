# CIS Init AI Agent 绑定指南

## 概述

`cis init` 命令现在默认将 **OpenCode** 作为推荐的 AI Agent，以支持更好的 DAG 任务执行体验。

## 变更摘要

### 1. 默认 Provider 变更

| 版本 | 默认 Provider |
|------|--------------|
| 之前 | Claude CLI |
| 现在 | **OpenCode** |

### 2. 初始化向导更新

**交互式选择：**
```
选择默认 AI Provider:
  1) OpenCode (推荐，DAG 任务优化)  ← 默认选项
  2) Claude CLI
  3) Kimi Code
  4) Aider
```

**自动检测优先级：**
1. OpenCode (最高优先级)
2. Claude CLI
3. Kimi Code
4. Aider

### 3. 生成的配置模板

```toml
[ai]
# 默认 AI Provider: opencode | claude | kimi | aider
default_provider = "opencode"

[ai.opencode]
# OpenCode 配置（DAG 任务推荐）
# 可用模型：
#   - opencode/glm-4.7-free (免费)
#   - opencode/kimi-k2.5-free (免费)
#   - opencode/gpt-5-nano (免费)
#   - anthropic/claude-3-opus-20240229 (付费)
#   - openai/gpt-4 (付费)
model = "opencode/glm-4.7-free"
max_tokens = 4096
temperature = 0.7

[ai.claude]
# Claude Code 配置
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7

[ai.kimi]
# Kimi Code 配置
model = "kimi-k2"
max_tokens = 8192
```

## 使用方式

### 标准初始化（交互式）

```bash
cis init
# 将提示选择 AI Provider，默认推荐 OpenCode
```

### 指定 Provider

```bash
# 通过命令行参数指定
cis init --provider opencode

# 或使用环境变量
CIS_PROVIDER=opencode cis init
```

### 非交互式初始化

```bash
# 使用默认配置（自动检测或默认 OpenCode）
cis init --non-interactive

# 强制覆盖现有配置
cis init --force
```

### 项目级初始化

```bash
# 在当前目录初始化项目配置
cis init --project
```

## 为什么选择 OpenCode？

| 特性 | OpenCode | Claude | Kimi |
|------|----------|--------|------|
| **无头模式** | ✅ 原生支持 | ❌ 需要 PTY 包装 | ✅ 支持 |
| **权限确认** | ✅ 无需确认 | ❌ 需要交互绕过 | ✅ 支持 |
| **DAG 优化** | ✅ 默认推荐 | ⚠️ 需要额外配置 | ⚠️ 需要额外配置 |
| **免费模型** | ✅ 多个选择 | ❌ 付费 | ⚠️ 部分免费 |
| **输出格式** | JSON 流 | 纯文本 | 纯文本 |

## 向后兼容

### 现有用户

现有配置文件无需修改，系统会继续使用已配置的 Provider。

### 回退到 Claude

如需将默认 Provider 改回 Claude：

```bash
# 编辑配置文件
vim ~/.cis/config.toml

# 修改 default_provider
[ai]
default_provider = "claude"
```

或在初始化时显式选择：

```bash
cis init
# 选择选项 2) Claude CLI
```

## 故障排查

### OpenCode 未安装

```bash
# 检查是否安装
opencode --version

# 安装 OpenCode
# （请参考 OpenCode 官方安装文档）
```

### 切换 Provider

```bash
# 查看当前配置
cat ~/.cis/config.toml | grep default_provider

# 手动修改配置文件
# 或使用向导重新初始化
cis init --force
```

### 验证配置

```bash
# 检查 AI Provider 是否可用
cis doctor

# 测试 Agent 连接
cis agent test
```

## 相关文档

- [OpenCode Provider 实现](../docs/OPENCODE_PROVIDER_IMPLEMENTATION.md)
- [DAG 迁移指南](DAG_MIGRATION_GUIDE.md)
- [Agent 配置指南](../docs/AGENT_CONFIGURATION_GUIDE.md)
