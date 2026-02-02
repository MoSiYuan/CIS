# CIS 飞书 IM - 双模式运行指南

## ✅ 已完成

### 架构设计

CIS 飞书 IM Skill 现在支持三种运行模式：

#### 1. **轮询模式 (PollingOnly)**
- 主动拉取消息，无需公网暴露
- 随时关机友好，冷冻模式
- 适合本地开发和测试

#### 2. **Webhook 模式 (WebhookOnly)**
- 飞书主动推送消息，实时性更好
- 需要公网可访问的 URL（使用 ngrok 或类似工具）
- 适合生产环境

#### 3. **双模式 (Both)** ⭐ 推荐
- 同时运行轮询和 Webhook
- Webhook 提供实时性
- 轮询作为备份，保证可靠性
- 适合生产环境

---

## 🚀 快速开始

### 配置文件

编辑 `~/.cis/config/feishu_im.toml`:

```toml
# 飞书应用凭证
app_id = "cli_a90a99e490f95cc7"
app_secret = "bfwq9pZbbPNddQwU8MpKwdM2bZetMxas"

# 运行模式选择
runtime_mode = "both"  # polling_only / webhook_only / both

# 触发模式
trigger_mode = "private_and_at_mention"

# AI Provider
[ai_provider]
provider_type = "claude"

# 上下文配置
[context_config]
persist_context = true
max_turns = 20
context_timeout_secs = 1800
sync_to_memory = true

# 数据库路径
im_db_path = "~/.cis/data/feishu_im.db"
memory_db_path = "~/.cis/data/memory.db"

# 轮询配置（runtime_mode = polling_only 或 both 时需要）
[polling]
http_interval = 10
batch_size = 20
process_history = false
conversation_check_interval = 60

# Webhook 配置（runtime_mode = webhook_only 或 both 时需要）
[webhook]
bind_address = "0.0.0.0"
port = 6767
path = "/webhook/feishu"
encrypt_key = ""  # 从飞书开放平台获取
verify_token = ""  # 从飞书开放平台获取
verify_signature = true
```

---

## 📋 运行模式详解

### 模式 1: 仅轮询 (PollingOnly)

**优点**:
- ✅ 无需公网 IP
- ✅ 无需配置 Webhook
- ✅ 随时关机友好

**缺点**:
- ❌ 实时性较差（取决于轮询间隔）
- ❌ 可能错过离线消息

**配置**:
```toml
runtime_mode = "polling_only"
```

**飞书开放平台配置**:
- 不需要配置事件订阅
- 只需要申请 `im:message` 和 `im:chat` 权限
- 发布应用即可

---

### 模式 2: 仅 Webhook (WebhookOnly)

**优点**:
- ✅ 实时性最好（消息即时推送）
- ✅ 不会错过任何消息

**缺点**:
- ❌ 需要公网可访问的 URL
- ❌ 需要配置 ngrok 或类似工具
- ❌ 关机后消息堆积

**配置**:
```toml
runtime_mode = "webhook_only"

[webhook]
bind_address = "0.0.0.0"
port = 6767
path = "/webhook/feishu"
encrypt_key = "your_encrypt_key"
verify_token = "your_verify_token"
```

**飞书开放平台配置**:
1. 配置事件订阅
   - 请求网址: `https://your-domain.com/webhook/feishu`
   - 加密 Key: 系统生成
   - 验证 Token: 系统生成
2. 订阅事件:
   - `im.message.receive_v1` - 接收消息事件
3. 申请权限:
   - `im:message` - 发送消息
   - `im:chat` - 获取会话列表
4. 发布应用

**启动步骤**:
```bash
# 1. 启动 ngrok（如果本地测试）
ngrok http 6767

# 2. 复制 ngrok URL
# https://abc123.ngrok.io

# 3. 在飞书开放平台配置事件订阅
# 请求网址: https://abc123.ngrok.io/webhook/feishu

# 4. 启动 CIS
cargo run --example run_poller
```

---

### 模式 3: 双模式 (Both) ⭐ 推荐

**优点**:
- ✅ Webhook 提供实时性
- ✅ 轮询作为备份
- ✅ 高可靠性
- ✅ 兼具两者优势

