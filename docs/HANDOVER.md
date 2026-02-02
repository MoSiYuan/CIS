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

**下一步**: 在 CIS 项目中实现 WASM Runtime 和 Host API
