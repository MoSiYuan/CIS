//! # WebSocket Authentication with DID
//!
//! Integrates DID challenge-response authentication into WebSocket handshake.
//!
//! ## Authentication Flow
//!
//! ```text
//! Server                              Client
//! ─────────────────────────────────────────────────────
//!   │                                   │
//!   │  1. WebSocket handshake complete  │
//!   │◄─────────────────────────────────►│
//!   │                                   │
//!   │  2. Send DidChallenge             │
//!   │ ───────────────────────────────►  │
//!   │     {                             │
//!   │       "nonce": "...",             │
//!   │       "challenger_did": "...",    │
//!   │       "timestamp": 1234567890     │
//!   │     }                             │
//!   │                                   │
//!   │  3. Send DidResponse              │
//!   │ ◄───────────────────────────────  │
//!   │     {                             │
//!   │       "responder_did": "...",     │
//!   │       "challenge_signature": "..."│
//!   │     }                             │
//!   │                                   │
//!   │  4. Verify signature              │
//!   │  5. Check ACL (whitelist/blacklist)
//!   │                                   │
//!   │  6. Auth result                   │
//!   │ ───────────────────────────────►  │
//!   │                                   │
//!   │  7. Normal Matrix protocol        │
//!   │◄─────────────────────────────────►│
//! ```
//!
//! ## Usage
//!
//! ### Server-side:
//! ```rust,ignore
//! let auth = WebSocketAuth::new_server(did_manager, acl, connection_id);
//! auth.send_challenge(&ws_sender).await?;
//! ```
//!
//! ### Client-side:
//! ```rust,ignore
//! let auth = WebSocketAuth::new_client(did_manager, connection_id);
//! auth.handle_challenge(challenge, &ws_sender).await?;
//! ```

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

use crate::identity::did::DIDManager;
use crate::network::{
    acl::{AclResult, NetworkAcl},
    did_verify::{DidChallenge, DidResponse, DidVerifier, VerifiedPeer},
    NetworkError,
};

/// Authentication state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthState {
    /// Initial state, connection established but auth not started
    Initial,
    /// Challenge sent by server, waiting for response
    ChallengeSent,
    /// Response received, verifying
    Verifying,
    /// Authentication successful, connection verified
    Verified,
    /// Authentication failed
    Failed,
}

impl AuthState {
    /// Check if authentication is complete (either success or failure)
    pub fn is_complete(&self) -> bool {
        matches!(self, AuthState::Verified | AuthState::Failed)
    }

    /// Check if authentication was successful
    pub fn is_verified(&self) -> bool {
        *self == AuthState::Verified
    }

    /// Check if authentication failed
    pub fn is_failed(&self) -> bool {
        *self == AuthState::Failed
    }
}

impl std::fmt::Display for AuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthState::Initial => write!(f, "initial"),
            AuthState::ChallengeSent => write!(f, "challenge_sent"),
            AuthState::Verifying => write!(f, "verifying"),
            AuthState::Verified => write!(f, "verified"),
            AuthState::Failed => write!(f, "failed"),
        }
    }
}

/// Role of this node in the authentication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthRole {
    /// Acting as server (sends challenge, verifies response)
    Server,
    /// Acting as client (receives challenge, sends response)
    Client,
}

/// WebSocket authentication result
#[derive(Debug, Clone)]
pub enum AuthResult {
    /// Authentication successful
    Success {
        /// Peer DID
        peer_did: String,
        /// When authenticated
        authenticated_at: i64,
    },
    /// Authentication failed
    Failed {
        /// Reason for failure
        reason: String,
    },
    /// Authentication pending (not complete yet)
    Pending,
}

/// Per-connection WebSocket authentication state manager
///
/// This struct manages the authentication state machine for a single
/// WebSocket connection, handling the challenge-response protocol.
pub struct WebSocketAuth {
    /// Connection identifier
    connection_id: String,
    /// This node's role (server or client)
    role: AuthRole,
    /// Current authentication state
    state: AuthState,
    /// DID manager for signing/verification
    did_manager: Arc<DIDManager>,
    /// Network ACL (server only)
    acl: Option<Arc<RwLock<NetworkAcl>>>,
    /// Pending challenge (server stores challenge, client stores for signing)
    pending_challenge: Option<DidChallenge>,
    /// Verified peer info (available after successful auth)
    verified_peer: Option<VerifiedPeer>,
    /// Authentication timeout
    timeout: Duration,
}

