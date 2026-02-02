**CIS-Matrix 标准化协议集成规范**  
*版本：v3.0-PROTOCOL | 核心定位：复用 Matrix 生态，零外生依赖*

---

## 一、 核心定位澄清

**CIS 节点 ≠ Synapse 部署，CIS 节点 = 轻量级 Matrix 协议实现体**

**标准化价值**：
- **客户端零开发**：直接复用 Element（全功能）、Cinny（轻量化）、Hydrogen（嵌入式）等成熟客户端
- **协议互认**：CIS 节点间通信数据格式天然与 Matrix 生态兼容，未来可无缝桥接外部 Matrix 服务器
- **安全继承**：直接采用 Matrix 的 E2EE（Olm/Megolm）标准，无需自研加密体系

**实现边界**：
- **7676 端口**：**严格遵循** Matrix Client-Server API（C-S API），暴露标准端点 `/_matrix/client/*`，确保任何标准客户端可连接
- **6767 端口**：**可选** Matrix Server-Server API（Federation），用于 CIS 节点互联；如暂不需要 Federation，可关闭或作为 CIS 扩展控制平面，但**数据格式**仍使用 Matrix 事件规范（`ruma::events`）

---

## 二、 最小可行协议集（MVP APIs）

为让 Element 正常登录、发消息、收推送，必须实现以下 C-S API：

| 类别           | 端点                                                             | 用途                      | 优先级 |
| -------------- | ---------------------------------------------------------------- | ------------------------- | ------ |
| ** Discovery** | `GET /_matrix/client/versions`                                   | 协议版本协商              | P0     |
|                | `GET /.well-known/matrix/client`                                 | 客户端自动发现 Homeserver | P1     |
| **Auth**       | `POST /_matrix/client/v3/login`                                  | 登录获取 Access Token     | P0     |
|                | `POST /_matrix/client/v3/logout`                                 | 登出                      | P1     |
| **Sync**       | `GET /_matrix/client/v3/sync`                                    | 长轮询获取消息（核心）    | P0     |
| **Rooms**      | `POST /_matrix/client/v3/createRoom`                             | 创建 CIS 控制室           | P0     |
|                | `POST /_matrix/client/v3/rooms/{roomId}/join`                    | 加入房间                  | P1     |
|                | `PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}` | 发送消息                  | P0     |
|                | `GET /_matrix/client/v3/rooms/{roomId}/messages`                 | 历史消息（分页）          | P1     |
| **E2EE**       | `POST /_matrix/client/v3/keys/upload`                            | 上传设备密钥              | P1     |
|                | `POST /_matrix/client/v3/keys/query`                             | 查询用户密钥              | P1     |
|                | `PUT /_matrix/client/v3/rooms/{roomId}/state/m.room.encryption`  | 开启加密                  | P2     |

**Federation APIs（6767 可选）**：
- `GET /_matrix/key/v2/server`：服务器公钥发现
- `PUT /_matrix/federation/v1/send/{txnId}`：接收其他服务器事务

---

## 三、 标准化数据流（基于 `ruma`）

**强制使用 `ruma` crate**：所有事件类型、ID 格式、JSON 结构**禁止自行定义**，必须从 `ruma::events`、`ruma::identifiers`、`ruma::api` 导入。

### 3.1 数据模型映射

```rust
// CIS 内部使用 ruma 定义，确保与 Matrix 标准 1:1 映射
use ruma::{
    events::room::message::{MessageEventContent, TextMessageEventContent},
    EventId, RoomId, UserId, MilliSecondsSinceUnixEpoch,
};

// CIS 任务结果 → Matrix 消息事件（标准格式）
pub fn task_to_matrix_event(task: &CisTask) -> MessageEventContent {
    MessageEventContent::text_plain(format!(
        "[CIS] Task {} completed: {}", 
        task.id, 
        task.result
    ))
}
```

### 3.2 双端口数据路径

**7676（Human）标准 Matrix 流**：
```text
Element ──HTTP/7676──► Axum Router ──ruma::api──► CIS Bridge ──► CIS Core
   │                                                    │
   │◄──Sync Response (JSON, standard Matrix format)─────┘
```

**6767（Bot/可选 Federation）标准 Matrix 流**：
```text
Peer CIS ──HTTP/6767──► Federation Handler ──ruma::federation──► CIS Core
   │                                                           │
   │◄──Transaction Response (Matrix PDU format)─────────────────┘
```

---

## 四、 与 CIS 架构的整合点

### 4.1 身份映射
- **Matrix User ID** (`@user:node.local`) ←→ **CIS Node ID** (`node.local`)
- **Matrix Device ID** ←→ **CIS 设备实例**（支持多设备登录同一节点）

### 4.2 房间语义映射
- **Matrix Room** (`#cis-control:node.local`) ←→ **CIS 任务频道/公域记忆空间**
- **Direct Message (DM)** ←→ **私域点对点指令通道**

