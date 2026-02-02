# MATRIX 化改造剩余任务清单

## 已完成的组件 ✅

| 组件 | 文件 | 状态 |
|------|------|------|
| Matrix HTTP API (7676) | `matrix/routes/*.rs` | ✅ 完整 |
| WebSocket Federation | `matrix/websocket/*.rs` | ✅ 基础实现 |
| SQLite 多库分离 | `storage/{memory,federation}_db.rs` | ✅ 完成 |
| WAL 模式 + 随时关机 | `storage/{wal,safety}.rs` | ✅ 完成 |
| Matrix Bridge | `matrix/bridge.rs` | ✅ 完成 |
| DID 基础类型 | `types.rs` | ✅ 部分 |

---

## 剩余关键任务

### 1. Cloud Anchor 云端锚点 🆕

**缺失**: 服务发现机制

**需要实现**:
```rust
// matrix/anchor.rs
pub struct CloudAnchor {
    endpoint: String,  // 云端锚点 HTTPS 地址
    node_did: String,
    node_id: String,
}

impl CloudAnchor {
    /// 每 30 秒心跳
    pub async fn heartbeat(&self) -> Result<Vec<PeerEndpoint>> {
        // POST /v1/heartbeat
        // 返回在线节点列表
    }
    
    /// 查询节点端点
    pub async fn lookup_peer(&self, node_id: &str) -> Result<PeerEndpoint>;
    
    /// 注册本节点公网映射
    pub async fn register(&self, public_endpoint: &str) -> Result<()>;
}
```

**验收**: 
- 节点启动时从云端获取 peers 列表
- 定期心跳维持在线状态

---

### 2. Noise Protocol 握手 🆕

**缺失**: WebSocket 连接加密握手

**需要实现**:
```rust
// matrix/websocket/noise.rs
use snow::Builder;

pub struct NoiseHandshake {
    static_key: KeyPair,
}

impl NoiseHandshake {
    /// 构建 Noise XX 模式握手
    pub fn new(static_key: KeyPair) -> Self;
    
    /// 作为发起方握手
    pub async fn initiator_handshake(&mut self, stream: &mut WebSocket) -> Result<TransportState>;
    
    /// 作为响应方握手
    pub async fn responder_handshake(&mut self, stream: &mut WebSocket) -> Result<TransportState>;
}
```

**依赖**: `snow` crate

---

### 3. DID 身份系统完善 🔄

**现状**: 有基础类型，缺少完整实现

**需要实现**:
```rust
// identity/did.rs
pub struct DIDManager {
    keypair: Ed25519KeyPair,
    did: String,  // did:cis:{node_id}:{pub_key_short}
}

impl DIDManager {
    /// 生成新 DID
    pub fn generate() -> Result<Self>;
    
    /// 从种子恢复
    pub fn from_seed(seed: &[u8]) -> Result<Self>;
    
    /// 签名数据
    pub fn sign(&self, data: &[u8]) -> Signature;
    
    /// 验证签名
    pub fn verify(&self, data: &[u8], sig: &Signature) -> bool;
}
```

**集成点**:
- WebSocket 握手时 DID 认证
- Matrix User ID 映射: `@user:node.local` ↔ `did:cis:node:abc123`

---

### 4. MatrixNucleus 核心结构 🆕

**缺失**: 统一 Matrix 核心

**需要实现**:
```rust
// matrix/nucleus.rs
pub struct MatrixNucleus {
    store: Arc<MatrixStore>,
    did: Arc<DIDManager>,
    event_bus: broadcast::Sender<MatrixEvent>,
    room_manager: RoomManager,
    crypto: Option<OlmMachine>, // E2EE 可选
}

impl MatrixNucleus {
    /// 创建 Room
    pub async fn create_room(&self, opts: RoomOptions) -> Result<RoomId>;
    
    /// 注册事件处理器
    pub async fn register_handler<F>(&self, event_type: &str, handler: F) -> HandlerId
    where F: Fn(MatrixEvent) -> Result<()> + Send + Sync;
    
    /// 发送事件到 Room
    pub async fn send_event(&self, room_id: &RoomId, content: impl Into<AnyMessageLikeEventContent>) -> Result<EventId>;
    
    /// 订阅 Room 事件
    pub async fn subscribe_room(&self, room_id: &RoomId) -> mpsc::Receiver<MatrixEvent>;
}
```

---

### 5. Skill = Matrix Room 视图 🔄

**现状**: Skill trait 没有 room_id 方法

**需要修改**:
```rust
// skill/mod.rs
#[async_trait]
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    
    /// Skill 对应的 Matrix Room ID
    fn room_id(&self) -> Option<&str> {
        // 默认实现: !{skill_name}:{node_id}.cis.local
        None
    }
    
    /// 初始化时创建/加入 Room
    async fn init(&mut self, nucleus: Arc<MatrixNucleus>) -> Result<()>;
    
    /// 处理 Matrix Event
    async fn on_matrix_event(&self, event: MatrixEvent) -> Result<()> {
        // 默认空实现
        Ok(())
    }
}
```