impl WebSocketAuth {
    /// Create a new server-side authentication manager
    ///
    /// # Arguments
    /// * `did_manager` - This node's DID manager for signing
    /// * `acl` - Network ACL for whitelist/blacklist checking
    /// * `connection_id` - Unique identifier for this connection
    pub fn new_server(
        did_manager: Arc<DIDManager>,
        acl: Arc<RwLock<NetworkAcl>>,
        connection_id: impl Into<String>,
    ) -> Self {
        Self {
            connection_id: connection_id.into(),
            role: AuthRole::Server,
            state: AuthState::Initial,
            did_manager,
            acl: Some(acl),
            pending_challenge: None,
            verified_peer: None,
            timeout: Duration::from_secs(30),
        }
    }

    /// Create a new client-side authentication manager
    ///
    /// # Arguments
    /// * `did_manager` - This node's DID manager for signing
    /// * `connection_id` - Unique identifier for this connection
    pub fn new_client(
        did_manager: Arc<DIDManager>,
        connection_id: impl Into<String>,
    ) -> Self {
        Self {
            connection_id: connection_id.into(),
            role: AuthRole::Client,
            state: AuthState::Initial,
            did_manager,
            acl: None,
            pending_challenge: None,
            verified_peer: None,
            timeout: Duration::from_secs(30),
        }
    }

    /// Get current authentication state
    pub fn state(&self) -> AuthState {
        self.state
    }

    /// Check if authentication is complete
    pub fn is_complete(&self) -> bool {
        self.state.is_complete()
    }

    /// Check if authentication was successful
    pub fn is_authenticated(&self) -> bool {
        self.state.is_verified()
    }

    /// Get the verified peer's DID (if authenticated)
    pub fn peer_did(&self) -> Option<&str> {
        self.verified_peer.as_ref().map(|p| p.did.as_str())
    }

    /// Get connection ID
    pub fn connection_id(&self) -> &str {
        &self.connection_id
    }

    /// Get role
    pub fn role(&self) -> AuthRole {
        self.role
    }

    /// Set authentication timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    //========================================================================
    // Server-side methods
    //========================================================================

    /// Server: Send DID challenge to client
    ///
    /// This should be called immediately after WebSocket handshake completes.
    ///
    /// # Arguments
    /// * `sender` - Channel to send the challenge message
    ///
    /// # Returns
    /// The challenge that was sent (store this for verification)
    pub async fn send_challenge(
        &mut self,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<DidChallenge, NetworkError> {
        if self.role != AuthRole::Server {
            return Err(NetworkError::VerificationFailed(
                "Only server can send challenge".into()
            ));
        }

        if self.state != AuthState::Initial {
            return Err(NetworkError::VerificationFailed(
                format!("Invalid state for sending challenge: {}", self.state)
            ));
        }

        // Generate challenge
        let challenge = DidChallenge::new(self.did_manager.did());
        self.pending_challenge = Some(challenge.clone());

        // Serialize and send
        let msg = WsAuthMessage::DidChallenge(challenge.clone());
        let json = serde_json::to_string(&msg)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;

        sender.send(json)
            .map_err(|_| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Failed to send challenge"
            )))?;

        self.state = AuthState::ChallengeSent;
        info!(
            connection_id = %self.connection_id,
            challenge_nonce = %challenge.nonce,
            "Sent DID challenge to client"
        );

