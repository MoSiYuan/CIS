//! # Permission Declaration Parser
//!
//! Parses permission declarations from skill.toml manifests.
//!
//! ## Format
//!
//! ```toml
//! [[permissions]]
//! category = "memory"
//! resource = "project/*"
//! access = "read-write"
//!
//! [[permissions]]
//! category = "ai"
//! resource = "*"
//! access = "call"
//! constraint = { type = "rate_limit", max = 100, period = 60 }
//! ```
//!
//! ## Permission Categories
//!
//! - **memory**: Read/write/delete memory
//! - **ai**: Call/stream AI models
//! - **network**: HTTP, P2P, TCP, UDP
//! - **filesystem**: Read/write/execute files
//! - **command**: Execute shell commands
//! - **process**: Spawn processes
//! - **environment**: Read environment variables
//! - **custom**: Custom permissions

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::{CisError, Result};
use crate::skill::permission_checker::{
    Constraint, PermissionCategory, PermissionScope, ResourcePattern,
};

/// Permission declaration in manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDeclaration {
    /// Permission category
    pub category: String,
    /// Resource pattern (e.g., "project/*" or "*")
    pub resource: String,
    /// Access type (read, write, execute, call, etc.)
    pub access: String,
    /// Optional constraint
    #[serde(default)]
    pub constraint: Option<ConstraintDeclaration>,
}

/// Constraint declaration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConstraintDeclaration {
    TimeWindow {
        start_hour: u32,
        end_hour: u32,
    },
    RateLimit {
        max: u32,
        period: u64,
    },
    MaxSize {
        bytes: usize,
    },
    PathRestriction {
        allowed_paths: Vec<String>,
    },
}

impl ConstraintDeclaration {
    /// Convert to runtime constraint
    pub fn to_constraint(&self) -> Result<crate::skill::permission_checker::Constraint> {
        match self {
            ConstraintDeclaration::TimeWindow {
                start_hour,
                end_hour,
            } => Ok(crate::skill::permission_checker::Constraint::TimeWindow {
                start_hour: *start_hour,
                end_hour: *end_hour,
            }),
            ConstraintDeclaration::RateLimit { max, period } => {
                Ok(crate::skill::permission_checker::Constraint::RateLimit {
                    max_operations: *max,
                    period_seconds: *period,
                })
            }
            ConstraintDeclaration::MaxSize { bytes } => {
                Ok(crate::skill::permission_checker::Constraint::MaxSize {
                    bytes: *bytes,
                })
            }
            ConstraintDeclaration::PathRestriction { allowed_paths } => {
                let paths = allowed_paths
                    .iter()
                    .map(|p| std::path::PathBuf::from(p))
                    .collect();

                Ok(crate::skill::permission_checker::Constraint::PathRestriction {
                    allowed_paths: paths,
                })
            }
        }
    }
}

/// Permission manifest section
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PermissionManifest {
    /// List of permission declarations
    #[serde(default)]
    pub permissions: Vec<PermissionDeclaration>,

    /// Permission template to inherit
    #[serde(default)]
    pub inherits: Option<String>,

    /// Additional permissions beyond template
    #[serde(default)]
    pub extra: Vec<PermissionDeclaration>,
}

impl PermissionManifest {
    /// Parse permission declarations into scopes
    pub fn parse_permissions(&self) -> Result<Vec<PermissionScope>> {
        let mut scopes = Vec::new();

        // Parse declared permissions
        for decl in &self.permissions {
            let scope = Self::parse_declaration(decl)?;
            scopes.push(scope);
        }

        // Parse extra permissions
        for decl in &self.extra {
            let scope = Self::parse_declaration(decl)?;
            scopes.push(scope);
        }

        // If inherits from template, add template permissions
        if let Some(template_name) = &self.inherits {
            let template_perms = Self::get_template_permissions(template_name)?;
            scopes.extend(template_perms);
        }

        Ok(scopes)
    }

