//! # WebSocket Integration for DID Verification
//!
//! Integrates DID verification into WebSocket handshake.
//!
//! ## Flow
//!
//! ```text
//! Client                              Server
//! ─────────────────────────────────────────────────
//!   │                                   │
//!   │  1. WebSocket handshake           │
//!   │ ───────────────────────────────►  │
//!   │                                   │
//!   │  2. Server sends DID Challenge    │
//!   │ ◄───────────────────────────────  │
//!   │     {                             │
//!   │       "type": "did_challenge",    │
//!   │       "nonce": "...",             │
//!   │       "challenger_did": "...",    │
//!   │       "timestamp": 1234567890     │
//!   │     }                             │
//!   │                                   │
//!   │  3. Client sends DID Response     │
//!   │ ───────────────────────────────►  │
//!   │     {                             │
//!   │       "type": "did_response",     │
//!   │       "responder_did": "...",     │
//!   │       "challenge_signature": "..."│
//!   │     }                             │
//!   │                                   │
//!   │  4. Server verifies               │
//!   │     - Signature valid?            │
//!   │     - DID in whitelist?           │
//!   │     - Not in blacklist?           │
//!   │                                   │
//!   │  5. Result                        │
//!   │ ◄───────────────────────────────  │
//!   │     {                             │
//!   │       "type": "auth_result",      │
//!   │       "success": true/false,      │
//!   │       "error": "..." (optional)   │
//!   │     }                             │
//!   │                                   │
//!   │  6. If success, normal Matrix     │
//!   │     protocol begins               │
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use tracing::{debug, info};

use crate::identity::did::DIDManager;
use crate::network::{
    did_verify::{DidChallenge, DidResponse, DidVerifier, PendingChallenges},
    acl::{AclResult, NetworkAcl},
    NetworkError,
};

/// WebSocket message types for DID verification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMessage {
    /// DID Challenge from server
    DidChallenge(DidChallenge),
    
    /// DID Response from client
    DidResponse(DidResponse),
    
    /// Authentication result
    AuthResult {
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        peer_did: Option<String>,
    },
}

/// Connection state during authentication
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthState {
    /// Initial state, waiting for handshake
    Initial,
    /// Challenge sent, waiting for response
    ChallengeSent,
    /// Response received, verifying
    Verifying,
    /// Authentication successful
    Authenticated,
    /// Authentication failed
    Failed,
}

/// Authenticated connection info
#[derive(Debug, Clone)]
pub struct AuthenticatedConnection {
    /// Peer DID (only available after authentication)
    pub peer_did: Option<String>,
    
    /// Authentication state
    pub auth_state: AuthState,
    
    /// When authentication completed
    pub authenticated_at: Option<i64>,
}

impl AuthenticatedConnection {
    pub fn new() -> Self {
        Self {
            peer_did: None,
            auth_state: AuthState::Initial,
            authenticated_at: None,
        }
    }
    
    pub fn is_authenticated(&self) -> bool {
        self.auth_state == AuthState::Authenticated
    }
}

/// WebSocket authenticator
pub struct WebSocketAuthenticator {
    /// DID manager for signing
    did_manager: DIDManager,
    
    /// Network ACL
    acl: Arc<RwLock<NetworkAcl>>,
    
    /// Pending challenges (for server side)
    pending_challenges: Arc<RwLock<PendingChallenges>>,
    
    /// Authenticated connections
    connections: Arc<RwLock<HashMap<String, AuthenticatedConnection>>>,
    
    /// Challenge timeout
    challenge_timeout: Duration,
}