        Ok(challenge)
    }

    /// Server: Handle DID response from client and verify
    ///
    /// # Arguments
    /// * `response` - The DID response from client
    /// * `sender` - Channel to send the result
    ///
    /// # Returns
    /// The verified peer info if successful
    pub async fn verify_and_accept(
        &mut self,
        response: DidResponse,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<VerifiedPeer, NetworkError> {
        if self.role != AuthRole::Server {
            return Err(NetworkError::VerificationFailed(
                "Only server can verify response".into()
            ));
        }

        if self.state != AuthState::ChallengeSent {
            return Err(NetworkError::VerificationFailed(
                format!("Invalid state for verification: {}", self.state)
            ));
        }

        let challenge = self.pending_challenge.take()
            .ok_or_else(|| NetworkError::VerificationFailed("No pending challenge".into()))?;

        self.state = AuthState::Verifying;
        debug!(
            connection_id = %self.connection_id,
            responder_did = %response.responder_did,
            "Verifying DID response"
        );

        // Create verifier
        let acl = self.acl.as_ref()
            .ok_or_else(|| NetworkError::VerificationFailed("ACL not configured".into()))?;
        let acl_guard = acl.read().await;

        let verifier = DidVerifier::new(
            (*self.did_manager).clone(),
            (*acl_guard).clone()
        );
        drop(acl_guard);

        // Verify response and check ACL
        match verifier.verify_response(&response, &challenge) {
            Ok(verified) => {
                self.state = AuthState::Verified;
                self.verified_peer = Some(verified.clone());

                // Send success result
                let result = WsAuthMessage::AuthResult {
                    success: true,
                    error: None,
                    peer_did: Some(verified.did.clone()),
                };
                let json = serde_json::to_string(&result)
                    .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;

                let _ = sender.send(json);

                info!(
                    connection_id = %self.connection_id,
                    peer_did = %verified.did,
                    "DID authentication successful"
                );

                Ok(verified)
            }
            Err(e) => {
                self.state = AuthState::Failed;

                // Send failure result
                let result = WsAuthMessage::AuthResult {
                    success: false,
                    error: Some(e.to_string()),
                    peer_did: None,
                };
                let json = serde_json::to_string(&result).unwrap_or_default();
                let _ = sender.send(json);

                warn!(
                    connection_id = %self.connection_id,
                    error = %e,
                    "DID authentication failed"
                );

                Err(e)
            }
        }
    }

    //========================================================================
    // Client-side methods
    //========================================================================

    /// Client: Handle challenge from server and send response
    ///
    /// # Arguments
    /// * `challenge` - The DID challenge from server
    /// * `sender` - Channel to send the response
    pub async fn handle_challenge(
        &mut self,
        challenge: DidChallenge,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<(), NetworkError> {
        if self.role != AuthRole::Client {
            return Err(NetworkError::VerificationFailed(
                "Only client can handle challenge".into()
            ));
        }

        if self.state != AuthState::Initial {
            return Err(NetworkError::VerificationFailed(
                format!("Invalid state for handling challenge: {}", self.state)
            ));
        }

        // Store challenge for potential re-signing
        self.pending_challenge = Some(challenge.clone());
        self.state = AuthState::ChallengeSent;

        debug!(
            connection_id = %self.connection_id,
            challenger_did = %challenge.challenger_did,
            "Received DID challenge, generating response"
        );

        // Generate response
        let response = DidResponse::new(
            self.did_manager.did(),
            &challenge,
            &self.did_manager
        )?;

        // Send response
        let msg = WsAuthMessage::DidResponse(response);
        let json = serde_json::to_string(&msg)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;

        sender.send(json)
            .map_err(|_| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Failed to send response"
            )))?;

        info!(
            connection_id = %self.connection_id,
            "Sent DID response to server"
        );

        Ok(())
    }

    /// Client: Handle authentication result from server
    ///
    /// # Arguments
    /// * `success` - Whether authentication was successful
    /// * `error` - Error message if failed
    /// * `peer_did` - Server's DID if successful
    ///
    /// # Returns
    /// Ok if successful, Err with reason if failed
    pub fn handle_auth_result(
        &mut self,
        success: bool,
        error: Option<String>,
        peer_did: Option<String>,
    ) -> Result<(), NetworkError> {
        if self.role != AuthRole::Client {
            return Err(NetworkError::VerificationFailed(
                "Only client can handle auth result".into()
            ));
        }

        if self.state != AuthState::ChallengeSent {
            return Err(NetworkError::VerificationFailed(
                format!("Invalid state for auth result: {}", self.state)
            ));
        }

        if success {
            self.state = AuthState::Verified;
            if let Some(did) = peer_did {
                self.verified_peer = Some(VerifiedPeer {
                    did,
                    verified_at: now(),
                });
            }

            info!(
                connection_id = %self.connection_id,
                peer_did = ?self.peer_did(),
                "DID authentication successful"
            );

            Ok(())
        } else {
            self.state = AuthState::Failed;
            let reason = error.unwrap_or_else(|| "Authentication failed".into());

            warn!(
                connection_id = %self.connection_id,
                reason = %reason,
                "DID authentication failed"
            );

            Err(NetworkError::VerificationFailed(reason))
        }
    }

    //========================================================================
    // Message handling
    //========================================================================

    /// Handle incoming authentication message
    ///
    /// This is the main entry point for processing auth messages.
    /// Returns:
    /// - `Ok(Some(result))` - Authentication complete
    /// - `Ok(None)` - Auth message processed, but auth not complete yet
    /// - `Err(e)` - Error processing message
    pub async fn handle_auth_message(
        &mut self,
        msg: &str,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<Option<AuthResult>, NetworkError> {
        // Try to parse as auth message
        let auth_msg: WsAuthMessage = match serde_json::from_str(msg) {
            Ok(m) => m,
            Err(e) => {
                // Not an auth message
                if self.is_authenticated() {
                    return Ok(None); // Pass through to normal handler
                } else {
                    return Err(NetworkError::VerificationFailed(
                        format!("Expected auth message: {}", e)
                    ));
                }
            }
        };

        match (self.role, auth_msg) {
            // Server handling client response
            (AuthRole::Server, WsAuthMessage::DidResponse(response)) => {
                match self.verify_and_accept(response, sender).await {
                    Ok(verified) => {
                        Ok(Some(AuthResult::Success {
                            peer_did: verified.did,
                            authenticated_at: verified.verified_at,
                        }))
                    }
                    Err(e) => Ok(Some(AuthResult::Failed {
                        reason: e.to_string(),
                    })),
                }
            }

            // Client handling server challenge
            (AuthRole::Client, WsAuthMessage::DidChallenge(challenge)) => {
                self.handle_challenge(challenge, sender).await?;
                Ok(None) // Still waiting for result
            }

            // Client handling auth result
            (AuthRole::Client, WsAuthMessage::AuthResult { success, error, peer_did }) => {
                match self.handle_auth_result(success, error, peer_did) {
                    Ok(()) => {
                        let peer = self.verified_peer.as_ref().unwrap();
                        Ok(Some(AuthResult::Success {
                            peer_did: peer.did.clone(),
                            authenticated_at: peer.verified_at,
                        }))
                    }
                    Err(e) => Ok(Some(AuthResult::Failed {
                        reason: e.to_string(),
                    })),
                }
            }

            // Server receiving unexpected challenge (another server?)
            (AuthRole::Server, WsAuthMessage::DidChallenge(_)) => {
                Err(NetworkError::VerificationFailed(
                    "Server received challenge, expected response".into()
                ))
            }

            // Client receiving unexpected response (out of order?)
            (AuthRole::Client, WsAuthMessage::DidResponse(_)) => {
                Err(NetworkError::VerificationFailed(
                    "Client received response, expected challenge or result".into()
                ))
            }

            // Server receiving auth result (protocol violation)
            (AuthRole::Server, WsAuthMessage::AuthResult { .. }) => {
                Err(NetworkError::VerificationFailed(
                    "Server received auth result, only client should receive this".into()
                ))
            }
        }
    }

    /// Check if a message should be handled as auth message
    ///
    /// Returns true if the message is an auth message that should be
    /// processed by this auth manager.
    pub fn is_auth_message(msg: &str) -> bool {
        serde_json::from_str::<WsAuthMessage>(msg).is_ok()
    }

    /// Reset authentication state (for re-auth)
    pub fn reset(&mut self) {
        self.state = AuthState::Initial;
        self.pending_challenge = None;
        self.verified_peer = None;
        debug!(connection_id = %self.connection_id, "Auth state reset");
    }
}

