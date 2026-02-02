//! MatrixNucleus - 统一 Matrix 核心

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ruma::events::AnyMessageLikeEventContent;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::identity::DIDManager;
use crate::matrix::broadcast::EventBroadcaster;
use crate::matrix::error::{MatrixError, MatrixResult};
use crate::matrix::federation::types::CisMatrixEvent;
use crate::matrix::store::MatrixStore;
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
        }
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
#[derive(Debug)]
pub struct RoomManager {
    /// 房间订阅者
    subscribers: Arc<RwLock<HashMap<RoomId, Vec<mpsc::Sender<MatrixEvent>>>>>,
    /// 房间元数据
    room_metadata: Arc<RwLock<HashMap<RoomId, RoomOptions>>>,
}

impl RoomManager {
    /// 创建新的房间管理器
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            room_metadata: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// 注册房间
    pub async fn register_room(&self, room_id: &RoomId, opts: &RoomOptions) {
        let mut metadata = self.room_metadata.write().await;
        metadata.insert(room_id.clone(), opts.clone());
    }
    
    /// 获取房间选项
    pub async fn get_room_options(&self, room_id: &RoomId) -> Option<RoomOptions> {
        let metadata = self.room_metadata.read().await;
        metadata.get(room_id).cloned()
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
}

impl std::fmt::Debug for MatrixNucleus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixNucleus")
            .field("did", &self.did)
            .field("event_bus", &"broadcast::Sender<MatrixEvent>")
            .field("room_manager", &self.room_manager)
            .field("tunnel_manager", &self.tunnel_manager.is_some())
            .field("broadcaster", &self.broadcaster.is_some())
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

impl MatrixNucleus {
    /// 创建新的 Nucleus
    pub fn new(
        store: Arc<MatrixStore>,
        did: Arc<DIDManager>,
        tunnel_manager: Option<Arc<TunnelManager>>,
    ) -> Self {
        let (event_bus, _) = broadcast::channel(1000);
        
        let nucleus = Self {
            store,
            did,
            event_bus,
            room_manager: RoomManager::new(),
            tunnel_manager,
            broadcaster: None,
            handlers: Arc::new(RwLock::new(HashMap::new())),
            handler_id_counter: AtomicU64::new(1),
        };
        
        // 启动事件处理任务
        nucleus.start_event_processor();
        
        nucleus
    }
    
    /// 创建没有 tunnel_manager 的 Nucleus
    pub fn new_simple(store: Arc<MatrixStore>, did: Arc<DIDManager>) -> Self {
        Self::new(store, did, None)
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
    
    /// 创建 Room
    pub async fn create_room(&self, opts: RoomOptions) -> MatrixResult<RoomId> {
        let creator = opts.creator.clone()
            .unwrap_or_else(|| format!("@{}", self.did.node_id()));
        
        let room_id = format!(
            "!{}:{}.cis.local",
            opts.name,
            self.did.node_id()
        );
        
        let room_id = RoomId::parse(&room_id)?;
        
        // 保存到数据库
        self.store.create_room(
            room_id.as_str(),
            &creator,
            Some(&opts.name),
            opts.topic.as_deref(),
        )?;
        
        // 注册房间
        self.room_manager.register_room(&room_id, &opts).await;
        
        // 如果 federate=true，广播给所有 peers
        if opts.federate {
            if let Some(tm) = &self.tunnel_manager {
                self.broadcast_room_creation(&room_id, tm).await?;
            }
        }
        
        info!("Created room: {}", room_id);
        Ok(room_id)
    }
    
    /// 广播房间创建
    async fn broadcast_room_creation(
        &self,
        room_id: &RoomId,
        tunnel_manager: &TunnelManager,
    ) -> MatrixResult<()> {
        let event = CisMatrixEvent::new(
            EventId::generate().as_str(),
            room_id.as_str(),
            format!("@{}", self.did.node_id()),
            "m.room.create",
            serde_json::json!({
                "room_id": room_id.as_str(),
                "creator": format!("@{}", self.did.node_id()),
            }),
        );
        
        tunnel_manager.broadcast(&event).await;
        debug!("Broadcasted room creation: {}", room_id);
        
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
        
        // 如果 room.federate，通过 WebSocket 广播
        self.maybe_broadcast_event(room_id, &event).await?;
        
        debug!("Sent event {} to room {}", event_id, room_id);
        Ok(event_id)
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
    fn infer_event_type(&self, _content: &AnyMessageLikeEventContent) -> String {
        // TODO: 根据 content 类型返回对应的事件类型
        // 暂时返回通用类型，实际需要根据 ruma 的类型系统来实现
        "m.room.message".to_string()
    }
    
    /// 可能需要广播事件
    async fn maybe_broadcast_event(
        &self,
        room_id: &RoomId,
        event: &MatrixEvent,
    ) -> MatrixResult<()> {
        // 检查房间是否启用了联邦
        if let Some(opts) = self.room_manager.get_room_options(room_id).await {
            if opts.federate {
                if let Some(tm) = &self.tunnel_manager {
                    let cis_event = CisMatrixEvent::new(
                        event.event_id.as_str(),
                        event.room_id.as_str(),
                        event.sender.as_str(),
                        &event.event_type,
                        event.content.clone(),
                    );
                    
                    tm.broadcast(&cis_event).await;
                }
            }
        }
        
        Ok(())
    }
    
    /// 注册事件处理器
    pub async fn register_handler<F>(&self, event_type: &str, handler: F) -> HandlerId
    where
        F: Fn(MatrixEvent) -> Result<()> + Send + Sync + 'static,
    {
        let id = self.handler_id_counter.fetch_add(1, Ordering::SeqCst);
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
    
    /// 加入房间
    pub async fn join_room(&self, room_id: &RoomId, user_id: &UserId) -> MatrixResult<()> {
        self.store.join_room(room_id.as_str(), user_id.as_str())?;
        info!("User {} joined room {}", user_id, room_id);
        Ok(())
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
}
