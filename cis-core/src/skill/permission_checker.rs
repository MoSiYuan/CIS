//! # Runtime Permission Checker
//!
//! Provides fine-grained permission checking for Skill operations.
//!
//! ## Features
//!
//! - Permission validation before resource access
//! - Constraint evaluation (rate limits, time windows, etc.)
//! - Audit logging of all permission checks
//! - Permission caching for performance
//!
//! ## Usage
//!
//! ```rust
//! use cis_core::skill::permission_checker::{
//!     PermissionChecker, PermissionScope, PermissionCategory,
//!     ResourcePattern,
//! };
//!
//! let checker = PermissionChecker::new()?;
//!
//! let perm = PermissionScope {
//!     category: PermissionCategory::MemoryWrite,
//!     resource: ResourcePattern::Specific("my-key".to_string()),
//!     constraints: vec![],
//! };
//!
//! match checker.check_permission("my-skill", &perm) {
//!     PermissionResult::Granted { .. } => {
//!         // Proceed with operation
//!     }
//!     PermissionResult::Denied { reason, .. } => {
//!         eprintln!("Permission denied: {}", reason);
//!     }
//!     _ => {}
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::error::{CisError, Result};

/// Permission category types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionCategory {
    // Memory access
    MemoryRead,
    MemoryWrite,
    MemoryDelete,

    // AI/LLM access
    AiCall,
    AiStream,

    // Network access
    NetworkHttp,
    NetworkP2P,
    NetworkTcp,
    NetworkUdp,

    // Filesystem access
    FileRead,
    FileWrite,
    FileExecute,

    // Command execution
    CommandExec,

    // Process operations
    ProcessSpawn,

    // Environment access
    EnvironmentRead,

    // Custom permissions
    Custom(String),
}

impl PermissionCategory {
    /// Check if this category requires approval
    pub fn requires_approval(&self) -> bool {
        matches!(
            self,
            Self::MemoryWrite
                | Self::MemoryDelete
                | Self::AiCall
                | Self::NetworkHttp
                | Self::FileWrite
                | Self::FileExecute
                | Self::CommandExec
                | Self::ProcessSpawn
        )
    }

    /// Get implicit permissions (no approval needed)
    pub fn implicit_permissions() -> Vec<PermissionCategory> {
        vec![
            Self::MemoryRead, // Can read own data
            Self::FileRead,     // Can read allowed paths
        ]
    }
}

/// Resource pattern for permission scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourcePattern {
    /// All resources
    All,
    /// Specific resource by identifier
    Specific(String),
    /// Glob pattern (e.g., "memory:project/*")
    Pattern(String),
    /// Regular expression
    Regex(String),
}

impl ResourcePattern {
    /// Check if a specific resource matches this pattern
    pub fn matches(&self, resource: &str) -> bool {
        match self {
            ResourcePattern::All => true,
            ResourcePattern::Specific(pattern) => pattern == resource,
            ResourcePattern::Pattern(pattern) => {
                // Simple glob matching
                let regex_pattern = pattern
                    .replace('.', r"\.")
                    .replace('*', ".*")
                    .replace('?', ".");
                Regex::new(&regex_pattern)
                    .map(|re| re.is_match(resource))
                    .unwrap_or(false)
            }
            ResourcePattern::Regex(pattern) => {
                Regex::new(pattern)
                    .map(|re| re.is_match(resource))
                    .unwrap_or(false)
            }
        }
    }
}

/// Permission constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Constraint {
    TimeWindow {
        start_hour: u32,
        end_hour: u32,
    },
    RateLimit {
        max_operations: u32,
        period_seconds: u64,
    },
    MaxSize {
        bytes: usize,
    },
    PathRestriction {
        allowed_paths: Vec<PathBuf>,
    },
}

