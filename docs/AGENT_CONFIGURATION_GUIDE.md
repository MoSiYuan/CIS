# CIS Agent 配置示例

## Agent 类型选择

CIS 支持以下 Agent 类型：

- **claude** - Claude Code CLI (默认)
- **kimi** - Kimi Code CLI
- **aider** - Aider CLI
- **opencode** - OpenCode CLI (开源)

## 配置方式

### 方式 1: 全局默认 Agent

在 `config.toml` 中设置：

```toml
[agent]
# 默认 Agent 类型: claude, kimi, aider, opencode
default_agent = "claude"

# 或者切换到 OpenCode
# default_agent = "opencode"

# 最大并发 Worker 数
max_workers = 4

# Task 超时时间（秒）
task_timeout_secs = 3600
```

### 方式 2: DAG 级别配置

在 DAG 配置文件中指定：

```toml
[dag]
name = "我的 DAG"
default_agent = "opencode"  # ← 使用 OpenCode

[[dag.tasks]]
id = "task1"
command = "实现一个登录功能"
agent = "claude"  # ← 这个任务使用 Claude

[[dag.tasks]]
id = "task2"
command = "测试登录功能"
agent = "opencode"  # ← 这个任务使用 OpenCode
```

### 方式 3: 命令行指定

```bash
# 使用指定 Agent 执行 DAG
cis dag run example-dag.toml --agent opencode

# 或者设置环境变量
export CIS_DEFAULT_AGENT=opencode
cis dag run example-dag.toml
```

## Agent 特定配置

### Claude 配置

```toml
[agent.claude]
model = "claude-sonnet-4-20250514"
max_tokens = 4096
temperature = 0.7
```

### OpenCode 配置

```toml
[agent.opencode]
model = "opencode/big-pickle"  # 免费模型

# 或者使用 Claude 模型（需要先通过 opencode auth 连接）
# model = "anthropic/claude-3-opus-20240229"

max_tokens = 4096
temperature = 0.7

# 可选：使用服务器模式（提高性能）
# server_url = "http://localhost:4096"
```

### Kimi 配置

```toml
[agent.kimi]
model = "kimi-k2.5-free"
max_tokens = 4096
temperature = 0.7
```

## CLI 命令

### 列出可用 Agent

```bash
cis agent list
```

输出：
```
可用的 Agent 类型:
  claude    - Claude Code CLI
  kimi      - Kimi Code CLI
  aider     - Aider CLI
  opencode  - OpenCode CLI (开源)
```

### 检查 Agent 是否可用

```bash
# 检查默认 Agent
cis agent check

# 检查指定 Agent
cis agent check opencode
```

输出：
```
检查 Agent: OpenCode
✅ OpenCode 可用
```

### 设置默认 Agent

```bash
cis agent set-default opencode
```

## 使用示例

### 示例 1: 使用 OpenCode 执行 DAG

```bash
# 临时使用 OpenCode
cis dag run my-dag.toml --agent opencode

# 永久设置默认为 OpenCode
cis agent set-default opencode
cis dag run my-dag.toml
```

### 示例 2: 混合使用多个 Agent

```toml
[dag]
name = "混合 Agent DAG"
default_agent = "claude"

[[dag.tasks]]
id = "design"
command = "设计数据库架构"
agent = "claude"  # 使用 Claude 进行设计

[[dag.tasks]]
id = "implement"
command = "实现数据库模型"
agent = "opencode"  # 使用 OpenCode 实现

[[dag.tasks]]
id = "test"
command = "编写测试用例"
agent = "claude"  # 使用 Claude 编写测试
```

### 示例 3: 根据任务复杂度选择 Agent

```bash
# 简单任务使用免费模型
CIS_DEFAULT_AGENT=opencode cis dag run simple-tasks.toml

# 复杂任务使用 Claude
CIS_DEFAULT_AGENT=claude cis dag run complex-tasks.toml
```

## 切换建议

### 推荐使用 Claude 的场景

- ✅ 需要高质量的代码生成
- ✅ 复杂的架构设计
- ✅ 需要深度推理
- ✅ 有 Claude 订阅

### 推荐使用 OpenCode 的场景

- ✅ 想要使用免费模型
- ✅ 需要多模型支持
- ✅ 想要完全开源的解决方案
- ✅ 需要跨模型切换

## 故障排查

### 问题：Agent 不可用

```bash
cis agent check opencode
```

如果显示不可用，请检查：

1. Agent 是否已安装
2. Agent 是否在 PATH 中
3. Agent 是否有正确的权限

### 问题：配置未生效

确认配置文件路径：

```bash
# 查看当前配置
cis config show

# 验证配置文件
cis config validate
```

## 迁移指南

### 从 Claude 迁移到 OpenCode

1. **安装 OpenCode**
   ```bash
   brew install anomalyco/tap/opencode
   ```

2. **测试 OpenCode**
   ```bash
   cis agent check opencode
   ```

3. **在测试 DAG 中试用**
   ```bash
   cis dag run test-dag.toml --agent opencode
   ```

4. **设置默认 Agent**
   ```bash
   cis agent set-default opencode
   ```

5. **回退（如果需要）**
   ```bash
   cis agent set-default claude
   ```

---

**文档结束**
