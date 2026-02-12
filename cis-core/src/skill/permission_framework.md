# Permission Framework Design

> **Version**: 1.0
> **Component**: CIS Skill System
> **Status**: Design Document (v1.1.6)

---

## Overview

This document defines the permission verification framework for CIS Skills. The framework provides runtime permission checking to ensure Skills only access resources they have been explicitly granted.

## Motivation

### Current Issues

From the code review (`docs/user/code-review-execution-layer.md`):

1. **WASM Sandbox Incomplete**: Skills can access all memories without permission checks
2. **Missing Resource Limits**: No CPU, I/O, or memory limits on Skill execution
3. **Simple Permission Checks**: Existing checks are too simplistic, no inheritance or roles
4. **No Audit Trail**: Permission denials not logged for security review

### Goals

1. **Principle of Least Privilege**: Skills run with minimum required permissions
2. **Explicit Authorization**: All resource access requires prior permission grant
3. **Runtime Verification**: Permissions checked at actual access time, not just load time
4. **Audit Logging**: All permission checks logged for security analysis
5. **Fine-grained Control**: Granular permissions (e.g., memory read vs write)

## Architecture

### Permission Categories

```rust
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

    // System resources
    ProcessSpawn,
    EnvironmentRead,

    // Custom permissions
    Custom(String),
}
```

### Permission Scope

Permissions are scoped to specific resources:

```rust
pub struct PermissionScope {
    pub category: PermissionCategory,
    pub resource: ResourcePattern,
    pub constraints: Vec<Constraint>,
}

pub enum ResourcePattern {
    // All resources of this type
    All,
    // Specific resource
    Specific(String),
    // Pattern match (e.g., "memory:project/*")
    Pattern(String),
    // Regular expression
    Regex(String),
}

pub enum Constraint {
    // Time-based (only allowed during certain hours)
    TimeWindow { start: u32, end: u32 },

    // Count-based (max N operations)
    RateLimit { max: u32, period: Duration },

    // Size-based (max data size)
    MaxSize { bytes: usize },

    // Location-based (only certain paths)
    PathRestriction { allowed_paths: Vec<PathBuf> },
}
```

### Permission Levels

Similar to task decision levels, permissions can be:

| Level | Behavior | Example |
|-------|-----------|----------|
| **Implicit** | Automatically granted, no declaration needed | Reading own skill data |
| **Requested** | Must be declared in skill.toml, auto-approved for safe ops | Reading public memory |
| **Approved** | Requires user/admin approval | Writing to system memory |
| **Denied** | Never allowed, even if declared | Deleting core system files |

### Permission Inheritance

Skills can inherit permissions from templates:

```toml
[permissions]
# Inherit from template
inherits = "standard-skill"

# Additional permissions
extra = ["memory:project/*"]
```

Built-in templates:
- **minimal**: No permissions (pure computation)
- **standard**: Read-only access to own data
- **extended**: Read-write to own data, read public memory
- **system**: Full access (admin tools only)

## Runtime Permission Checking

### Checker Interface

```rust
pub trait PermissionChecker: Send + Sync {
    /// Check if permission is granted
    fn check_permission(
        &self,
        skill_id: &str,
        permission: &PermissionScope,
    ) -> PermissionResult;

    /// Check permission with context
    fn check_permission_with_context(
        &self,
        skill_id: &str,
        permission: &PermissionScope,
        context: &ExecutionContext,
    ) -> PermissionResult;

    /// Grant permission (admin operation)
    fn grant_permission(
        &mut self,
        skill_id: &str,
        permission: &PermissionScope,
    ) -> Result<()>;

    /// Revoke permission
    fn revoke_permission(
        &mut self,
        skill_id: &str,
        permission: &PermissionScope,
    ) -> Result<()>;

    /// List all permissions for a skill
    fn list_permissions(&self, skill_id: &str) -> Vec<PermissionScope>;
}
```

### Permission Result

```rust
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
        approval_type: ApprovalType,
    },
}
```

### Execution Context

Context provided during permission checks:

```rust
pub struct ExecutionContext {
    pub skill_id: String,
    pub operation: String,
    pub timestamp: SystemTime,
    pub arguments: HashMap<String, serde_json::Value>,
    pub caller_type: CallerType,
}

pub enum CallerType {
    User,
    Skill(String),
    System,
    External(String),
}
```

## Implementation

### File: `cis-core/src/skill/permission_checker.rs`

**Components**:

1. **`PermissionChecker`**: Main permission verification interface
2. **`PermissionStore`**: Persistent storage of granted permissions
3. **`PermissionCache`**: In-memory cache for performance
4. **`PermissionAuditor`**: Audit logging for all checks

### Permission Store Schema

```sql
CREATE TABLE skill_permissions (
    skill_id TEXT NOT NULL,
    category TEXT NOT NULL,
    resource TEXT NOT NULL,
    constraints TEXT, -- JSON encoded
    granted_at INTEGER NOT NULL,
    granted_by TEXT,
    expires_at INTEGER,
    PRIMARY KEY (skill_id, category, resource)
);

CREATE TABLE permission_audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    skill_id TEXT NOT NULL,
    category TEXT NOT NULL,
    resource TEXT NOT NULL,
    result TEXT NOT NULL, -- "granted" or "denied"
    reason TEXT,
    context TEXT, -- JSON encoded
    timestamp INTEGER NOT NULL
);

CREATE INDEX idx_audit_skill ON permission_audit(skill_id);
CREATE INDEX idx_audit_timestamp ON permission_audit(timestamp);
```