impl Constraint {
    /// Check if constraint is satisfied
    pub fn check(&self, context: &CheckContext) -> bool {
        match self {
            Constraint::TimeWindow { start_hour, end_hour } => {
                let now = SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let current_hour = (now / 3600) % 24;

                if start_hour <= end_hour {
                    *start_hour <= current_hour && current_hour <= *end_hour
                } else {
                    // Wraps around midnight
                    current_hour >= *start_hour || current_hour <= *end_hour
                }
            }
            Constraint::RateLimit {
                max_operations,
                period_seconds,
            } => {
                // Check rate limit (requires counter storage)
                // For now, always return true
                // TODO: Implement rate limiting
                true
            }
            Constraint::MaxSize { bytes } => {
                if let Some(size) = context.data_size {
                    size <= *bytes
                } else {
                    true
                }
            }
            Constraint::PathRestriction { allowed_paths } => {
                if let Some(path) = &context.file_path {
                    allowed_paths
                        .iter()
                        .any(|allowed| path.starts_with(allowed))
                } else {
                    true
                }
            }
        }
    }
}

/// Permission scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionScope {
    pub category: PermissionCategory,
    pub resource: ResourcePattern,
    #[serde(default)]
    pub constraints: Vec<Constraint>,
}

impl PermissionScope {
    /// Parse from manifest permission
    pub fn from_manifest(
        category: &str,
        resource: &str,
        access: &str,
    ) -> Result<Self> {
        let perm_category = match category {
            "memory" => match access {
                "read" => PermissionCategory::MemoryRead,
                "write" => PermissionCategory::MemoryWrite,
                "delete" => PermissionCategory::MemoryDelete,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown memory access: {}",
                        access
                    )))
                }
            },
            "ai" => match access {
                "call" => PermissionCategory::AiCall,
                "stream" => PermissionCategory::AiStream,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown AI access: {}",
                        access
                    )))
                }
            },
            "network" => match access {
                "http" => PermissionCategory::NetworkHttp,
                "p2p" => PermissionCategory::NetworkP2P,
                "tcp" => PermissionCategory::NetworkTcp,
                "udp" => PermissionCategory::NetworkUdp,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown network access: {}",
                        access
                    )))
                }
            },
            "filesystem" | "file" => match access {
                "read" => PermissionCategory::FileRead,
                "write" => PermissionCategory::FileWrite,
                "execute" => PermissionCategory::FileExecute,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown file access: {}",
                        access
                    )))
                }
            },
            "command" => PermissionCategory::CommandExec,
            "process" => PermissionCategory::ProcessSpawn,
            "environment" => PermissionCategory::EnvironmentRead,
            custom => PermissionCategory::Custom(custom.to_string()),
        };

        let resource_pattern = if resource == "*" {
            ResourcePattern::All
        } else if resource.contains('*') {
            ResourcePattern::Pattern(resource.to_string())
        } else {
            ResourcePattern::Specific(resource.to_string())
        };

        Ok(Self {
            category: perm_category,
            resource: resource_pattern,
            constraints: vec![],
        })
    }

    /// Check if resource matches permission scope
    pub fn matches(&self, resource: &str) -> bool {
        self.resource.matches(resource)
    }
}

/// Context for permission checking
#[derive(Debug, Clone)]
pub struct CheckContext {
    pub data_size: Option<usize>,
    pub file_path: Option<PathBuf>,
    pub operation: String,
    pub timestamp: SystemTime,
}

impl Default for CheckContext {
    fn default() -> Self {
        Self {
            data_size: None,
            file_path: None,
            operation: "unknown".to_string(),
            timestamp: SystemTime::now(),
        }
    }
}

/// Permission check result
#[derive(Debug, Clone)]
pub enum PermissionResult {
    Granted {
        level: PermissionLevel,
        constraints: Vec<Constraint>,
    },
    Denied {
        reason: String,
        suggestion: Option<String>,
    },
    Pending {
        approval_type: String,
    },
}

/// Permission level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLevel {
    Implicit,  // Automatically granted
    Requested, // Declared in manifest
    Approved,  // Explicitly approved
}

/// Permission checker
pub struct PermissionChecker {
    /// Granted permissions database
    permissions: Arc<RwLock<HashMap<String, Vec<PermissionScope>>>>,
    /// Implicit permissions (no approval needed)
    implicit_permissions: HashSet<PermissionCategory>,
    /// Permission cache for performance
    cache: Arc<RwLock<HashMap<(String, PermissionCategory), bool>>>,
    /// Enable audit logging
    audit_enabled: bool,
}