---

### 6. 强类型 Skill 消息 🆕

**缺失**: `io.cis.*` 事件类型

**需要实现**:
```rust
// matrix/events/skill.rs
use ruma::events::macros::EventContent;

/// io.cis.task.invoke
#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "io.cis.task.invoke", kind = Message)]
pub struct TaskInvokeEventContent {
    pub task_id: String,
    pub skill_name: String,
    pub params: serde_json::Value,
}

/// io.cis.git.push
#[derive(Clone, Debug, Deserialize, Serialize, EventContent)]
#[ruma_event(type = "io.cis.git.push", kind = Message)]
pub struct GitPushEventContent {
    pub repo: String,
    pub commit: String,
    pub objects: Vec<String>,
}

// ... 其他 Skill 事件类型
```

---

### 7. Room 联邦标记 🔄

**缺失**: `federate` 字段

**需要修改**:
```rust
// matrix/store.rs
pub struct RoomMetadata {
    pub room_id: RoomId,
    pub creator: UserId,
    pub federate: bool,  // true = 公域，通过 WebSocket 广播
}

// 创建 Room 时指定
pub async fn create_room(&self, opts: RoomOptions) -> Result<RoomId> {
    if opts.federate {
        // 广播给所有 peers
        self.broadcast_room_creation(&room_id).await?;
    }
}
```

---

### 8. WebSocket 事件广播 🔄

**缺失**: Room 事件自动广播

**需要实现**:
```rust
// matrix/websocket/broadcast.rs

/// 当本地 Room 有新事件时，广播给所有 peers
pub async fn broadcast_event(
    tunnel_manager: &TunnelManager,
    room_id: &RoomId,
    event: &MatrixEvent,
) -> Result<()> {
    // 1. 检查 room.federate
    // 2. 获取所有在线 peers
    // 3. 通过 WebSocket 发送
    // 4. 等待 Ack，失败则重试
}
```

---

### 9. 断线同步队列消费 🔄

**现状**: 有 `pending_sync` 表，缺少消费逻辑

**需要实现**:
```rust
// matrix/sync/consumer.rs

pub struct SyncConsumer {
    federation_db: Arc<FederationDb>,
    tunnel_manager: Arc<TunnelManager>,
}

impl SyncConsumer {
    /// 后台任务：定期消费同步队列
    pub async fn run(&self) {
        loop {
            let tasks = self.federation_db.get_pending_tasks(10).await?;
            for task in tasks {
                // 通过 WebSocket 请求缺失事件
                // 收到后插入本地 matrix_events
                // 标记任务完成
            }
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    }
}
```

---

### 10. E2EE 加密（可选 P2）🆕

**缺失**: Olm/Megolm 端到端加密

**需要实现**:
```rust
// matrix/crypto.rs
use matrix_sdk_crypto::OlmMachine;

pub struct CryptoManager {
    olm: OlmMachine,
}

impl CryptoManager {
    /// 初始化加密（可选）
    pub async fn init(&self) -> Result<()>;
    
    /// 加密事件
    pub async fn encrypt(&self, room_id: &RoomId, content: impl EventContent) -> EncryptedContent;
    
    /// 解密事件
    pub async fn decrypt(&self, event: &EncryptedEvent) -> Result<DecryptedEvent>;
}
```

**依赖**: `matrix-sdk-crypto` crate

---

## 优先级建议

### P0（核心功能，1-2 周）
1. MatrixNucleus - 统一核心结构
2. DID 身份系统 - WebSocket 认证基础
3. Skill = Room 视图 - 架构统一
4. Room 联邦标记 - 广播控制

### P1（联邦功能，1-2 周）
5. Cloud Anchor - 服务发现
6. Noise Protocol - 加密握手
7. WebSocket 事件广播 - 联邦同步
8. 强类型 Skill 消息 - 协议标准化

### P2（优化功能，2-4 周）
9. 断线同步队列消费 - 可靠性
10. E2EE 加密 - 安全性

---

## 当前架构 vs 目标架构

### 当前
```
CIS Core
├── Matrix HTTP API (7676) ✅
├── WebSocket (6768) ✅ 基础
├── Bridge ✅
└── Storage ✅ 多库分离
```

### 目标 (MATRIX-final.md)
```
CIS Node
├── MatrixNucleus 🆕
│   ├── HTTP API (7676) ✅
│   ├── WebSocket Federation (6768) 🔄 需 Noise+DID
│   └── DID Identity 🆕
├── Cloud Anchor 🆕
├── Skill = Room View 🔄
└── SQLite 主权化 ✅
    ├── node.db (Matrix + DID)
    ├── memory.db
    └── skills/*.db
```

---

## 最短路径（MVP）

如果资源有限，优先实现:

1. **MatrixNucleus** - 统一入口
2. **Skill room_id** - Room 关联
3. **Room federate 标记** - 广播控制
4. **Cloud Anchor（简化）** - 手动配置 peers，跳过云端

这样可以实现:
- Skill 通过 Matrix Room 通信
- 本地节点内完整功能
- 联邦功能后续添加