impl WebSocketAuthenticator {
    /// Create new authenticator
    pub fn new(did_manager: DIDManager, acl: Arc<RwLock<NetworkAcl>>) -> Self {
        Self {
            did_manager,
            acl,
            pending_challenges: Arc::new(RwLock::new(PendingChallenges::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
            challenge_timeout: Duration::from_secs(30),
        }
    }
    
    /// Server: Send challenge to client after WebSocket handshake
    pub async fn server_send_challenge(
        &self,
        connection_id: &str,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<DidChallenge, NetworkError> {
        let challenge = DidChallenge::new(self.did_manager.did());
        
        // Store pending challenge
        {
            let mut pending = self.pending_challenges.write().await;
            pending.insert(challenge.clone());
        }
        
        // Send challenge
        let msg = AuthMessage::DidChallenge(challenge.clone());
        let json = serde_json::to_string(&msg)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;
        
        sender.send(json)
            .map_err(|_| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Failed to send challenge"
            )))?;
        
        // Update connection state
        {
            let mut conns = self.connections.write().await;
            conns.insert(connection_id.to_string(), AuthenticatedConnection {
                peer_did: None,
                auth_state: AuthState::ChallengeSent,
                authenticated_at: None,
            });
        }
        
        info!("Sent DID challenge to {}", connection_id);
        Ok(challenge)
    }
    
    /// Server: Handle response from client
    pub async fn server_handle_response(
        &self,
        connection_id: &str,
        response: DidResponse,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<AuthenticatedConnection, NetworkError> {
        // Get and remove pending challenge
        let challenge = {
            let mut pending = self.pending_challenges.write().await;
            pending.take(&response.challenge_signature)
                .ok_or_else(|| NetworkError::VerificationFailed("No pending challenge found".into()))?
        };
        
        // Verify response
        let verifier = DidVerifier::new(
            self.did_manager.clone(),
            self.acl.read().await.clone()
        );
        
        let verified = match verifier.verify_response(&response, &challenge) {
            Ok(v) => v,
            Err(e) => {
                // Send failure result
                let result = AuthMessage::AuthResult {
                    success: false,
                    error: Some(e.to_string()),
                    peer_did: None,
                };
                let _ = sender.send(serde_json::to_string(&result).unwrap_or_default());
                
                // Update connection state
                {
                    let mut conns = self.connections.write().await;
                    if let Some(conn) = conns.get_mut(connection_id) {
                        conn.auth_state = AuthState::Failed;
                    }
                }
                
                return Err(e);
            }
        };
        
        // Success! Update connection
        let conn = AuthenticatedConnection {
            peer_did: Some(verified.did.clone()),
            auth_state: AuthState::Authenticated,
            authenticated_at: Some(now()),
        };
        
        {
            let mut conns = self.connections.write().await;
            conns.insert(connection_id.to_string(), conn.clone());
        }
        
        // Send success result
        let result = AuthMessage::AuthResult {
            success: true,
            error: None,
            peer_did: Some(verified.did),
        };
        let json = serde_json::to_string(&result)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;
        
        sender.send(json)
            .map_err(|_| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Failed to send auth result"
            )))?;
        
        info!("Authenticated connection {} with DID {}", connection_id, conn.peer_did.as_ref().unwrap());
        Ok(conn)
    }
    
    /// Client: Handle challenge from server and send response
    pub async fn client_handle_challenge(
        &self,
        challenge: DidChallenge,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<(), NetworkError> {
        // Generate response
        let verifier = DidVerifier::new(
            self.did_manager.clone(),
            self.acl.read().await.clone()
        );
        
        let response = verifier.generate_response(&challenge)?;
        
        // Send response
        let msg = AuthMessage::DidResponse(response);
        let json = serde_json::to_string(&msg)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;
        
        sender.send(json)
            .map_err(|_| NetworkError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "Failed to send response"
            )))?;
        
