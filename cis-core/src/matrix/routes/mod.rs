//! # Matrix API Routes
//!
//! Implementation of Matrix Client-Server API endpoints.
//!
//! ## Current Endpoints
//!
//! Phase 0:
//! - `GET /_matrix/client/versions` - Discovery
//! - `POST /_matrix/client/v3/login` - Login
//! - `GET /_matrix/client/v3/register` - Registration flows
//! - `POST /_matrix/client/v3/register` - Register user
//! - `GET /_matrix/client/v3/register/available` - Check username
//!
//! Phase 1:
//! - `GET /_matrix/client/v3/sync` - Synchronization
//! - `POST /_matrix/client/v3/createRoom` - Create room
//! - `PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}` - Send message
//! - `GET /_matrix/client/v3/rooms/{roomId}/messages` - Get messages
//! - `POST /_matrix/client/v3/rooms/{roomId}/join` - Join room
//! - `GET /_matrix/client/v3/rooms/{roomId}/state` - Get room state
//!
//! ## Architecture Note
//!
//! 路由现在接收两个存储：
//! - `store`: MatrixStore - 协议事件存储 (matrix-events.db)
//! - `social_store`: MatrixSocialStore - 人类用户数据 (matrix-social.db)
//!
//! 这种分离允许注册逻辑迁移到 Skill，同时保持协议处理在核心。

pub mod auth;
pub mod discovery;
pub mod login;
pub mod presence;
pub mod receipts;
pub mod register;
pub mod room;
pub mod sync;
pub mod typing;

use axum::{
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;

use super::store::MatrixStore;
use super::store_social::MatrixSocialStore;
use super::presence::PresenceService;
use super::receipts::ReceiptService;
use super::typing::TypingService;

/// 应用状态，包含两个存储
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<MatrixStore>,
    pub social_store: Arc<MatrixSocialStore>,
    pub presence_service: Arc<PresenceService>,
    pub typing_service: Arc<TypingService>,
    pub receipt_service: Arc<ReceiptService>,
}

impl AppState {
    pub fn new(store: Arc<MatrixStore>, social_store: Arc<MatrixSocialStore>) -> Self {
        let presence_service = Arc::new(PresenceService::new(social_store.clone()));
        // 启动 Presence 清理任务
        presence_service.spawn_cleanup_task();

        let typing_service = Arc::new(TypingService::new());
        // 启动 Typing 清理任务
        typing_service.spawn_cleanup_task();

        let receipt_service = Arc::new(ReceiptService::new());
        // 启动 Receipt 清理任务
        receipt_service.spawn_cleanup_task();

        Self {
            store,
            social_store,
            presence_service,
            typing_service,
            receipt_service,
        }
    }
}

/// Create the Matrix API router with separated stores
///
/// # Arguments
/// - `store`: Protocol event storage (matrix-events.db)
/// - `social_store`: Human user data storage (matrix-social.db)
pub fn router(store: Arc<MatrixStore>, social_store: Arc<MatrixSocialStore>) -> Router {
    let state = AppState::new(store, social_store);
    
    Router::new()
        // Discovery
        .route("/_matrix/client/versions", get(discovery::versions))
        // Registration
        .route("/_matrix/client/v3/register", get(register::get_register_flows))
        .route("/_matrix/client/v3/register", post(register::register))
        .route("/_matrix/client/v3/register/available", get(register::check_username_available))
        // Login
        .route("/_matrix/client/v3/login", post(login::login))
        // Sync
        .route("/_matrix/client/v3/sync", get(sync::sync))
        // Room creation
        .route("/_matrix/client/v3/createRoom", post(room::create_room))
        // Room join
        .route("/_matrix/client/v3/join/{room_id}", post(room::join_room_post))
        .route("/_matrix/client/v3/rooms/{room_id}/join", post(room::join_room))
        // Room messages
        .route("/_matrix/client/v3/rooms/{room_id}/messages", get(room::get_messages))
        // Send message
        .route(
            "/_matrix/client/v3/rooms/{room_id}/send/{event_type}/{txn_id}",
            put(room::send_message),
        )
        // Room state
        .route("/_matrix/client/v3/rooms/{room_id}/state", get(room::get_room_state))
        // Presence (v1.1.6 新增)
        .route(
            "/_matrix/client/v3/presence/:user_id/status",
            get(presence::get_presence_status).put(presence::set_presence_status),
        )
        .route(
            "/_matrix/client/v3/presence/list/:user_id",
            get(presence::get_presence_list).post(presence::update_presence_list),
        )
        // Typing (v1.1.6 新增)
        .route(
            "/_matrix/client/v3/rooms/:room_id/typing/:user_id",
            put(typing::send_typing),
        )
        // Receipts (v1.1.6 新增)
        .route(
            "/_matrix/client/v3/rooms/:room_id/receipt/:receipt_type/:event_id",
            post(receipts::send_receipt),
        )
        .route(
            "/_matrix/client/v3/rooms/:room_id/receipts",
            get(receipts::get_receipts),
        )
        // Store state (both stores)
        .with_state(state)
}

/// 兼容性：创建只使用 protocol store 的 router
/// 
/// 用于测试和向后兼容。社交功能会创建内存中的存储。
#[cfg(test)]
pub fn router_with_store_only(store: Arc<MatrixStore>) -> Router {
    let social_store = Arc::new(MatrixSocialStore::open_in_memory().unwrap());
    router(store, social_store)
}
