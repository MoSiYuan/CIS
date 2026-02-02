# CIS 飞书 IM Skill - 长连接架构设计 v2

## 架构变更

### 从 Webhook 推送模式 → 长连接拉取模式

#### 旧架构 (Webhook 模式)
```
飞书服务器 ──推送──> Webhook 服务器 (需要公网 IP)
                    ↓
               消息处理
```

**问题**：
- 需要暴露公网端口（ngrok）
- 违反 CIS 本地主权原则
- 安全风险（端口暴露）

#### 新架构 (长连接模式)
```
飞书 API <───拉取─── 长连接客户端 (本地)
                       ↓
                  消息处理
```

**优势**：
- ✅ 完全本地化，无公网暴露
- ✅ 符合 CIS 安全策略
- ✅ 支持离线消息队列
- ✅ 可控制轮询频率

---

## 技术实现

### 方案选择：智能轮询 + 长连接优化

#### 1. HTTP 轮询 (基础方案)
- 调用飞书 API: `im.message.list`
- 定期检查新消息
- 支持增量拉取（使用 `container_id_type` 和 `start_time`）

#### 2. WebSocket 长连接 (优化方案)
- 使用飞书 WebSocket API
- 实时接收消息推送
- 断线自动重连

#### 3. 混合方案 (推荐)
- **正常运行**: WebSocket 长连接
- **断线降级**: HTTP 轮询兜底
- **智能调度**: 根据网络状况自适应

---

## 架构设计

### 核心组件

#### 1. MessagePoller (消息轮询器)
```rust
pub struct MessagePoller {
    config: FeishuImConfig,
    client: FeishuApiClient,
    last_check_time: Arc<RwLock<i64>>,
    running: Arc<RwLock<bool>>,
}

impl MessagePoller {
    /// 启动轮询
    pub async fn start(&mut self) -> Result<()>;

    /// HTTP 轮询模式
    async fn poll_http(&self) -> Result<Vec<FeishuMessage>>;

    /// WebSocket 长连接模式
    async fn poll_websocket(&self) -> Result<()>;

    /// 混合模式（自动降级）
    async fn poll_hybrid(&self) -> Result<()>;
}
```

#### 2. FeishuApiClient (飞书 API 客户端)
```rust
pub struct FeishuApiClient {
    app_id: String,
    app_secret: String,
    access_token: Arc<RwLock<Option<String>>>,
    http_client: reqwest::Client,
}

impl FeishuApiClient {
    /// 获取 tenant_access_token
    async fn get_access_token(&self) -> Result<String>;

    /// 获取消息列表
    async fn list_messages(
        &self,
        container_id: &str,
        start_time: Option<i64>,
    ) -> Result<Vec<Message>>;

    /// 发送消息
    async fn send_message(
        &self,
        receive_id: &str,
        content: &str,
    ) -> Result<()>;

    /// 获取会话列表
    async fn list_conversations(&self) -> Result<Vec<Conversation>>;
}
```

#### 3. ConversationTracker (会话追踪器)
```rust
pub struct ConversationTracker {
    /// 已处理的会话
    conversations: HashMap<String, ConversationState>,
    /// 最后检查时间
    last_check_time: i64,
}

pub struct ConversationState {
    /// 最后一条消息的时间戳
    last_message_time: i64,
    /// 最后一条消息的 ID
    last_message_id: String,
}
```

---

## 数据流

### 启动流程
```
1. 初始化 FeishuApiClient
   ↓
2. 获取 tenant_access_token
   ↓
3. 获取所有会话列表 (im.chat.list)
   ↓
4. 为每个会话创建 ConversationTracker
   ↓
5. 启动轮询任务 (tokio::spawn)
   ↓
6. 进入消息处理循环
```

### 轮询循环
```
while running {
    for conversation in conversations {
        1. 调用 im.message.list
        2. 对比 last_message_time
        3. 如果有新消息：
           - 调用 AI Provider 生成回复
           - 调用 im.message.send 发送回复
           - 更新 last_message_time
    }
    sleep(interval)  // 根据配置的轮询间隔
}
```

---

## 配置变更

### 新增配置项

```toml
# ==================== 消息拉取配置 ====================
[polling]
# 拉取模式: "http", "websocket", "hybrid"
mode = "hybrid"

# HTTP 轮询间隔（秒）
http_interval = 10

# WebSocket 心跳间隔（秒）
websocket_heartbeat = 30

# 是否处理历史消息
process_history = false

# 批量拉取消息数量
batch_size = 20

# 会话拉取间隔（秒，用于发现新会话）
conversation_check_interval = 60
```