        info!("Sent DID response to {}", challenge.challenger_did);
        Ok(())
    }
    
    /// Client: Handle auth result from server
    pub async fn client_handle_result(
        &self,
        result: AuthMessage,
    ) -> Result<AuthenticatedConnection, NetworkError> {
        match result {
            AuthMessage::AuthResult { success, error, peer_did } => {
                if success {
                    let conn = AuthenticatedConnection {
                        peer_did,
                        auth_state: AuthState::Authenticated,
                        authenticated_at: Some(now()),
                    };
                    info!("Authentication successful, peer DID: {:?}", conn.peer_did);
                    Ok(conn)
                } else {
                    Err(NetworkError::VerificationFailed(
                        error.unwrap_or_else(|| "Authentication failed".into())
                    ))
                }
            }
            _ => Err(NetworkError::VerificationFailed("Unexpected message type".into())),
        }
    }
    
    /// Check if connection is authenticated
    pub async fn is_authenticated(&self, connection_id: &str) -> bool {
        let conns = self.connections.read().await;
        conns.get(connection_id)
            .map(|conn| conn.is_authenticated())
            .unwrap_or(false)
    }
    
    /// Get peer DID for connection
    pub async fn get_peer_did(&self, connection_id: &str) -> Option<String> {
        let conns = self.connections.read().await;
        conns.get(connection_id)
            .and_then(|conn| conn.peer_did.clone())
    }
    
    /// Remove connection
    pub async fn remove_connection(&self, connection_id: &str) {
        let mut conns = self.connections.write().await;
        conns.remove(connection_id);
    }
    
    /// Check if peer is allowed to communicate
    pub async fn check_communication_allowed(&self, connection_id: &str) -> Result<(), NetworkError> {
        let conns = self.connections.read().await;
        let conn = conns.get(connection_id)
            .ok_or_else(|| NetworkError::VerificationFailed("Connection not found".into()))?;
        
        if !conn.is_authenticated() {
            return Err(NetworkError::VerificationFailed("Not authenticated".into()));
        }
        
        let peer_did = conn.peer_did.as_ref()
            .ok_or_else(|| NetworkError::VerificationFailed("Peer DID not available".into()))?;
        
        // Check ACL
        let acl = self.acl.read().await;
        match acl.check_did(peer_did) {
            AclResult::Allowed => Ok(()),
            AclResult::Denied(reason) => Err(NetworkError::NotInWhitelist(reason)),
            AclResult::Quarantine => Ok(()), // Allow but restricted
        }
    }
    
    /// Clean up expired challenges periodically
    pub async fn cleanup_task(&self) {
        let pending = self.pending_challenges.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let mut challenges = pending.write().await;
                challenges.cleanup_expired();
                
                debug!("Cleaned up expired challenges");
            }
        });
    }
}

/// Current timestamp
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Integration helper for WebSocket handlers
pub struct AuthIntegration {
    authenticator: Arc<WebSocketAuthenticator>,
}

impl AuthIntegration {
    pub fn new(authenticator: Arc<WebSocketAuthenticator>) -> Self {
        Self { authenticator }
    }
    
    /// Handle incoming message during auth phase
    pub async fn handle_auth_message(
        &self,
        connection_id: &str,
        msg: &str,
        sender: &mpsc::UnboundedSender<String>,
    ) -> Result<Option<AuthenticatedConnection>, NetworkError> {
        // Try to parse as auth message
        let auth_msg: AuthMessage = match serde_json::from_str(msg) {
            Ok(m) => m,
            Err(_) => {
                // Not an auth message, check if already authenticated
                if self.authenticator.is_authenticated(connection_id).await {
                    return Ok(None); // Pass through to normal handler
                } else {
                    return Err(NetworkError::VerificationFailed(
                        "Expected auth message, got something else".into()
                    ));
                }
            }
        };
        
        match auth_msg {
            AuthMessage::DidResponse(response) => {
                // Server handling client response
                let conn = self.authenticator
                    .server_handle_response(connection_id, response, sender)
                    .await?;
                Ok(Some(conn))
            }
            AuthMessage::DidChallenge(challenge) => {
                // Client handling server challenge
                self.authenticator.client_handle_challenge(challenge, sender).await?;
                Ok(None) // Wait for result
            }
            AuthMessage::AuthResult { success, error, peer_did } => {
                // Client handling result
                let result = AuthMessage::AuthResult { success, error, peer_did };
                let conn = self.authenticator
                    .client_handle_result(result)
                    .await?;
                Ok(Some(conn))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Tests would require mocking DIDManager
    // For now, just test message serialization
    
    #[test]
    fn test_auth_message_serialization() {
        let challenge = DidChallenge::new("did:cis:server:abc123");
        let msg = AuthMessage::DidChallenge(challenge);
        
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("did_challenge"));
        
        let decoded: AuthMessage = serde_json::from_str(&json).unwrap();
        match decoded {
            AuthMessage::DidChallenge(c) => {
                assert_eq!(c.challenger_did, "did:cis:server:abc123");
            }
            _ => panic!("Wrong message type"),
        }
    }
    
    #[test]
    fn test_auth_result_serialization() {
        let msg = AuthMessage::AuthResult {
            success: true,
            error: None,
            peer_did: Some("did:cis:peer:xyz789".into()),
        };
        
        let json = serde_json::to_string(&msg).unwrap();
        let decoded: AuthMessage = serde_json::from_str(&json).unwrap();
        
        match decoded {
            AuthMessage::AuthResult { success, peer_did, .. } => {
                assert!(success);
                assert_eq!(peer_did, Some("did:cis:peer:xyz789".into()));
            }
            _ => panic!("Wrong message type"),
        }
    }
}
