//! # Network Access Control List (ACL)
//!
//! Manages whitelist/blacklist for network admission control.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};



/// Network admission mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum NetworkMode {
    /// Open mode - accept any verified DID (insecure, for testing only)
    Open,
    
    /// Whitelist mode - only allow whitelisted DIDs (recommended)
    #[default]
    Whitelist,
    
    /// Solitary mode - reject all new connections, only talk to existing peers
    Solitary,
    
    /// Quarantine mode - allow connection but don't forward data
    Quarantine,
}


impl std::fmt::Display for NetworkMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkMode::Open => write!(f, "open"),
            NetworkMode::Whitelist => write!(f, "whitelist"),
            NetworkMode::Solitary => write!(f, "solitary"),
            NetworkMode::Quarantine => write!(f, "quarantine"),
        }
    }
}

/// ACL entry for whitelist or blacklist
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AclEntry {
    /// DID
    pub did: String,
    
    /// When added
    pub added_at: i64,
    
    /// Who added (DID of the node that added this entry)
    pub added_by: String,
    
    /// Optional reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    
    /// Optional expiration timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

impl AclEntry {
    /// Create new ACL entry
    pub fn new(did: impl Into<String>, added_by: impl Into<String>) -> Self {
        Self {
            did: did.into(),
            added_at: now(),
            added_by: added_by.into(),
            reason: None,
            expires_at: None,
        }
    }
    
    /// Set reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
    
    /// Set expiration
    pub fn with_expiration(mut self, expires_at: i64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
    
    /// Check if entry is expired
    pub fn is_expired(&self) -> bool {
        self.expires_at.map(|exp| now() > exp).unwrap_or(false)
    }
}

/// Result of ACL check
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AclResult {
    /// Allowed
    Allowed,
    /// Denied with reason
    Denied(String),
    /// Quarantine (allow connection but restrict forwarding)
    Quarantine,
}

/// Network ACL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkAcl {
    /// Local node DID
    pub local_did: String,
    
    /// Admission mode
    #[serde(default)]
    pub mode: NetworkMode,
    
    /// Whitelist - explicitly allowed DIDs
    #[serde(default)]
    pub whitelist: Vec<AclEntry>,
    
    /// Blacklist - explicitly denied DIDs
    #[serde(default)]
    pub blacklist: Vec<AclEntry>,
    
    /// ACL version (monotonically increasing)
    #[serde(default = "default_version")]
    pub version: u64,
    
    /// Last update timestamp
    #[serde(default = "now")]
    pub updated_at: i64,
    
    /// Signature of this ACL (to prevent tampering)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

fn default_version() -> u64 {
    1
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

impl Default for NetworkAcl {
    fn default() -> Self {
        Self {
            local_did: String::new(),
            mode: NetworkMode::Whitelist,
            whitelist: Vec::new(),
            blacklist: Vec::new(),
            version: 1,
            updated_at: now(),
            signature: None,
        }
    }
}

impl NetworkAcl {
    /// Create new ACL with local DID
    pub fn new(local_did: impl Into<String>) -> Self {
        Self {
            local_did: local_did.into(),
            mode: NetworkMode::Whitelist,
            whitelist: Vec::new(),
            blacklist: Vec::new(),
            version: 1,
            updated_at: now(),
            signature: None,
        }
    }
    
    /// Load ACL from file
    pub fn load(path: impl AsRef<Path>) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let acl: NetworkAcl = toml::from_str(&content)
            .map_err(|e| crate::error::CisError::configuration(format!("Invalid ACL format: {}", e)))?;
        Ok(acl)
    }
    
    /// Save ACL to file
    pub fn save(&self, path: impl AsRef<Path>) -> crate::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        
        let content = toml::to_string_pretty(self)
            .map_err(|e| crate::error::CisError::serialization(e.to_string()))?;
        
        std::fs::write(&path, content)?;
        
        // Set restrictive permissions (Unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)?.permissions();
            perms.set_mode(0o600); // Owner read/write only
            std::fs::set_permissions(&path, perms)?;
        }
        
        Ok(())
    }
    
    /// Check if DID is allowed
    pub fn check_did(&self, did: &str) -> AclResult {
        // Check mode first
        match self.mode {
            NetworkMode::Solitary => {
                // In solitary mode, only allow if explicitly whitelisted
                if self.is_whitelisted(did) {
                    AclResult::Allowed
                } else {
                    AclResult::Denied("Solitary mode: not in whitelist".into())
                }
            }
            
            NetworkMode::Whitelist => {
                // Check blacklist first
                if self.is_blacklisted(did) {
                    return AclResult::Denied("In blacklist".into());
                }
                
                // Then check whitelist
                if self.is_whitelisted(did) {
                    AclResult::Allowed
                } else {
                    AclResult::Denied("Not in whitelist".into())
                }
            }
            
            NetworkMode::Open => {
                // In open mode, only check blacklist
                if self.is_blacklisted(did) {
                    AclResult::Denied("In blacklist".into())
                } else {
                    AclResult::Allowed
                }
            }
            
            NetworkMode::Quarantine => {
                if self.is_blacklisted(did) {
                    AclResult::Denied("In blacklist".into())
                } else {
                    AclResult::Quarantine
                }
            }
        }
    }
    
    /// Check if DID is in whitelist
    pub fn is_whitelisted(&self, did: &str) -> bool {
        self.whitelist.iter().any(|e| e.did == did && !e.is_expired())
    }
    
    /// Check if DID is in blacklist
    pub fn is_blacklisted(&self, did: &str) -> bool {
        self.blacklist.iter().any(|e| e.did == did && !e.is_expired())
    }
    
    /// Add DID to whitelist
    pub fn allow(&mut self, did: impl Into<String>, added_by: impl Into<String>) -> &mut Self {
        let did = did.into();
        let added_by = added_by.into();
        let did_clone = did.clone();
        
        // Remove from blacklist if present
        self.blacklist.retain(|e| e.did != did);
        
        // Add to whitelist if not already present
        if !self.is_whitelisted(&did) {
            self.whitelist.push(AclEntry::new(did, added_by));
            self.bump_version();
            info!("Added {} to whitelist", did_clone);
        }
        
        self
    }
    
    /// Add DID to blacklist
    pub fn deny(&mut self, did: impl Into<String>, added_by: impl Into<String>) -> &mut Self {
        let did = did.into();
        let added_by = added_by.into();
        let did_clone = did.clone();
        
        // Remove from whitelist if present
        self.whitelist.retain(|e| e.did != did);
        
        // Add to blacklist if not already present
        if !self.is_blacklisted(&did) {
            self.blacklist.push(AclEntry::new(did, added_by));
            self.bump_version();
            info!("Added {} to blacklist", did_clone);
        }
        
        self
    }
    
    /// Remove from whitelist
    pub fn unallow(&mut self, did: &str) -> bool {
        let len = self.whitelist.len();
        self.whitelist.retain(|e| e.did != did);
        if self.whitelist.len() < len {
            self.bump_version();
            info!("Removed {} from whitelist", did);
            true
        } else {
            false
        }
    }
    
    /// Remove from blacklist
    pub fn undeny(&mut self, did: &str) -> bool {
        let len = self.blacklist.len();
        self.blacklist.retain(|e| e.did != did);
        if self.blacklist.len() < len {
            self.bump_version();
            info!("Removed {} from blacklist", did);
            true
        } else {
            false
        }
    }
    
    /// Set mode
    pub fn set_mode(&mut self, mode: NetworkMode) -> &mut Self {
        if self.mode != mode {
            self.mode = mode;
            self.bump_version();
            info!("Changed network mode to {}", mode);
        }
        self
    }
    
    /// Enter solitary mode
    pub fn enter_solitary_mode(&mut self) -> &mut Self {
        self.set_mode(NetworkMode::Solitary);
        warn!("Entered solitary mode. Only whitelisted peers can connect.");
        self
    }
    
    /// Bump version
    pub fn bump_version(&mut self) {
        self.version += 1;
        self.updated_at = now();
    }
    
    /// Get summary
    pub fn summary(&self) -> AclSummary {
        AclSummary {
            mode: self.mode,
            whitelist_count: self.whitelist.len(),
            blacklist_count: self.blacklist.len(),
            version: self.version,
            updated_at: self.updated_at,
        }
    }
    
    /// Clean up expired entries
    pub fn cleanup_expired(&mut self) {
        let before_w = self.whitelist.len();
        let before_b = self.blacklist.len();
        
        self.whitelist.retain(|e| !e.is_expired());
        self.blacklist.retain(|e| !e.is_expired());
        
        let removed_w = before_w - self.whitelist.len();
        let removed_b = before_b - self.blacklist.len();
        
        if removed_w > 0 || removed_b > 0 {
            info!("Cleaned up {} expired whitelist and {} expired blacklist entries", removed_w, removed_b);
            self.bump_version();
        }
    }
    
    /// Verify ACL signature
    pub fn verify(&self) -> Result<(), crate::network::NetworkError> {
        use ed25519_dalek::Verifier;
        use crate::identity::did::DIDManager;
        
        let signature = match &self.signature {
            Some(sig) => sig,
            None => return Err(crate::network::NetworkError::VerificationFailed(
                "ACL has no signature".to_string()
            )),
        };
        
        // Create payload without signature for verification
        let payload = NetworkAclPayload {
            local_did: self.local_did.clone(),
            mode: self.mode,
            whitelist: self.whitelist.clone(),
            blacklist: self.blacklist.clone(),
            version: self.version,
            updated_at: self.updated_at,
        };
        
        let payload_bytes = serde_json::to_vec(&payload)
            .map_err(|e| crate::network::NetworkError::VerificationFailed(format!("Serialization failed: {}", e)))?;
        
        // Parse signature
        let signature_bytes = hex::decode(signature)
            .map_err(|e| crate::network::NetworkError::VerificationFailed(format!("Invalid signature hex: {}", e)))?;
        
        // Get public key from local_did
        let (_, public_key_hex) = DIDManager::parse_did(&self.local_did)
            .ok_or_else(|| crate::network::NetworkError::VerificationFailed(format!("Invalid DID format: {}", self.local_did)))?;
        
        let verifying_key = DIDManager::verifying_key_from_hex(&public_key_hex)
            .map_err(|e| crate::network::NetworkError::VerificationFailed(format!("Failed to parse public key: {}", e)))?;
        
        // Verify
        let sig = ed25519_dalek::Signature::from_slice(&signature_bytes)
            .map_err(|e| crate::network::NetworkError::VerificationFailed(format!("Invalid signature: {}", e)))?;
        
        verifying_key.verify(&payload_bytes, &sig)
            .map_err(|_| crate::network::NetworkError::InvalidSignature)?;
        
        Ok(())
    }
}

