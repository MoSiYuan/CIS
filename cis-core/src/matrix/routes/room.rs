//! # Matrix Room Endpoints
//!
//! Implements room management endpoints for the Matrix Client-Server API.
//!
//! ## Phase 1 Implementation
//!
//! - `POST /_matrix/client/v3/createRoom` - Create a new room
//! - `PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}` - Send a message
//! - `GET /_matrix/client/v3/rooms/{roomId}/messages` - Get room messages
//! - `POST /_matrix/client/v3/join/{roomId}` - Join a room
//! - `GET /_matrix/client/v3/rooms/{roomId}/state` - Get room state

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::matrix::error::{MatrixError, MatrixResult};
use crate::matrix::routes::auth::authenticate;
use crate::matrix::routes::AppState;

// ==================== Create Room ====================

/// Create room request
#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    /// A public visibility indicates that the room will be shown in the published room list
    #[serde(rename = "visibility")]
    _visibility: Option<String>,
    /// A list of user IDs to invite to the room
    #[serde(rename = "invite")]
    _invite: Option<Vec<String>>,
    /// The room name
    #[serde(rename = "name")]
    name: Option<String>,
    /// The room topic/description
    #[serde(rename = "topic")]
    topic: Option<String>,
    /// The room alias (localpart only)
    #[serde(rename = "room_alias_name")]
    _room_alias_name: Option<String>,
    /// Whether this is a direct message room
    #[serde(rename = "is_direct")]
    _is_direct: Option<bool>,
    /// Preset configuration
    #[serde(rename = "preset")]
    _preset: Option<String>,
    /// Initial state events
    #[serde(rename = "initial_state")]
    _initial_state: Option<Vec<InitialStateEvent>>,
    /// Room creation content
    #[serde(rename = "creation_content")]
    _creation_content: Option<serde_json::Value>,
}

/// Initial state event for room creation
#[derive(Debug, Deserialize)]
pub struct InitialStateEvent {
    #[serde(rename = "type")]
    _event_type: String,
    #[serde(rename = "state_key")]
    _state_key: Option<String>,
    _content: serde_json::Value,
}

/// Create room response
#[derive(Debug, Serialize)]
pub struct CreateRoomResponse {
    /// The created room ID
    room_id: String,
}

/// POST /_matrix/client/v3/createRoom
///
/// Create a new room.
pub async fn create_room(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> MatrixResult<Json<CreateRoomResponse>> {
    let store = &state.store;
    
    // Authenticate the request using social_store
    let user = authenticate(&headers, &state.social_store)?;
    // Generate room ID
    let room_id = generate_room_id();

    // Create room in database
    store.create_room(
        &room_id,
        &user.user_id,
        req.name.as_deref(),
        req.topic.as_deref(),
    )?;

    // Join the creator to the room
    store.join_room(&room_id, &user.user_id)?;

    // Create initial state events
    let now = current_timestamp();

    // m.room.create event
    let create_event = serde_json::json!({
        "creator": user.user_id,
        "room_version": "1",
    });
    store.save_event(
        &room_id,
        &format!("${}", generate_event_id()),
        &user.user_id,
        "m.room.create",
        &create_event.to_string(),
        now,
        None,
        Some(""),
    )?;

    // m.room.member event for creator
    let member_event = serde_json::json!({
        "membership": "join",
    });
    store.save_event(
        &room_id,
        &format!("${}", generate_event_id()),
        &user.user_id,
        "m.room.member",
        &member_event.to_string(),
        now,
        None,
        Some(&user.user_id),
    )?;

    // m.room.name event (if provided)
    if let Some(name) = req.name {
        let name_event = serde_json::json!({
            "name": name,
        });
        store.save_event(
            &room_id,
            &format!("${}", generate_event_id()),
            &user.user_id,
            "m.room.name",
            &name_event.to_string(),
            now,
            None,
            Some(""),
        )?;
    }

    // m.room.topic event (if provided)
    if let Some(topic) = req.topic {
        let topic_event = serde_json::json!({
            "topic": topic,
        });
        store.save_event(
            &room_id,
            &format!("${}", generate_event_id()),
            &user.user_id,
            "m.room.topic",
            &topic_event.to_string(),
            now,
            None,
            Some(""),
        )?;
    }

    // Invite other users (if provided)
    if let Some(invitees) = req.invite {
        for invitee in invitees {
            let invite_event = serde_json::json!({
                "membership": "invite",
            });
            store.save_event(
                &room_id,
                &format!("${}", generate_event_id()),
                &user.user_id,
                "m.room.member",
                &invite_event.to_string(),
                now,
                None,
                Some(&invitee),
            )?;
        }
    }

    Ok(Json(CreateRoomResponse { room_id }))
}

// ==================== Send Message ====================

/// Send message response
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    /// The event ID of the sent message
    event_id: String,
}

