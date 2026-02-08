//! # ACL Synchronization (DNS-style propagation)
//!
//! Distributes ACL updates across the network in a DNS-style fashion.
//!
//! ## Propagation Model
//!
//! ```text
//! Node A (admin) updates ACL
//!     │
//!     ├───► Node B (verified peer)
//!     │       │
//!     │       ├───► Node D (verified peer)
//!     │       │
//!     │       └───► Node E (verified peer)
//!     │
//!     └───► Node C (verified peer)
//!             │
//!             └───► Node F (verified peer)
//!
//! Each node verifies signature before applying and forwarding.
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::identity::did::DIDManager;
use crate::network::{acl::AclEntry, acl::NetworkAcl, NetworkError};
use ed25519_dalek::Verifier;

/// ACL update actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AclAction {
    /// Add DID to whitelist
    AddToWhitelist,
    /// Remove DID from whitelist
    RemoveFromWhitelist,
    /// Add DID to blacklist
    AddToBlacklist,
    /// Remove DID from blacklist
    RemoveFromBlacklist,
    /// Request full ACL sync
    FullSyncRequest,
    /// Response with full ACL
    FullSyncResponse,
    /// Change network mode
    ChangeMode,
}

/// ACL update event for propagation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclUpdateEvent {
    /// Action type
    pub action: AclAction,
    
    /// Target DID (for add/remove actions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_did: Option<String>,
    
    /// Optional entry details (for add actions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<AclEntry>,
    
    /// Network mode (for ChangeMode action)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    
    /// ACL version after this update
    pub version: u64,
    
    /// Timestamp
    pub timestamp: i64,
    
    /// Updater's DID
    pub updated_by: String,
    
    /// Signature by updater
    pub signature: String,
}

impl AclUpdateEvent {
    /// Create new update event
    pub fn new(
        action: AclAction,
        target_did: Option<String>,
        acl: &NetworkAcl,
        did_manager: &DIDManager,
    ) -> Result<Self, NetworkError> {
        let timestamp = now();
        let version = acl.version + 1;
        
        let mut event = Self {
            action,
            target_did,
            entry: None,
            mode: None,
            version,
            timestamp,
            updated_by: did_manager.did().to_string(),
            signature: String::new(), // Will be set after signing
        };
        
        // Sign the event
        event.sign(did_manager)?;
        
        Ok(event)
    }
    
    /// Sign the event
    pub fn sign(&mut self, did_manager: &DIDManager) -> Result<(), NetworkError> {
        // Create payload without signature
        let payload = AclUpdatePayload {
            action: self.action,
            target_did: self.target_did.clone(),
            entry: self.entry.clone(),
            mode: self.mode.clone(),
            version: self.version,
            timestamp: self.timestamp,
            updated_by: self.updated_by.clone(),
        };
        
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;
        
        self.signature = did_manager.sign_to_hex(&payload_bytes);
        Ok(())
    }
    
    /// Verify event signature
    pub fn verify(&self) -> Result<(), NetworkError> {
        // Reconstruct payload
        let payload = AclUpdatePayload {
            action: self.action,
            target_did: self.target_did.clone(),
            entry: self.entry.clone(),
            mode: self.mode.clone(),
            version: self.version,
            timestamp: self.timestamp,
            updated_by: self.updated_by.clone(),
        };
        
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|e| NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;
        
        // Parse signature
        let signature = hex::decode(&self.signature)
            .map_err(|e| NetworkError::VerificationFailed(format!("Invalid signature hex: {}", e)))?;
        
        // Get public key from updated_by DID
        let verifying_key = resolve_did_to_verifying_key(&self.updated_by)?;
        
        // Verify
        let sig = ed25519_dalek::Signature::from_slice(&signature)
            .map_err(|e| NetworkError::VerificationFailed(format!("Invalid signature: {}", e)))?;
        
        verifying_key.verify(&payload_bytes, &sig)
            .map_err(|_| NetworkError::InvalidSignature)?;
        
        Ok(())
    }
}

/// Payload for signing (without signature field)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AclUpdatePayload {
    pub action: AclAction,
    pub target_did: Option<String>,
    pub entry: Option<AclEntry>,
    pub mode: Option<String>,
    pub version: u64,
    pub timestamp: i64,
    pub updated_by: String,
}

/// ACL synchronizer
pub struct AclSync {
    /// Local ACL
    acl: Arc<RwLock<NetworkAcl>>,
    
    /// DID manager for signing
    did_manager: DIDManager,
    
    /// Pending updates (for deduplication)
    seen_versions: Arc<RwLock<HashMap<String, u64>>>, // peer_did -> last_seen_version
    
    /// Update callbacks
    #[allow(clippy::type_complexity)]
    callbacks: Vec<Box<dyn Fn(&AclUpdateEvent) + Send + Sync>>,
}