    /// Parse single permission declaration
    fn parse_declaration(decl: &PermissionDeclaration) -> Result<PermissionScope> {
        let resource_pattern = if decl.resource == "*" {
            ResourcePattern::All
        } else if decl.resource.contains('*') {
            ResourcePattern::Pattern(decl.resource.clone())
        } else {
            ResourcePattern::Specific(decl.resource.clone())
        };

        let mut constraints = Vec::new();

        if let Some(constraint_decl) = &decl.constraint {
            constraints.push(constraint_decl.to_constraint()?);
        }

        Ok(PermissionScope {
            category: PermissionScope::parse_category(&decl.category, &decl.access)?,
            resource: resource_pattern,
            constraints,
        })
    }

    /// Get permissions from built-in template
    fn get_template_permissions(template_name: &str) -> Result<Vec<PermissionScope>> {
        match template_name {
            "minimal" => Ok(vec![]),
            "standard" => Ok(vec![
                PermissionScope {
                    category: PermissionCategory::MemoryRead,
                    resource: ResourcePattern::Pattern(format!("skill:{}", "current")),
                    constraints: vec![],
                },
            ]),
            "extended" => Ok(vec![
                PermissionScope {
                    category: PermissionCategory::MemoryRead,
                    resource: ResourcePattern::All,
                    constraints: vec![],
                },
                PermissionScope {
                    category: PermissionCategory::MemoryWrite,
                    resource: ResourcePattern::Pattern("skill:*".to_string()),
                    constraints: vec![],
                },
            ]),
            "system" => Ok(vec![
                PermissionScope {
                    category: PermissionCategory::MemoryRead,
                    resource: ResourcePattern::All,
                    constraints: vec![],
                },
                PermissionScope {
                    category: PermissionCategory::MemoryWrite,
                    resource: ResourcePattern::All,
                    constraints: vec![],
                },
                PermissionScope {
                    category: PermissionCategory::MemoryDelete,
                    resource: ResourcePattern::All,
                    constraints: vec![],
                },
            ]),
            _ => Err(CisError::invalid_input(format!(
                "Unknown permission template: {}",
                template_name
            ))),
        }
    }

    /// Validate permission declarations
    pub fn validate(&self) -> Result<()> {
        // Check for conflicting permissions
        let mut resource_access = HashMap::new();

        for decl in &self.permissions {
            let key = (&decl.resource, &decl.access);
            if let Some(existing) = resource_access.get(&key) {
                if existing != &decl.category {
                    return Err(CisError::invalid_input(format!(
                        "Conflicting permission for resource '{}' access '{}': {} vs {}",
                        decl.resource, decl.access, existing, decl.category
                    )));
                }
            }
            resource_access.insert(key, decl.category.clone());
        }

        // Validate constraint syntax
        for decl in &self.permissions {
            if let Some(constraint) = &decl.constraint {
                constraint.validate()?;
            }
        }

        Ok(())
    }
}

impl PermissionScope {
    /// Parse category and access into PermissionCategory
    fn parse_category(category: &str, access: &str) -> Result<PermissionCategory> {
        let parts: Vec<&str> = access.split('-').collect();
        let base_access = parts[0];

        let perm_category = match category {
            "memory" => match base_access {
                "read" => PermissionCategory::MemoryRead,
                "write" => PermissionCategory::MemoryWrite,
                "delete" => PermissionCategory::MemoryDelete,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown memory access: {}",
                        base_access
                    )))
                }
            },
            "ai" => match base_access {
                "call" => PermissionCategory::AiCall,
                "stream" => PermissionCategory::AiStream,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown AI access: {}",
                        base_access
                    )))
                }
            },
            "network" => match base_access {
                "http" => PermissionCategory::NetworkHttp,
                "p2p" => PermissionCategory::NetworkP2P,
                "tcp" => PermissionCategory::NetworkTcp,
                "udp" => PermissionCategory::NetworkUdp,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown network access: {}",
                        base_access
                    )))
                }
            },
            "filesystem" | "file" => match base_access {
                "read" => PermissionCategory::FileRead,
                "write" => PermissionCategory::FileWrite,
                "execute" => PermissionCategory::FileExecute,
                _ => {
                    return Err(CisError::invalid_input(format!(
                        "Unknown file access: {}",
                        base_access
                    )))
                }
            },
            "command" => PermissionCategory::CommandExec,
            "process" => PermissionCategory::ProcessSpawn,
            "environment" => PermissionCategory::EnvironmentRead,
            custom => PermissionCategory::Custom(custom.to_string()),
        };

        Ok(perm_category)
    }
}

