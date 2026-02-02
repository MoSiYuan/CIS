# 技术设计: cis-feishu-im Skill

## 方案对比

### 方案 A: Webhook Server (推荐)

**架构**:
```
飞书服务器
    ↓ Webhook 事件
cis-node HTTP 服务器
    ↓ 解析事件
FeishuImSkill (处理逻辑)
    ↓ 调用 AI
cis-core::ai::AiProvider
    ↓ 生成回复
飞书服务器 (发送回复)
```

**优点**:
- ✅ 实时性好（Webhook 主动推送）
- ✅ 飞书官方推荐模式
- ✅ 资源占用低（无轮询）
- ✅ 与 cis-node 集成简单

**缺点**:
- ⚠️ 需要公网可访问的 Webhook URL
- ⚠️ 需要配置 NAT 端口转发（本地开发）

**适用场景**: 生产环境、有固定服务器

---

### 方案 B: 轮询模式

**架构**:
```
cis-node 定时器
    ↓ 拉取消息
飞书 API (list_messages)
    ↓ 处理新消息
FeishuImSkill
    ↓ 同方案 A
```

**优点**:
- ✅ 无需公网 IP
- ✅ 部署简单

**缺点**:
- ❌ 实时性差（轮询延迟）
- ❌ 浪费 API 配额
- ❌ 可能漏掉消息

**适用场景**: 内网环境、测试环境

---

### 方案 C: 混合模式

**架构**:
- 生产环境: Webhook 模式
- 开发环境: 轮询模式

**优点**:
- ✅ 兼顾两种场景
- ✅ 灵活切换

**缺点**:
- ⚠️ 代码复杂度增加

---

## 推荐方案

**选择**: **方案 A (Webhook Server)**
**理由**:
1. 符合飞书官方最佳实践
2. 实时性最优（用户体验好）
3. 资源效率高
4. 开发环境可通过内网穿透工具（如 ngrok）解决