/// PUT /_matrix/client/v3/rooms/{roomId}/send/{eventType}/{txnId}
///
/// Send a message event to a room.
pub async fn send_message(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path((room_id, event_type, _txn_id)): Path<(String, String, String)>,
    Json(content): Json<serde_json::Value>,
) -> MatrixResult<Json<SendMessageResponse>> {
    let store = &state.store;
    
    // Authenticate the request using social_store
    let user = authenticate(&headers, &state.social_store)?;
    // Verify user is in the room
    if !store.is_user_in_room(&room_id, &user.user_id)
        .map_err(|e| MatrixError::Store(format!("Failed to check room membership: {}", e)))? {
        return Err(MatrixError::Forbidden(
            "You are not in this room".to_string(),
        ));
    }

    // Generate event ID
    let event_id = format!("${}", generate_event_id());
    let now = current_timestamp();

    // Save the event
    store.save_event(
        &room_id,
        &event_id,
        &user.user_id,
        &event_type,
        &content.to_string(),
        now,
        None,
        None,
    )?;

    Ok(Json(SendMessageResponse { event_id }))
}

// ==================== Get Messages ====================

/// Get messages request query parameters
#[derive(Debug, Deserialize, Default)]
pub struct GetMessagesRequest {
    /// The direction to return events from
    #[serde(rename = "dir")]
    _dir: Option<String>,
    /// The token to start returning events from
    #[serde(rename = "from")]
    from: Option<String>,
    /// The token to stop returning events at
    #[serde(rename = "to")]
    _to: Option<String>,
    /// The maximum number of events to return
    #[serde(rename = "limit")]
    limit: Option<usize>,
    /// A filter to apply to the returned events
    #[serde(rename = "filter")]
    _filter: Option<String>,
}

/// Get messages response
#[derive(Debug, Serialize)]
pub struct GetMessagesResponse {
    /// The token the pagination starts from
    start: String,
    /// The token the pagination ends at
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<String>,
    /// A list of room events
    chunk: Vec<serde_json::Value>,
    /// A list of state events
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<Vec<serde_json::Value>>,
}

/// GET /_matrix/client/v3/rooms/{roomId}/messages
///
/// Get messages for a room.
pub async fn get_messages(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(params): Query<GetMessagesRequest>,
) -> MatrixResult<Json<GetMessagesResponse>> {
    let store = &state.store;
    
    // Authenticate the request using social_store
    let user = authenticate(&headers, &state.social_store)?;
    // Verify user is in the room
    if !store.is_user_in_room(&room_id, &user.user_id)
        .map_err(|e| MatrixError::Store(format!("Failed to check room membership: {}", e)))? {
        return Err(MatrixError::Forbidden(
            "You are not in this room".to_string(),
        ));
    }

    // Parse pagination parameters
    let since_ts = params
        .from
        .as_ref()
        .and_then(|f| f.parse::<i64>().ok())
        .unwrap_or(0);
    let limit = params.limit.unwrap_or(50).min(1000); // Cap at 1000

    // Get messages from store
    let messages = store.get_room_messages(&room_id, since_ts, limit)?;

    // Convert to response format
    let chunk: Vec<serde_json::Value> = messages
        .into_iter()
        .map(|msg| {
            serde_json::json!({
                "event_id": msg.event_id,
                "sender": msg.sender,
                "type": msg.event_type,
                "content": serde_json::from_str::<serde_json::Value>(&msg.content)
                    .unwrap_or(serde_json::json!({})),
                "origin_server_ts": msg.origin_server_ts,
                "unsigned": msg.unsigned.as_ref()
                    .and_then(|u| serde_json::from_str::<serde_json::Value>(u).ok()),
            })
        })
        .collect();

    // Generate pagination tokens
    let start = params.from.unwrap_or_else(|| "0".to_string());
    let end = if chunk.len() >= limit {
        Some(format!("{}", since_ts + chunk.len() as i64))
    } else {
        None
    };

    Ok(Json(GetMessagesResponse {
        start,
        end,
        chunk,
        state: Some(vec![]),
    }))
}

// ==================== Join Room ====================

