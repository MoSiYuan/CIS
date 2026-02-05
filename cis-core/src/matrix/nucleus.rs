//! MatrixNucleus - 联邦核心
//!
//! 负责：
//! - 房间生命周期管理
//! - 事件联邦广播
//! - 节点间同步
//! - 断线重连

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use ed25519_dalek::VerifyingKey;
use ruma::events::AnyMessageLikeEventContent;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::interval;
use tracing::{debug, info, warn};

use crate::error::{CisError, Result};
use crate::identity::DIDManager;
use crate::matrix::broadcast::EventBroadcaster;
use crate::matrix::error::{MatrixError, MatrixResult};
use crate::matrix::federation::{
    types::{CisMatrixEvent as FederationEvent},
};
use crate::matrix::RoomInfo;
use crate::matrix::federation_impl::{
    FederationManager, FederationManagerConfig,
};
use crate::matrix::federation::PeerDiscovery;
use crate::matrix::store::MatrixStore;
use crate::matrix::sync::{SyncPriority, SyncQueue, SyncQueueConfig, SyncTask};
use crate::matrix::websocket::tunnel::TunnelManager;

/// Matrix 事件
#[derive(Debug, Clone)]
pub struct MatrixEvent {
    /// 房间 ID
    pub room_id: RoomId,
    /// 事件 ID
    pub event_id: EventId,
    /// 发送者
    pub sender: UserId,
    /// 事件类型
    pub event_type: String,
    /// 事件内容
    pub content: serde_json::Value,
    /// 时间戳（毫秒）
    pub timestamp: i64,
    /// 是否为联邦事件
    pub federated: bool,
    /// 来源节点（如果是联邦事件）
    pub origin_node: Option<String>,
}

impl MatrixEvent {
    /// 创建新事件
    pub fn new(
        room_id: RoomId,
        event_id: EventId,
        sender: UserId,
        event_type: impl Into<String>,
        content: serde_json::Value,
    ) -> Self {
        Self {
            room_id,
            event_id,
            sender,
            event_type: event_type.into(),
            content,
            timestamp: chrono::Utc::now().timestamp_millis(),
            federated: false,
            origin_node: None,
        }
    }

    /// 创建联邦事件
    pub fn new_federated(
        room_id: RoomId,
        event_id: EventId,
        sender: UserId,
        event_type: impl Into<String>,
        content: serde_json::Value,
        origin_node: String,
    ) -> Self {
        Self {
            room_id,
            event_id,
            sender,
            event_type: event_type.into(),
            content,
            timestamp: chrono::Utc::now().timestamp_millis(),
            federated: true,
            origin_node: Some(origin_node),
        }
    }

    /// 获取房间 ID 字符串
    pub fn room_id_str(&self) -> &str {
        self.room_id.as_str()
    }

    /// 获取事件 ID 字符串
    pub fn event_id_str(&self) -> &str {
        self.event_id.as_str()
    }

    /// 获取发送者字符串
    pub fn sender_str(&self) -> &str {
        self.sender.as_str()
    }

    /// 转换为联邦事件格式
    pub fn to_federation_event(&self) -> FederationEvent {
        FederationEvent::new(
            self.event_id_str(),
            self.room_id_str(),
            self.sender_str(),
            &self.event_type,
            self.content.clone(),
        )
    }
}

/// 房间 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoomId(String);

impl RoomId {
    /// 创建新的房间 ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 解析房间 ID
    pub fn parse(id: &str) -> MatrixResult<Self> {
        if id.starts_with('!') && id.contains(':') {
            Ok(Self(id.to_string()))
        } else {
            Err(MatrixError::InvalidParameter(format!(
                "Invalid room ID format: {}",
                id
            )))
        }
    }

    /// 获取房间 ID 字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 提取服务器名称
    pub fn server_name(&self) -> Option<&str> {
        self.0.split(':').nth(1)
    }
}

impl std::fmt::Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 事件 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventId(String);

impl EventId {
    /// 创建新的事件 ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 生成新的事件 ID
    pub fn generate() -> Self {
        Self(format!("${}", uuid::Uuid::new_v4()))
    }

    /// 获取事件 ID 字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EventId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 用户 ID
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    /// 创建新的用户 ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// 从 DID 创建用户 ID
    pub fn from_did(did: &DIDManager) -> Self {
        Self(format!("@{}", did.node_id()))
    }