impl AclSync {
    /// Create new sync manager
    pub fn new(acl: Arc<RwLock<NetworkAcl>>, did_manager: DIDManager) -> Self {
        Self {
            acl,
            did_manager,
            seen_versions: Arc::new(RwLock::new(HashMap::new())),
            callbacks: Vec::new(),
        }
    }
    
    /// Create update event and apply locally
    pub async fn create_update(
        &self,
        action: AclAction,
        target_did: Option<String>,
    ) -> Result<AclUpdateEvent, NetworkError> {
        let acl = self.acl.read().await;
        let event = AclUpdateEvent::new(action, target_did, &acl, &self.did_manager)?;
        drop(acl);
        
        // Apply locally first
        self.apply_update(&event).await?;
        
        Ok(event)
    }
    
    /// Apply update to local ACL
    pub async fn apply_update(&self, event: &AclUpdateEvent) -> Result<(), NetworkError> {
        let mut acl = self.acl.write().await;
        
        // Check version
        if event.version <= acl.version {
            debug!("Ignoring old ACL update: event.version={}, local.version={}", 
                   event.version, acl.version);
            return Ok(());
        }
        
        // Check version continuity (prevent gaps)
        if event.version > acl.version + 1 {
            warn!("ACL version gap detected: event.version={}, local.version={}",
                  event.version, acl.version);
            // Request full sync
            return Err(NetworkError::VersionConflict {
                local: acl.version,
                remote: event.version,
            });
        }
        
        // Apply action
        match event.action {
            AclAction::AddToWhitelist => {
                if let Some(ref did) = event.target_did {
                    let added_by = event.updated_by.clone();
                    let mut entry = AclEntry::new(did, added_by);
                    if let Some(ref e) = event.entry {
                        entry.reason = e.reason.clone();
                        entry.expires_at = e.expires_at;
                    }
                    
                    // Remove from blacklist first
                    acl.blacklist.retain(|e| &e.did != did);
                    
                    // Add to whitelist
                    if !acl.is_whitelisted(did) {
                        acl.whitelist.push(entry);
                    }
                }
            }
            AclAction::RemoveFromWhitelist => {
                if let Some(ref did) = event.target_did {
                    acl.whitelist.retain(|e| &e.did != did);
                }
            }
            AclAction::AddToBlacklist => {
                if let Some(ref did) = event.target_did {
                    let added_by = event.updated_by.clone();
                    let mut entry = AclEntry::new(did, added_by);
                    if let Some(ref e) = event.entry {
                        entry.reason = e.reason.clone();
                        entry.expires_at = e.expires_at;
                    }
                    
                    // Remove from whitelist first
                    acl.whitelist.retain(|e| &e.did != did);
                    
                    // Add to blacklist
                    if !acl.is_blacklisted(did) {
                        acl.blacklist.push(entry);
                    }
                }
            }
            AclAction::RemoveFromBlacklist => {
                if let Some(ref did) = event.target_did {
                    acl.blacklist.retain(|e| &e.did != did);
                }
            }
            AclAction::ChangeMode => {
                if let Some(ref mode_str) = event.mode {
                    let mode = parse_mode(mode_str)?;
                    acl.mode = mode;
                }
            }
            _ => {
                // Full sync handled separately
            }
        }
        
        // Update version
        acl.version = event.version;
        acl.updated_at = event.timestamp;
        
        info!("Applied ACL update v{}: {:?} {:?}", 
              event.version, event.action, event.target_did);
        
        // Trigger callbacks
        for callback in &self.callbacks {
            callback(event);
        }
        
        Ok(())
    }
    
    /// Receive update from network
    pub async fn receive_update(
        &self,
        event: AclUpdateEvent,
        from_did: &str,
    ) -> Result<bool, NetworkError> {
        // 1. Verify signature
        event.verify()?;
        
        // 2. Check if updater is trusted
        if !self.is_trusted_updater(&event.updated_by).await {
            return Err(NetworkError::UntrustedUpdater(event.updated_by.clone()));
        }
        
        // 3. Check if we've seen this version from this peer
        {
            let seen = self.seen_versions.read().await;
            if let Some(&last_version) = seen.get(from_did) {
                if event.version <= last_version {
                    debug!("Already processed v{} from {}", event.version, from_did);
                    return Ok(false); // Already processed
                }
            }
        }
        
        // 4. Apply update
        match self.apply_update(&event).await {
            Ok(()) => {
                // Record seen version
                let mut seen = self.seen_versions.write().await;
                seen.insert(from_did.to_string(), event.version);
                
                info!("Received and applied ACL update v{} from {}", 
                      event.version, from_did);
                Ok(true)
            }
            Err(NetworkError::VersionConflict { .. }) => {
                // Need full sync
                Err(NetworkError::VersionConflict {
                    local: self.acl.read().await.version,
                    remote: event.version,
                })
            }
            Err(e) => Err(e),
        }
    }
    