**缺点**:
- ⚠️ 需要公网 IP（用于 Webhook）
- ⚠️ 资源消耗稍高

**配置**:
```toml
runtime_mode = "both"
```

**飞书开放平台配置**:
- 同时满足轮询和 Webhook 的要求
- 配置事件订阅（同 WebhookOnly）
- 申请权限（同 PollingOnly）

---

## 🎯 使用示例

### 代码中启动

```rust
use cis_feishu_im::FeishuImSkill;

// 创建 Skill
let mut skill = FeishuImSkill::with_config(config);

// 方式 1: 根据 runtime_mode 自动启动
skill.start().await?;

// 方式 2: 手动启动特定模式
skill.start_polling().await?;   // 仅轮询
skill.start_webhook().await?;   // 仅 Webhook
```

### 命令行启动

```bash
# 使用 run_poller 示例（自动根据配置选择模式）
cargo run --example run_poller

# 或直接使用 cargo run
cargo run
```

---

## 📊 模式选择建议

| 场景 | 推荐模式 | 原因 |
|------|---------|------|
| 本地开发 | `polling_only` | 无需公网，配置简单 |
| 内网部署 | `polling_only` | 内网环境无公网访问 |
| 生产环境 | `both` | 高可靠性，实时性好 |
| 测试环境 | `webhook_only` | 验证实时推送 |
| 离线设备 | `polling_only` | 随时关机友好 |

---

## 🔧 飞书开放平台配置

### 轮询模式 (PollingOnly)

1. **权限申请**
   - 访问 https://open.feishu.cn/app/cli_a90a99e490f95cc7/permission
   - 申请权限: `im:message`, `im:chat`

2. **发布应用**
   - 访问 https://open.feishu.cn/app/cli_a90a99e490f95cc7/release
   - 创建版本 0.1.0
   - 申请发布

3. **完成**
   - ✅ 不需要配置事件订阅

### Webhook 模式 (WebhookOnly)

1. **权限申请**（同上）

2. **事件订阅**
   - 访问 https://open.feishu.cn/app/cli_a90a99e490f95cc7/event
   - 配置请求网址: `https://your-domain.com/webhook/feishu`
   - 系统自动生成 Encrypt Key 和 Verify Token
   - 复制到配置文件

3. **订阅事件**
   - 订阅 `im.message.receive_v1`

4. **发布应用**（同上）

### 双模式 (Both)

同时完成上述两个步骤。

---

## 🐛 故障排查

### 问题 1: 轮询模式无法接收消息

**可能原因**:
- 应用未发布或未激活
- 权限未申请
- 飞书 API 限流

**解决方案**:
1. 检查应用发布状态
2. 检查权限是否已开通
3. 调整 `http_interval` 参数（默认 10 秒）

---

### 问题 2: Webhook 模式无法接收消息

**可能原因**:
- 公网 URL 无法访问
- Encrypt Key / Verify Token 配置错误
- 事件订阅未配置

**解决方案**:
1. 使用 curl 测试 Webhook URL 可访问性
2. 检查配置文件中的 `encrypt_key` 和 `verify_token`
3. 确认飞书开放平台已配置事件订阅
4. 检查 ngrok 或代理工具是否正常运行

---

### 问题 3: 双模式下消息重复

**现象**: 同一条消息收到两次

**原因**: Webhook 和轮询同时接收到同一条消息

**解决方案**:
- 这是正常的，消息处理逻辑会自动去重
- 会话管理器基于 `message_id` 去重
- 不会导致重复回复

---

## 📚 相关文档

- [会话管理集成](./SESSION_INTEGRATION.md)
- [架构设计 V2](./ARCHITECTURE_V2.md)
- [重构总结](../REFACTOR_SUMMARY.md)

---

## ✅ 总结

**当前状态**: 双模式架构已完成并测试通过

**下一步**:
1. 根据实际需求选择运行模式
2. 配置飞书开放平台（权限/事件订阅）
3. 测试消息收发
4. 配置会话管理

**推荐配置**: 生产环境使用 `runtime_mode = "both"`，获得最佳可靠性和实时性。