impl ConstraintDeclaration {
    /// Validate constraint syntax
    fn validate(&self) -> Result<()> {
        match self {
            ConstraintDeclaration::TimeWindow {
                start_hour,
                end_hour,
            } => {
                if *start_hour >= 24 || *end_hour >= 24 {
                    return Err(CisError::invalid_input(
                        "Hour must be 0-23".to_string(),
                    ));
                }
            }
            ConstraintDeclaration::RateLimit { max, period } => {
                if *max == 0 {
                    return Err(CisError::invalid_input(
                        "Rate limit max must be > 0".to_string(),
                    ));
                }
                if *period == 0 {
                    return Err(CisError::invalid_input(
                        "Rate limit period must be > 0".to_string(),
                    ));
                }
            }
            ConstraintDeclaration::MaxSize { bytes } => {
                if *bytes == 0 {
                    return Err(CisError::invalid_input(
                        "Max size must be > 0".to_string(),
                    ));
                }
            }
            ConstraintDeclaration::PathRestriction { allowed_paths } => {
                if allowed_paths.is_empty() {
                    return Err(CisError::invalid_input(
                        "Path restriction must have at least one path".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_permission() {
        let decl = PermissionDeclaration {
            category: "memory".to_string(),
            resource: "project/*".to_string(),
            access: "read".to_string(),
            constraint: None,
        };

        let scope = PermissionManifest::parse_declaration(&decl).unwrap();
        assert!(matches!(scope.category, PermissionCategory::MemoryRead));
        assert!(matches!(scope.resource, ResourcePattern::Pattern(_)));
    }

    #[test]
    fn test_parse_all_resource() {
        let decl = PermissionDeclaration {
            category: "ai".to_string(),
            resource: "*".to_string(),
            access: "call".to_string(),
            constraint: None,
        };

        let scope = PermissionManifest::parse_declaration(&decl).unwrap();
        assert!(matches!(scope.category, PermissionCategory::AiCall));
        assert!(matches!(scope.resource, ResourcePattern::All));
    }

    #[test]
    fn test_parse_with_constraint() {
        let decl = PermissionDeclaration {
            category: "memory".to_string(),
            resource: "project/data".to_string(),
            access: "write".to_string(),
            constraint: Some(ConstraintDeclaration::MaxSize {
                bytes: 1024 * 1024,
            }),
        };

        let scope = PermissionManifest::parse_declaration(&decl).unwrap();
        assert!(matches!(scope.category, PermissionCategory::MemoryWrite));
        assert_eq!(scope.constraints.len(), 1);
    }

    #[test]
    fn test_get_template_standard() {
        let scopes = PermissionManifest::get_template_permissions("standard").unwrap();
        assert!(!scopes.is_empty());
        assert!(scopes
            .iter()
            .any(|s| matches!(s.category, PermissionCategory::MemoryRead)));
    }

    #[test]
    fn test_get_template_minimal() {
        let scopes = PermissionManifest::get_template_permissions("minimal").unwrap();
        assert!(scopes.is_empty());
    }

    #[test]
    fn test_get_template_unknown() {
        let result = PermissionManifest::get_template_permissions("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_conflicting_permissions() {
        let manifest = PermissionManifest {
            permissions: vec![
                PermissionDeclaration {
                    category: "memory".to_string(),
                    resource: "test".to_string(),
                    access: "read".to_string(),
                    constraint: None,
                },
                PermissionDeclaration {
                    category: "ai".to_string(), // Different category
                    resource: "test".to_string(),
                    access: "read".to_string(),
                    constraint: None,
                },
            ],
            inherits: None,
            extra: vec![],
        };

        assert!(manifest.validate().is_err());
    }
}
