//! # Matrix Sync Endpoint
//!
//! Implements `GET /_matrix/client/v3/sync` for synchronizing client state.
//!
//! ## Phase 1 Implementation
//!
//! This is a simplified sync that:
//! - Returns joined rooms with their messages
//! - Supports `since` parameter for pagination
//! - Returns presence and account data (empty for now)

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::matrix::error::MatrixResult;
use crate::matrix::routes::auth::authenticate;
use crate::matrix::store::MatrixStore;
use crate::matrix::routes::AppState;

/// Sync request query parameters
#[derive(Debug, Deserialize, Default)]
pub struct SyncRequest {
    /// A filter to apply to the sync
    #[serde(rename = "filter")]
    filter: Option<String>,
    /// A point in time to continue a sync from
    since: Option<String>,
    /// Whether to wait for new events
    #[serde(rename = "timeout")]
    timeout: Option<u64>,
    /// Whether the client supports full state
    #[serde(rename = "full_state")]
    full_state: Option<bool>,
    /// ID of the client device
    #[serde(rename = "set_presence")]
    set_presence: Option<String>,
}

/// Sync response
#[derive(Debug, Serialize)]
pub struct SyncResponse {
    /// The batch token to supply in the `since` param of the next `/sync` request
    next_batch: String,
    /// Updates to rooms
    rooms: Rooms,
    /// Updates to presence
    #[serde(skip_serializing_if = "Option::is_none")]
    presence: Option<serde_json::Value>,
    /// The updates to the account data
    #[serde(skip_serializing_if = "Option::is_none")]
    account_data: Option<serde_json::Value>,
    /// The updates to the to-device events
    #[serde(skip_serializing_if = "Option::is_none")]
    to_device: Option<serde_json::Value>,
    /// Counts of unread notifications
    #[serde(skip_serializing_if = "Option::is_none")]
    device_lists: Option<serde_json::Value>,
    /// Devices which have new E2E keys
    #[serde(skip_serializing_if = "Option::is_none")]
    device_one_time_keys_count: Option<HashMap<String, u64>>,
}

/// Rooms update in sync response
#[derive(Debug, Serialize, Default)]
pub struct Rooms {
    /// The rooms that the user has joined
    join: HashMap<String, JoinedRoom>,
    /// The rooms that the user has been invited to
    invite: HashMap<String, InvitedRoom>,
    /// The rooms that the user has knocked on
    knock: HashMap<String, KnockedRoom>,
    /// The rooms that the user has left
    leave: HashMap<String, LeftRoom>,
}

/// Joined room information
#[derive(Debug, Serialize)]
pub struct JoinedRoom {
    /// The timeline of messages and state changes
    timeline: Timeline,
    /// The state updates for the room
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<StateEvents>,
    /// The private data that the user has defined for this room
    #[serde(skip_serializing_if = "Option::is_none")]
    account_data: Option<serde_json::Value>,
    /// Counts of unread notifications for this room
    #[serde(skip_serializing_if = "Option::is_none")]
    unread_notifications: Option<UnreadNotifications>,
}

/// Timeline of events
#[derive(Debug, Serialize, Default)]
pub struct Timeline {
    /// List of events
    events: Vec<serde_json::Value>,
    /// Token to request more events in the past
    #[serde(skip_serializing_if = "Option::is_none")]
    prev_batch: Option<String>,
    /// Whether there are more events in the past
    #[serde(skip_serializing_if = "Option::is_none")]
    limited: Option<bool>,
}

/// State events
#[derive(Debug, Serialize, Default)]
pub struct StateEvents {
    /// List of state events
    events: Vec<serde_json::Value>,
}

/// Unread notification counts
#[derive(Debug, Serialize, Default)]
pub struct UnreadNotifications {
    /// The number of unread notifications
    #[serde(skip_serializing_if = "Option::is_none")]
    notification_count: Option<u64>,
    /// The number of unread highlights
    #[serde(skip_serializing_if = "Option::is_none")]
    highlight_count: Option<u64>,
}