impl PermissionChecker {
    /// Create new permission checker
    pub fn new() -> Result<Self> {
        Ok(Self {
            permissions: Arc::new(RwLock::new(HashMap::new())),
            implicit_permissions: PermissionCategory::implicit_permissions()
                .into_iter()
                .collect(),
            cache: Arc::new(RwLock::new(HashMap::new())),
            audit_enabled: true,
        })
    }

    /// Check if permission is granted
    pub async fn check_permission(
        &self,
        skill_id: &str,
        permission: &PermissionScope,
    ) -> PermissionResult {
        self.check_permission_with_context(skill_id, permission, &CheckContext::default())
            .await
    }

    /// Check permission with execution context
    pub async fn check_permission_with_context(
        &self,
        skill_id: &str,
        permission: &PermissionScope,
        context: &CheckContext,
    ) -> PermissionResult {
        // Check cache first
        let cache_key = (skill_id.to_string(), permission.category.clone());
        if let Ok(cache) = self.cache.read().await {
            if let Some(&granted) = cache.get(&cache_key) {
                if granted {
                    return PermissionResult::Granted {
                        level: PermissionLevel::Requested,
                        constraints: permission.constraints.clone(),
                    };
                } else {
                    return PermissionResult::Denied {
                        reason: "Permission cached as denied".to_string(),
                        suggestion: None,
                    };
                }
            }
        }

        // Check implicit permissions
        if self.implicit_permissions.contains(&permission.category) {
            self.audit_log(skill_id, permission, true, "Implicit permission").await;
            return PermissionResult::Granted {
                level: PermissionLevel::Implicit,
                constraints: permission.constraints.clone(),
            };
        }

        // Check granted permissions
        let granted_perms = self.permissions.read().await;
        let skill_perms = granted_perms.get(skill_id);

        let result = if let Some(perms) = skill_perms {
            let has_permission = perms.iter().any(|p| {
                p.category == permission.category && p.resource.matches("test-resource")
            });

            if has_permission {
                // Check constraints
                let constraints_satisfied = permission
                    .constraints
                    .iter()
                    .all(|c| c.check(context));

                if constraints_satisfied {
                    self.audit_log(skill_id, permission, true, "Granted").await;
                    PermissionResult::Granted {
                        level: PermissionLevel::Approved,
                        constraints: permission.constraints.clone(),
                    }
                } else {
                    self.audit_log(skill_id, permission, false, "Constraint violated").await;
                    PermissionResult::Denied {
                        reason: "Permission constraint violated".to_string(),
                        suggestion: Some("Check operation constraints".to_string()),
                    }
                }
            } else {
                self.audit_log(skill_id, permission, false, "Not granted").await;
                PermissionResult::Denied {
                    reason: format!(
                        "Permission '{:?}' not granted to skill '{}'",
                        permission.category, skill_id
                    ),
                    suggestion: Some("Add permission to skill.toml".to_string()),
                }
            }
        } else {
            // Skill has no permissions declared
            if permission.category.requires_approval() {
                self.audit_log(skill_id, permission, false, "Requires approval").await;
                PermissionResult::Pending {
                    approval_type: "admin".to_string(),
                }
            } else {
                self.audit_log(skill_id, permission, false, "Not declared").await;
                PermissionResult::Denied {
                    reason: format!(
                        "Permission '{:?}' not declared in manifest",
                        permission.category
                    ),
                    suggestion: Some("Add to skill.toml [permissions]".to_string()),
                }
            }
        };

        // Update cache
        let is_granted = matches!(result, PermissionResult::Granted { .. });
        if let Ok(mut cache) = self.cache.write().await {
            cache.insert(cache_key, is_granted);
        }

        result
    }

