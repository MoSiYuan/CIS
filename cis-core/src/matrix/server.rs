//! # Matrix HTTP Server

use axum::{extract::Extension, http::Method, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use super::bridge::MatrixBridge;
use super::error::MatrixError;
use super::store::MatrixStore;

pub struct MatrixServer {
    port: u16,
    store: Arc<MatrixStore>,
    bridge: Option<Arc<MatrixBridge>>,
}

impl MatrixServer {
    pub fn new(port: u16, store: Arc<MatrixStore>) -> Self {
        Self { port, store, bridge: None }
    }

    pub fn with_bridge(port: u16, store: Arc<MatrixStore>, bridge: Arc<MatrixBridge>) -> Self {
        Self { port, store, bridge: Some(bridge) }
    }

    pub async fn run(&self) -> Result<(), MatrixError> {
        let app = self.create_router();
        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        
        info!("Matrix server starting on {}", addr);
        if self.bridge.is_some() {
            info!("CIS-Matrix Bridge enabled");
        }
        
        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| MatrixError::Internal(format!("Failed to bind: {}", e)))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| MatrixError::Internal(format!("Server error: {}", e)))?;

        Ok(())
    }

    fn create_router(&self) -> Router {
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
            .allow_headers(Any);

        Router::new()
            .route("/_matrix/client/versions", axum::routing::get(super::routes::discovery::versions))
            .route("/_matrix/client/v3/login", axum::routing::post(super::routes::login::login))
            .route("/_matrix/client/v3/sync", axum::routing::get(super::routes::sync::sync))
            .route("/_matrix/client/v3/createRoom", axum::routing::post(super::routes::room::create_room))
            .route("/_matrix/client/v3/join/{room_id}", axum::routing::post(super::routes::room::join_room_post))
            .route("/_matrix/client/v3/rooms/{room_id}/join", axum::routing::post(super::routes::room::join_room))
            .route("/_matrix/client/v3/rooms/{room_id}/messages", axum::routing::get(super::routes::room::get_messages))
            .route("/_matrix/client/v3/rooms/{room_id}/send/{event_type}/{txn_id}", axum::routing::put(super::routes::room::send_message))
            .route("/_matrix/client/v3/rooms/{room_id}/state", axum::routing::get(super::routes::room::get_room_state))
            .route("/.well-known/matrix/client", axum::routing::get(well_known_client))
            .layer(cors)
            .layer(TraceLayer::new_for_http())
            .layer(Extension(self.store.clone()))
            .with_state(self.store.clone())
    }

    pub fn port(&self) -> u16 { self.port }
    pub fn has_bridge(&self) -> bool { self.bridge.is_some() }
}

async fn well_known_client() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"m.homeserver": {"base_url": "http://localhost:7676"}}))
}

pub struct MatrixServerBuilder {
    port: Option<u16>,
    store_path: Option<String>,
    bridge: Option<Arc<MatrixBridge>>,
}

impl MatrixServerBuilder {
    pub fn new() -> Self { Self { port: None, store_path: None, bridge: None } }
    pub fn port(mut self, port: u16) -> Self { self.port = Some(port); self }
    pub fn store_path(mut self, path: impl Into<String>) -> Self { self.store_path = Some(path.into()); self }
    pub fn bridge(mut self, bridge: Arc<MatrixBridge>) -> Self { self.bridge = Some(bridge); self }

    pub fn build(self) -> Result<MatrixServer, MatrixError> {
        let port = self.port.unwrap_or(7676);
        let store = if let Some(path) = self.store_path {
            MatrixStore::open(&path)?
        } else {
            MatrixStore::open_in_memory()?
        };
        let store = Arc::new(store);
        match self.bridge {
            Some(bridge) => Ok(MatrixServer::with_bridge(port, store, bridge)),
            None => Ok(MatrixServer::new(port, store)),
        }
    }
}

impl Default for MatrixServerBuilder { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let store = Arc::new(MatrixStore::open_in_memory().unwrap());
        let server = MatrixServer::new(0, store);
        assert_eq!(server.port(), 0);
        assert!(!server.has_bridge());
    }
}