/// WebSocket authentication message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsAuthMessage {
    /// DID Challenge from server
    DidChallenge(DidChallenge),

    /// DID Response from client
    DidResponse(DidResponse),

    /// Authentication result from server
    AuthResult {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        peer_did: Option<String>,
    },
}

/// WebSocket authentication middleware
///
/// Wraps a WebSocket connection and handles authentication transparently.
pub struct WebSocketAuthMiddleware {
    /// Underlying auth state manager
    auth: WebSocketAuth,
    /// Message channel for sending
    sender: mpsc::UnboundedSender<String>,
}

impl WebSocketAuthMiddleware {
    /// Create new middleware for server role
    pub fn new_server(
        did_manager: Arc<DIDManager>,
        acl: Arc<RwLock<NetworkAcl>>,
        connection_id: impl Into<String>,
        sender: mpsc::UnboundedSender<String>,
    ) -> Self {
        Self {
            auth: WebSocketAuth::new_server(did_manager, acl, connection_id),
            sender,
        }
    }

    /// Create new middleware for client role
    pub fn new_client(
        did_manager: Arc<DIDManager>,
        connection_id: impl Into<String>,
        sender: mpsc::UnboundedSender<String>,
    ) -> Self {
        Self {
            auth: WebSocketAuth::new_client(did_manager, connection_id),
            sender,
        }
    }

    /// Start authentication (server sends challenge)
    pub async fn start_auth(&mut self) -> Result<(), NetworkError> {
        self.auth.send_challenge(&self.sender).await.map(|_| ())
    }

