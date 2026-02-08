//! # Security Audit Log
//!
//! Records security-relevant events for analysis and forensics.
//!
//! ## Events
//!
//! - DID verification attempts (success/failure)
//! - ACL changes (add/remove from whitelist/blacklist)
//! - Network mode changes
//! - Blocked communication attempts
//! - Authentication failures

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Security event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    /// DID verification succeeded
    DidVerificationSuccess,
    /// DID verification failed
    DidVerificationFailure,
    /// Connection blocked by ACL
    ConnectionBlocked,
    /// Communication blocked (already connected but ACL changed)
    CommunicationBlocked,
    /// Added to whitelist
    WhitelistAdd,
    /// Removed from whitelist
    WhitelistRemove,
    /// Added to blacklist
    BlacklistAdd,
    /// Removed from blacklist
    BlacklistRemove,
    /// Network mode changed
    ModeChange,
    /// Authentication attempt (WebSocket)
    AuthAttempt,
    /// Authentication success
    AuthSuccess,
    /// Authentication failure
    AuthFailure,
    /// ACL synced from peer
    AclSync,
    /// Solitary mode triggered
    SolitaryTriggered,
}

/// Severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Timestamp (seconds since epoch)
    pub timestamp: i64,
    
    /// Event type
    pub event_type: AuditEventType,
    
    /// Severity
    pub severity: Severity,
    
    /// Subject (who performed the action)
    pub subject: String,
    
    /// Object (who was affected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    
    /// Event details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    
    /// Result (success/failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    
    /// Reason (for failures)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl AuditEntry {
    /// Create new audit entry
    pub fn new(
        event_type: AuditEventType,
        severity: Severity,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            timestamp: now(),
            event_type,
            severity,
            subject: subject.into(),
            object: None,
            details: None,
            result: None,
            reason: None,
        }
    }
    
    /// Set object
    pub fn with_object(mut self, object: impl Into<String>) -> Self {
        self.object = Some(object.into());
        self
    }
    
    /// Set details
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
    
    /// Set result
    pub fn with_result(mut self, result: impl Into<String>) -> Self {
        self.result = Some(result.into());
        self
    }
    
    /// Set reason
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
    
    /// Format as human-readable string
    pub fn format(&self) -> String {
        let ts = chrono::DateTime::from_timestamp(self.timestamp, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_else(|| self.timestamp.to_string());
        
        let object_str = self.object.as_ref()
            .map(|o| format!(" -> {}", o))
            .unwrap_or_default();
        
        let result_str = self.result.as_ref()
            .map(|r| format!(" [{}]", r))
            .unwrap_or_default();
        
        let reason_str = self.reason.as_ref()
            .map(|r| format!(" ({})", r))
            .unwrap_or_default();
        
        format!(
            "[{}] {:?} {}{}{}{}",
            ts,
            self.severity,
            self.subject,
            object_str,
            result_str,
            reason_str
        )
    }
}

/// Audit logger configuration
#[derive(Debug, Clone)]
pub struct AuditConfig {
    /// Maximum number of entries in memory buffer
    pub max_memory_entries: usize,
    
    /// Whether to write to file
    pub file_enabled: bool,
    
    /// Log file path
    pub file_path: std::path::PathBuf,
    
    /// Maximum log file size in MB before rotation
    pub max_file_size_mb: u64,
    
    /// Maximum number of rotated files to keep
    pub max_rotated_files: usize,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            max_memory_entries: 10000,
            file_enabled: true,
            file_path: crate::storage::paths::Paths::data_dir().join("audit.log"),
            max_file_size_mb: 100,
            max_rotated_files: 10,
        }
    }
}

/// Audit logger
pub struct AuditLogger {
    config: AuditConfig,
    entries: Arc<RwLock<VecDeque<AuditEntry>>>,
}

impl AuditLogger {
    /// Create new audit logger
    pub fn new(config: AuditConfig) -> Self {
        let entries = Arc::new(RwLock::new(VecDeque::with_capacity(
            config.max_memory_entries
        )));
        
        Self {
            config,
            entries,
        }
    }
    