### 4.3 事件与 CIS 记忆
Matrix 事件**直接存储**于 CIS SQLite，格式为标准 Matrix JSON：

```sql
-- cis_store.sql
CREATE TABLE matrix_events (
    event_id TEXT PRIMARY KEY,  -- 标准 Matrix Event ID ($xxx)
    room_id TEXT NOT NULL,      -- 标准 Room ID (!xxx:node.local)
    sender TEXT NOT NULL,       -- 标准 User ID (@bot:node.local)
    type TEXT NOT NULL,         -- m.room.message, cis.task.result, etc.
    content JSONB NOT NULL,     -- 严格遵循 Matrix 事件 Content Schema
    origin_server_ts INTEGER,   -- Matrix 标准时间戳
    unsigned JSONB              -- Matrix 标准 unsigned 字段（用于 sync position）
);

-- CIS 扩展字段（不破坏 Matrix 兼容性）
CREATE TABLE cis_event_meta (
    event_id TEXT PRIMARY KEY REFERENCES matrix_events(event_id),
    is_cis_generated BOOLEAN,   -- 区分人类输入 vs CIS 自动生成
    task_ref TEXT               -- 关联 CIS 内部任务 ID
);
```

---

## 五、 客户端接入配置（标准化）

用户配置 Element/Cinny 时，**完全遵循 Matrix 标准配置流程**：

### Cinny 配置示例
```
Homeserver URL: http://192.168.1.50:7676
（Cinny 自动请求 /.well-known/matrix/client 和 /_matrix/client/versions）

用户名: @admin:kitchen.local
密码: [CIS 初始化时生成的密码或空密码+Token]
```

### Element 配置示例
```
服务器地址: http://kitchen.local:7676
（勾选"自定义服务器"，无需 .well-known 文件也可手动输入）

登录方式: 密码登录（CIS 实现简化版登录，或 Token 登录）
```

---

## 六、 实现策略（Agent 执行）

### Phase 0: 协议骨架（Week 1）
**目标**：让 Element 能发现服务器并完成登录握手

**核心代码**：
```rust
use ruma::api::client::{discovery::get_supported_versions, login};
use axum::{Router, routing::get, Json};

// 7676 路由 - 严格使用 ruma 类型
let hmi_app = Router::new()
    .route("/_matrix/client/versions", 
        get(|| async { Json(get_supported_versions::Response::new()) }))
    .route("/_matrix/client/v3/login", post(login_handler));

// login_handler 返回 ruma::api::client::login::Response 的 JSON 序列化
```

### Phase 1: 消息管道（Week 2）
**目标**：Element 发送的消息进入 CIS Skill，CIS 结果返回 Element

**核心代码**：
```rust
// 处理 m.room.message 事件（标准 Matrix 事件）
async fn handle_room_message(
    room_id: RoomId,
    content: MessageEventContent,
) {
    if let MessageEventContent::Text(TextMessageEventContent { body, .. }) = content {
        if body.starts_with("!") {
            // CIS 指令解析
            let task = parse_cis_command(&body);
            CIS_CORE.spawn(task).await;
        }
    }
}

// CIS 任务完成回调 - 构造标准 Matrix 事件通过 7676 发送回客户端
fn on_task_complete(task: Task) {
    let event_content = MessageEventContent::text_plain(task.summary());
    EMS.inject_event(TARGET_ROOM, event_content); // 写入 DB，Sync 时推送给 Element
}
```

### Phase 2: 节点互联（Week 3，可选）
**目标**：两个 CIS 节点通过 6767 互发消息（使用 Matrix Federation 或 CIS 私有协议但承载 Matrix 事件）

**方案选择**：
- **方案 A（标准 Federation）**：实现 `PUT /_matrix/federation/v1/send/{txn}`，节点间互相同步房间事件（重 but 标准）
- **方案 B（CIS Bridge）**：6767 跑简化 HTTP API，但 Body 使用 `ruma::events::AnyMessageLikeEvent` 序列化（轻量且数据格式标准）

**推荐**：Phase 2 先用方案 B，数据标准化即可，协议可简化。

---

## 七、 关键约束重申

1. **数据格式零定制**：所有事件必须经过 `ruma` 类型系统，禁止手写 JSON 模拟 Matrix 事件
2. **端口功能严格分离**：
   - 7676：仅 Matrix C-S API，供人类客户端
   - 6767：CIS 控制平面，使用 Matrix 事件格式但可自定义传输优化
3. **存储标准化**：SQLite 中 `content` 字段必须是合法 Matrix JSON，可被标准 Matrix 客户端直接读取（无数据转换层）

---

**文档结束。此规范确保 CIS 通过标准化 Matrix 协议获得完整客户端生态支持，同时保持实现轻量与主权独立。**