    /// 获取用户 ID 字符串
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// 提取服务器名称
    pub fn server_name(&self) -> Option<&str> {
        self.0.split(':').nth(1)
    }
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 房间选项
#[derive(Debug, Clone)]
pub struct RoomOptions {
    /// 房间名称
    pub name: String,
    /// 房间主题
    pub topic: Option<String>,
    /// 是否联邦同步
    pub federate: bool,
    /// 是否 E2EE
    pub encrypted: bool,
    /// 创建者用户 ID
    pub creator: Option<String>,
}

impl RoomOptions {
    /// 创建新的房间选项
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            topic: None,
            federate: false,
            encrypted: false,
            creator: None,
        }
    }

    /// 设置主题
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// 设置联邦同步
    pub fn with_federate(mut self, federate: bool) -> Self {
        self.federate = federate;
        self
    }

    /// 设置加密
    pub fn with_encrypted(mut self, encrypted: bool) -> Self {
        self.encrypted = encrypted;
        self
    }

    /// 设置创建者
    pub fn with_creator(mut self, creator: impl Into<String>) -> Self {
        self.creator = Some(creator.into());
        self
    }
}

/// 处理器 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(u64);

impl HandlerId {
    fn new(id: u64) -> Self {
        Self(id)
    }
}

/// 房间管理器
#[derive(Debug, Clone)]
pub struct RoomManager {
    /// 房间订阅者
    subscribers: Arc<RwLock<HashMap<RoomId, Vec<mpsc::Sender<MatrixEvent>>>>>,
    /// 房间元数据
    room_metadata: Arc<RwLock<HashMap<RoomId, RoomOptions>>>,
    /// 房间状态缓存
    room_state: Arc<RwLock<HashMap<RoomId, RoomState>>>,
}

/// 房间状态
#[derive(Debug, Clone)]
pub struct RoomState {
    /// 房间 ID
    pub room_id: RoomId,
    /// 当前成员
    pub members: Vec<String>,
    /// 最后活跃时间
    pub last_activity: i64,
    /// 房间版本
    pub version: u64,
}

impl RoomManager {
    /// 创建新的房间管理器
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            room_metadata: Arc::new(RwLock::new(HashMap::new())),
            room_state: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册房间
    pub async fn register_room(&self, room_id: &RoomId, opts: &RoomOptions) {
        let mut metadata = self.room_metadata.write().await;
        metadata.insert(room_id.clone(), opts.clone());

        // Initialize room state
        let mut state = self.room_state.write().await;
        state.insert(
            room_id.clone(),
            RoomState {
                room_id: room_id.clone(),
                members: Vec::new(),
                last_activity: chrono::Utc::now().timestamp(),
                version: 1,
            },
        );
    }

    /// 获取房间选项
    pub async fn get_room_options(&self, room_id: &RoomId) -> Option<RoomOptions> {
        let metadata = self.room_metadata.read().await;
        metadata.get(room_id).cloned()
    }

    /// 获取房间状态
    pub async fn get_room_state(&self, room_id: &RoomId) -> Option<RoomState> {
        let state = self.room_state.read().await;
        state.get(room_id).cloned()
    }

    /// 更新房间状态
    pub async fn update_room_state<F>(&self, room_id: &RoomId, updater: F)
    where
        F: FnOnce(&mut RoomState),
    {
        let mut state = self.room_state.write().await;
        if let Some(room_state) = state.get_mut(room_id) {
            updater(room_state);
            room_state.last_activity = chrono::Utc::now().timestamp();
            room_state.version += 1;
        }
    }

    /// 添加订阅者
    pub async fn add_subscriber(&self, room_id: &RoomId, sender: mpsc::Sender<MatrixEvent>) {
        let mut subs = self.subscribers.write().await;
        subs.entry(room_id.clone()).or_default().push(sender);
    }

    /// 广播事件到房间订阅者
    pub async fn broadcast_to_room(&self, room_id: &RoomId, event: &MatrixEvent) {
        let subs = self.subscribers.read().await;
        if let Some(subscribers) = subs.get(room_id) {
            for sender in subscribers.iter() {
                let _ = sender.send(event.clone()).await;
            }
        }
    }

    /// 获取房间成员
    pub async fn get_members(&self, room_id: &RoomId) -> Vec<String> {
        let state = self.room_state.read().await;
        state
            .get(room_id)
            .map(|s| s.members.clone())
            .unwrap_or_default()
    }

    /// 添加房间成员
    pub async fn add_member(&self, room_id: &RoomId, user_id: String) {
        self.update_room_state(room_id, |state| {
            if !state.members.contains(&user_id) {
                state.members.push(user_id);
            }
        })
        .await;
    }

    /// 移除房间成员
    pub async fn remove_member(&self, room_id: &RoomId, user_id: &str) {
        self.update_room_state(room_id, |state| {
            state.members.retain(|m| m != user_id);
        })
        .await;
    }
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}

