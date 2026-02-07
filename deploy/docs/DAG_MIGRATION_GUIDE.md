# DAG Agent 迁移指南：Claude → OpenCode

## 概述

CIS 已将 DAG 执行的默认 Agent 从 **Claude** 迁移到 **OpenCode**，以获得更好的无头模式支持和成本效益。

## 迁移原因

| 特性 | Claude | OpenCode |
|------|--------|----------|
| **无头模式** | 需要 PTY 包装 (`script -q`) | 原生支持 |
| **权限确认** | 默认需要交互确认 | 无需确认 |
| **输出格式** | 纯文本 | JSON 事件流 |
| **模型选择** | Anthropic 模型 | 多供应商（包括免费模型） |
| **成本** | 付费 | 支持免费模型 |

## 自动迁移

### 1. 默认配置已更新

`AgentClusterConfig::default()` 现在使用 `AgentType::OpenCode`：

```rust
impl Default for AgentClusterConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            default_agent: AgentType::OpenCode,  // ← 已更改
            // ...
        }
    }
}
```

### 2. DAG 执行命令

```bash
# 使用默认 Agent (OpenCode) 执行 DAG
cis dag execute --run-id <run-id> --use-agent

# 或明确指定使用 Agent
cis dag execute --run-id <run-id> --use-agent --agent opencode
```

## 手动回退（如需使用 Claude）

如果某些任务仍需要使用 Claude：

```rust
use cis_core::agent::cluster::{AgentClusterConfig, AgentClusterExecutor};
use cis_core::agent::AgentType;

// 显式使用 Claude
let config = AgentClusterConfig {
    default_agent: AgentType::Claude,
    ..Default::default()
};

let executor = AgentClusterExecutor::new(config)?;
```

## 配置文件更新

### 新的 OpenCode 配置

```toml
# ~/.cis/config.toml

[ai]
default_provider = "opencode"

[ai.opencode]
model = "opencode/glm-4.7-free"  # 或其他模型
max_tokens = 4096
temperature = 0.7

[dag]
agent_type = "opencode"
max_workers = 4
```

### 可用模型列表

| 模型 | 费用 | 推荐用途 |
|------|------|----------|
| `opencode/glm-4.7-free` | 免费 | 日常任务 |
| `opencode/kimi-k2.5-free` | 免费 | 中文任务 |
| `opencode/gpt-5-nano` | 免费 | 轻量级任务 |
| `opencode/big-pickle` | 免费 | 代码任务 |
| `anthropic/claude-3-opus` | 付费 | 复杂任务 |
| `openai/gpt-4` | 付费 | 复杂任务 |

## 代码兼容性

### 向后兼容

现有代码无需修改即可工作：

```rust
// 旧代码 - 仍然有效
let executor = AgentClusterExecutor::default_executor()?;
// 现在默认使用 OpenCode
```

### 显式 Agent 选择

```rust
// 如果任务需要特定 Agent
let config = AgentClusterConfig {
    default_agent: AgentType::Claude,  // 或 Kimi, Aider
    ..Default::default()
};
```

## 测试验证

### 运行迁移测试

```bash
cd cis-core
cargo test opencode_migration
```

### 手动测试

```bash
# 创建测试 DAG
cis dag run examples/single_node_simulation.toml --paused

# 查看 DAG 运行 ID
cis dag list

# 使用 Agent 执行（默认 OpenCode）
cis dag execute --run-id <run-id> --use-agent

# 查看执行状态
cis dag status --run-id <run-id> --verbose
```

## 故障排查

### 问题：OpenCode CLI 未安装

```bash
# 检查安装
opencode --version

# 安装（如果未安装）
# 参考 OpenCode 官方文档
```

### 问题：模型不可用

```toml
# 切换模型
[ai.opencode]
model = "opencode/kimi-k2.5-free"  # 尝试其他模型
```

### 问题：JSON 解析失败

OpenCode Provider 已处理非 JSON 输出，无需担心。

## 总结

| 方面 | 说明 |
|------|------|
| **默认 Agent** | OpenCode |
| **向后兼容** | ✅ 支持 |
| **配置方式** | 自动（默认）或显式 |
| **免费模型** | ✅ 支持 |
| **无头模式** | ✅ 原生支持 |

---

**注意**：如需帮助或遇到问题，请查看 `cis-core/src/agent/cluster/opencode_migration_test.rs` 中的测试示例。