### 移除配置项

```toml
# 不再需要 Webhook 配置
# [webhook]
# bind_address = "0.0.0.0"
# port = 6767
# path = "/webhook/feishu"
```

---

## 文件结构变更

### 新增文件
```
src/
├── poller.rs          # 消息轮询器（新增）
├── feishu_api.rs      # 飞书 API 客户端（新增）
└── tracker.rs         # 会话追踪器（新增）
```

### 修改文件
```
src/
├── lib.rs             # 移除 Webhook 相关代码
├── config.rs          # 新增 polling 配置
└── error.rs           # 新增 API 错误类型
```

### 移除文件
```
src/
└── webhook.rs         # 不再需要（删除）
```

---

## API 调用说明

### 飞书 API 端点

#### 1. 获取访问令牌
```
POST https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal
{
  "app_id": "cli_xxx",
  "app_secret": "xxx"
}
```

#### 2. 获取消息列表
```
GET https://open.feishu.cn/open-apis/im/v1/messages?container_id_type=chat&start_time={timestamp}
Authorization: Bearer {tenant_access_token}
```

#### 3. 发送消息
```
POST https://open.feishu.cn/open-apis/im/v1/messages
Authorization: Bearer {tenant_access_token}
{
  "receive_id": "ou_xxx",
  "msg_type": "text",
  "content": "{\"text\":\"你好\"}"
}
```

#### 4. 获取会话列表
```
GET https://open.feishu.cn/open-apis/im/v1/chats
Authorization: Bearer {tenant_access_token}
```

---

## 性能优化

### 1. 批量处理
- 一次 API 调用获取多条消息
- 减少网络开销

### 2. 增量拉取
- 使用 `start_time` 参数
- 只拉取新消息

### 3. 并发处理
- 使用 `tokio::spawn` 并发处理多个会话
- 限制并发数（避免 API 限流）

### 4. 智能调度
```rust
// 根据消息活跃度动态调整轮询间隔
struct AdaptivePoller {
    active_conversations: Vec<String>,  // 活跃会话
    inactive_conversations: Vec<String>, // 非活跃会话
}

// 活跃会话：5秒轮询一次
// 非活跃会话：60秒轮询一次
```

---

## 错误处理

### API 限流处理
```rust
if response.status == 429 {
    // 等待 Retry-After 时间
    let wait_time = response.headers["Retry-After"];
    tokio::time::sleep(Duration::from_secs(wait_time)).await;
}
```

### 网络错误处理
```rust
match api_client.list_messages().await {
    Ok(messages) => process(messages),
    Err(Error::Network) => {
        // 降级到离线模式
        // 使用本地缓存
    }
    Err(Error::AuthExpired) => {
        // 刷新 access_token
        api_client.refresh_token().await;
    }
}
```

---

## 安全考虑

### 1. Token 管理
- access_token 定期刷新（2小时有效期）
- 敏感信息加密存储

### 2. API 限流
- 遵守飞书 API 限流规则
- 实现退避算法

### 3. 数据隔离
- IM 消息仅存储在 feishu_im.db
- 不同会话的消息隔离

---

## 实施计划

### Phase 1: 基础轮询 (立即可行)
- 实现 `FeishuApiClient`
- 实现 `MessagePoller` (HTTP 模式)
- 测试消息拉取和发送

### Phase 2: 优化增强
- 实现会话追踪
- 实现批量处理
- 实现并发处理

### Phase 3: 长连接升级
- 实现 WebSocket 长连接
- 实现混合模式
- 实现智能调度

---

## 与原方案对比

| 特性 | Webhook 方案 | 长连接方案 |
|------|-------------|-----------|
| 公网暴露 | 需要 | 不需要 |
| 实时性 | 高 | 中（可配置） |
| 安全性 | 中 | 高 |
| 离线支持 | 无 | 有 |
| 资源消耗 | 低 | 中 |
| 实现复杂度 | 低 | 中 |

---

## 示例配置

### 开发环境 (快速轮测)
```toml
[polling]
mode = "http"
http_interval = 5  # 5秒轮询一次
process_history = true  # 处理历史消息
```

### 生产环境 (优化性能)
```toml
[polling]
mode = "hybrid"
http_interval = 30  # 降级时 30 秒轮询
websocket_heartbeat = 60
process_history = false
conversation_check_interval = 300
```

---

**下一步**: 开始实现 Phase 1 基础轮询功能