    /// Create with default config
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::new(AuditConfig::default())
    }
    
    /// Log an event
    pub async fn log(&self, entry: AuditEntry) {
        // Add to memory buffer
        {
            let mut entries = self.entries.write().await;
            
            // Remove oldest if at capacity
            if entries.len() >= self.config.max_memory_entries {
                entries.pop_front();
            }
            
            entries.push_back(entry.clone());
        }
        
        // Write to file if enabled
        if self.config.file_enabled {
            if let Err(e) = self.write_to_file(&entry).await {
                warn!("Failed to write audit log: {}", e);
            }
        }
        
        // Also log via tracing
        match entry.severity {
            Severity::Info => info!("AUDIT: {}", entry.format()),
            Severity::Warning => warn!("AUDIT: {}", entry.format()),
            Severity::Critical => tracing::error!("AUDIT: {}", entry.format()),
        }
    }
    
    /// Convenience: Log DID verification success
    pub async fn log_verification_success(&self, subject: &str, object: &str) {
        let entry = AuditEntry::new(
            AuditEventType::DidVerificationSuccess,
            Severity::Info,
            subject,
        )
        .with_object(object)
        .with_result("success");
        
        self.log(entry).await;
    }
    
    /// Convenience: Log DID verification failure
    pub async fn log_verification_failure(&self, subject: &str, object: &str, reason: &str) {
        let entry = AuditEntry::new(
            AuditEventType::DidVerificationFailure,
            Severity::Warning,
            subject,
        )
        .with_object(object)
        .with_result("failure")
        .with_reason(reason);
        
        self.log(entry).await;
    }
    
    /// Convenience: Log connection blocked
    pub async fn log_connection_blocked(&self, source: &str, target: &str, reason: &str) {
        let entry = AuditEntry::new(
            AuditEventType::ConnectionBlocked,
            Severity::Warning,
            source,
        )
        .with_object(target)
        .with_reason(reason);
        
        self.log(entry).await;
    }
    
    /// Convenience: Log ACL change
    pub async fn log_acl_change(
        &self,
        event_type: AuditEventType,
        changed_by: &str,
        target_did: &str,
        reason: Option<&str>,
    ) {
        let mut entry = AuditEntry::new(
            event_type,
            Severity::Info,
            changed_by,
        )
        .with_object(target_did)
        .with_result("success");
        
        if let Some(r) = reason {
            entry = entry.with_reason(r);
        }
        
        self.log(entry).await;
    }
    
    /// Convenience: Log mode change
    pub async fn log_mode_change(&self, changed_by: &str, old_mode: &str, new_mode: &str) {
        let entry = AuditEntry::new(
            AuditEventType::ModeChange,
            Severity::Info,
            changed_by,
        )
        .with_result("success")
        .with_details(serde_json::json!({
            "old_mode": old_mode,
            "new_mode": new_mode,
        }));
        
        self.log(entry).await;
    }
    
    /// Convenience: Log authentication attempt
    pub async fn log_auth_attempt(&self, connection_id: &str, peer_addr: &str) {
        let entry = AuditEntry::new(
            AuditEventType::AuthAttempt,
            Severity::Info,
            connection_id,
        )
        .with_object(peer_addr);
        
        self.log(entry).await;
    }
    
    /// Convenience: Log authentication success
    pub async fn log_auth_success(&self, connection_id: &str, did: &str) {
        let entry = AuditEntry::new(
            AuditEventType::AuthSuccess,
            Severity::Info,
            connection_id,
        )
        .with_object(did)
        .with_result("success");
        
        self.log(entry).await;
    }
    
    /// Convenience: Log authentication failure
    pub async fn log_auth_failure(&self, connection_id: &str, reason: &str) {
        let entry = AuditEntry::new(
            AuditEventType::AuthFailure,
            Severity::Warning,
            connection_id,
        )
        .with_result("failure")
        .with_reason(reason);
        
        self.log(entry).await;
    }
    
    /// Get recent entries
    pub async fn get_recent(&self, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read().await;
        entries.iter()
            .rev() // Newest first
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get entries by type
    pub async fn get_by_type(&self, event_type: AuditEventType, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read().await;
        entries.iter()
            .filter(|e| e.event_type == event_type)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Get entries by subject
    pub async fn get_by_subject(&self, subject: &str, limit: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read().await;
        entries.iter()
            .filter(|e| e.subject == subject)
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }
    
    /// Clear memory buffer
    pub async fn clear(&self) {
        let mut entries = self.entries.write().await;
        entries.clear();
    }
    
    /// Write entry to file
    async fn write_to_file(&self, entry: &AuditEntry) -> crate::Result<()> {
        let line = serde_json::to_string(entry)?.to_string();
        
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.config.file_path)
            .await?;
        
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;
        
        Ok(())
    }
    
    /// Export to JSON
    pub async fn export_json(&self) -> crate::Result<String> {
        let entries = self.entries.read().await;
        let entries_vec: Vec<_> = entries.iter().cloned().collect();
        Ok(serde_json::to_string_pretty(&entries_vec)?)
    }
    
    /// Export to human-readable text
    pub async fn export_text(&self) -> String {
        let entries = self.entries.read().await;
        entries.iter()
            .map(|e| e.format())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Current timestamp
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_audit_entry_format() {
        let entry = AuditEntry::new(
            AuditEventType::DidVerificationSuccess,
            Severity::Info,
            "did:cis:challenger:abc123",
        )
        .with_object("did:cis:responder:def456")
        .with_result("success");
        
        let formatted = entry.format();
        assert!(formatted.contains("Info"));
        assert!(formatted.contains("did:cis:challenger:abc123"));
        assert!(formatted.contains("did:cis:responder:def456"));
    }
    
    #[test]
    fn test_audit_entry_serialization() {
        let entry = AuditEntry::new(
            AuditEventType::ConnectionBlocked,
            Severity::Warning,
            "did:cis:source:abc123",
        )
        .with_object("did:cis:target:def456")
        .with_reason("Not in whitelist");
        
        let json = serde_json::to_string(&entry).unwrap();
        let decoded: AuditEntry = serde_json::from_str(&json).unwrap();
        
        assert_eq!(entry.event_type, decoded.event_type);
        assert_eq!(entry.subject, decoded.subject);
    }
}
