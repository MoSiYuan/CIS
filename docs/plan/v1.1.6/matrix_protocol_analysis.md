# Matrix 协议完整性分析报告

> **版本**: v1.1.6
> **团队**: Team N
> **任务**: P2-2 Matrix 联邦协议补充
> **日期**: 2026-02-12
> **目标**: 分析现有 Matrix 实现，识别功能差距，制定补充计划

---

## 目录

- [1. 执行摘要](#1-执行摘要)
- [2. Matrix 规范概览](#2-matrix-规范概览)
- [3. 现有实现分析](#3-现有实现分析)
- [4. 功能差距分析](#4-功能差距分析)
- [5. 实现优先级](#5-实现优先级)
- [6. 详细实现方案](#6-详细实现方案)
- [7. 测试策略](#7-测试策略)
- [8. 交付物清单](#8-交付物清单)

---

## 1. 执行摘要

### 1.1 分析结论

CIS 当前的 Matrix 实现已经具备**基础联邦能力**，但在与标准 Matrix Homeserver 互操作性方面存在显著差距。

**核心发现**:
- ✅ **已实现**: 基础 Client-Server API (登录、同步、消息发送)
- ⚠️ **部分实现**: 房间管理、事件类型 (10/50+)
- ❌ **缺失**: Presence、Typing、Receipts、Media Upload、Search、E2EE (完整)

### 1.2 关键指标

| 类别 | Matrix 规范要求 | CIS 当前状态 | 完成度 |
|-----|---------------|------------|--------|
| 核心事件类型 | 50+ | 10 | 20% |
| Presence API | 完整 | 无 | 0% |
| Typing API | 完整 | 无 | 0% |
| Receipts API | 完整 | 无 | 0% |
| Media Upload | 完整 | 无 | 0% |
| Search API | 完整 | 无 | 0% |
| E2EE (Olm/Megolm) | 完整 | 框架存在 | 30% |
| Room State Sync | 完整 | 部分 | 60% |

### 1.3 优先级建议

1. **P0 (核心功能)**: Presence, Typing, Receipts, Media Upload
2. **P1 (增强功能)**: Search, E2EE 完善, Room State 完整同步
3. **P2 (高级功能)**: Push Rules, Spaces, Account Data 管理

---

## 2. Matrix 规范概览

### 2.1 Matrix Client-Server API (v1.11)

Matrix Client-Server API 定义了客户端与 Homeserver 之间的交互协议。

**核心端点分类**:

```
/_matrix/client/
├── v3/                    # 当前稳定版本
│   ├── sync              # 同步端点 (已实现)
│   ├── login             # 认证端点 (已实现)
│   ├── register          # 注册端点 (已实现)
│   ├── rooms             # 房间管理 (部分实现)
│   ├── presence          # 在线状态 (❌ 缺失)
│   ├── receipts          # 已读回执 (❌ 缺失)
│   ├── typing            # 输入状态 (❌ 缺失)
│   ├── search            # 搜索 (❌ 缺失)
│   ├── media             # 媒体上传 (❌ 缺失)
│   ├── keys              # E2E 密钥 (框架存在)
│   ├── devices           # 设备管理 (部分实现)
│   └── account_data      # 账户数据 (部分实现)
└── v1/                   # 未来版本 (暂不实现)
```

### 2.2 事件类型规范

Matrix 事件类型分为两类：

#### 状态事件 (State Events)

影响房间全局状态，需要 `state_key`：

| 事件类型 | 状态键 | CIS 状态 | 描述 |
|---------|--------|---------|------|
| `m.room.create` | `""` | ✅ 已实现 | 房间创建事件 |
| `m.room.member` | 用户 ID | ✅ 已实现 | 成员关系变更 |
| `m.room.power_levels` | `""` | ✅ 已实现 | 权限等级 |
| `m.room.join_rules` | `""` | ✅ 已实现 | 加入规则 |
| `m.room.name` | `""` | ✅ 已实现 | 房间名称 |
| `m.room.topic` | `""` | ✅ 已实现 | 房间主题 |
| `m.room.avatar` | `""` | ✅ 已实现 | 房间头像 |
| `m.room.encryption` | `""` | ⚠️ 框架存在 | 加密设置 |
| `m.room.history_visibility` | `""` | ❌ 缺失 | 历史可见性 |
| `m.room.guest_access` | `""` | ❌ 缺失 | 访客权限 |
| `m.room canonical_alias` | `""` | ❌ 缺失 | 标准别名 |

#### 消息事件 (Message Events)

瞬态事件，不改变房间状态：

| 事件类型 | CIS 状态 | 描述 |
|---------|---------|------|
| `m.room.message` | ✅ 已实现 | 房间消息 |
| `m.room.redaction` | ✅ 已实现 | 消息撤回 |
| `m.presence` | ❌ 缺失 | 在线状态 |
| `m.receipt` | ❌ 缺失 | 已读回执 |
| `m.typing` | ❌ 缺失 | 输入指示器 |

### 2.3 联邦协议 (Federation)

Matrix Server-Server API 用于 Homeserver 之间通信：

**已实现**:
- ✅ 基础联邦端点 (端口 7676)
- ✅ WebSocket 联邦 (端口 6768)
- ✅ CIS 自定义事件转发

**标准联邦缺失**:
- ❌ Server Key 发现 (`/_matrix/key/v2/server`)
- ❌ 事件签名验证
- ❌ PDU (Persistent Data Unit) 路由
- ❌ 查询端点 (`/_matrix/federation/v1/query/...`)

---

## 3. 现有实现分析

### 3.1 目录结构

```
cis-core/src/matrix/
├── mod.rs                    # 模块导出 (176 行)
├── events/
│   ├── mod.rs               # 事件模块 (71 行)
│   ├── event_types.rs       # 事件类型枚举 (734 行) ✅
│   ├── dag.rs              # DAG 事件 ✅
│   └── skill.rs            # Skill 事件 ✅
├── routes/
│   ├── mod.rs              # 路由定义
│   ├── auth.rs             # 认证中间件
│   ├── login.rs            # POST /login ✅
│   ├── register.rs         # POST /register ✅
│   ├── discovery.rs        # GET /versions ✅
│   ├── room.rs             # 房间端点 ⚠️ 部分实现
│   └── sync.rs            # GET /sync ✅ (388 行)
├── server.rs               # HTTP Server (端口 6767) ✅
├── store.rs                # SQLite 存储层 ✅
├── store_social.rs         # 社交存储 (用户/设备) ✅
├── nucleus.rs              # 核心联邦逻辑 ✅
├── federation/             # 联邦模块
│   ├── client.rs          # 联邦客户端 ✅
│   ├── server.rs          # 联邦服务器 ✅
│   ├── discovery.rs       # 节点发现 ✅
│   └── types.rs           # 联邦类型 ✅
├── websocket/             # WebSocket 联邦 ✅
├── e2ee/                  # E2E 加密框架 ⚠️
│   ├── olm.rs             # Olm 实现 (部分)
│   ├── megolm.rs          # Megolm 实现 (部分)
│   └── mod.rs             # E2EE 模块
├── sync/                  # 同步队列 ✅
├── bridge.rs              # CIS-Skill 桥接 ✅
├── broadcast.rs           # 事件广播 ✅
└── cloud.rs              # Cloud Anchor (NAT 穿透) ✅
```

### 3.2 已实现功能清单

#### 3.2.1 核心端点 (cis-core/src/matrix/routes/)

| 端点 | 方法 | 路径 | 状态 | 文件 |
|-----|------|------|------|------|
| 版本发现 | GET | `/_matrix/client/versions` | ✅ 完整 | discovery.rs |
| 登录 | POST | `/_matrix/client/v3/login` | ✅ 完整 | login.rs |
| 注册 | POST | `/_matrix/client/v3/register` | ✅ 完整 | register.rs |
| 同步 | GET | `/_matrix/client/v3/sync` | ✅ 完整 | sync.rs |
| 创建房间 | POST | `/_matrix/client/v3/createRoom` | ✅ 完整 | room.rs |
| 发送消息 | PUT | `/_matrix/client/v3/rooms/{id}/send/{type}` | ✅ 完整 | room.rs |
| 加入房间 | POST | `/_matrix/client/v3/rooms/{id}/join` | ✅ 完整 | room.rs |
| 离开房间 | POST | `/_matrix/client/v3/rooms/{id}/leave` | ✅ 完整 | room.rs |
| 邀请用户 | POST | `/_matrix/client/v3/rooms/{id}/invite` | ✅ 完整 | room.rs |
| 获取成员 | GET | `/_matrix/client/v3/rooms/{id}/members` | ✅ 完整 | room.rs |
| 获取状态 | GET | `/_matrix/client/v3/rooms/{id}/state` | ✅ 完整 | room.rs |

#### 3.2.2 事件类型 (cis-core/src/matrix/events/event_types.rs)

**已实现 (10 种)**:
- ✅ `m.room.message` - 房间消息
- ✅ `m.room.member` - 成员变更
- ✅ `m.room.create` - 房间创建
- ✅ `m.room.power_levels` - 权限变更
- ✅ `m.room.join_rules` - 加入规则
- ✅ `m.room.name` - 房间名称
- ✅ `m.room.topic` - 房间主题
- ✅ `m.room.avatar` - 房间头像
- ✅ `m.room.encryption` - 加密设置
- ✅ `m.room.redaction` - 消息撤回

**CIS 自定义事件 (7 种)**:
- ✅ `cis.task.request` - 任务请求
- ✅ `cis.task.response` - 任务响应
- ✅ `cis.skill.invoke` - Skill 调用
- ✅ `cis.skill.result` - Skill 结果
- ✅ `cis.room.federate` - 联邦标记
- ✅ `cis.peer.discover` - 节点发现
- ✅ `cis.peer.heartbeat` - 节点心跳

#### 3.2.3 存储层 (cis-core/src/matrix/store.rs)

**已实现功能**:
- ✅ 房间创建/查询/删除
- ✅ 消息存储/分页查询
- ✅ 成员关系管理
- ✅ 同步 token 管理
- ✅ 事件去重

**数据表**:
```sql
-- rooms 表
CREATE TABLE rooms (
    room_id TEXT PRIMARY KEY,
    creator TEXT,
    created_at INTEGER,
    federation_enabled INTEGER DEFAULT 0,
    -- ...
);

-- events 表
CREATE TABLE events (
    event_id TEXT PRIMARY KEY,
    room_id TEXT,
    sender TEXT,
    event_type TEXT,
    content TEXT,
    origin_server_ts INTEGER,
    state_key TEXT,
    -- ...
);

-- room_members 表
CREATE TABLE room_members (
    room_id TEXT,
    user_id TEXT,
    membership TEXT, -- 'join', 'invite', 'leave', 'ban'
    -- ...
);
```

#### 3.2.4 联邦实现 (cis-core/src/matrix/federation/)

**已实现**:
- ✅ 联邦服务器 (端口 7676)
- ✅ 联邦客户端
- ✅ 节点发现 (mDNS + DHT)
- ✅ WebSocket 联邦 (端口 6768)
- ✅ 事件广播
- ✅ 同步队列 (优先级 + 批处理)
- ✅ FederationManager (连接管理)

**缺失**:
- ❌ 标准联邦协议 (Server-Server API v1.11)
- ❌ Server Key 签名验证
- ❌ 跨服务器查询 (Query API)
- ❌ 事件签名

#### 3.2.5 E2E 加密 (cis-core/src/matrix/e2ee/)

**已实现**:
- ✅ 框架结构
- ⚠️ Olm 协议部分实现
- ⚠️ Megolm 协议部分实现

**缺失**:
- ❌ 完整 Olm 会话管理
- ❌ Megolm Inbound/Outbound Session
- ❌ 设备密钥管理
- ❌ 密钥备份/恢复
- ❌ SAS (Short Authentication String) 验证

---

## 4. 功能差距分析

### 4.1 缺失的 Matrix 标准功能

#### 4.1.1 Presence API (在线状态)

**规范**: `/_matrix/client/v3/presence/{userId}/status`

**缺失功能**:
- ❌ `GET /presence/{userId}/status` - 获取用户在线状态
- ❌ `PUT /presence/{userId}/status` - 设置自己的在线状态
- ❌ `GET /presence/list/{userId}` - 获取订阅的用户状态列表
- ❌ `POST /presence/list/{userId}` - 订阅用户状态

**影响**: 无法显示用户在线/离线状态，降低协作体验

#### 4.1.2 Typing API (输入指示器)

**规范**: `/_matrix/client/v3/rooms/{roomId}/typing`

**缺失功能**:
- ❌ `PUT /rooms/{roomId}/typing/{userId}` - 发送输入状态
- ❌ 同步响应中的 `m.typing` 事件

**影响**: 无法显示"正在输入..."提示

#### 4.1.3 Receipts API (已读回执)

**规范**: `/_matrix/client/v3/rooms/{roomId}/receipt`

**缺失功能**:
- ❌ `POST /rooms/{roomId}/receipt/{receiptType}/{eventId}` - 发送已读回执
- ❌ 同步响应中的 `m.receipt` 事件
- ❌ 读取回执查询端点

**影响**: 无法标记消息为已读

#### 4.1.4 Media Upload API

**规范**: `/_matrix/media/v1/upload`

**缺失功能**:
- ❌ `POST /_matrix/media/v1/upload` - 上传媒体文件
- ❌ `GET /_matrix/media/v1/download/{serverName}/{mediaId}` - 下载媒体
- ❌ `GET /_matrix/media/v1/thumbnail/{serverName}/{mediaId}` - 缩略图

**影响**: 无法发送图片、视频、文件

#### 4.1.5 Search API

**规范**: `/_matrix/client/v3/search`

**缺失功能**:
- ❌ `POST /search` - 房间事件搜索
- ❌ 分页支持 (`next_batch`)

**影响**: 无法搜索历史消息

#### 4.1.6 Devices API (设备管理)

**部分实现**:
- ✅ 设备列表获取
- ❌ 设备删除
- ❌ 设备重命名

#### 4.1.7 Keys API (E2E 密钥)

**部分实现**:
- ✅ `POST /keys/upload` - 上传设备密钥
- ❌ `POST /keys/query` - 查询用户密钥
- ❌ `POST /keys/claim` - 请求 One-Time Key
- ❌ `POST /keys/backup` - 密钥备份

#### 4.1.8 Account Data API

**部分实现**:
- ✅ 全局账户数据
- ❌ 房间级账户数据
- ❌ 标签管理 (Tags)

### 4.2 缺失的事件类型 (共 40+)

#### 状态事件

| 事件类型 | 描述 | 优先级 |
|---------|------|-------|
| `m.room.history_visibility` | 历史消息可见性 | P1 |
| `m.room.guest_access` | 访客访问规则 | P1 |
| `m.room.canonical_alias` | 房间标准别名 | P1 |
| `m.room.aliases` | 房间别名列表 | P1 |
| `m.room.join_rules` (扩展) | 受限加入规则 | P2 |
| `m.room.related_groups` | 关联群组 | P2 |
| `m.room.pinned_events` | 固定消息 | P2 |
| `m.room.server_acl` | 服务器 ACL | P2 |

#### 消息事件

| 事件类型 | 描述 | 优先级 |
|---------|------|-------|
| `m.presence` | 在线状态事件 | P0 |
| `m.typing` | 输入指示器事件 | P0 |
| `m.receipt` | 已读回执事件 | P0 |
| `m.room.redaction` (完整) | 消息撤回 | P1 |
| `m.room.encrypted` | 加密消息 | P1 |
| `m.sticker` | 贴纸消息 | P2 |
| `m.poll` | 投票消息 | P2 |
| `m.call.*` | 音视频通话 | P2 |

### 4.3 高级功能缺失

#### 4.3.1 Push Rules (推送规则)

**缺失**: 完整的推送规则 API

```
/_matrix/client/v3/pushrules/
├── /                     # 获取所有规则
├── /{scope}/{kind}/{id}  # 管理规则
└── /{scope}/{kind}/{id}/enabled  # 启用/禁用
```

#### 4.3.2 Spaces (空间)

**缺失**: Spaces 群组管理

```
/_matrix/client/v3/rooms/{roomId}/hierarchy  # 获取空间层级
```

#### 4.3.3 Threads (话题)

**缺失**: 话题 threaded 视图

```
/_matrix/client/v3/rooms/{roomId}/threads  # 获取话题列表
```

#### 4.3.4 Locations (位置共享)

**缺失**: `m.location` 消息类型

---

## 5. 实现优先级

### 5.1 优先级定义

| 优先级 | 说明 | 时间投入 |
|-------|------|---------|
| **P0** | 核心功能，严重影响互操作性 | 3-4 天 |
| **P1** | 增强功能，改善用户体验 | 2-3 天 |
| **P2** | 高级功能，可延后 | 1-2 天 |

### 5.2 P0 任务 (核心功能)

#### 任务 1: Presence API (1 天)

**目标**: 实现在线状态管理

**实现**:
```rust
// cis-core/src/matrix/presence.rs (新建)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceState {
    pub presence: String, // "online", "offline", "unavailable"
    pub last_active_ago: u64, // 毫秒
    pub status_msg: Option<String>,
}

// 端点: GET /presence/{userId}/status
pub async fn get_presence_status(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
) -> MatrixResult<Json<PresenceState>>

// 端点: PUT /presence/{userId}/status
pub async fn set_presence_status(
    State(state): State<AppState>,
    Path(user_id): Path<String>,
    Json(payload): Json<PresenceState>,
) -> MatrixResult<Json<()>>

// 数据库表
pub fn init_presence_db(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS presence (
            user_id TEXT PRIMARY KEY,
            presence TEXT NOT NULL,
            last_active_ts INTEGER,
            status_msg TEXT
        )",
        [],
    )?;
    Ok(())
}
```

**验收**:
- [ ] GET `/presence/{userId}/status` 返回用户状态
- [ ] PUT `/presence/{userId}/status` 更新自己的状态
- [ ] 同步响应包含 presence 事件
- [ ] 单元测试覆盖率 > 80%

#### 任务 2: Typing API (0.5 天)

**目标**: 实现输入指示器

**实现**:
```rust
// cis-core/src/matrix/typing.rs (新建)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingEvent {
    pub room_id: String,
    pub user_ids: Vec<String>,
}

// 端点: PUT /rooms/{roomId}/typing/{userId}
pub async fn send_typing(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(String, String)>,
    Json(payload): Json<TypingRequest>,
) -> MatrixResult<Json<()>>

// 内存存储 (Typing 是瞬态)
lazy_static! {
    static ref TYPING_STORE: RwLock<HashMap<String, TypingEvent>> =
        RwLock::new(HashMap::new());
}

// 定时清理 (30 秒超时)
pub fn start_typing_cleanup() {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
            // 清理过期 typing 状态
        }
    });
}
```

**验收**:
- [ ] PUT `/rooms/{roomId}/typing/{userId}` 发送输入状态
- [ ] 同步响应包含 m.typing 事件
- [ ] 30 秒超时自动清理

#### 任务 3: Receipts API (1 天)

**目标**: 实现已读回执

**实现**:
```rust
// cis-core/src/matrix/receipts.rs (新建)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptEvent {
    pub room_id: String,
    pub content: ReceiptContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptContent {
    #[serde(rename = "m.read")]
    pub read: HashMap<String, ReceiptInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptInfo {
    pub data: HashMap<String, serde_json::Value>,
    pub event_ids: Vec<String>,
}

// 端点: POST /rooms/{roomId}/receipt/{receiptType}/{eventId}
pub async fn send_receipt(
    State(state): State<AppState>,
    Path((room_id, receipt_type, event_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> MatrixResult<Json<()>> {
    let user = authenticate(&headers, &state.social_store)?;

    // 验证 receipt_type (支持 "m.read")
    if receipt_type != "m.read" {
        return Err(MatrixError::InvalidArgument(format!(
            "Unsupported receipt type: {}", receipt_type
        )));
    }

    // 存储回执
    state.store.set_receipt(
        &room_id,
        &user.user_id,
        &event_id,
        chrono::Utc::now().timestamp_millis(),
    )?;

    // 广播给房间其他用户
    let receipt_event = ReceiptEvent {
        room_id: room_id.clone(),
        content: ReceiptContent {
            read: HashMap::from([(
                user.user_id.clone(),
                ReceiptInfo {
                    data: HashMap::new(),
                    event_ids: vec![event_id.clone()],
                },
            )]),
        },
    };

    state.nucleus.broadcast_room_event(&room_id, receipt_event).await?;

    Ok(Json(()))
}
```

**数据库表**:
```sql
CREATE TABLE IF NOT EXISTS receipts (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    receipt_ts INTEGER NOT NULL,
    PRIMARY KEY (room_id, user_id)
);
```

**验收**:
- [ ] POST `/rooms/{roomId}/receipt/m.read/{eventId}` 存储回执
- [ ] 同步响应包含 m.receipt 事件
- [ ] 查询端点 GET `/rooms/{roomId}/receipt`

#### 任务 4: Media Upload API (1.5 天)

**目标**: 实现媒体文件上传和下载

**实现**:
```rust
// cis-core/src/matrix/media.rs (新建)

#[derive(Debug, Clone)]
pub struct MediaConfig {
    pub upload_limit: usize, // 默认 50MB
    pub storage_path: PathBuf,
}

// 端点: POST /_matrix/media/v1/upload
pub async fn upload_media(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> MatrixResult<Json<MediaUploadResponse>> {
    // 验证 Content-Type
    let content_type = headers
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .ok_or(MatrixError::MissingHeader("Content-Type".into()))?;

    // 读取文件
    while let Some(field) = multipart.next_field().await? {
        let filename = field.file_name().unwrap_or("file").to_string();
        let data = field.bytes().await?;

        // 验证大小
        if data.len() > state.media_config.upload_limit {
            return Err(MatrixError::MediaTooLarge);
        }

        // 生成 Media ID
        let media_id = format!("{}", Ulid::new());
        let extension = PathBuf::from(&filename)
            .extension()
            .and_then(|s| s.to_str().map(String::from));

        // 存储文件
        let file_path = state.media_config.storage_path
            .join(format!("{}{}", media_id, extension.unwrap_or_default()));

        tokio::fs::write(&file_path, &data).await?;

        // 存储元数据
        state.store.add_media(
            &media_id,
            &content_type.to_string(),
            data.len() as i64,
            &filename,
        )?;

        return Ok(Json(MediaUploadResponse {
            content_uri: format!("mxc://cis.local/{}", media_id),
        }));
    }

    Err(MatrixError::NoFile)
}

// 端点: GET /_matrix/media/v1/download/{serverName}/{mediaId}
pub async fn download_media(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
) -> MatrixResult<impl IntoResponse> {
    // 验证 server_name
    if server_name != "cis.local" {
        return Err(MatrixError::NotFound(format!(
            "Unknown server: {}", server_name
        )));
    }

    // 查询媒体元数据
    let media = state.store.get_media(&media_id)?;

    // 读取文件
    let file_path = state.media_config.storage_path.join(&media_id);
    let data = tokio::fs::read(&file_path).await?;

    Ok(Response::builder()
        .header("Content-Type", media.content_type)
        .header("Content-Disposition", format!("filename=\"{}\"", media.filename))
        .body(Body::from(data))
        .unwrap())
}

// 端点: GET /_matrix/media/v1/thumbnail/{serverName}/{mediaId}
pub async fn get_thumbnail(
    State(state): State<AppState>,
    Path((server_name, media_id)): Path<(String, String)>,
    Query(params): Query<ThumbnailParams>,
) -> MatrixResult<impl IntoResponse> {
    // 生成缩略图 (使用 image crate)
    // ...
}
```

**数据库表**:
```sql
CREATE TABLE IF NOT EXISTS media (
    media_id TEXT PRIMARY KEY,
    content_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    upload_ts INTEGER NOT NULL,
    filename TEXT NOT NULL
);
```

**验收**:
- [ ] POST `/_matrix/media/v1/upload` 上传文件
- [ ] GET `/_matrix/media/v1/download/{server}/{id}` 下载文件
- [ ] GET `/_matrix/media/v1/thumbnail/{server}/{id}` 获取缩略图
- [ ] 文件大小限制 (默认 50MB)
- [ ] 支持 JPEG, PNG, GIF, WebP

### 5.3 P1 任务 (增强功能)

#### 任务 5: Search API (1.5 天)

**目标**: 实现消息搜索功能

**实现**:
```rust
// cis-core/src/matrix/search.rs (新建)

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub search_categories: SearchCategories,
}

#[derive(Debug, Deserialize)]
pub struct SearchCategories {
    pub room_events: RoomEventSearch,
}

#[derive(Debug, Deserialize)]
pub struct RoomEventSearch {
    pub search_term: String,
    pub keys: Option<Vec<String>>,
    pub filter: Option<SearchFilter>,
    pub order_by: Option<String>,
    pub before_limit: Option<u64>,
    pub after_limit: Option<u64>,
    pub include_state: Option<bool>,
}

pub async fn search_events(
    State(state): State<AppState>,
    Json(payload): Json<SearchRequest>,
) -> MatrixResult<Json<SearchResponse>> {
    // 使用 FTS (Full-Text Search)
    let results = state.store.search_events(
        &payload.search_categories.room_events.search_term,
        payload.search_categories.room_events.before_limit.unwrap_or(10),
    )?;

    Ok(Json(SearchResponse {
        search_categories: SearchResponseCategories {
            room_events: RoomEventSearchResponse {
                results,
                highlights: vec![],
                state: vec![],
            },
        },
    }))
}
```

**数据库 FTS**:
```sql
-- 使用 SQLite FTS5
CREATE VIRTUAL TABLE events_fts USING fts5(
    event_id, room_id, sender, content
);

INSERT INTO events_fts(event_id, room_id, sender, content)
SELECT event_id, room_id, sender, content FROM events;
```

**验收**:
- [ ] POST `/search` 搜索消息
- [ ] 支持分页 (`next_batch`)
- [ ] 返回高亮结果

#### 任务 6: E2EE 完善 (2 天)

**目标**: 完善 Olm/Megolm 实现

**实现**:
```rust
// cis-core/src/matrix/e2ee/olm.rs (扩展)

// 完善 Olm Account
pub struct OlmAccount {
    account: olm::Account,
    identity_keys: IdentityKeys,
    stored: bool,
}

impl OlmAccount {
    pub fn new() -> Result<Self> {
        let account = olm::Account::new();
        let identity_keys = account.identity_keys();

        Ok(Self {
            account,
            identity_keys,
            stored: false,
        })
    }

    pub fn generate_one_time_keys(&mut self, count: usize) -> Result<Vec<OneTimeKey>> {
        self.account.generate_one_time_keys(count);
        // ...
    }

    pub fn max_number_of_one_time_keys(&self) -> usize {
        self.account.max_number_of_one_time_keys()
    }
}

// 完善 Megolm Session
pub struct OutboundMegolmSession {
    session: olm::Session,
    room_id: String,
    session_key: String,
}

impl OutboundMegolmSession {
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<String> {
        Ok(self.session.encrypt(plaintext)?.to_base64())
    }
}

pub struct InboundMegolmSession {
    session: olm::InboundGroupSession,
    room_id: String,
    sender_key: String,
}
```

**数据库表**:
```sql
CREATE TABLE IF NOT EXISTS olm_accounts (
    user_id TEXT PRIMARY KEY,
    account_pickle BLOB NOT NULL,
    shared INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS olm_sessions (
    sender_key TEXT NOT NULL,
    session_id TEXT NOT NULL,
    session_pickle BLOB NOT NULL,
    created_ts INTEGER NOT NULL,
    last_used_ts INTEGER NOT NULL,
    PRIMARY KEY (sender_key, session_id)
);

CREATE TABLE IF NOT EXISTS megolm_inbound_sessions (
    room_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    sender_key TEXT NOT NULL,
    session_pickle BLOB NOT NULL,
    PRIMARY KEY (room_id, session_id)
);
```

**验收**:
- [ ] Olm Account 创建和密钥生成
- [ ] One-Time Key 上传/查询
- [ ] Megolm Outbound/Inbound Session
- [ ] 加密消息发送/解密

#### 任务 7: Room State 完整同步 (1 天)

**目标**: 完善房间状态同步逻辑

**实现**:
```rust
// cis-core/src/matrix/state_sync.rs (新建)

pub struct RoomStateSync {
    store: Arc<MatrixStore>,
    nucleus: Arc<MatrixNucleus>,
}

impl RoomStateSync {
    /// 同步房间状态到联邦节点
    pub async fn sync_room_state(&self, room_id: &str) -> Result<()> {
        // 获取所有状态事件
        let state_events = self.store.get_room_state(room_id)?;

        // 构建同步消息
        let sync_msg = CisMatrixEvent {
            event_id: format!("$state_sync_{}", Ulid::new()),
            room_id: room_id.to_string(),
            sender: "@system:cis.local".into(),
            kind: "cis.room.state_sync".into(),
            content: serde_json::json!({
                "state_events": state_events,
            }),
            origin_server_ts: chrono::Utc::now().timestamp_millis(),
        };

        // 广播到联邦节点
        self.nucleus.broadcast_to_fed_peers(sync_msg).await?;

        Ok(())
    }

    /// 处理收到的状态同步
    pub async fn handle_state_sync(&self, event: CisMatrixEvent) -> Result<()> {
        let room_id = &event.room_id;
        let state_events: Vec<StateEvent> = serde_json::from_value(
            event.content["state_events"].clone()
        )?;

        // 更新本地状态
        for state_event in state_events {
            self.store.update_state(room_id, &state_event)?;
        }

        Ok(())
    }
}
```

**验收**:
- [ ] 房间创建时自动同步状态
- [ ] 成员变更时同步
- [ ] 权限变更时同步

### 5.4 P2 任务 (高级功能)

#### 任务 8: Account Data 管理 (1 天)

#### 任务 9: Devices API 完善 (0.5 天)

#### 任务 10: Push Rules (1 天)

---

## 6. 详细实现方案

### 6.1 模块结构设计

```
cis-core/src/matrix/
├── client.rs                # [新建] Matrix 客户端 (1200 行)
├── presence.rs             # [新建] Presence API (300 行)
├── typing.rs              # [新建] Typing API (200 行)
├── receipts.rs            # [新建] Receipts API (400 行)
├── media.rs               # [新建] Media Upload (600 行)
├── search.rs              # [新建] Search API (400 行)
├── state_sync.rs          # [新建] Room State 同步 (300 行)
├── routes/
│   ├── presence.rs        # [新建] Presence 路由 (200 行)
│   ├── typing.rs          # [新建] Typing 路由 (150 行)
│   ├── receipts.rs        # [新建] Receipts 路由 (150 行)
│   └── media.rs          # [新建] Media 路由 (200 行)
└── e2ee/
    ├── account.rs         # [新建] Olm Account (600 行)
    ├── session.rs         # [新建] Megolm Session (800 行)
    └── store.rs          # [新建] E2EE 密钥存储 (400 行)
```

### 6.2 数据库扩展

```sql
-- Presence 表
CREATE TABLE IF NOT EXISTS presence (
    user_id TEXT PRIMARY KEY,
    presence TEXT NOT NULL CHECK(presence IN ('online', 'offline', 'unavailable')),
    last_active_ts INTEGER NOT NULL,
    status_msg TEXT
);

CREATE INDEX IF NOT EXISTS idx_presence_status ON presence(presence);

-- Receipts 表
CREATE TABLE IF NOT EXISTS receipts (
    room_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    receipt_ts INTEGER NOT NULL,
    PRIMARY KEY (room_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_receipts_event ON receipts(room_id, event_id);

-- Media 表
CREATE TABLE IF NOT EXISTS media (
    media_id TEXT PRIMARY KEY,
    content_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    upload_ts INTEGER NOT NULL,
    filename TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_media_upload_ts ON media(upload_ts);

-- FTS 索引
CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    content,
    room_id,
    sender,
    tokenize='porter unicode61'
);

-- Trigger: 同步事件到 FTS
CREATE TRIGGER IF NOT EXISTS events_fts_sync AFTER INSERT ON events
BEGIN
    INSERT INTO events_fts(rowid, content, room_id, sender)
    VALUES (NEW.rowid, NEW.content, NEW.room_id, NEW.sender);
END;
```

### 6.3 路由集成

```rust
// cis-core/src/matrix/routes/mod.rs (扩展)

pub fn create_matrix_router() -> Router<AppState> {
    Router::new()
        // 现有路由
        .route("/_matrix/client/versions", get(discovery))
        .route("/_matrix/client/v3/login", post(login))
        .route("/_matrix/client/v3/register", post(register))
        .route("/_matrix/client/v3/sync", get(sync))
        .route("/_matrix/client/v3/createRoom", post(create_room))

        // [新增] Presence 路由
        .route("/_matrix/client/v3/presence/:user_id/status",
               get(get_presence_status))
        .route("/_matrix/client/v3/presence/:user_id/status",
               put(set_presence_status))

        // [新增] Typing 路由
        .route("/_matrix/client/v3/rooms/:room_id/typing/:user_id",
               put(send_typing))

        // [新增] Receipts 路由
        .route("/_matrix/client/v3/rooms/:room_id/receipt/:receipt_type/:event_id",
               post(send_receipt))

        // [新增] Media 路由
        .route("/_matrix/media/v1/upload", post(upload_media))
        .route("/_matrix/media/v1/download/:server_name/:media_id",
               get(download_media))
        .route("/_matrix/media/v1/thumbnail/:server_name/:media_id",
               get(get_thumbnail))

        // [新增] Search 路由
        .route("/_matrix/client/v3/search", post(search_events))
}
```

### 6.4 Matrix 客户端实现

```rust
// cis-core/src/matrix/client.rs (新建)

//! # Matrix Client
//!
//! 高级 Matrix 客户端，封装 Client-Server API 交互。

use reqwest::{Client as HttpClient, Response};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct MatrixClient {
    http: Arc<HttpClient>,
    base_url: String,
    access_token: Arc<RwLock<Option<String>>>,
    user_id: Arc<RwLock<Option<String>>>,
    device_id: Arc<RwLock<Option<String>>>,
}

impl MatrixClient {
    /// 创建新的 Matrix 客户端
    pub fn new(homeserver_url: impl Into<String>) -> Self {
        Self {
            http: Arc::new(HttpClient::new()),
            base_url: homeserver_url.into(),
            access_token: Arc::new(RwLock::new(None)),
            user_id: Arc::new(RwLock::new(None)),
            device_id: Arc::new(RwLock::new(None)),
        }
    }

    /// 登录
    pub async fn login(&self,
        user: &str,
        password: &str,
        device_id: Option<&str>,
    ) -> Result<LoginResponse> {
        let request = LoginRequest {
            user: user.to_string(),
            password: password.to_string(),
            device_id: device_id.map(String::from),
        };

        let response = self.http
            .post(format!("{}/_matrix/client/v3/login", self.base_url))
            .json(&request)
            .send()
            .await?
            .error_for_status()?;

        let login_response: LoginResponse = response.json().await?;

        // 保存会话
        *self.access_token.write().await = Some(login_response.access_token.clone());
        *self.user_id.write().await = Some(login_response.user_id.clone());
        *self.device_id.write().await = Some(login_response.device_id.clone());

        Ok(login_response)
    }

    /// 同步
    pub async fn sync(&self,
        since: Option<&str>,
        timeout: Option<u32>,
        filter: Option<&SyncFilter>,
    ) -> Result<SyncResponse> {
        let mut url = format!("{}/_matrix/client/v3/sync?", self.base_url);

        if let Some(s) = since {
            url.push_str(&format!("since={}&", s));
        }
        if let Some(t) = timeout {
            url.push_str(&format!("timeout={}&", t));
        }

        let token = self.access_token.read().await
            .as_ref()
            .ok_or(Error::NotAuthenticated)?;

        let response = self.http
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

    /// 发送消息
    pub async fn send_message(&self,
        room_id: &str,
        event_type: &str,
        content: serde_json::Value,
        txn_id: Option<&str>,
    ) -> Result<SendMessageResponse> {
        let txn_id = txn_id.unwrap_or(&format!("t{}", Ulid::new()));

        let token = self.access_token.read().await
            .as_ref()
            .ok_or(Error::NotAuthenticated)?;

        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/{}/{}",
            self.base_url, room_id, event_type, txn_id
        );

        let response = self.http
            .put(&url)
            .header("Authorization", format!("Bearer {}", token))
            .json(&content)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

    /// 设置 Presence
    pub async fn set_presence(&self,
        presence: &str,
        status_msg: Option<&str>,
    ) -> Result<()> {
        let user_id = self.user_id.read().await
            .as_ref()
            .ok_or(Error::NotAuthenticated)?;

        let token = self.access_token.read().await
            .as_ref()
            .ok_or(Error::NotAuthenticated)?;

        let content = serde_json::json!({
            "presence": presence,
            "status_msg": status_msg
        });

        self.http
            .put(format!(
                "{}/_matrix/client/v3/presence/{}/status",
                self.base_url, user_id
            ))
            .header("Authorization", format!("Bearer {}", token))
            .json(&content)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }

    /// 上传媒体
    pub async fn upload_media(&self,
        filename: &str,
        content_type: &str,
        data: Vec<u8>,
    ) -> Result<MediaUploadResponse> {
        let token = self.access_token.read().await
            .as_ref()
            .ok_or(Error::NotAuthenticated)?;

        let part = reqwest::multipart::Part::bytes(data)
            .file_name(filename.to_string())
            .mime_str(content_type)?;

        let form = reqwest::multipart::Form::new()
            .part("file", part);

        let response = self.http
            .post(format!("{}/_matrix/media/v1/upload", self.base_url))
            .header("Authorization", format!("Bearer {}", token))
            .multipart(form)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }

    /// 搜索消息
    pub async fn search(&self,
        search_term: &str,
        rooms: Option<Vec<String>>,
        limit: Option<u64>,
    ) -> Result<SearchResponse> {
        let token = self.access_token.read().await
            .as_ref()
            .ok_or(Error::NotAuthenticated)?;

        let request = serde_json::json!({
            "search_categories": {
                "room_events": {
                    "search_term": search_term,
                    "rooms": rooms,
                    "limit": limit.unwrap_or(10),
                }
            }
        });

        let response = self.http
            .post(format!("{}/_matrix/client/v3/search", self.base_url))
            .header("Authorization", format!("Bearer {}", token))
            .json(&request)
            .send()
            .await?
            .error_for_status()?;

        Ok(response.json().await?)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    #[serde(rename = "type")]
    pub login_type: String, // "m.login.password"
    pub user: String,
    pub password: String,
    pub device_id: Option<String>,
    pub initial_device_display_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user_id: String,
    pub access_token: String,
    pub device_id: String,
    pub well_known: Option<DiscoveryInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendMessageResponse {
    pub event_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MediaUploadResponse {
    pub content_uri: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub search_categories: SearchResponseCategories,
}
```

---

## 7. 测试策略

### 7.1 单元测试

#### 7.1.1 Presence API 测试

```rust
// cis-core/tests/matrix_presence_test.rs

#[tokio::test]
async fn test_set_presence_online() {
    let state = setup_test_state().await;

    let client = TestClient::new(&state);
    client.set_presence("online", Some("Available")).await;

    // 验证数据库
    let presence = state.store.get_presence("@user1:cis.local").unwrap();
    assert_eq!(presence.presence, "online");
    assert_eq!(presence.status_msg, Some("Available".to_string()));
}

#[tokio::test]
async fn test_get_presence_status() {
    let state = setup_test_state().await;

    // 设置初始状态
    state.store.set_presence(
        "@user1:cis.local",
        "offline",
        None,
    ).unwrap();

    // 查询状态
    let response = state.app
        .oneshot(Request::builder()
            .uri("/_matrix/client/v3/presence/%40user1%3Acis.local/status")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

#### 7.1.2 Typing API 测试

```rust
#[tokio::test]
async fn test_send_typing() {
    let state = setup_test_state().await;

    let response = state.app
        .oneshot(Request::builder()
            .method(Method::PUT)
            .uri("/_matrix/client/v3/rooms/!room1:cis.local/typing/%40user1%3Acis.local")
            .header("Authorization", "Bearer token123")
            .body(Body::from(r#"{"typing": true, "timeout": 30000}"#))
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_typing_timeout() {
    // 验证 30 秒超时后 typing 状态清理
}
```

#### 7.1.3 Receipts 测试

```rust
#[tokio::test]
async fn test_send_read_receipt() {
    let state = setup_test_state().await;

    // 先发送一条消息
    let msg_id = send_test_message(&state, "!room1:cis.local").await;

    // 发送已读回执
    let response = state.app
        .oneshot(Request::builder()
            .method(Method::POST)
            .uri(format!("/_matrix/client/v3/rooms/!room1:cis.local/receipt/m.read/{}", msg_id))
            .header("Authorization", "Bearer token123")
            .body(Body::empty())
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    // 验证回执存储
    let receipt = state.store.get_receipt("!room1:cis.local", "@user1:cis.local").unwrap();
    assert_eq!(receipt.event_id, msg_id);
}
```

#### 7.1.4 Media Upload 测试

```rust
#[tokio::test]
async fn test_upload_image() {
    let state = setup_test_state().await;

    let image_data = vec![0u8; 1024]; // 1KB 测试数据

    let part = reqwest::multipart::Part::bytes(image_data.clone())
        .file_name("test.jpg")
        .mime_str("image/jpeg")
        .unwrap();

    let form = reqwest::multipart::Form::new().part("file", part);

    let response = reqwest::Client::new()
        .post(format!("{}/_matrix/media/v1/upload", state.server_url))
        .header("Authorization", "Bearer token123")
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let upload_response: MediaUploadResponse = response.json().await.unwrap();
    assert!(upload_response.content_uri.starts_with("mxc://"));
}

#[tokio::test]
async fn test_upload_too_large() {
    let state = setup_test_state().await;

    let large_data = vec![0u8; 51 * 1024 * 1024]; // 51MB (超过 50MB 限制)

    // 应该返回 413 Payload Too Large
}

#[tokio::test]
async fn test_download_media() {
    let state = setup_test_state().await;

    // 上传测试文件
    let media_id = upload_test_media(&state).await;

    // 下载
    let response = reqwest::Client::new()
        .get(format!("{}/_matrix/media/v1/download/cis.local/{}",
                     state.server_url, media_id))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["Content-Type"], "image/jpeg");
}
```

#### 7.1.5 Search 测试

```rust
#[tokio::test]
async fn test_search_messages() {
    let state = setup_test_state().await;

    // 插入测试消息
    state.store.add_event(&MatrixEvent {
        event_id: "$1".into(),
        room_id: "!room1:cis.local".into(),
        content: serde_json::json!({"body": "Hello world"}),
        event_type: "m.room.message".into(),
        // ...
    }).unwrap();

    state.store.add_event(&MatrixEvent {
        event_id: "$2".into(),
        room_id: "!room1:cis.local".into(),
        content: serde_json::json!({"body": "Goodbye world"}),
        event_type: "m.room.message".into(),
        // ...
    }).unwrap();

    // 搜索
    let search_request = SearchRequest {
        search_categories: SearchCategories {
            room_events: RoomEventSearch {
                search_term: "world".into(),
                limit: Some(10),
            },
        },
    };

    let response = state.app
        .oneshot(Request::builder()
            .method(Method::POST)
            .uri("/_matrix/client/v3/search")
            .header("Authorization", "Bearer token123")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&search_request).unwrap()))
            .unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let search_response: SearchResponse = read_json(response).await;
    assert_eq!(search_response.search_categories.room_events.results.len(), 2);
}
```

### 7.2 集成测试

#### 7.2.1 Matrix Homeserver 兼容性测试

```rust
// cis-core/tests/matrix_compatibility_test.rs

#[tokio::test]
async fn test_element_client_connection() {
    // 启动 CIS Matrix Server
    let cis_server = start_cis_matrix_server().await;

    // 使用 Element 客户端模拟登录
    let client = MatrixClient::new(cis_server.homeserver_url());
    let login_response = client.login("@user1:cis.local", "password", None).await.unwrap();

    assert!(login_response.access_token.len() > 0);
    assert_eq!(login_response.user_id, "@user1:cis.local");

    // 创建房间
    let room = client.create_room(CreateRoomRequest {
        name: Some("Test Room".into()),
        ..Default::default()
    }).await.unwrap();

    // 发送消息
    client.send_message(
        &room.room_id,
        "m.room.message",
        serde_json::json!({"body": "Hello from Element!", "msgtype": "m.text"}),
        None
    ).await.unwrap();

    // 同步
    let sync_response = client.sync(None, Some(5000), None).await.unwrap();
    assert!(sync_response.rooms.join.contains_key(&room.room_id));
}

#[tokio::test]
async fn test_interop_with_synapse() {
    // 测试 CIS 与 Synapse Homeserver 的互操作性

    // 1. 启动 Synapse
    let synapse = DockerCompose::up("synapse").await;

    // 2. CIS 作为客户端连接到 Synapse
    let cis_client = MatrixClient::new(synapse.homeserver_url());
    cis_client.login("@user:synapse", "password", None).await.unwrap();

    // 3. 测试基本操作
    let rooms = cis_client.get_joined_rooms().await.unwrap();
    // ...
}
```

#### 7.2.2 联邦测试

```rust
#[tokio::test]
async fn test_federation_sync() {
    // 启动两个 CIS 节点
    let node1 = start_cis_node(7676, 6767).await;
    let node2 = start_cis_node(7677, 6768).await;

    // 节点 1 创建房间
    let room = node1.create_room("Federated Room").await;

    // 节点 2 加入房间 (通过联邦)
    node2.join_room(&room.room_id).await.unwrap();

    // 节点 1 发送消息
    node1.send_message(&room.room_id, "Hello Federation!").await;

    // 节点 2 应该收到消息 (通过同步)
    let sync = node2.sync(None).await.unwrap();
    assert!(sync.rooms.join[&room.room_id].timeline.events.len() > 0);
}
```

### 7.3 性能测试

#### 7.3.1 压力测试

```rust
#[tokio::test]
async fn test_concurrent_message_send() {
    let state = setup_test_state().await;
    let room_id = "!room1:cis.local";

    // 并发发送 100 条消息
    let tasks: Vec<_> = (0..100)
        .map(|i| {
            let state = state.clone();
            let room_id = room_id.to_string();
            tokio::spawn(async move {
                send_test_message(&state, &room_id, i).await
            })
        })
        .collect();

    // 等待所有任务完成
    let results = futures::future::join_all(tasks).await;

    // 验证所有消息都成功发送
    for result in results {
        assert!(result.is_ok());
    }

    // 验证数据库
    let messages = state.store.get_room_messages(room_id, 0, 200).unwrap();
    assert_eq!(messages.len(), 100);
}

#[tokio::test]
async fn test_large_file_upload() {
    let state = setup_test_state().await;

    // 上传 10MB 文件
    let data = vec![0u8; 10 * 1024 * 1024];
    let start = std::time::Instant::now();

    let response = upload_media(&state, "large.bin", "application/octet-stream", &data).await;

    let duration = start.elapsed();
    assert!(response.is_ok());
    println!("Upload 10MB in {:?}", duration);

    // 验证上传时间 < 5 秒
    assert!(duration.as_secs() < 5);
}
```

### 7.4 测试覆盖率目标

| 模块 | 目标覆盖率 | 测试行数 |
|-----|----------|---------|
| Presence | > 80% | 200 |
| Typing | > 80% | 150 |
| Receipts | > 80% | 200 |
| Media | > 75% | 300 |
| Search | > 70% | 250 |
| E2EE | > 70% | 400 |
| **总计** | **> 75%** | **~1500** |

---

## 8. 交付物清单

### 8.1 文档交付物

| 序号 | 交付物 | 路径 | 行数 | 状态 |
|-----|-------|------|------|------|
| 1 | Matrix 协议分析报告 | `/docs/plan/v1.1.6/matrix_protocol_analysis.md` | ~400 | ✅ 本文档 |
| 2 | Matrix 客户端 API 文档 | `/docs/matrix_client_api.md` | ~300 | 待创建 |
| 3 | E2EE 实现指南 | `/docs/e2ee_implementation.md` | ~400 | 待创建 |
| 4 | Matrix 联邦测试报告 | `/docs/matrix_federation_test_report.md` | ~200 | 待创建 |

### 8.2 代码交付物

| 序号 | 交付物 | 路径 | 代码行数 | 状态 |
|-----|-------|------|---------|------|
| 1 | Matrix 客户端实现 | `/cis-core/src/matrix/client.rs` | ~1200 | 待实现 |
| 2 | Presence API | `/cis-core/src/matrix/presence.rs` | ~300 | 待实现 |
| 3 | Typing API | `/cis-core/src/matrix/typing.rs` | ~200 | 待实现 |
| 4 | Receipts API | `/cis-core/src/matrix/receipts.rs` | ~400 | 待实现 |
| 5 | Media Upload | `/cis-core/src/matrix/media.rs` | ~600 | 待实现 |
| 6 | Search API | `/cis-core/src/matrix/search.rs` | ~400 | 待实现 |
| 7 | Room State 同步 | `/cis-core/src/matrix/state_sync.rs` | ~300 | 待实现 |
| 8 | E2EE Olm 完善 | `/cis-core/src/matrix/e2ee/account.rs` | ~600 | 待实现 |
| 9 | E2EE Megolm 完善 | `/cis-core/src/matrix/e2ee/session.rs` | ~800 | 待实现 |
| 10 | E2EE 密钥存储 | `/cis-core/src/matrix/e2ee/store.rs` | ~400 | 待实现 |
| 11 | Presence 路由 | `/cis-core/src/matrix/routes/presence.rs` | ~200 | 待实现 |
| 12 | Typing 路由 | `/cis-core/src/matrix/routes/typing.rs` | ~150 | 待实现 |
| 13 | Receipts 路由 | `/cis-core/src/matrix/routes/receipts.rs` | ~150 | 待实现 |
| 14 | Media 路由 | `/cis-core/src/matrix/routes/media.rs` | ~200 | 待实现 |
| 15 | 事件处理器扩展 | `/cis-core/src/matrix/events/` (扩展) | ~500 | 待实现 |
| 16 | 数据库迁移脚本 | `/cis-core/src/matrix/migrations/` | ~200 | 待实现 |
| **总计** | - | - | **~6050** | - |

### 8.3 测试交付物

| 序号 | 交付物 | 路径 | 测试行数 | 状态 |
|-----|-------|------|---------|------|
| 1 | Presence 测试 | `/cis-core/tests/matrix_presence_test.rs` | ~200 | 待实现 |
| 2 | Typing 测试 | `/cis-core/tests/matrix_typing_test.rs` | ~150 | 待实现 |
| 3 | Receipts 测试 | `/cis-core/tests/matrix_receipts_test.rs` | ~200 | 待实现 |
| 4 | Media 测试 | `/cis-core/tests/matrix_media_test.rs` | ~300 | 待实现 |
| 5 | Search 测试 | `/cis-core/tests/matrix_search_test.rs` | ~250 | 待实现 |
| 6 | E2EE 测试 | `/cis-core/tests/matrix_e2ee_test.rs` | ~400 | 待实现 |
| 7 | 兼容性测试 | `/cis-core/tests/matrix_compat_test.rs` | ~300 | 待实现 |
| 8 | 联邦测试 | `/cis-core/tests/matrix_federation_test.rs` | ~250 | 待实现 |
| **总计** | - | - | **~2050** | - |

### 8.4 总工作量估算

| 类别 | 工作量 | 时间 (天) |
|-----|-------|----------|
| 文档编写 | 4 个文档 | 2 天 |
| 代码实现 | ~6050 行代码 | 6 天 |
| 测试编写 | ~2050 行测试 | 2 天 |
| **总计** | - | **10 天** |

---

## 9. 实施计划

### 9.1 时间表

| 周 | 任务 | 交付物 |
|----|-----|-------|
| **第 1 天** | 协议分析 + 文档编写 | 本分析报告 |
| **第 2 天** | Presence API + 单元测试 | presence.rs, 测试 |
| **第 3 天** | Typing API + Receipts API | typing.rs, receipts.rs, 测试 |
| **第 4 天** | Media Upload API | media.rs, 测试 |
| **第 5 天** | Search API | search.rs, 测试 |
| **第 6 天** | Matrix 客户端实现 | client.rs |
| **第 7 天** | E2EE Olm 完善 | e2ee/account.rs, 测试 |
| **第 8 天** | E2EE Megolm 完善 | e2ee/session.rs, 测试 |
| **第 9 天** | 兼容性测试 + 修复 | 测试报告 |
| **第 10 天** | 文档完善 + 代码审查 | 完整交付 |

### 9.2 里程碑

- **M1 (第 3 天)**: P0 核心功能完成 (Presence, Typing, Receipts)
- **M2 (第 5 天)**: P1 增强功能完成 (Media, Search)
- **M3 (第 8 天)**: E2EE 完整实现
- **M4 (第 10 天)**: 全部验收通过

---

## 10. 风险和缓解措施

### 10.1 技术风险

| 风险 | 影响 | 概率 | 缓解措施 |
|-----|------|------|---------|
| E2EE 实现复杂度超预期 | 高 | 中 | 优先实现基础加密,高级功能延后 |
| Media 存储空间不足 | 中 | 低 | 配置自动清理策略 |
| FTS 性能问题 | 中 | 中 | 使用 SQLite FTS5,考虑索引优化 |

### 10.2 兼容性风险

| 风险 | 影响 | 概率 | 缓解措施 |
|-----|------|------|---------|
| Matrix 规范变更 | 中 | 低 | 锁定 v1.11 规范,跟进更新 |
| Element 客户端兼容性 | 高 | 中 | 使用官方测试套件验证 |

---

## 11. 验收标准总结

### 11.1 功能完整性

- [ ] **Presence API**: 完整实现 GET/PUT `/presence/{userId}/status`
- [ ] **Typing API**: 实现 PUT `/rooms/{roomId}/typing/{userId}`
- [ ] **Receipts API**: 实现 POST `/rooms/{roomId}/receipt/{type}/{eventId}`
- [ ] **Media Upload**: 实现 POST `/_matrix/media/v1/upload` 和下载
- [ ] **Search API**: 实现 POST `/search` 并支持 FTS
- [ ] **E2EE**: 完整的 Olm/Megolm 会话管理
- [ ] **事件类型**: 支持至少 20 种 Matrix 标准事件
- [ ] **客户端**: 实现完整的 Matrix Client 封装

### 11.2 测试标准

- [ ] 单元测试覆盖率 > 75%
- [ ] 集成测试通过率 100%
- [ ] Element 客户端兼容性测试通过
- [ ] 性能测试: 并发 100 条消息 < 2 秒
- [ ] Media 上传 10MB < 5 秒

### 11.3 文档标准

- [ ] API 文档完整,包含所有端点
- [ ] E2EE 实现指南详细
- [ ] 测试报告包含覆盖率
- [ ] 本分析报告完整 (当前文档)

---

## 12. 附录

### 12.1 Matrix 规范参考

- [Matrix Client-Server API v1.11](https://spec.matrix.org/v1.11/client-server-api/)
- [Matrix Server-Server API v1.11](https://spec.matrix.org/v1.11/server-server-api/)
- [Event Schemas](https://spec.matrix.org/v1.11/event-schemas/)
- [E2EE Implementation Guide](https://spec.matrix.org/v1.11/appendices/)

### 12.2 现有代码分析

**已分析文件** (11 个核心文件):
- `/cis-core/src/matrix/mod.rs` (176 行)
- `/cis-core/src/matrix/events/mod.rs` (71 行)
- `/cis-core/src/matrix/events/event_types.rs` (734 行)
- `/cis-core/src/matrix/routes/sync.rs` (388 行)
- `/cis-core/src/matrix/routes/room.rs` (未完整展示)
- `/cis-core/src/matrix/store.rs` (未完整展示)
- `/cis-core/src/matrix/federation/` (多个文件)
- `/cis-core/src/matrix/e2ee/` (部分实现)

### 12.3 依赖项

```toml
# Cargo.toml 新增依赖

[dependencies]
# Matrix E2EE
olm = { version = "0.1", optional = true }
vodozemac = { version = "0.1", optional = true }  # 替代 libolm

# Media 处理
mime = "0.3"
mime_guess = "2.0"

# FTS
rusqlite = { version = "0.30", features = ["bundled", "fts5"] }

# 测试
proptest = "1.4"
```

---

## 结论

本分析报告识别了 CIS Matrix 实现的 **40+ 处功能差距**,主要集中在:

1. **Presence/Typing/Receipts API** (P0 核心功能)
2. **Media Upload** (P0 核心功能)
3. **Search API** (P1 增强功能)
4. **E2EE 完善** (P1 安全增强)

预计总工作量为 **~6050 行代码 + ~2050 行测试**, 需要 **10 天**完成。

**下一步**: 开始 P2-2.2 缺失功能实现,优先级为 Presence → Typing → Receipts → Media。

---

**文档状态**: ✅ 完成
**最后更新**: 2026-02-12
**作者**: Team N
