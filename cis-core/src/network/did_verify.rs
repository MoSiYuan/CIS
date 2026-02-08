//! # DID Verification Protocol
//!
//! Challenge-Response protocol for DID-based authentication.
//!
//! ## Protocol Flow
//!
//! ```text
//! Challenger (A)              Responder (B)
//! ─────────────────────────────────────────
//!     │                           │
//!     │  1. DID Challenge         │
//!     │ ───────────────────────►  │
//!     │  {                        │
//!     │    nonce,                 │
//!     │    challenger_did,        │
//!     │    timestamp              │
//!     │  }                        │
//!     │                           │
//!     │  2. DID Response          │
//!     │ ◄───────────────────────  │
//!     │  {                        │
//!     │    responder_did,         │
//!     │    challenge_signature    │
//!     │  }                        │
//!     │                           │
//!     │  3. Verify                │
//!     │     - Parse responder_did │
//!     │     - Extract public key  │
//!     │     - Verify signature    │
//!     │     - Check whitelist     │
//! ```

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

use crate::identity::did::DIDManager;
use crate::network::{AclResult, NetworkAcl, NetworkError};

/// Default challenge timeout in seconds
pub const CHALLENGE_TIMEOUT_SECS: i64 = 30;

/// Challenge nonce length in bytes
pub const NONCE_LENGTH: usize = 32;

/// DID Challenge sent by challenger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidChallenge {
    /// Random nonce (hex encoded)
    pub nonce: String,
    
    /// Challenger's DID
    pub challenger_did: String,
    
    /// Timestamp when challenge was created
    pub timestamp: i64,
    
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: i64,
}

fn default_timeout() -> i64 {
    CHALLENGE_TIMEOUT_SECS
}

impl DidChallenge {
    /// Create new challenge
    pub fn new(challenger_did: impl Into<String>) -> Self {
        let nonce = generate_nonce();
        
        Self {
            nonce: hex::encode(nonce),
            challenger_did: challenger_did.into(),
            timestamp: now(),
            timeout_secs: CHALLENGE_TIMEOUT_SECS,
        }
    }
    
    /// Verify challenge is valid (not expired)
    pub fn verify(&self) -> Result<(), NetworkError> {
        let now = now();
        let age = now - self.timestamp;
        
        if age < 0 {
            return Err(NetworkError::VerificationFailed(
                "Challenge timestamp in future".into()
            ));
        }
        
        if age > self.timeout_secs {
            return Err(NetworkError::VerificationFailed(
                format!("Challenge expired (age={}s, timeout={}s)", age, self.timeout_secs)
            ));
        }
        
        Ok(())
    }
    
    /// Get nonce bytes
    pub fn nonce_bytes(&self) -> Result<Vec<u8>, NetworkError> {
        hex::decode(&self.nonce)
            .map_err(|e| NetworkError::VerificationFailed(format!("Invalid nonce: {}", e)))
    }
}

/// DID Response sent by responder
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidResponse {
    /// Responder's DID
    pub responder_did: String,
    
    /// Signature of the challenge (hex encoded)
    /// Signature = sign(challenge_bytes, responder_private_key)
    pub challenge_signature: String,
}

impl DidResponse {
    /// Create new response by signing challenge
    pub fn new(
        responder_did: impl Into<String>,
        challenge: &DidChallenge,
        did_manager: &DIDManager,
    ) -> Result<Self, NetworkError> {
        let challenge_bytes = serde_json::to_vec(challenge)
            .map_err(|e| NetworkError::VerificationFailed(format!("Failed to serialize challenge: {}", e)))?;
        
        let signature = did_manager.sign_to_hex(&challenge_bytes);
        
        Ok(Self {
            responder_did: responder_did.into(),
            challenge_signature: signature,
        })
    }
    
