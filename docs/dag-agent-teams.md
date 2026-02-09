# DAG Agent Teams 配置指南

## 概述

CIS 现在支持在 DAG 任务中配置 Agent Runtime，允许你为不同的任务指定不同的 AI Agent，或复用已有的 Agent 实例。

## 配置字段

### DagTaskDefinition 新增字段

```rust
pub struct DagTaskDefinition {
    // ... 原有字段 ...
    
    /// 指定使用的 Agent Runtime
    pub agent_runtime: Option<RuntimeType>,
    
    /// 复用已有 Agent ID（同 DAG 内）
    pub reuse_agent: Option<String>,
    
    /// 是否保持 Agent（执行后不销毁）
    pub keep_agent: bool,
    
    /// Agent 配置（创建新 Agent 时用）
    pub agent_config: Option<AgentConfig>,
}
```

### RuntimeType 枚举

```rust
pub enum RuntimeType {
    Claude,    // Claude Code
    Kimi,      // Kimi Code
    Aider,     // Aider
    OpenCode,  // OpenCode
    Default,   // 使用 DAG 默认配置
}
```

## 使用示例

### 示例 1: 为任务指定特定 Agent

```toml
[[dag.tasks]]
id = "analyze"
name = "代码分析"
skill = "code-analyze"
level = { type = "mechanical", retry = 3 }
agent_runtime = "claude"  # 使用 Claude Code
keep_agent = true         # 执行后保持 Agent 运行
```

### 示例 2: 复用已有的 Agent

```toml
# 第一个任务创建 Agent
[[dag.tasks]]
id = "init"
name = "初始化"
skill = "agent-init"
agent_runtime = "kimi"
keep_agent = true

# 后续任务复用同一个 Agent
[[dag.tasks]]
id = "process"
name = "处理数据"
skill = "data-process"
deps = ["init"]
reuse_agent = "init"  # 复用 init 任务的 Agent
```

### 示例 3: 自定义 Agent 配置

```toml
[[dag.tasks]]
id = "complex-analysis"
name = "复杂分析"
skill = "complex-analyze"
agent_runtime = "claude"
keep_agent = true

[dag.tasks.agent_config]
provider_type = "claude"
model = "claude-3-5-sonnet-20241022"
timeout_secs = 600
max_tokens = 8192
temperature = 0.5
```

### 示例 4: 多 Agent 协作

```toml
[[dag.tasks]]
id = "security-scan"
name = "安全扫描"
skill = "security-scan"
agent_runtime = "kimi"  # Kimi 负责安全分析
keep_agent = true

[[dag.tasks]]
id = "performance-check"
name = "性能检查"
skill = "perf-check"
agent_runtime = "opencode"  # OpenCode 负责性能分析
keep_agent = true

[[dag.tasks]]
id = "report"
name = "生成报告"
skill = "report-gen"
deps = ["security-scan", "performance-check"]
# 复用 security-scan 的 Agent 生成报告
reuse_agent = "security-scan"
keep_agent = false  # 最后销毁 Agent
```

## 向后兼容

所有新字段都有默认值，现有 YAML/TOML 配置无需修改即可正常工作：

- `agent_runtime`: `None` (使用 DAG 默认配置)
- `reuse_agent`: `None` (不复用 Agent)
- `keep_agent`: `false` (执行后销毁 Agent)
- `agent_config`: `None` (使用默认配置)

## DagNode 结构更新

内部 `DagNode` 结构也同步添加了这些字段，支持运行时 Agent 管理：

```rust
pub struct DagNode {
    // ... 原有字段 ...
    pub agent_runtime: Option<RuntimeType>,
    pub reuse_agent: Option<String>,
    pub keep_agent: bool,
    pub agent_config: Option<AgentConfig>,
}
```

## From 转换

支持从 `DagTask` 到 `DagNode` 的转换：

```rust
impl From<DagTask> for DagNode {
    fn from(task: DagTask) -> Self {
        Self {
            task_id: task.task_id,
            dependencies: task.dependencies,
            // ...
            agent_runtime: task.agent_runtime,
            reuse_agent: task.reuse_agent,
            keep_agent: task.keep_agent,
            agent_config: task.agent_config,
        }
    }
}
```

## 最佳实践

1. **Agent 复用**: 对于一系列相关的任务，建议复用同一个 Agent 以保持一致性和性能
2. **资源管理**: 只在需要长时间保持上下文时设置 `keep_agent = true`
3. **任务完成**: 最后一个复用链上的任务应该设置 `keep_agent = false` 来清理资源
4. **差异化**: 不同的专业任务可以使用不同的 Agent Runtime（如 Kimi 做安全分析，Claude 做代码审查）
