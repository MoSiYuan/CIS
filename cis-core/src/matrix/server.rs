//! # Matrix HTTP Server

use axum::{http::Method, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use super::bridge::MatrixBridge;
use super::error::MatrixError;
use super::store::MatrixStore;
use super::store_social::MatrixSocialStore;

/// Matrix HTTP Server
/// 
/// 使用分离的数据库存储：
/// - `store`: MatrixStore (matrix-events.db) - 协议事件存储
/// - `social_store`: MatrixSocialStore (matrix-social.db) - 人类用户数据存储
/// 
/// 这种分离允许：
/// 1. 独立备份/恢复用户数据
/// 2. Skill 化注册逻辑
/// 3. 灵活卸载人类功能而不影响联邦
pub struct MatrixServer {
    port: u16,
    store: Arc<MatrixStore>,
    social_store: Arc<MatrixSocialStore>,
    bridge: Option<Arc<MatrixBridge>>,
}

impl MatrixServer {
    pub fn new(port: u16, store: Arc<MatrixStore>, social_store: Arc<MatrixSocialStore>) -> Self {
        Self { port, store, social_store, bridge: None }
    }

    pub fn with_bridge(
        port: u16,
        store: Arc<MatrixStore>,
        social_store: Arc<MatrixSocialStore>,
        bridge: Arc<MatrixBridge>,
    ) -> Self {
        Self { port, store, social_store, bridge: Some(bridge) }
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

        // Use the routes module's router with both stores
        super::routes::router(self.store.clone(), self.social_store.clone())
            .route("/.well-known/matrix/client", axum::routing::get(well_known_client))
            .layer(cors)
            .layer(TraceLayer::new_for_http())
    }

    pub fn port(&self) -> u16 { self.port }
    pub fn has_bridge(&self) -> bool { self.bridge.is_some() }
}

/// 人机交互端口（对外暴露）: 6767
/// 用于 Matrix 客户端访问和智能体 bearer 鉴权 API
pub const MATRIX_HUMAN_PORT: u16 = 6767;

/// 节点间交互端口（集群内部）: 7676
/// 用于节点间 Matrix 同步、DAG 分发、room 通信等
pub const MATRIX_NODE_PORT: u16 = 7676;

async fn well_known_client() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({"m.homeserver": {"base_url": "http://localhost:6767"}}))
}

/// Matrix Server Builder
/// 
/// 自动创建两个数据库：
/// - matrix-events.db: 协议事件存储
/// - matrix-social.db: 人类用户数据存储
pub struct MatrixServerBuilder {
    port: Option<u16>,
    events_db_path: Option<String>,
    social_db_path: Option<String>,
    bridge: Option<Arc<MatrixBridge>>,
}

impl MatrixServerBuilder {
    pub fn new() -> Self { 
        Self { 
            port: None, 
            events_db_path: None, 
            social_db_path: None, 
            bridge: None 
        } 
    }
    
    pub fn port(mut self, port: u16) -> Self { self.port = Some(port); self }
    
    /// 设置事件数据库路径 (matrix-events.db)
    pub fn events_db_path(mut self, path: impl Into<String>) -> Self { 
        self.events_db_path = Some(path.into()); 
        self 
    }
    
    /// 设置社交数据库路径 (matrix-social.db)
    pub fn social_db_path(mut self, path: impl Into<String>) -> Self { 
        self.social_db_path = Some(path.into()); 
        self 
    }
    
    /// 设置两个数据库的基础目录
    pub fn data_dir(mut self, dir: impl Into<String>) -> Self {
        let dir = dir.into();
        self.events_db_path = Some(format!("{}/matrix-events.db", dir));
        self.social_db_path = Some(format!("{}/matrix-social.db", dir));
        self
    }
    
    /// 兼容旧 API：设置存储路径（默认用于事件数据库）
    #[deprecated(since = "0.2.0", note = "Use events_db_path or data_dir instead")]
    pub fn store_path(mut self, path: impl Into<String>) -> Self { 
        self.events_db_path = Some(path.into()); 
        self 
    }
    
    pub fn bridge(mut self, bridge: Arc<MatrixBridge>) -> Self { self.bridge = Some(bridge); self }

    pub fn build(self) -> Result<MatrixServer, MatrixError> {
        let port = self.port.unwrap_or(MATRIX_HUMAN_PORT);
        
        // 打开或创建事件数据库
        let store = if let Some(path) = self.events_db_path {
            MatrixStore::open(&path)?
        } else {
            MatrixStore::open_in_memory()?
        };
        let store = Arc::new(store);
        
        // 打开或创建社交数据库
        let social_store = if let Some(path) = self.social_db_path {
            MatrixSocialStore::open(&path)?
        } else {
            MatrixSocialStore::open_in_memory()?
        };
        let social_store = Arc::new(social_store);
        
        match self.bridge {
            Some(bridge) => Ok(MatrixServer::with_bridge(port, store, social_store, bridge)),
            None => Ok(MatrixServer::new(port, store, social_store)),
        }
    }
}

impl Default for MatrixServerBuilder { fn default() -> Self { Self::new() } }

/// 便捷函数：使用默认路径创建 Matrix 服务器
/// 
/// 使用 StoragePaths 自动确定数据库位置
#[cfg(feature = "storage")]
pub fn create_with_default_paths(port: Option<u16>) -> Result<MatrixServer, MatrixError> {
    use crate::storage::StoragePaths;
    
    MatrixServerBuilder::new()
        .port(port.unwrap_or(MATRIX_HUMAN_PORT))
        .events_db_path(StoragePaths::matrix_events_db())
        .social_db_path(StoragePaths::matrix_social_db())
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_creation() {
        let store = Arc::new(MatrixStore::open_in_memory().unwrap());
        let social_store = Arc::new(MatrixSocialStore::open_in_memory().unwrap());
        let server = MatrixServer::new(0, store, social_store);
        assert_eq!(server.port(), 0);
        assert!(!server.has_bridge());
    }
    
    #[test]
    fn test_builder_with_data_dir() {
        let temp_dir = std::env::temp_dir().join(format!("cis_test_{}", std::process::id()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        
        let server = MatrixServerBuilder::new()
            .port(0)
            .data_dir(temp_dir.to_str().unwrap())
            .build()
            .unwrap();
        
        assert_eq!(server.port(), 0);
        
        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