**开发环境解决方案**:
- 使用 [ngrok](https://ngrok.com) 或 [localtunnel](https://localtunnel.github.io/) 提供公网 URL
- 或在 CIS 中集成内网穿透功能（作为可选 Skill）

---

## 数据结构设计

### 1. 配置结构

```rust
use cis_core::ai::{AiProviderConfig, ProviderType};
use serde::{Deserialize, Serialize};

/// 飞书 IM Skill 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuImConfig {
    /// 飞书 App ID
    pub app_id: String,

    /// 飞书 App Secret（用于签名验证）
    pub app_secret: String,

    /// 飞书 Encrypt Key（用于解密消息）
    pub encrypt_key: String,

    /// Webhook 验证签名（安全选项）
    pub verify_signature: bool,

    /// 对话触发模式
    pub trigger_mode: TriggerMode,

    /// AI Provider 配置
    pub ai_provider: AiProviderConfig,

    /// 对话上下文配置
    pub context_config: ContextConfig,
}

/// 对话触发模式
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum TriggerMode {
    /// 仅 @ 机器人时响应
    AtMentionOnly,
    /// 私聊自动响应 + @机器人
    PrivateAndAtMention,
    /// 所有消息都响应
    All,
}

/// 对话上下文配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// 是否持久化对话历史
    pub persist_context: bool,

    /// 最大对话轮次（超过后清空上下文）
    pub max_turns: usize,

    /// 上下文超时时间（秒）
    pub context_timeout_secs: u64,
}

impl Default for FeishuImConfig {
    fn default() -> Self {
        Self {
            app_id: String::new(),
            app_secret: String::new(),
            encrypt_key: String::new(),
            verify_signature: true,
            trigger_mode: TriggerMode::PrivateAndAtMention,
            ai_provider: AiProviderConfig::default(),
            context_config: ContextConfig {
                persist_context: true,
                max_turns: 20,
                context_timeout_secs: 1800, // 30 分钟
            },
        }
    }
}
```

---

### 2. 消息处理结构

```rust
use cis_core::ai::{Message, Role};

/// 飞书事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeishuEvent {
    /// 收到消息
    MessageReceived(FeishuMessage),
    /// 用户加入群组
    UserJoined(FeishuUserEvent),
    /// 用户离开群组
    UserLeft(FeishuUserEvent),
}

/// 飞书消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    /// 消息 ID
    pub message_id: String,

    /// 消息类型
    pub msg_type: String,

    /// 发送者信息
    pub sender: FeishuSender,

    /// 消息内容
    pub content: FeishuContent,

    /// 群聊信息（如果是群聊）
    pub chat_type: Option<String>,

    /// 是否被 @
    pub is_at: bool,
}

/// 飞书发送者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSender {
    /// 用户 ID
    pub user_id: String,

    /// 用户名
    pub name: String,
}

/// 飞书消息内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuContent {
    /// 文本内容
    pub text: Option<String>,

    /// 富文本内容
    pub post: Option<FeishuPost>,

    /// 卡片内容
    pub card: Option<serde_json::Value>,
}

/// 飞书富文本
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuPost {
    /// 富文本内容
    pub zh_cn: FeishuPostContent,
}

/// 飞书富文本内容
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuPostContent {
    pub title: Option<String>,
    pub content: Vec<FeishuTextElement>,
}

/// 飞书文本元素
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tag")]
pub enum FeishuTextElement {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "a")]
    Link { text: String, href: String },

    #[serde(rename = "at")]
    At { user_id: String, name: String },
}

impl FeishuMessage {
    /// 提取用户消息文本
    pub fn extract_text(&self) -> String {
        if let Some(ref text) = self.content.text {
            return text.clone();
        }

        if let Some(ref post) = self.content.post {
            let mut result = String::new();
            for elem in &post.zh_cn.content {
                match elem {
                    FeishuTextElement::Text { text } => result.push_str(text),
                    FeishuTextElement::Link { text, .. } => result.push_str(text),
                    FeishuTextElement::At { name, .. } => result.push_str(&format!("@{}", name)),
                }
            }
            return result;
        }

        String::new()
    }

    /// 转换为 AI Message
    pub fn to_ai_message(&self) -> Message {
        Message::user(self.extract_text())
    }
}

/// 飞书用户事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuUserEvent {
    pub user_ids: Vec<String>,
    pub chat_id: String,
}
```

---

### 3. 对话上下文

```rust
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// 对话会话
#[derive(Debug, Clone)]
pub struct ConversationSession {
    /// 会话 ID（通常是 chat_id）
    pub session_id: String,

    /// 对话历史
    pub messages: Vec<Message>,

    /// 创建时间
    pub created_at: Instant,

    /// 最后活跃时间
    pub last_active: Instant,
}

/// 对话上下文管理器
pub struct ConversationContext {
    /// 所有会话（session_id -> session）
    sessions: RwLock<HashMap<String, ConversationSession>>,

    /// 配置
    config: ContextConfig,
}

impl ConversationContext {
    pub fn new(config: ContextConfig) -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// 添加消息到会话
    pub async fn add_message(&self, session_id: &str, message: Message) {
        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(session_id.to_string())
            .or_insert_with(|| ConversationSession {
                session_id: session_id.to_string(),
                messages: Vec::new(),
                created_at: Instant::now(),
                last_active: Instant::now(),
            });

        session.messages.push(message);
        session.last_active = Instant::now();

        // 清理过期会话
        self.cleanup_expired_sessions(&mut sessions).await;
    }

    /// 获取会话历史（用于 AI 上下文）
    pub async fn get_history(&self, session_id: &str) -> Vec<Message> {
        let sessions = self.sessions.read().await;

        if let Some(session) = sessions.get(session_id) {
            return session.messages.clone();
        }

        Vec::new()
    }

    /// 清理过期会话
    async fn cleanup_expired_sessions(&self, sessions: &mut HashMap<String, ConversationSession>) {
        let timeout = Duration::from_secs(self.config.context_timeout_secs);
        let now = Instant::now();

        sessions.retain(|_, session| {
            now.duration_since(session.last_active) < timeout
        });

        // 如果会话消息过多，保留最近的 N 条
        for session in sessions.values_mut() {
            if session.messages.len() > self.config.max_turns {
                let start = session.messages.len() - self.config.max_turns;
                session.messages = session.messages.split_off(start).collect();
            }
        }
    }
}
```

---

## API 接口设计

### 1. Skill Trait (统一接口)

```rust
use cis_core::{CisError, Result};
use cis_core::ai::AiProvider;
use async_trait::async_trait;

/// 飞书 IM Skill Trait
#[async_trait]
pub trait FeishuImSkill: Send + Sync {
    /// 启动 Webhook 服务器
    async fn start(&self, config: FeishuImConfig) -> Result<()>;

    /// 停止 Webhook 服务器
    async fn stop(&self) -> Result<()>;

    /// 处理飞书事件
    async fn handle_event(&self, event: FeishuEvent) -> Result<()>;

    /// 检查运行状态
    async fn is_running(&self) -> bool;
}
```

### 2. Webhook Handler

```rust
use axum::{
    extract::{Path as AxumPath, State},
    http::StatusCode,
    Json,
};
use serde_json::Value;

/// Webhook 路由处理器
pub struct WebhookHandler {
    config: FeishuImConfig,
    ai_provider: Box<dyn AiProvider>,
    context: ConversationContext,
}

impl WebhookHandler {
    /// 处理飞书 Webhook 请求
    pub async fn handle_webhook(
        &self,
        payload: Value,
    ) -> Result<Value> {
        // 1. 验证签名
        if self.config.verify_signature {
            self.verify_signature(&payload)?;
        }

        // 2. 解析事件
        let event = Self::parse_event(payload)?;

        // 3. 处理事件
        self.process_event(event).await?;

        // 4. 返回成功响应
        Ok(json!({
            "code": 0,
            "msg": "success"
        }))
    }

    /// 验证飞书签名
    fn verify_signature(&self, payload: &Value) -> Result<()> {
        // TODO: 实现签名验证逻辑
        // 参考: https://open.feishu.cn/document/ukTMukTMukTM/uEjNwUjLxYDOxTMzYjLxjN
        Ok(())
    }

    /// 解析飞书事件
    fn parse_event(payload: Value) -> Result<FeishuEvent> {
        // TODO: 解析 JSON 为 FeishuEvent
        Ok(FeishuEvent::MessageReceived(/* ... */))
    }
}

/// 实现事件处理
impl WebhookHandler {
    async fn process_event(&self, event: FeishuEvent) -> Result<()> {
        match event {
            FeishuEvent::MessageReceived(msg) => {
                self.handle_message(msg).await?;
            }
            FeishuEvent::UserJoined(event) => {
                tracing::info!("User joined: {:?}", event.user_ids);
            }
            FeishuEvent::UserLeft(event) => {
                tracing::info!("User left: {:?}", event.user_ids);
            }
        }
        Ok(())
    }

    async fn handle_message(&self, msg: FeishuMessage) -> Result<()> {
        // 1. 检查是否应该响应
        if !self.should_respond(&msg) {
            return Ok(());
        }

        // 2. 获取会话历史
        let session_id = msg.get_session_id();
        let history = self.context.get_history(&session_id).await;

        // 3. 构建 AI 对话请求
        let mut messages = history;
        messages.push(msg.to_ai_message());

        // 4. 调用 AI Provider
        let system_prompt = "你是 CIS 飞书机器人助手，负责回答用户问题...";
        let response = self.ai_provider
            .chat_with_context(system_prompt, &messages)
            .await?;

        // 5. 保存对话历史
        self.context.add_message(&session_id, msg.to_ai_message()).await;
        self.context.add_message(&session_id, Message::assistant(response.clone())).await;

        // 6. 发送回复到飞书
        self.send_reply(&msg, &response).await?;

        Ok(())
    }

    /// 判断是否应该响应
    fn should_respond(&self, msg: &FeishuMessage) -> bool {
        match self.config.trigger_mode {
            TriggerMode::AtMentionOnly => msg.is_at,
            TriggerMode::PrivateAndAtMention => {
                msg.is_at || msg.chat_type.as_deref() == Some("p2p")
            }
            TriggerMode::All => true,
        }
    }

    /// 发送回复到飞书
    async fn send_reply(&self, original_msg: &FeishuMessage, reply: &str) -> Result<()> {
        // TODO: 调用飞书 API 发送消息
        // 使用 larkrs-client SDK
        tracing::info!("Sending reply to chat_id: {:?}", original_msg.chat_type);
        Ok(())
    }
}
```

---

## 依赖配置

### Cargo.toml

```toml
[package]
name = "cis-feishu-im"
version = "0.1.0"
edition = "2021"

[lib]
name = "cis_feishu_im"
path = "src/lib.rs"

[dependencies]
cis-core = { path = "../../cis-core" }
larkrs-client = "0.1"  # 飞书 Rust SDK
tokio = { version = "1", features = ["full"] }
axum = "0.7"        # HTTP 服务器
tower = "0.4"       # 中间件
tower-http = "0.4"  # HTTP 特定中间件
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
thiserror = "1.0"

[dev-dependencies]
tokio-test = "0.4"
```

---

## 部署架构

### 集成到 cis-node

```rust
// cis-node/src/main.rs
use cis_feishu_im::FeishuImSkillImpl;

#[tokio::main]
async fn main() -> Result<()> {
    // ... 其他初始化

    // 启动飞书 IM Skill
    let feishu_skill = FeishuImSkillImpl::new(config.feishu_im)?;
    feishu_skill.start().await?;

    // ... 其他逻辑

    Ok(())
}
```

---

## 安全考虑

### 1. Webhook 签名验证

```rust
fn verify_webhook_signature(
    payload: &str,
    signature: &str,
    timestamp: &str,
    nonce: &str,
    secret: &str,
) -> bool {
    // 1. 检查时间戳（防重放）
    let now = chrono::Utc::now().timestamp();
    if (now - timestamp.parse::<i64>().unwrap_or(0)).abs() > 3600 {
        return false;
    }

    // 2. 构造签名字符串
    let sign_str = format!("{}\n{}\n{}", timestamp, nonce, payload);

    // 3. 计算 HMAC-SHA256
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes());
    mac.update(sign_str.as_bytes());
    let expected_signature = format!("{:x}", mac.finalize().into_bytes());

    // 4. 对比签名
    expected_signature == signature
}
```

### 2. 输入验证

```rust
// 限制消息长度
const MAX_MESSAGE_LENGTH: usize = 10000;

// 限制会话数量
const MAX_SESSIONS: usize = 1000;
```

---

## 性能优化

### 1. 异步处理

```rust
// 消息处理不阻塞 Webhook 响应
pub async fn handle_webhook_async(&self, payload: Value) -> impl Responder {
    // 立即返回 200
    tokio::spawn(async move {
        // 异步处理消息
        self.process_event(payload).await;
    });

    StatusCode::OK
}
```

### 2. 会话缓存

```rust
// 使用 LRU 缓存限制内存占用
use lru::LruCache;

const MAX_SESSIONS: usize = 1000;
```

---

## 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_should_respond() {
        let config = FeishuImConfig::default();
        let handler = WebhookHandler::new(config);

        // 测试 @ 机器人触发
        let msg = create_test_message(true);
        assert!(handler.should_respond(&msg));

        // 测试私聊自动响应
        let msg = create_private_message();
        assert!(handler.should_respond(&msg));
    }

    #[tokio::test]
    async fn test_context_cleanup() {
        // 测试会话清理逻辑
    }
}
```

### 集成测试

```rust
#[tokio::test]
async fn test_webhook_e2e() {
    // 启动测试服务器
    // 发送模拟 Webhook 请求
    // 验证响应
}
```

---

## 总结

**技术方案**: Webhook 服务器模式 + larkrs-client SDK
**AI Provider**: 可配置（Claude/Kimi）
**部署方式**: 集成到 cis-node
**对话模式**: @机器人触发 + 私聊自动响应
**上下文管理**: SQLite 持久化

**关键文件**:
- `src/lib.rs` - Skill 入口
- `src/webhook.rs` - Webhook 处理
- `src/messenger.rs` - 消息处理和 AI 对话
- `src/context.rs` - 对话上下文管理
- `src/config.rs` - 配置管理

**下一步**: 确认技术方案后进入阶段 3（代码开发）

---

**文档版本**: v1.0
**创建时间**: 2026-02-02
**作者**: CIS Team
