# CIS Vector Intelligence 使用指南

## 安装

```bash
cargo install --path cis-node
```

## 快速开始

### 1. 初始化 CIS

```bash
# 交互式初始化
cis init

# 非交互式初始化
cis init --non-interactive --provider claude
```

### 2. 使用自然语言调用 Skill

```bash
# 分析今天的销售数据
cis skill do "分析今天的销售数据"

# 显示候选技能
cis skill do "优化数据库查询" --candidates

# 预览 Skill Chain
cis skill chain "分析数据并生成报告" --preview
```

### 3. 语义搜索记忆

```bash
# 基本搜索
cis memory search "暗黑模式"

# 限制结果数量
cis memory search "用户偏好" --limit 10

# 设置相似度阈值
cis memory search "配置" --threshold 0.8

# JSON 输出
cis memory search "配置" --format json

# Table 输出
cis memory search "配置" --format table
```

### 4. 带上下文的 AI 对话

```bash
# 使用当前会话上下文
cis agent context "如何优化查询？"

# 指定会话
cis agent context "解释这段代码" --session abc123
```

### 5. 查看遥测日志

```bash
# 查看最近日志
cis telemetry logs

# 限制数量
cis telemetry logs --limit 50

# 只显示成功请求
cis telemetry logs --success-only

# 查看会话统计
cis telemetry stats
```

## 高级用法

### 私域/公域记忆

```rust
use cis_core::memory::{MemoryService, MemoryDomain, MemoryCategory};

let service = MemoryService::open_default("node-1")?;

// 私域记忆（加密，不同步）
service.set(
    "private-key",
    b"secret data",
    MemoryDomain::Private,
    MemoryCategory::Context,
)?;

// 公域记忆（明文，可同步）
service.set(
    "public-key",
    b"shared knowledge",
    MemoryDomain::Public,
    MemoryCategory::Result,
)?;
```

### Skill Chain 编排

```rust
use cis_core::skill::router::SkillVectorRouter;

let router = SkillVectorRouter::new(storage, embedding);

// 发现 Skill Chain
let chain = router.discover_skill_chain("data-analyzer", &intent).await?;

// 执行 Chain
let result = router.execute_chain(&chain, &params).await?;
```

### RAG 集成

```rust
use cis_core::conversation::ConversationContext;
use cis_core::ai::AiProvider;

// 创建对话上下文
let mut ctx = ConversationContext::new();
ctx.add_user_message("我喜欢暗黑模式").await?;

// 使用 RAG 查询
let response = ai.chat_with_rag("推荐什么主题？", Some(&ctx)).await?;
```

## 配置

### 配置文件位置

- 全局配置: `~/.cis/config.toml`
- 项目配置: `.cis/project.toml`

### 配置示例

```toml
[ai]
provider = "claude"  # 或 "kimi", "aider"

[vector]
embedding_dim = 768
use_hnsw = true

[memory]
encryption_key = "your-key"
```

## 故障排除

### 常见问题

1. **向量搜索慢**
   - 确保启用了 HNSW 索引: `use_hnsw = true`
   - 检查数据库大小，考虑清理旧向量

2. **Skill 匹配不准确**
   - 调整置信度阈值
   - 检查 Skill 语义描述是否完整

3. **内存使用过高**
   - 限制批量向量化大小
   - 定期清理 telemetry 日志