/// Payload for ACL signing (without signature field)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct NetworkAclPayload {
    pub local_did: String,
    pub mode: NetworkMode,
    pub whitelist: Vec<AclEntry>,
    pub blacklist: Vec<AclEntry>,
    pub version: u64,
    pub updated_at: i64,
}

/// ACL summary for display
#[derive(Debug, Clone)]
pub struct AclSummary {
    pub mode: NetworkMode,
    pub whitelist_count: usize,
    pub blacklist_count: usize,
    pub version: u64,
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acl_whitelist_mode() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        acl.set_mode(NetworkMode::Whitelist);
        
        // Not in whitelist
        assert!(matches!(
            acl.check_did("did:cis:unknown:xyz789"),
            AclResult::Denied(_)
        ));
        
        // Add to whitelist
        acl.allow("did:cis:friend:def456", "did:cis:local:abc123");
        assert_eq!(acl.check_did("did:cis:friend:def456"), AclResult::Allowed);
        
        // Add to blacklist
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert!(matches!(
            acl.check_did("did:cis:enemy:bad999"),
            AclResult::Denied(_)
        ));
        
        // Allow moves from blacklist to whitelist
        acl.allow("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert_eq!(acl.check_did("did:cis:enemy:bad999"), AclResult::Allowed);
    }

    #[test]
    fn test_acl_solitary_mode() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        acl.enter_solitary_mode();
        
        // Not in whitelist - denied
        assert!(matches!(
            acl.check_did("did:cis:anyone:xyz789"),
            AclResult::Denied(_)
        ));
        
        // In whitelist - allowed
        acl.allow("did:cis:friend:def456", "did:cis:local:abc123");
        assert_eq!(acl.check_did("did:cis:friend:def456"), AclResult::Allowed);
    }

    #[test]
    fn test_acl_version_bump() {
        let mut acl = NetworkAcl::new("did:cis:local:abc123");
        let v1 = acl.version;
        
        acl.allow("did:cis:friend:def456", "did:cis:local:abc123");
        assert_eq!(acl.version, v1 + 1);
        
        acl.deny("did:cis:enemy:bad999", "did:cis:local:abc123");
        assert_eq!(acl.version, v1 + 2);
    }

    #[test]
    fn test_acl_entry_expiration() {
        let mut entry = AclEntry::new("did:test", "did:local");
        assert!(!entry.is_expired());
        
        // Set expiration in the past
        entry.expires_at = Some(1);
        assert!(entry.is_expired());
    }
}