    /// Verify response against challenge
    pub fn verify(&self, challenge: &DidChallenge) -> Result<VerificationResult, NetworkError> {
        // 1. Verify challenge is not expired
        challenge.verify()?;
        
        // 2. Parse responder DID to get public key
        let verifying_key = parse_did_to_public_key(&self.responder_did)?;
        
        // 3. Serialize challenge
        let challenge_bytes = serde_json::to_vec(challenge)
            .map_err(|e| NetworkError::VerificationFailed(format!("Failed to serialize challenge: {}", e)))?;
        
        // 4. Decode signature
        let signature_bytes = hex::decode(&self.challenge_signature)
            .map_err(|e| NetworkError::VerificationFailed(format!("Invalid signature hex: {}", e)))?;
        
        let signature = Signature::from_slice(&signature_bytes)
            .map_err(|e| NetworkError::VerificationFailed(format!("Invalid signature: {}", e)))?;
        
        // 5. Verify signature
        match verifying_key.verify(&challenge_bytes, &signature) {
            Ok(_) => {
                debug!("DID signature verified for {}", self.responder_did);
                Ok(VerificationResult::Success {
                    did: self.responder_did.clone(),
                    public_key: hex::encode(verifying_key.to_bytes()),
                })
            }
            Err(e) => {
                warn!("DID signature verification failed for {}: {}", self.responder_did, e);
                Err(NetworkError::VerificationFailed("Invalid signature".into()))
            }
        }
    }
}

/// Verification result
#[derive(Debug, Clone)]
pub enum VerificationResult {
    Success {
        did: String,
        public_key: String,
    },
    Failed {
        reason: String,
    },
}

/// DID Verifier handles challenge-response protocol
pub struct DidVerifier {
    did_manager: DIDManager,
    acl: NetworkAcl,
}

impl DidVerifier {
    /// Create new verifier
    pub fn new(did_manager: DIDManager, acl: NetworkAcl) -> Self {
        Self { did_manager, acl }
    }
    
    /// Generate challenge to send to peer
    pub fn generate_challenge(&self) -> DidChallenge {
        DidChallenge::new(self.did_manager.did())
    }
    
    /// Generate response to challenge from peer
    pub fn generate_response(&self, challenge: &DidChallenge) -> Result<DidResponse, NetworkError> {
        DidResponse::new(self.did_manager.did(), challenge, &self.did_manager)
    }
    
    /// Verify response from peer and check ACL
    pub fn verify_response(&self, response: &DidResponse, challenge: &DidChallenge) -> Result<VerifiedPeer, NetworkError> {
        // 1. Verify cryptographic signature
        let result = response.verify(challenge)?;
        
        let did = match result {
            VerificationResult::Success { did, .. } => did,
            VerificationResult::Failed { reason } => {
                return Err(NetworkError::VerificationFailed(reason));
            }
        };
        
        // 2. Check ACL
        match self.acl.check_did(&did) {
            AclResult::Allowed => {
                info!("DID {} verified and allowed", did);
                Ok(VerifiedPeer {
                    did,
                    verified_at: now(),
                })
            }
            AclResult::Denied(reason) => {
                warn!("DID {} verified but denied by ACL: {}", did, reason);
                Err(NetworkError::NotInWhitelist(did))
            }
            AclResult::Quarantine => {
                info!("DID {} verified but quarantined", did);
                Ok(VerifiedPeer {
                    did,
                    verified_at: now(),
                })
            }
        }
    }
    
    /// Quick verify without ACL check (for internal use)
    pub fn verify_signature_only(&self, response: &DidResponse, challenge: &DidChallenge) -> Result<String, NetworkError> {
        let result = response.verify(challenge)?;
        match result {
            VerificationResult::Success { did, .. } => Ok(did),
            VerificationResult::Failed { reason } => Err(NetworkError::VerificationFailed(reason)),
        }
    }
}

/// Verified peer information
#[derive(Debug, Clone)]
pub struct VerifiedPeer {
    pub did: String,
    pub verified_at: i64,
}

