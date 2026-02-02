# CIS Feishu IM Skill

飞书即时通讯集成 Skill，支持 AI 对话助手功能。

## 功能特性

- ✅ **Webhook 消息接收** - 实时接收飞书消息
- 🤖 **AI 对话响应** - 支持 Claude/Kimi AI Provider
- 💬 **多轮对话** - 完整的对话上下文管理
- 🔐 **数据分离** - IM 数据库与记忆数据库严格分离
- ⚙️ **灵活配置** - 支持多种触发模式

## 快速开始

### 1. 安装依赖

```bash
cd skills/cis-feishu-im
cargo build --features native
```

### 2. 配置飞书应用

1. 登录[飞书开放平台](https://open.feishu.cn/)
2. 创建应用，获取 `App ID` 和 `App Secret`
3. 配置事件订阅：
   - 请求地址 URL: `http://your-server:8080/webhook/feishu`
   - 订阅事件: `im.message.receive_v1`

### 3. 配置 CIS

创建配置文件 `~/.cis/config/feishu_im.toml`:

```toml
# 飞书应用配置
app_id = "cli_xxxxxxxxxxxxx"
app_secret = "xxxxxxxxxxxxxxxxxxxx"
encrypt_key = "xxxxxxxxxxxxxxxxxxxx"
verify_token = "xxxxxxxxxxxxxxxxxxxx"

# 对话触发模式
trigger_mode = "private_and_at_mention"  # 私聊自动响应 + @机器人

# AI Provider 配置
[ai_provider]
provider_type = "claude"  # 或 "kimi"

# 对话上下文配置
[context_config]
persist_context = true
max_turns = 20
context_timeout_secs = 1800  # 30 分钟

# Webhook 服务器配置
[webhook]
bind_address = "0.0.0.0"
port = 8080
path = "/webhook/feishu"
```

### 4. 启动服务

```bash
# 启动 cis-node
cd ../../cis-node
cargo run --features cis-feishu-im
```

## 架构设计

### 数据库分离

遵循 CIS 第一性原理，严格分离 IM 信息和记忆信息：

```
~/.cis/data/
├── feishu_im.db    # IM 信息数据库（临时通信数据）
│   ├── 对话历史
│   ├── 用户信息
│   ├── 群组信息
│   └── Webhook 日志
│
└── memory.db       # 记忆数据库（核心主权记忆）
    ├── 业务记忆
    ├── 项目知识
    ├── 技能经验
    └── 向量索引
```

### 触发模式

- `at_mention_only`: 仅 @ 机器人时响应
- `private_and_at_mention`: 私聊自动响应 + @机器人（推荐）
- `all`: 所有消息都响应

### AI Provider

支持多种 AI Provider：

- **Claude CLI**: Anthropic Claude，适合复杂推理
- **Kimi Code**: Moonshot AI Kimi，适合代码相关任务

## API 使用

### 作为 Skill 使用

```rust
use cis_feishu_im::FeishuImSkill;
use cis_skill_sdk::{Skill, SkillConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let mut skill = FeishuImSkill::new();

    // 初始化配置
    let config = SkillConfig::default();
    skill.init(config).await?;

    // 启动 Webhook 服务器
    skill.start_webhook().await?;

    Ok(())
}
```

### 直接集成到 cis-node

```rust
use cis_feishu_im::FeishuImSkill;

let mut feishu_skill = FeishuImSkill::with_config(config);
feishu_skill.init(skill_config).await?;
feishu_skill.start_webhook().await?;
```

## 开发

### 运行测试

```bash
# 单元测试
cargo test --features native

# 集成测试
cargo test --test '*' --features native

# 显示输出
cargo test --features native -- --nocapture
```

### 代码结构

```
src/
├── lib.rs          # Skill 主入口
├── config.rs       # 配置管理
├── context.rs      # 对话上下文管理
├── webhook.rs      # Webhook 服务器
└── feishu/
    └── mod.rs      # 飞书 API 封装
```

## 故障排查

### Webhook 无法接收消息

1. 检查飞书开放平台的事件订阅配置
2. 确认服务器可以公网访问（或使用 ngrok）
3. 查看日志：`tail -f ~/.cis/logs/feishu_im.log`

### AI 响应异常

1. 检查 AI Provider 配置
2. 测试 CLI 工具：`claude --version` 或 `kimi --version`
3. 查看 AI 调用日志

### 数据库错误

1. 确认数据目录存在：`ls -la ~/.cis/data/`
2. 检查数据库权限：`chmod 644 ~/.cis/data/*.db`
3. 查看数据库 schema：`sqlite3 ~/.cis/data/feishu_im.db ".schema"`

## 许可证

MIT License

## 相关链接

- [CIS 文档](https://github.com/your-org/CIS)
- [飞书开放平台](https://open.feishu.cn/)
- [cis-skill-sdk](../../cis-skill-sdk/)
- [cis-core](../../cis-core/)