/// MatrixNucleus - 统一 Matrix 核心
pub struct MatrixNucleus {
    /// 存储
    store: Arc<MatrixStore>,
    /// DID 身份
    did: Arc<DIDManager>,
    /// 事件总线
    event_bus: broadcast::Sender<MatrixEvent>,
    /// 房间管理器
    room_manager: RoomManager,
    /// WebSocket 隧道管理器（可选）
    tunnel_manager: Option<Arc<TunnelManager>>,
    /// 事件广播器（可选）
    broadcaster: Option<Arc<EventBroadcaster>>,
    /// 事件处理器
    handlers: Arc<RwLock<HashMap<HandlerId, EventHandlerEntry>>>,
    /// 处理器 ID 计数器
    handler_id_counter: AtomicU64,
    /// 联邦管理器（新增）
    federation: Option<Arc<FederationManager>>,
    /// 同步队列（新增）
    sync_queue: Arc<SyncQueue>,
    /// 本节点 DID
    pub node_did: String,
    /// 房间集合（用于联邦同步）
    rooms: Arc<RwLock<HashMap<String, MatrixRoom>>>,
}

impl std::fmt::Debug for MatrixNucleus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixNucleus")
            .field("did", &self.did)
            .field("event_bus", &"broadcast::Sender<MatrixEvent>")
            .field("room_manager", &self.room_manager)
            .field("tunnel_manager", &self.tunnel_manager.is_some())
            .field("broadcaster", &self.broadcaster.is_some())
            .field("federation", &self.federation.is_some())
            .field("handler_count", &self.handlers.blocking_read().len())
            .finish()
    }
}

/// 事件处理器条目
struct EventHandlerEntry {
    event_type: String,
    handler: Box<dyn Fn(MatrixEvent) -> Result<()> + Send + Sync>,
}

impl std::fmt::Debug for EventHandlerEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventHandlerEntry")
            .field("event_type", &self.event_type)
            .finish()
    }
}

/// Matrix 房间（用于联邦）
#[derive(Debug, Clone)]
pub struct MatrixRoom {
    room_id: String,
    config: RoomOptions,
    members: Vec<String>,
}

impl MatrixRoom {
    /// 创建新房间
    pub fn new(room_id: String, config: RoomOptions) -> Self {
        Self {
            room_id,
            config,
            members: Vec::new(),
        }
    }

    /// 从远程信息创建
    pub fn from_remote(info: RoomInfo) -> Self {
        Self {
            room_id: info.room_id,
            config: RoomOptions {
                name: info.name.unwrap_or_default(),
                topic: info.topic,
                federate: info.federate,
                encrypted: false,
                creator: Some(info.creator),
            },
            members: Vec::new(),
        }
    }

    /// 获取房间配置
    pub fn config(&self) -> &RoomOptions {
        &self.config
    }

    /// 获取房间成员
    pub async fn members(&self) -> Result<Vec<String>> {
        Ok(self.members.clone())
    }

    /// 添加成员
    pub fn add_member(&mut self, user_id: String) {
        if !self.members.contains(&user_id) {
            self.members.push(user_id);
        }
    }

    /// 追加事件
    pub async fn append_event(&self, event: MatrixEvent) -> Result<()> {
        // In a real implementation, this would persist the event
        debug!("Appended event {} to room {}", event.event_id, self.room_id);
        Ok(())
    }
}

impl MatrixNucleus {
    /// 创建新的 Nucleus
    pub fn new(
        store: Arc<MatrixStore>,
        did: Arc<DIDManager>,
        tunnel_manager: Option<Arc<TunnelManager>>,
    ) -> Self {
        let (event_bus, _) = broadcast::channel(1000);

        // Create sync queue with optimized configuration
        let sync_config = SyncQueueConfig {
            max_queue_size: 10000,
            max_retries: 5,
            batch_size: 50,
            batch_timeout_ms: 100,
            enable_batching: true,
            worker_count: 4,
            persistent: false,
        };
        let sync_queue = Arc::new(SyncQueue::new(sync_config));

        let nucleus = Self {
            store,
            did: did.clone(),
            event_bus,
            room_manager: RoomManager::new(),
            tunnel_manager,
            broadcaster: None,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            handler_id_counter: AtomicU64::new(1),
            federation: None,
            sync_queue,
            node_did: did.did().to_string(),
            rooms: Arc::new(RwLock::new(HashMap::new())),
        };

        // Start event processing task
        nucleus.start_event_processor();

        nucleus
    }

    /// 创建没有 tunnel_manager 的 Nucleus
    pub fn new_simple(store: Arc<MatrixStore>, did: Arc<DIDManager>) -> Self {
        Self::new(store, did, None)
    }

