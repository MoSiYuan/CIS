//! # Matrix API Routes
//!
//! Implementation of Matrix Client-Server API endpoints.
//!
//! ## Current Endpoints
//!
//! Phase 0:
//! - `GET /_matrix/client/versions` - Discovery
//! - `POST /_matrix/client/v3/login` - Login
//!
//! Phase 1:
//! - `GET /_matrix/client/v3/sync` - Synchronization
//! - `POST /_matrix/client/v3/createRoom` - Create room
//! - `PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}` - Send message
//! - `GET /_matrix/client/v3/rooms/{roomId}/messages` - Get messages
//! - `POST /_matrix/client/v3/rooms/{roomId}/join` - Join room
//! - `GET /_matrix/client/v3/rooms/{roomId}/state` - Get room state

pub mod auth;
pub mod discovery;
pub mod login;
pub mod room;
pub mod sync;

use axum::{
    routing::{get, post, put},
    Router,
};
use std::sync::Arc;

use super::store::MatrixStore;

/// Create the Matrix API router
pub fn router(store: Arc<MatrixStore>) -> Router {
    Router::new()
        // Discovery
        .route("/_matrix/client/versions", get(discovery::versions))
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
        // Store state
        .with_state(store)
}