## Skill Manifest Integration

### Permission Declaration in skill.toml

```toml
[skill]
name = "my-skill"
version = "1.0.0"

[[permissions]]
category = "memory"
resource = "project/*"
access = "read"
constraint = { type = "rate_limit", max = 100, period = 60 }

[[permissions]]
category = "ai"
resource = "*"
access = "call"
constraint = { type = "max_tokens", value = 10000 }

[[permissions]]
category = "filesystem"
resource = "/tmp/cis/*"
access = "read-write"
```

### Permission Parsing

```rust
impl SkillManifest {
    pub fn parse_permissions(&self) -> Result<Vec<PermissionScope>> {
        self.permissions.iter()
            .map(|p| PermissionScope::from_manifest(p))
            .collect()
    }
}
```

## Integration with Skill Manager

### Load-time Check

When a Skill is loaded:

```rust
impl SkillManager {
    pub fn load(&self, name: &str, options: LoadOptions) -> Result<()> {
        // 1. Parse manifest permissions
        let manifest = self.load_manifest(name)?;
        let requested_permissions = manifest.parse_permissions()?;

        // 2. Check if permissions are auto-granted
        let checker = self.permission_checker();
        for perm in requested_permissions {
            match checker.check_permission(name, &perm) {
                PermissionResult::Denied { reason, .. } => {
                    return Err(CisError::PermissionDenied(format!(
                        "Skill '{}' requires permission '{}': {}",
                        name, perm, reason
                    )));
                }
                PermissionResult::Pending { .. } => {
                    // Request approval
                    self.request_permission_approval(name, &perm)?;
                }
                PermissionResult::Granted { .. } => {
                    // OK
                }
            }
        }

        // 3. Continue with load
        // ...
    }
}
```

### Runtime Check

When a Skill attempts an operation:

```rust
// In SkillContext implementation
impl SkillContext for SkillRuntime {
    fn memory_set(&self, key: &str, value: &[u8]) -> Result<()> {
        let perm = PermissionScope {
            category: PermissionCategory::MemoryWrite,
            resource: ResourcePattern::Specific(key.to_string()),
            constraints: vec![],
        };

        match self.checker.check_permission(&self.skill_id, &perm) {
            PermissionResult::Granted { .. } => {
                // Proceed with write
                self.backend.memory_set(key, value)
            }
            PermissionResult::Denied { reason, .. } => {
                Err(CisError::PermissionDenied(format!(
                    "Cannot write to memory '{}': {}",
                    key, reason
                )))
            }
            _ => Err(CisError::PermissionDenied(
                "Permission check failed".to_string()
            )),
        }
    }
}
```

## Security Considerations

### TOCTOU Issues

Permission checks must happen **at resource access time**, not just at load time:

**Bad**: Check once, cache forever
```rust
if self.permissions.contains(&MemoryWrite) {
    // Always allows write after first check
    self.backend.write(key, value);
}
```

**Good**: Check on every access
```rust
self.checker.check_permission(&self.skill_id, &perm)?;
self.backend.write(key, value);
```

### Privilege Escalation

Prevent skills from granting themselves permissions:

1. **Grant Permission**: Admin-only operation
2. **Modify Checker**: Requires system-level access
3. **Bypass Checks**: Code review path, no shortcuts

### Audit Trail

All permission checks logged:

```rust
pub enum AuditEvent {
    Granted {
        skill_id: String,
        permission: PermissionScope,
        context: ExecutionContext,
    },
    Denied {
        skill_id: String,
        permission: PermissionScope,
        reason: String,
        context: ExecutionContext,
    },
}
```

## Performance

### Optimization Strategies

1. **In-memory cache**: LRU cache of recent check results
2. **Batch validation**: Pre-validate multiple permissions
3. **Async checks**: Don't block on permission checks for safe ops
4. **Permission tokens**: Short-lived tokens for batch operations

### Benchmarks

Target performance:
- Simple check: < 10μs
- With constraints: < 100μs
- Audit log write: < 1ms

## Testing

### Unit Tests

- Permission parsing from manifests
- Constraint evaluation
- Pattern matching (all, specific, glob, regex)
- Cache invalidation
- Audit log integrity

### Integration Tests

- Full skill load with permissions
- Runtime permission enforcement
- Permission grant/revoke operations
- Cross-skill isolation

### Security Tests

- Attempted privilege escalation
- TOCTOU race conditions
- Constraint bypass attempts
- Audit log injection

## Migration Path

### Phase 1: Basic Enforcement (v1.1.6)
- Implement permission checker
- Add runtime checks to SkillContext
- Update manifest parsing
- Basic audit logging

### Phase 2: Advanced Features (v1.2.0)
- Permission templates
- Constraint system
- Admin CLI for permission management
- Audit log viewer

### Phase 3: Hardening (v1.3.0)
- Permission revocation
- Time-based constraints
- Rate limiting per skill
- HSM integration for permission store

## References

- [Capability-based Security](https://en.wikipedia.org/wiki/Capability-based_security)
- [Principle of Least Privilege](https://en.wikipedia.org/wiki/Principle_of_least_privilege)
- [seccomp Linux](https://man7.org/linux/man-pages/man2/seccomp.2.html)
- [Capsicum FreeBSD](https://www.freebsd.org/cgi/man.cgi?query=capsicum&sektion=4)