    /// 创建并初始化联邦功能
    pub async fn with_federation(
        store: Arc<MatrixStore>,
        did: Arc<DIDManager>,
        discovery: PeerDiscovery,
    ) -> Result<Self> {
        let mut nucleus = Self::new_simple(store.clone(), did.clone());

        // Create event channel for federation
        let (event_tx, mut event_rx) = mpsc::channel(1000);

        // Create federation manager
        let fed_config = FederationManagerConfig {
            use_websocket: true,
            auto_reconnect: true,
            reconnect_base_delay: 2,
            max_reconnect_attempts: 10,
            connection_timeout: 30,
            heartbeat_interval: 5,
            verify_dids: true,
        };

        let federation = Arc::new(FederationManager::with_config(
            did.clone(),
            discovery,
            store.clone(),
            event_tx,
            fed_config,
        )?);

        // Start federation
        federation.start().await?;

        nucleus.federation = Some(federation);

        // Spawn event handler for incoming federation events
        let _nucleus_arc = Arc::new(RwLock::new(())); // Placeholder for self reference
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                // Handle incoming federation event
                debug!("Received federation event: {}", event.event_id);
                // In full implementation, would process through nucleus.handle_remote_event
            }
        });

        Ok(nucleus)
    }

    /// 启动事件处理器
    fn start_event_processor(&self) {
        let mut rx = self.event_bus.subscribe();
        let handlers = self.handlers.clone();

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                let handlers_guard = handlers.read().await;
                for entry in handlers_guard.values() {
                    if entry.event_type == event.event_type {
                        if let Err(e) = (entry.handler)(event.clone()) {
                            warn!("Event handler error for {}: {:?}", event.event_type, e);
                        }
                    }
                }
            }
        });
    }

    /// 启动 Nucleus 并初始化所有组件
    pub async fn start(&self) -> Result<()> {
        info!("Starting MatrixNucleus for node: {}", self.node_did);

        // Start federation if available
        if let Some(ref federation) = self.federation {
            federation.start().await?;
        }

        // Start sync queue
        let sync_queue = self.sync_queue.clone();
        let federation = self.federation.clone();

        sync_queue
            .start(move |task| {
                // Process sync task
                if let Some(ref fed) = federation {
                    let rt = tokio::runtime::Handle::try_current();
                    if let Ok(rt) = rt {
                        let fed = fed.clone();
                        let event = task.event.clone();
                        let target = task.target_node.clone();
                        rt.spawn(async move {
                            if let Err(e) = fed.send_event(&target, &event).await {
                                warn!("Failed to sync event to {}: {}", target, e);
                            }
                        });
                    }
                }
                Ok(())
            })
            .await?;

        // Start room state sync task
        self.start_room_state_sync().await;

        info!("MatrixNucleus started successfully");
        Ok(())
    }

    /// 启动房间状态同步任务
    async fn start_room_state_sync(&self) {
        let rooms = self.rooms.clone();
        let federation = self.federation.clone();
        let room_manager = self.room_manager.clone();

        tokio::spawn(async move {
            let mut sync_interval = interval(Duration::from_secs(30));

            loop {
                sync_interval.tick().await;

                // Sync room states with federation peers
                if let Some(ref fed) = federation {
                    let room_ids: Vec<String> = {
                        let rooms_guard = rooms.read().await;
                        rooms_guard.keys().cloned().collect()
                    };

                    for room_id in room_ids {
                        // Check if room is federated
                        let is_ready = if let Some(conn) = fed.get_connection(&room_id).await {
                            conn.is_ready().await
                        } else {
                            false
                        };
                        
                        if is_ready {
                            // Sync room state
                            if let Err(e) = Self::sync_room_state_with_peers(
                                &room_id,
                                fed.clone(),
                                &room_manager,
                            )
                            .await
                            {
                                warn!("Failed to sync room state for {}: {}", room_id, e);
                            }
                        }
                    }
                }
            }
        });
    }

    /// 与对等节点同步房间状态
    async fn sync_room_state_with_peers(
        room_id: &str,
        federation: Arc<FederationManager>,
        room_manager: &RoomManager,
    ) -> Result<()> {
        // Query room info from connected peers
        let connections = federation.get_ready_connections().await;

        for conn in connections {
            match federation.query_room(room_id).await {
                Ok(room_info) => {
                    // Update local room state with remote info
                    if let Ok(room_id) = RoomId::parse(&room_info.room_id) {
                        room_manager
                            .update_room_state(&room_id, |state| {
                                state.last_activity = room_info.created_at;
                            })
                            .await;
                    }
                }
                Err(e) => {
                    debug!("Failed to query room from {}: {}", conn.node_id, e);
                }
            }
        }

        Ok(())
    }

    /// 创建 Room
    pub async fn create_room(&self, opts: RoomOptions) -> MatrixResult<RoomId> {
        let creator = opts
            .creator
            .clone()
            .unwrap_or_else(|| format!("@{}", self.did.node_id()));

        let room_id = format!("!{}:{}.cis.local", opts.name, self.did.node_id());

        let room_id = RoomId::parse(&room_id)?;

        // 保存到数据库
        self.store.create_room(
            room_id.as_str(),
            &creator,
            Some(&opts.name),
            opts.topic.as_deref(),
        )?;

        // Register room
        self.room_manager.register_room(&room_id, &opts).await;

        // Store in federation rooms
        {
            let mut rooms = self.rooms.write().await;
            rooms.insert(
                room_id.as_str().to_string(),
                MatrixRoom::new(room_id.as_str().to_string(), opts.clone()),
            );
        }

        // 如果 federate=true，广播给所有 peers
        if opts.federate {
            if let Some(ref federation) = self.federation {
                let event = FederationEvent::new(
                    EventId::generate().as_str(),
                    room_id.as_str(),
                    format!("@{}", self.did.node_id()),
                    "m.room.create",
                    serde_json::json!({
                        "room_id": room_id.as_str(),
                        "creator": format!("@{}", self.did.node_id()),
                        "name": opts.name,
                    }),
                );

                federation.broadcast_event(&event).await;
                debug!("Broadcasted room creation: {}", room_id);
            }
        }

        info!("Created room: {}", room_id);
        Ok(room_id)
    }

    /// 加入房间
    pub async fn join_room(&self, room_id: &RoomId, user_id: &UserId) -> MatrixResult<()> {
        // Check if this is a remote room
        let room_server = room_id.server_name();
        let local_server = self.did.node_id();

        if let Some(server) = room_server {
            if server != local_server {
                // Remote room - query federation
                if let Some(ref federation) = self.federation {
                    let room_info = match federation.query_room(room_id.as_str()).await {
                        Ok(info) => info,
                        Err(e) => return Err(MatrixError::ServerError(format!("Failed to query room: {}", e))),
                    };

                    // Store room locally
                    let mut rooms = self.rooms.write().await;
                    rooms.insert(
                        room_id.as_str().to_string(),
                        MatrixRoom::from_remote(room_info),
                    );

                    // Sync history
                    if let Err(e) = self.sync_room_history(room_id.as_str()).await {
                        warn!("Failed to sync room history: {}", e);
                    }
                }
            }
        }

        // Local join
        self.store.join_room(room_id.as_str(), user_id.as_str())?;
        self.room_manager.add_member(room_id, user_id.as_str().to_string()).await;

        info!("User {} joined room {}", user_id, room_id);
        Ok(())
    }

    /// 发送事件到 Room
    pub async fn send_event(
        &self,
        room_id: &RoomId,
        content: impl Into<AnyMessageLikeEventContent>,
    ) -> MatrixResult<EventId> {
        let content = content.into();
        let event_id = EventId::generate();
        let sender = format!("@{}", self.did.node_id());

        // 提取事件类型和内容 JSON
        let (event_type, content_json) = self.convert_content(&content)?;

        // 保存到数据库
        self.store.save_event(
            room_id.as_str(),
            event_id.as_str(),
            &sender,
            &event_type,
            &content_json.to_string(),
            chrono::Utc::now().timestamp_millis(),
            None,
            None,
        )?;

        // 创建 MatrixEvent
        let event = MatrixEvent::new(
            room_id.clone(),
            event_id.clone(),
            UserId::new(&sender),
            &event_type,
            content_json,
        );

        // 广播到事件总线
        let _ = self.event_bus.send(event.clone());

        // 广播到房间订阅者
        self.room_manager.broadcast_to_room(room_id, &event).await;

        // 联邦广播
        self.broadcast_event_to_federation(room_id, &event).await?;

        debug!("Sent event {} to room {}", event_id, room_id);
        Ok(event_id)
    }

    /// 广播事件到联邦
    async fn broadcast_event_to_federation(
        &self,
        room_id: &RoomId,
        event: &MatrixEvent,
    ) -> MatrixResult<()> {
        // Check if room is federated
        if let Some(opts) = self.room_manager.get_room_options(room_id).await {
            if opts.federate {
                if self.federation.is_some() {
                    let fed_event = event.to_federation_event();

                    // Get federated nodes for this room
                    let nodes = match self.get_federated_nodes(room_id).await {
                        Ok(n) => n,
                        Err(e) => {
                            warn!("Failed to get federated nodes: {}", e);
                            return Ok(());
                        }
                    };

                    for node in nodes {
                        if node != self.node_did {
                            // Create sync task with appropriate priority
                            let priority = match event.event_type.as_str() {
                                "m.room.create" | "m.room.member" => SyncPriority::Critical,
                                "m.room.message" => SyncPriority::High,
                                _ => SyncPriority::Normal,
                            };

                            let task = SyncTask::new(node, fed_event.clone(), priority);

                            if let Err(e) = self.sync_queue.enqueue(task).await {
                                warn!("Failed to enqueue sync task: {}", e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取房间的联邦节点
    async fn get_federated_nodes(&self, room_id: &RoomId) -> Result<Vec<String>> {
        let rooms = self.rooms.read().await;
        let room = rooms
            .get(room_id.as_str())
            .ok_or_else(|| CisError::p2p("Room not found"))?;

        if !room.config().federate {
            return Ok(vec![self.node_did.clone()]);
        }

        // Query room members
        let members = self.room_manager.get_members(room_id).await;

        // Extract node domains from user IDs
        let nodes: Vec<String> = members
            .iter()
            .filter_map(|m| m.split(':').nth(1).map(|s| s.to_string()))
            .filter(|n| n != &self.node_did)
            .collect::<std::collections::HashSet<_>>() // 去重
            .into_iter()
            .map(|n| format!("did:cis:{}:unknown", n))
            .collect();

        Ok(nodes)
    }

    /// 处理远程事件（来自联邦）
    pub async fn handle_remote_event(&self, event: MatrixEvent) -> Result<()> {
        let room_id = event.room_id_str();

        // 验证事件签名
        if !self.verify_event_signature(&event).await? {
            return Err(CisError::p2p("Invalid event signature"));
        }

        // 存储到本地房间
        {
            let rooms = self.rooms.read().await;
            if let Some(room) = rooms.get(room_id) {
                room.append_event(event.clone()).await?;
            } else if event.event_type == "m.room.create" {
                // 自动加入新房间
                drop(rooms);
                if let Ok(room_id) = RoomId::parse(room_id) {
                    let _ = self.join_room(&room_id, &event.sender).await;
                }
            }
        }

        // 更新房间状态
        self.room_manager
            .update_room_state(&event.room_id, |state| {
                state.last_activity = event.timestamp / 1000;
            })
            .await;

        // 触发事件总线
        let _ = self.event_bus.send(event.clone());

        // 广播到房间订阅者
        self.room_manager.broadcast_to_room(&event.room_id, &event).await;

        info!("Handled remote event {} in room {}", event.event_id, room_id);
        Ok(())
    }

    /// 同步房间历史
    pub async fn sync_room_history(&self, room_id: &str) -> Result<()> {
        if let Some(ref federation) = self.federation {
            let nodes = self.get_federated_nodes(&RoomId::new(room_id)).await?;

            for node in nodes {
                // Extract node ID from DID
                let node_id = node.split(':').nth(2).unwrap_or(&node);

                match federation.sync_history(node_id, room_id, None).await {
                    Ok(events) => {
                        let rooms = self.rooms.read().await;
                        if let Some(room) = rooms.get(room_id) {
                            for event in events {
                                // Convert federation event to MatrixEvent
                                let matrix_event = MatrixEvent::new_federated(
                                    RoomId::new(&event.room_id),
                                    EventId::new(&event.event_id),
                                    UserId::new(&event.sender),
                                    &event.event_type,
                                    event.content,
                                    node.clone(),
                                );
                                room.append_event(matrix_event).await?;
                            }
                        }
                        break; // 成功获取后退出
                    }
                    Err(e) => {
                        tracing::warn!("Failed to sync from {}: {}", node_id, e);
                        continue;
                    }
                }
            }
        }

        Ok(())
    }

    /// 验证事件签名
    /// 
    /// 对于联邦事件，验证发送者的身份：
    /// 1. 解析发送者的 DID 获取公钥
    /// 2. 验证事件内容的签名（如果事件中包含签名）
    /// 
    /// 注意：完整的 Matrix 事件签名验证需要 canonical JSON 和事件哈希，
    /// 这里实现基础的 DID 验证。
    async fn verify_event_signature(&self, event: &MatrixEvent) -> Result<bool> {
        if !event.federated {
            // Local events don't need verification
            return Ok(true);
        }

        let sender = event.sender_str();

        // Resolve sender's DID
        let _sender_key = match self.resolve_did_for_user(sender).await {
            Ok(key) => key,
            Err(e) => {
                warn!("Failed to resolve DID for {}: {}", sender, e);
                return Ok(false);
            }
        };

        // 检查事件中是否包含签名
        if let Some(signatures) = event.content.get("signatures") {
            // 验证签名
            if let Some(sender_signatures) = signatures.get(sender) {
                // 创建事件内容的 canonical JSON
                let event_json = serde_json::json!({
                    "room_id": event.room_id.to_string(),
                    "sender": sender,
                    "event_type": event.event_type,
                    "content": event.content,
                    "timestamp": event.timestamp,
                });
                
                let event_bytes = serde_json::to_vec(&event_json)
                    .map_err(|e| CisError::matrix(format!("Failed to serialize event: {}", e)))?;
                
                // 验证每个签名
                for (key_id, sig_value) in sender_signatures.as_object().unwrap_or(&serde_json::Map::new()) {
                    if let Some(sig_hex) = sig_value.as_str() {
                        match crate::identity::did::DIDManager::signature_from_hex(sig_hex) {
                            Ok(signature) => {
                                // 使用 ed25519_dalek 验证签名
                                use ed25519_dalek::Verifier;
                                match _sender_key.verify(&event_bytes, &signature) {
                                    Ok(()) => {
                                        debug!("Event signature verified for {} with key {}", sender, key_id);
                                        return Ok(true);
                                    }
                                    Err(e) => {
                                        warn!("Signature verification failed for {}: {:?}", sender, e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse signature for {}: {}", sender, e);
                            }
                        }
                    }
                }
                
                warn!("No valid signature found for event from {}", sender);
                Ok(false)
            } else {
                warn!("No signatures found for sender {} in event", sender);
                Ok(false)
            }
        } else {
            // 没有签名的旧式事件，仅验证 DID 存在性
            debug!("Event from {} has no signatures, accepting based on DID resolution", sender);
            Ok(true)
        }
    }

    /// 解析用户 ID 获取公钥
    async fn resolve_did_for_user(&self, user_id: &str) -> Result<VerifyingKey> {
        // Extract server from user ID
        let server = user_id
            .split(':')
            .nth(1)
            .ok_or_else(|| CisError::identity("Invalid user ID format"))?;

        let did = format!("did:cis:{}:unknown", server);

        // Check federation cache first
        if let Some(ref federation) = self.federation {
            if let Some(key) = federation.get_cached_key(&did).await {
                return Ok(key);
            }

            // Resolve from remote
            let key = federation.resolve_did(&did).await?;
            return Ok(key);
        }

        Err(CisError::identity("Federation not available"))
    }

    /// 转换事件内容为 (event_type, json)
    fn convert_content(
        &self,
        content: &AnyMessageLikeEventContent,
    ) -> MatrixResult<(String, serde_json::Value)> {
        // 使用 serde_json 序列化然后提取类型
        let json = serde_json::to_value(content)
            .map_err(|e| MatrixError::InvalidJson(e.to_string()))?;

        // 提取事件类型 - 由于 AnyMessageLikeEventContent 是枚举，
        // 我们通过序列化后的结构来推断类型
        let event_type = self.infer_event_type(content);

        Ok((event_type, json))
    }

    /// 推断事件类型
    fn infer_event_type(&self, content: &AnyMessageLikeEventContent) -> String {
        // 根据 content 类型返回对应的事件类型
        // 使用 serde_json 序列化后检查类型字段
        if let Ok(json) = serde_json::to_value(content) {
            // 尝试从 JSON 结构中提取类型信息
            if let Some(obj) = json.as_object() {
                // 检查是否有 msgtype 字段（用于 m.room.message）
                if obj.contains_key("msgtype") {
                    return "m.room.message".to_string();
                }
                // 检查是否有 membership 字段（用于 m.room.member）
                if obj.contains_key("membership") {
                    return "m.room.member".to_string();
                }
                // 检查其他特定字段
                if obj.contains_key("name") {
                    return "m.room.name".to_string();
                }
                if obj.contains_key("topic") {
                    return "m.room.topic".to_string();
                }
                if obj.contains_key("url") && obj.contains_key("info") {
                    return "m.room.avatar".to_string();
                }
            }
        }
        
        // 使用 std::any::type_name 获取类型名称作为后备方案
        let type_name = std::any::type_name_of_val(content);
        if type_name.contains("RoomMessage") {
            "m.room.message"
        } else if type_name.contains("RoomEncrypted") {
            "m.room.encrypted"
        } else if type_name.contains("Reaction") {
            "m.reaction"
        } else if type_name.contains("Sticker") {
            "m.sticker"
        } else if type_name.contains("CallInvite") {
            "m.call.invite"
        } else if type_name.contains("CallAnswer") {
            "m.call.answer"
        } else if type_name.contains("CallHangup") {
            "m.call.hangup"
        } else if type_name.contains("RoomRedaction") {
            "m.room.redaction"
        } else {
            "m.room.message" // 默认返回消息类型
        }.to_string()
    }

    /// 注册事件处理器
    pub async fn register_handler<F>(&self, event_type: &str, handler: F) -> HandlerId
    where
        F: Fn(MatrixEvent) -> Result<()> + Send + Sync + 'static,
    {
        let id = self
            .handler_id_counter
            .fetch_add(1, Ordering::SeqCst);
        let handler_id = HandlerId::new(id);

        let entry = EventHandlerEntry {
            event_type: event_type.to_string(),
            handler: Box::new(handler),
        };

        let mut handlers = self.handlers.write().await;
        handlers.insert(handler_id, entry);

        debug!("Registered handler {} for event type {}", id, event_type);
        handler_id
    }

    /// 注销事件处理器
    pub async fn unregister_handler(&self, handler_id: HandlerId) {
        let mut handlers = self.handlers.write().await;
        handlers.remove(&handler_id);
        debug!("Unregistered handler {:?}", handler_id);
    }

    /// 订阅 Room 事件
    pub async fn subscribe_room(&self, room_id: &RoomId) -> mpsc::Receiver<MatrixEvent> {
        let (tx, rx) = mpsc::channel(100);
        self.room_manager.add_subscriber(room_id, tx).await;
        rx
    }

    /// 订阅事件总线
    pub fn subscribe_events(&self) -> broadcast::Receiver<MatrixEvent> {
        self.event_bus.subscribe()
    }

    /// 获取 DID
    pub fn did(&self) -> &DIDManager {
        &self.did
    }

    /// 获取 Store
    pub fn store(&self) -> &MatrixStore {
        &self.store
    }

    /// 获取房间管理器
    pub fn room_manager(&self) -> &RoomManager {
        &self.room_manager
    }

    /// 获取隧道管理器
    pub fn tunnel_manager(&self) -> Option<&TunnelManager> {
        self.tunnel_manager.as_deref()
    }

    /// 获取事件总线发送者
    pub fn event_bus(&self) -> &broadcast::Sender<MatrixEvent> {
        &self.event_bus
    }

    /// 设置事件广播器
    pub fn with_broadcaster(mut self, broadcaster: Arc<EventBroadcaster>) -> Self {
        self.broadcaster = Some(broadcaster);
        self
    }

    /// 获取事件广播器
    pub fn broadcaster(&self) -> Option<&EventBroadcaster> {
        self.broadcaster.as_deref()
    }

    /// 获取联邦管理器
    pub fn federation(&self) -> Option<&FederationManager> {
        self.federation.as_deref()
    }

    /// 获取同步队列
    pub fn sync_queue(&self) -> &SyncQueue {
        &self.sync_queue
    }

    /// 获取房间消息
    pub fn get_room_messages(
        &self,
        room_id: &RoomId,
        since_ts: i64,
        limit: usize,
    ) -> MatrixResult<Vec<crate::matrix::store::MatrixMessage>> {
        self.store.get_room_messages(room_id.as_str(), since_ts, limit)
    }

    /// 检查用户是否在房间中
    pub fn is_user_in_room(&self, room_id: &RoomId, user_id: &UserId) -> MatrixResult<bool> {
        self.store.is_user_in_room(room_id.as_str(), user_id.as_str())
    }

    /// 获取用户加入的房间
    pub fn get_joined_rooms(&self, user_id: &UserId) -> MatrixResult<Vec<String>> {
        self.store.get_joined_rooms(user_id.as_str())
    }

    /// 关闭 Nucleus
    pub async fn shutdown(&self) {
        info!("Shutting down MatrixNucleus");

        // Shutdown federation
        if let Some(ref federation) = self.federation {
            federation.shutdown().await;
        }

        // Shutdown sync queue
        self.sync_queue.shutdown().await;

        info!("MatrixNucleus shutdown complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_id_parse() {
        let valid = RoomId::parse("!test:example.com");
        assert!(valid.is_ok());

        let invalid = RoomId::parse("invalid");
        assert!(invalid.is_err());
    }

    #[test]
    fn test_event_id_generate() {
        let id1 = EventId::generate();
        let id2 = EventId::generate();
        assert_ne!(id1.as_str(), id2.as_str());
        assert!(id1.as_str().starts_with('$'));
    }

    #[test]
    fn test_room_options_builder() {
        let opts = RoomOptions::new("Test Room")
            .with_topic("A test room")
            .with_federate(true)
            .with_encrypted(true);

        assert_eq!(opts.name, "Test Room");
        assert_eq!(opts.topic, Some("A test room".to_string()));
        assert!(opts.federate);
        assert!(opts.encrypted);
    }

    #[test]
    fn test_user_id_from_did() {
        let did = DIDManager::generate("test-node").unwrap();
        let user_id = UserId::from_did(&did);
        assert_eq!(user_id.as_str(), "@test-node");
    }

    #[tokio::test]
    async fn test_room_manager() {
        let manager = RoomManager::new();
        let room_id = RoomId::new("!test:example.com");
        let opts = RoomOptions::new("Test");

        manager.register_room(&room_id, &opts).await;

        let retrieved = manager.get_room_options(&room_id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Test");

        let state = manager.get_room_state(&room_id).await;
        assert!(state.is_some());
    }

    #[tokio::test]
    async fn test_room_subscriber() {
        let manager = RoomManager::new();
        let room_id = RoomId::new("!test:example.com");

        let (tx, mut rx) = mpsc::channel(10);
        manager.add_subscriber(&room_id, tx).await;

        let event = MatrixEvent::new(
            room_id.clone(),
            EventId::generate(),
            UserId::new("@test"),
            "m.room.message",
            serde_json::json!({"body": "Hello"}),
        );

        manager.broadcast_to_room(&room_id, &event).await;

        let received = rx.recv().await;
        assert!(received.is_some());
        assert_eq!(received.unwrap().event_type, "m.room.message");
    }

    #[test]
    fn test_matrix_event_to_federation() {
        let event = MatrixEvent::new(
            RoomId::new("!test:example.com"),
            EventId::generate(),
            UserId::new("@alice:example.com"),
            "m.room.message",
            serde_json::json!({"body": "Hello"}),
        );

        let fed_event = event.to_federation_event();
        assert_eq!(fed_event.event_type, "m.room.message");
        assert_eq!(fed_event.sender, "@alice:example.com");
    }
}
