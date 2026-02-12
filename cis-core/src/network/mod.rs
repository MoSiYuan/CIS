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
//! - Certificate Pinning (TOFU / Strict mode)
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
pub mod acl_rules;
pub mod agent_session;
pub mod audit;
pub mod cert_pinning;
pub mod did_verify;
pub mod rate_limiter;
pub mod session_manager;
pub mod pairing;
pub mod simple_discovery;
pub mod sync;
pub mod websocket;
pub mod websocket_auth;
pub mod websocket_integration;
pub mod clock_tolerance;

#[cfg(test)]
mod acl_tests;



pub use acl::{NetworkAcl, NetworkMode, AclEntry, AclResult};
pub use acl_rules::{
    AclRule, AclRulesEngine, AclAction, Condition, RuleContext, RulesSummary
};
pub use agent_session::{
    AgentSession,
    AgentSessionServer,
    SessionControlMessage,
    SessionInfo,
    SessionManager as AgentSessionManager,
    SessionState,
    SessionId,
    AGENT_SESSION_PORT,
};
pub use cert_pinning::{
    CertificatePinning, MemoryPinStore, SqlitePinStore, PinStore,
    PinEntry, PinningPolicy, PinVerification, HashAlgorithm,
    compute_fingerprint,
};
pub use session_manager::{
    EnhancedSessionManager,
    ManagedSession,
    PersistentSession,
    SessionCheckpoint,
    SessionStats,
    AgentSwitchEvent,
};
pub use did_verify::{DidChallenge, DidResponse, DidVerifier, VerificationResult};
pub use sync::{AclSync, AclUpdateEvent, AclAction as SyncAction};
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
pub use rate_limiter::{
    RateLimiter,
    TokenBucket,
    RateLimitConfig,
    LimitConfig,
    BanConfig,
    LimitType,
};
pub use websocket::{
    WsClient,
    WsServer,
    WsConnection,
    WsConnectionConfig,
    WsNetworkMessage,
    ErrorCode,
    ConnectionState,
    DEFAULT_WS_PORT,
    DEFAULT_CONNECTION_TIMEOUT,
    DEFAULT_HEARTBEAT_INTERVAL,
    MAX_RECONNECT_ATTEMPTS,
};
pub use audit::{AuditLogger, AuditEntry, AuditEventType, Severity};
pub use pairing::{PairingManager, PairingService, PairingSession, PairingState, PairingResult, PairingNodeInfo};
pub use simple_discovery::{SimpleDiscovery, DiscoveredNode};

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

    /// Certificate pinning mismatch detected
    #[error("Certificate pinning mismatch for {domain}: {message}")]
    CertificatePinMismatch { domain: String, message: String },

    /// Certificate pin not found
    #[error("No certificate pin found for {0}")]
    CertificatePinNotFound(String),

    /// Certificate pin expired
    #[error("Certificate pin expired for {0}")]
    CertificatePinExpired(String),
}

impl From<NetworkError> for CisError {
    fn from(e: NetworkError) -> Self {
        CisError::network(e.to_string())
    }
}