/// Parse DID to extract public key
fn parse_did_to_public_key(did: &str) -> Result<VerifyingKey, NetworkError> {
    // DID format: did:cis:{node_id}:{pub_key_short}
    // pub_key_short is first 8 bytes (16 hex chars) of public key
    // Full public key needs to be obtained from DID document or storage
    
    let parts: Vec<&str> = did.split(':').collect();
    if parts.len() != 4 || parts[0] != "did" || parts[1] != "cis" {
        return Err(NetworkError::VerificationFailed(
            format!("Invalid DID format: {}", did)
        ));
    }
    
    let pub_key_short = parts[3];
    if pub_key_short.len() != 16 {
        return Err(NetworkError::VerificationFailed(
            format!("Invalid public key short in DID: {}", pub_key_short)
        ));
    }
    
    // Try to get full public key from storage/network
    // For now, we assume the full key is stored or can be retrieved
    // In a real implementation, this would query a DID resolver
    
    // Placeholder: attempt to decode full key from storage
    match resolve_did_to_full_key(did) {
        Some(key_bytes) => {
            VerifyingKey::from_bytes(&key_bytes)
                .map_err(|e| NetworkError::VerificationFailed(
                    format!("Invalid public key: {}", e)
                ))
        }
        None => Err(NetworkError::VerificationFailed(
            format!("Cannot resolve DID: {}", did)
        ))
    }
}

/// Resolve DID to full public key bytes
/// 
/// In production, this would:
/// Resolve DID to full public key
/// 
/// 从 DID 格式 `did:cis:{node_id}:{public_key_hex}` 提取完整公钥
fn resolve_did_to_full_key(did: &str) -> Option<[u8; 32]> {
    use crate::identity::did::DIDManager;
    
    // 解析 DID
    let (_, public_key_hex) = DIDManager::parse_did(did)?;
    
    // 从十六进制解码公钥
    let bytes = hex::decode(&public_key_hex).ok()?;
    if bytes.len() != 32 {
        return None;
    }
    
    bytes.try_into().ok()
}

/// Generate cryptographically secure nonce
fn generate_nonce() -> [u8; NONCE_LENGTH] {
    use rand::RngCore;
    let mut nonce = [0u8; NONCE_LENGTH];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}

/// Current timestamp
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Pending challenges store (for tracking)
pub struct PendingChallenges {
    challenges: std::collections::HashMap<String, DidChallenge>,
}

impl Default for PendingChallenges {
    fn default() -> Self {
        Self::new()
    }
}

impl PendingChallenges {
    pub fn new() -> Self {
        Self {
            challenges: std::collections::HashMap::new(),
        }
    }
    
    /// Add pending challenge
    pub fn insert(&mut self, challenge: DidChallenge) {
        self.challenges.insert(challenge.nonce.clone(), challenge);
    }
    
    /// Get and remove challenge by nonce
    pub fn take(&mut self, nonce: &str) -> Option<DidChallenge> {
        self.challenges.remove(nonce)
    }
    
    /// Clean up expired challenges
    pub fn cleanup_expired(&mut self) {
        let now = now();
        self.challenges.retain(|_, challenge| {
            now - challenge.timestamp <= challenge.timeout_secs
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests require actual DIDManager instances
    // For unit tests, we would need to mock the signing/verification
    
    #[test]
    fn test_challenge_creation() {
        let challenge = DidChallenge::new("did:cis:challenger:abc123");
        
        assert_eq!(challenge.challenger_did, "did:cis:challenger:abc123");
        assert_eq!(challenge.nonce.len(), NONCE_LENGTH * 2); // hex encoded
        assert!(challenge.verify().is_ok()); // Not expired
    }
    
    #[test]
    fn test_challenge_expiration() {
        let mut challenge = DidChallenge::new("did:cis:challenger:abc123");
        challenge.timestamp = now() - 100; // 100 seconds ago
        challenge.timeout_secs = 30;
        
        assert!(challenge.verify().is_err()); // Should be expired
    }
    
    #[test]
    fn test_did_parsing() {
        // Valid DID
        let did = "did:cis:node:abc12345";
        let parts: Vec<&str> = did.split(':').collect();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0], "did");
        assert_eq!(parts[1], "cis");
        assert_eq!(parts[2], "node");
        assert_eq!(parts[3], "abc12345");
        
        // Invalid DID
        let invalid = "did:other:node:key";
        assert!(parse_did_to_public_key(invalid).is_err());
    }
}