/// Join room request (for POST /join)
#[derive(Debug, Deserialize, Default)]
pub struct JoinRoomRequest {
    /// The reason for joining (optional)
    #[serde(rename = "reason")]
    _reason: Option<String>,
    /// Third-party signed signature (for restricted rooms)
    #[serde(rename = "third_party_signed")]
    _third_party_signed: Option<serde_json::Value>,
}

/// Join room response
#[derive(Debug, Serialize)]
pub struct JoinRoomResponse {
    /// The room ID that was joined
    room_id: String,
}

/// POST /_matrix/client/v3/rooms/{roomId}/join
///
/// Join a room.
pub async fn join_room(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<String>,
) -> MatrixResult<Json<JoinRoomResponse>> {
    let store = &state.store;
    
    // Authenticate the request using social_store
    let user = authenticate(&headers, &state.social_store)?;
    // Check if room exists
    if !store.room_exists(&room_id)? {
        return Err(MatrixError::NotFound(format!("Room {} not found", room_id)));
    }

    // Check if already joined
    if store.is_user_in_room(&room_id, &user.user_id)? {
        // Already joined, return success
        return Ok(Json(JoinRoomResponse { room_id }));
    }

    // Join the room
    store.join_room(&room_id, &user.user_id)?;

    // Create member event
    let now = current_timestamp();
    let member_event = serde_json::json!({
        "membership": "join",
    });
    store.save_event(
        &room_id,
        &format!("${}", generate_event_id()),
        &user.user_id,
        "m.room.member",
        &member_event.to_string(),
        now,
        None,
        Some(&user.user_id),
    )?;

    Ok(Json(JoinRoomResponse { room_id }))
}

// Alternative join endpoint (POST /_matrix/client/v3/join/{roomId})
/// POST /_matrix/client/v3/join/{roomId}
pub async fn join_room_post(
    headers: HeaderMap,
    state: State<AppState>,
    path: Path<String>,
    _req: Json<JoinRoomRequest>,
) -> MatrixResult<Json<JoinRoomResponse>> {
    join_room(headers, state, path).await
}

// ==================== Get Room State ====================

/// GET /_matrix/client/v3/rooms/{roomId}/state
///
/// Get the state of a room.
pub async fn get_room_state(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(room_id): Path<String>,
) -> MatrixResult<Json<Vec<serde_json::Value>>> {
    let store = &state.store;
    
    // Authenticate the request using social_store
    let user = authenticate(&headers, &state.social_store)?;
    // Verify user is in the room
    if !store.is_user_in_room(&room_id, &user.user_id)? {
        return Err(MatrixError::Forbidden(
            "You are not in this room".to_string(),
        ));
    }

    // Get room state from store
    let state_events = store.get_room_state(&room_id)?;

    // Convert to response format
    let events: Vec<serde_json::Value> = state_events
        .into_iter()
        .map(|(event_type, state_key, sender, content)| {
            serde_json::json!({
                "type": event_type,
                "state_key": state_key,
                "sender": sender,
                "content": serde_json::from_str::<serde_json::Value>(&content)
                    .unwrap_or(serde_json::json!({})),
            })
        })
        .collect();

    Ok(Json(events))
}

// ==================== Helper Functions ====================

/// Generate a unique room ID
fn generate_room_id() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    format!("!{}:cis.local", &id[..16])
}

/// Generate a unique event ID
fn generate_event_id() -> String {
    let id = uuid::Uuid::new_v4().simple().to_string();
    id.to_string()
}

/// Get current timestamp in milliseconds
fn current_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_room_id_generation() {
        let room_id1 = generate_room_id();
        let room_id2 = generate_room_id();

        // Should start with ! and end with :cis.local
        assert!(room_id1.starts_with('!'));
        assert!(room_id1.ends_with(":cis.local"));

        // Should be unique
        assert_ne!(room_id1, room_id2);
    }

    #[test]
    fn test_event_id_generation() {
        let event_id1 = generate_event_id();
        let event_id2 = generate_event_id();

        // Should be unique and non-empty
        assert!(!event_id1.is_empty());
        assert_ne!(event_id1, event_id2);
    }

    #[test]
    fn test_create_room_response() {
        let response = CreateRoomResponse {
            room_id: "!test:cis.local".to_string(),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["room_id"], "!test:cis.local");
    }

    #[test]
    fn test_send_message_response() {
        let response = SendMessageResponse {
            event_id: "$abcdef123456".to_string(),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["event_id"], "$abcdef123456");
    }
}
