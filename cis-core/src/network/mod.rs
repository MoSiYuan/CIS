//! # CIS Network Access Control
//!
//! Network admission control based on DID verification and whitelist.
//!
//! ## Features
//!
//! - Manual DID configuration (out-of-band trust establishment)
//! - Immediate DID challenge after WebSocket handshake
//! - Whitelist-based admission control
//! - Solitary mode for complete isolation
//! - DNS-style ACL propagation
//!
//! ## Architecture
//!
//! ```text
//! User Configuration          Network Connection          ACL Sync
//! ───────────────────         ───────────────────         ─────────
//! Add DID to whitelist        WebSocket connect           Broadcast update
//!       │                           │                           │
//!       ▼                           ▼                           ▼
//! ~/.cis/network_acl.toml     DID Challenge/Response      DNS-style propagation
//!       │                           │                           │
//!       └───────────────────────────┴───────────────────────────┘
//!                                   │
//!                                   ▼
//!                         Verification + Whitelist check
//!                                   │
//!                                   ▼
//!                         Allow/Deny communication
//! ```

pub mod acl;
pub mod agent_session;
pub mod did_verify;
pub mod sync;
pub mod websocket_integration;
pub mod websocket_auth;
pub mod audit;

pub use acl::{NetworkAcl, NetworkMode, AclEntry, AclResult};
pub use agent_session::{
    AgentSession,
    AgentSessionServer,
    SessionControlMessage,
    SessionInfo,
    SessionManager,
    SessionState,
    SessionId,
    AGENT_SESSION_PORT,
};
pub use did_verify::{DidChallenge, DidResponse, DidVerifier, VerificationResult};
pub use sync::{AclSync, AclUpdateEvent, AclAction};
pub use websocket_integration::{
    WebSocketAuthenticator, 
    AuthIntegration, 
    AuthMessage as IntegrationAuthMessage, 
    AuthState as IntegrationAuthState,
    AuthenticatedConnection
};
pub use websocket_auth::{
    WebSocketAuth,
    AuthState,
    AuthRole,
    AuthResult,
    WsAuthMessage,
    WebSocketAuthMiddleware,
    check_acl_for_peer,
};
pub use audit::{AuditLogger, AuditEntry, AuditEventType, Severity};

use crate::error::{CisError, Result};
use std::path::PathBuf;

/// Default ACL config path
pub fn default_acl_path() -> PathBuf {
    crate::storage::paths::Paths::config_dir().join("network_acl.toml")
}

/// Initialize network module
pub async fn init() -> Result<()> {
    let acl_path = default_acl_path();
    
    // Create default ACL if not exists
    if !acl_path.exists() {
        let default_acl = NetworkAcl::default();
        default_acl.save(&acl_path)?;
        tracing::info!("Created default network ACL at {:?}", acl_path);
    }
    
    Ok(())
}

/// Network error types
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("DID verification failed: {0}")]
    VerificationFailed(String),
    
    #[error("DID not in whitelist: {0}")]
    NotInWhitelist(String),
    
    #[error("DID in blacklist: {0}")]
    InBlacklist(String),
    
    #[error("Solitary mode: rejecting connection from {0}")]
    SolitaryMode(String),
    
    #[error("ACL version conflict: local={local}, remote={remote}")]
    VersionConflict { local: u64, remote: u64 },
    
    #[error("Invalid ACL signature")]
    InvalidSignature,
    
    #[error("Untrusted updater: {0}")]
    UntrustedUpdater(String),
    
    #[error("Network IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<NetworkError> for CisError {
    fn from(e: NetworkError) -> Self {
        CisError::network(e.to_string())
    }
}