/// Invited room information
#[derive(Debug, Serialize, Default)]
pub struct InvitedRoom {
    /// The state of the room at the time of the invite
    #[serde(skip_serializing_if = "Option::is_none")]
    invite_state: Option<InviteState>,
}

/// Invite state
#[derive(Debug, Serialize, Default)]
pub struct InviteState {
    /// List of events
    events: Vec<serde_json::Value>,
}

/// Knocked room information
#[derive(Debug, Serialize, Default)]
pub struct KnockedRoom {
    /// The state of the room at the time of knock
    #[serde(skip_serializing_if = "Option::is_none")]
    knock_state: Option<KnockState>,
}

/// Knock state
#[derive(Debug, Serialize, Default)]
pub struct KnockState {
    /// List of events
    events: Vec<serde_json::Value>,
}

/// Left room information
#[derive(Debug, Serialize, Default)]
pub struct LeftRoom {
    /// The timeline of messages and state changes in the room up to the point when the user left
    #[serde(skip_serializing_if = "Option::is_none")]
    timeline: Option<Timeline>,
    /// The state updates for the room up to the start of the timeline
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<StateEvents>,
}

/// GET /_matrix/client/v3/sync
///
/// Synchronize the client's state with the server.
/// Phase 1: Simplified - returns joined rooms with messages.
pub async fn sync(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<SyncRequest>,
) -> MatrixResult<Json<SyncResponse>> {
    let store = &state.store;
    let social_store = &state.social_store;
    
    // Authenticate the request using social_store
    let user = authenticate(&headers, social_store)?;
    // Generate next batch token (timestamp-based)
    let next_batch = generate_next_batch();

    // Get joined rooms for the user
    let joined_room_ids = store.get_joined_rooms(&user.user_id)
        .map_err(|e| crate::matrix::error::MatrixError::Store(format!("Failed to get joined rooms: {}", e)))?;

    // Build rooms response
    let mut rooms = Rooms::default();

    for room_id in joined_room_ids {
        // Get messages for this room since the given timestamp
        let since_ts = params
            .since
            .as_ref()
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let messages = store.get_room_messages(&room_id, since_ts, 100)
            .map_err(|e| crate::matrix::error::MatrixError::Store(format!("Failed to get room messages: {}", e)))?;

        // Convert messages to timeline events
        let events: Vec<serde_json::Value> = messages
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

        // Build state events (simplified - just room member events)
        let state_events = vec![serde_json::json!({
            "type": "m.room.member",
            "state_key": user.user_id,
            "sender": user.user_id,
            "content": {
                "membership": "join"
            }
        })];

        let joined_room = JoinedRoom {
            timeline: Timeline {
                events,
                prev_batch: params.since.clone(),
                limited: Some(false),
            },
            state: Some(StateEvents {
                events: state_events,
            }),
            account_data: None,
            unread_notifications: Some(UnreadNotifications {
                notification_count: Some(0),
                highlight_count: Some(0),
            }),
        };

        rooms.join.insert(room_id, joined_room);
    }

    Ok(Json(SyncResponse {
        next_batch,
        rooms,
        presence: Some(serde_json::json!({"events": []})),
        account_data: Some(serde_json::json!({"events": []})),
        to_device: Some(serde_json::json!({"events": []})),
        device_lists: None,
        device_one_time_keys_count: None,
    }))
}

/// Generate next batch token (timestamp-based)
fn generate_next_batch() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    format!("s{}_0", now.as_millis())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_response_structure() {
        let response = SyncResponse {
            next_batch: "s1234567890_0".to_string(),
            rooms: Rooms::default(),
            presence: None,
            account_data: None,
            to_device: None,
            device_lists: None,
            device_one_time_keys_count: None,
        };

        let json = serde_json::to_value(&response).unwrap();
        assert!(json.get("next_batch").is_some());
        assert!(json.get("rooms").is_some());
    }

    #[test]
    fn test_next_batch_generation() {
        let batch1 = generate_next_batch();
        let batch2 = generate_next_batch();

        // Should start with 's'
        assert!(batch1.starts_with('s'));
        assert!(batch2.starts_with('s'));

        // Should contain timestamp
        assert!(batch1.len() > 2);
    }
}