    /// Process incoming message
    ///
    /// Returns:
    /// - `Ok(true)` - Message was handled (auth message)
    /// - `Ok(false)` - Not an auth message, pass to normal handler
    /// - `Err(e)` - Error processing message
    pub async fn process_message(&mut self, msg: &str) -> Result<bool, NetworkError> {
        if !WebSocketAuth::is_auth_message(msg) {
            // Not an auth message
            if !self.auth.is_authenticated() {
                return Err(NetworkError::VerificationFailed(
                    "Not authenticated".into()
                ));
            }
            return Ok(false); // Pass through
        }

        match self.auth.handle_auth_message(msg, &self.sender).await? {
            Some(result) => {
                match result {
                    AuthResult::Success { peer_did, .. } => {
                        info!(peer_did = %peer_did, "Authentication complete");
                    }
                    AuthResult::Failed { reason } => {
                        warn!(reason = %reason, "Authentication failed");
                    }
                    AuthResult::Pending => {}
                }
                Ok(true)
            }
            None => Ok(true), // Auth message processed but not complete
        }
    }

    /// Check if authenticated
    pub fn is_authenticated(&self) -> bool {
        self.auth.is_authenticated()
    }

    /// Get peer DID
    pub fn peer_did(&self) -> Option<&str> {
        self.auth.peer_did()
    }

    /// Get auth state
    pub fn state(&self) -> AuthState {
        self.auth.state()
    }
}

/// ACL check helper
///
/// Checks if a peer is allowed to communicate based on ACL rules
pub async fn check_acl_for_peer(
    acl: &Arc<RwLock<NetworkAcl>>,
    peer_did: &str,
) -> Result<(), NetworkError> {
    let acl_guard = acl.read().await;
    match acl_guard.check_did(peer_did) {
        AclResult::Allowed => Ok(()),
        AclResult::Denied(reason) => {
            warn!(peer_did = %peer_did, reason = %reason, "Peer denied by ACL");
            Err(NetworkError::NotInWhitelist(reason))
        }
        AclResult::Quarantine => {
            info!(peer_did = %peer_did, "Peer in quarantine, allowing restricted communication");
            Ok(())
        }
    }
}

/// Current timestamp helper
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_transitions() {
        assert!(!AuthState::Initial.is_complete());
        assert!(!AuthState::ChallengeSent.is_complete());
        assert!(AuthState::Verified.is_complete());
        assert!(AuthState::Failed.is_complete());

        assert!(AuthState::Verified.is_verified());
        assert!(!AuthState::Initial.is_verified());

        assert!(AuthState::Failed.is_failed());
        assert!(!AuthState::Verified.is_failed());
    }

    #[test]
    fn test_auth_message_serialization() {
        let challenge = DidChallenge::new("did:cis:server:abc123");
        let msg = WsAuthMessage::DidChallenge(challenge);

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("did_challenge"));

        let decoded: WsAuthMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            WsAuthMessage::DidChallenge(c) => {
                assert_eq!(c.challenger_did, "did:cis:server:abc123");
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_auth_result_serialization() {
        let msg = WsAuthMessage::AuthResult {
            success: true,
            error: None,
            peer_did: Some("did:cis:peer:xyz789".into()),
        };

        let json = serde_json::to_string(&msg).unwrap();
        let decoded: WsAuthMessage = serde_json::from_str(&json).unwrap();

        match decoded {
            WsAuthMessage::AuthResult { success, peer_did, .. } => {
                assert!(success);
                assert_eq!(peer_did, Some("did:cis:peer:xyz789".into()));
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn test_is_auth_message() {
        let challenge = DidChallenge::new("did:cis:server:abc123");
        let msg = WsAuthMessage::DidChallenge(challenge);
        let json = serde_json::to_string(&msg).unwrap();

        assert!(WebSocketAuth::is_auth_message(&json));
        assert!(!WebSocketAuth::is_auth_message("{\"not\": \"auth\"}"));
        assert!(!WebSocketAuth::is_auth_message("not json at all"));
    }

    #[test]
    fn test_auth_state_display() {
        assert_eq!(format!("{}", AuthState::Initial), "initial");
        assert_eq!(format!("{}", AuthState::ChallengeSent), "challenge_sent");
        assert_eq!(format!("{}", AuthState::Verifying), "verifying");
        assert_eq!(format!("{}", AuthState::Verified), "verified");
        assert_eq!(format!("{}", AuthState::Failed), "failed");
    }
}