    /// Check if updater is trusted
    async fn is_trusted_updater(&self, updater_did: &str) -> bool {
        // Self is always trusted
        if updater_did == self.did_manager.did() {
            return true;
        }
        
        // Check if in whitelist
        let acl = self.acl.read().await;
        acl.is_whitelisted(updater_did)
    }
    
    /// Create full sync request
    pub async fn create_full_sync_request(&self) -> Result<AclUpdateEvent, NetworkError> {
        self.create_update(AclAction::FullSyncRequest, None).await
    }
    
    /// Create full sync response
    pub async fn create_full_sync_response(&self) -> Result<(AclUpdateEvent, NetworkAcl), NetworkError> {
        let acl = self.acl.read().await.clone();
        let event = self.create_update(AclAction::FullSyncResponse, None).await?;
        Ok((event, acl))
    }
    
    /// Apply full sync
    pub async fn apply_full_sync(&self, remote_acl: &NetworkAcl) -> Result<(), NetworkError> {
        let mut local = self.acl.write().await;
        
        // Only apply if remote is newer
        if remote_acl.version <= local.version {
            return Err(NetworkError::VersionConflict {
                local: local.version,
                remote: remote_acl.version,
            });
        }
        
        // Verify signature of remote ACL
        if let Some(ref _sig) = remote_acl.signature {
            remote_acl.verify()?;
        }
        
        // Merge: keep local entries not in remote, update with remote
        let _old_whitelist: HashMap<String, AclEntry> = 
            local.whitelist.drain(..).map(|e| (e.did.clone(), e)).collect();
        let _old_blacklist: HashMap<String, AclEntry> = 
            local.blacklist.drain(..).map(|e| (e.did.clone(), e)).collect();
        
        // Apply remote whitelist
        for entry in &remote_acl.whitelist {
            local.whitelist.push(entry.clone());
        }
        
        // Apply remote blacklist
        for entry in &remote_acl.blacklist {
            local.blacklist.push(entry.clone());
        }
        
        // Update metadata
        local.version = remote_acl.version;
        local.updated_at = remote_acl.updated_at;
        local.mode = remote_acl.mode;
        
        info!("Applied full ACL sync to version {}", local.version);
        
        Ok(())
    }
    
    /// Add callback for update events
    pub fn on_update<F>(&mut self, callback: F)
    where
        F: Fn(&AclUpdateEvent) + Send + Sync + 'static,
    {
        self.callbacks.push(Box::new(callback));
    }
    
    /// Get current ACL version
    pub async fn current_version(&self) -> u64 {
        self.acl.read().await.version
    }
}

/// Parse mode string
fn parse_mode(mode: &str) -> Result<crate::network::acl::NetworkMode, NetworkError> {
    match mode {
        "open" => Ok(crate::network::acl::NetworkMode::Open),
        "whitelist" => Ok(crate::network::acl::NetworkMode::Whitelist),
        "solitary" => Ok(crate::network::acl::NetworkMode::Solitary),
        "quarantine" => Ok(crate::network::acl::NetworkMode::Quarantine),
        _ => Err(NetworkError::VerificationFailed(format!("Invalid mode: {}", mode))),
    }
}

/// Resolve DID to verifying key
/// 
/// 解析 DID 格式 `did:cis:{node_id}:{public_key_hex}` 提取公钥
fn resolve_did_to_verifying_key(did: &str) -> Result<ed25519_dalek::VerifyingKey, NetworkError> {
    use crate::identity::did::DIDManager;
    
    // 解析 DID
    let (_, public_key_hex) = DIDManager::parse_did(did)
        .ok_or_else(|| NetworkError::VerificationFailed(format!("Invalid DID format: {}", did)))?;
    
    // 从十六进制解析公钥
    DIDManager::verifying_key_from_hex(&public_key_hex)
        .map_err(|e| NetworkError::VerificationFailed(format!("Failed to parse public key: {}", e)))
}

/// Current timestamp
fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    // Note: These tests require actual DIDManager instances
    // For comprehensive testing, mock implementations would be needed
    
    #[test]
    fn test_acl_action_serialization() {
        let actions = vec![
            AclAction::AddToWhitelist,
            AclAction::RemoveFromWhitelist,
            AclAction::AddToBlacklist,
            AclAction::RemoveFromBlacklist,
        ];
        
        for action in actions {
            let json = serde_json::to_string(&action).unwrap();
            let decoded: AclAction = serde_json::from_str(&json).unwrap();
            assert_eq!(action, decoded);
        }
    }
}