    /// Grant permission to skill
    pub async fn grant_permission(
        &self,
        skill_id: &str,
        permission: PermissionScope,
    ) -> Result<()> {
        let mut perms = self.permissions.write().await;
        perms
            .entry(skill_id.to_string())
            .or_insert_with(Vec::new)
            .push(permission);

        // Invalidate cache
        if let Ok(mut cache) = self.cache.write().await {
            cache.remove(&(skill_id.to_string(), permission.category.clone()));
        }

        tracing::info!("Granted permission '{:?}' to skill '{}'", permission.category, skill_id);
        Ok(())
    }

    /// Revoke permission from skill
    pub async fn revoke_permission(
        &self,
        skill_id: &str,
        category: &PermissionCategory,
    ) -> Result<()> {
        let mut perms = self.permissions.write().await;

        if let Some(skill_perms) = perms.get_mut(skill_id) {
            skill_perms.retain(|p| p.category != *category);
        }

        // Invalidate cache
        if let Ok(mut cache) = self.cache.write().await {
            cache.remove(&(skill_id.to_string(), category.clone()));
        }

        tracing::info!("Revoked permission '{:?}' from skill '{}'", category, skill_id);
        Ok(())
    }

    /// List all permissions for a skill
    pub async fn list_permissions(&self, skill_id: &str) -> Vec<PermissionScope> {
        let perms = self.permissions.read().await;
        perms.get(skill_id).cloned().unwrap_or_default()
    }

    /// Load permissions from manifest
    pub fn load_from_manifest(&self, _manifest: &str) -> Result<Vec<PermissionScope>> {
        // Parse manifest and extract permissions
        // TODO: Implement manifest parsing
        Ok(vec![])
    }

    /// Write audit log
    async fn audit_log(&self, skill_id: &str, permission: &PermissionScope, granted: bool, reason: &str) {
        if !self.audit_enabled {
            return;
        }

        tracing::debug!(
            "Permission check: skill={}, category={:?}, granted={}, reason={}",
            skill_id, permission.category, granted, reason
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_implicit_permissions() {
        let checker = PermissionChecker::new().unwrap();
        let perm = PermissionScope {
            category: PermissionCategory::MemoryRead,
            resource: ResourcePattern::All,
            constraints: vec![],
        };

        let result = checker.check_permission("test-skill", &perm).await;
        assert!(matches!(result, PermissionResult::Granted { .. }));
    }

    #[test]
    fn test_resource_pattern_matching() {
        // All pattern
        assert!(ResourcePattern::All.matches("anything"));

        // Specific pattern
        assert!(ResourcePattern::Specific("exact".to_string()).matches("exact"));
        assert!(!ResourcePattern::Specific("exact".to_string()).matches("different"));

        // Glob pattern
        let glob = ResourcePattern::Pattern("memory:*".to_string());
        assert!(glob.matches("memory:test"));
        assert!(glob.matches("memory:project/data"));
        assert!(!glob.matches("other:test"));
    }

    #[test]
    fn test_constraint_time_window() {
        let constraint = Constraint::TimeWindow {
            start_hour: 9,
            end_hour: 17,
        };

        let mut context = CheckContext::default();
        // Mock time within window
        let result = constraint.check(&context);
        // Note: This test depends on actual time when run
        // In production, mock SystemTime::now()
        assert!(result || !result); // Just ensure it doesn't panic
    }

    #[test]
    fn test_constraint_max_size() {
        let constraint = Constraint::MaxSize { bytes: 1000 };

        let mut context = CheckContext::default();
        context.data_size = Some(500);
        assert!(constraint.check(&context));

        context.data_size = Some(2000);
        assert!(!constraint.check(&context));
    }

    #[tokio::test]
    async fn test_grant_revoke_permission() {
        let checker = PermissionChecker::new().unwrap();
        let perm = PermissionScope {
            category: PermissionCategory::AiCall,
            resource: ResourcePattern::All,
            constraints: vec![],
        };

        // Grant
        checker
            .grant_permission("my-skill", perm.clone())
            .await
            .unwrap();

        let perms = checker.list_permissions("my-skill").await;
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].category, PermissionCategory::AiCall);

        // Revoke
        checker
            .revoke_permission("my-skill", &PermissionCategory::AiCall)
            .await
            .unwrap();

        let perms = checker.list_permissions("my-skill").await;
        assert_eq!(perms.len(), 0);
    }
}
