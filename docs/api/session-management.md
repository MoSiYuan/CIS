# Session Management API Reference

**Version**: CIS v1.1.6
**Module**: `cis_core::task::session`
**Last Updated**: 2026-02-13

---

## Table of Contents

- [Overview](#overview)
- [Session Lifecycle](#session-lifecycle)
- [Session Repository](#session-repository)
- [Agent Repository](#agent-repository)
- [Data Models](#data-models)
- [Usage Patterns](#usage-patterns)
- [Performance Optimization](#performance-optimization)
- [Best Practices](#best-practices)

---

## Overview

The Session Management system provides intelligent session lifecycle management for AI agents, enabling:

- **Context Reuse**: Share conversation context across multiple task executions
- **Resource Efficiency**: Reduce API calls and token usage by reusing sessions
- **TTL Management**: Automatic expiration and cleanup of stale sessions
- **Capacity Tracking**: Monitor and manage context token usage
- **Multi-Runtime Support**: Support for Claude, OpenCode, Kimi, and other agents

### Key Features

- **Automatic Session Acquisition**: Find or create sessions based on capacity requirements
- **Context Pooling**: Reuse active sessions to minimize overhead
- **Token Budgeting**: Track and limit context usage per session
- **Graceful Expiration**: TTL-based session expiration with cleanup
- **Multi-Agent Support**: Independent session pools per agent type

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Session Repository                       │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │   Claude     │  │  OpenCode    │  │    Kimi      │  │
│  │  Sessions    │  │  Sessions    │  │  Sessions    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│                                                           │
│  • Context Capacity Tracking                                │
│  • TTL Management                                          │
│  • Session Reuse                                          │
│  • Automatic Cleanup                                       │
└─────────────────────────────────────────────────────────────┘
```

---

## Session Lifecycle

### State Transitions

```
┌──────────┐    acquire    ┌──────────┐    release    ┌──────────┐
│ Created  │ ──────────────>│  Active  │ ─────────────>│   Idle   │
└──────────┘               └──────────┘               └──────────┘
     │                           │                           │
     │                           │ expire                    │ reuse
     ▼                           ▼                           ▼
┌──────────┐              ┌──────────┐              ┌──────────┐
│ Expired  │<─────────────│  Idle    │<─────────────│  Active  │
└──────────┘              └──────────┘              └──────────┘
                               │
                               │ delete
                               ▼
                         ┌──────────┐
                         │ Released │
                         └──────────┘
```

### State Descriptions

| State | Description | Reusable |
|-------|-------------|-----------|
| **Active** | Session currently in use by a task | Yes (after release) |
| **Idle** | Session available for reuse | Yes |
| **Expired** | Session TTL expired, pending cleanup | No |
| **Released** | Session explicitly released or cleaned up | No |

---

## Session Repository

### Creating Sessions

```rust
use cis_core::task::session::SessionRepository;
use cis_core::task::db::DatabasePool;

let session_repo = SessionRepository::new(pool);

// Create a new session
let session_id = session_repo.create(
    123,                   // agent_id
    "claude",              // runtime_type
    200000,               // context_capacity (tokens)
    60                    // ttl_minutes
).await?;

println!("Created session: {}", session_id);
```

**Parameters:**

- `agent_id: i64` - Database ID of the agent
- `runtime_type: &str` - Agent runtime identifier ("claude", "opencode", etc.)
- `context_capacity: i64` - Maximum context tokens for the session
- `ttl_minutes: i64` - Time-to-live in minutes

**Returns:**
- `Result<i64>` - Database ID of the created session

**Automatic Timestamps:**
- `created_at`: Set to current time
- `last_used_at`: Set to current time
- `expires_at`: `created_at + (ttl_minutes * 60)`

---

### Acquiring Sessions

Find or create a reusable session with sufficient capacity.

```rust
// Find an existing session with at least 50000 tokens capacity
let session = session_repo.acquire_session(
    123,      // agent_id
    50000     // min_capacity
).await?;

if let Some(session) = session {
    println!("Reusing session: {}", session.session_id);
    println!("Available capacity: {} tokens",
        session.context_capacity - session.context_used);
} else {
    println!("No suitable session found, create a new one");
}
```

**Selection Criteria:**

1. Same `agent_id`
2. Status is `active` or `idle`
3. Sufficient remaining capacity (`capacity - used >= min_capacity`)
4. Not expired (`expires_at > now`)
5. Oldest `last_used_at` (LRU policy)

**Returns:**
- `Result<Option<AgentSessionEntity>>` - Session if found, `None` otherwise

**Side Effect:**
- Updates session status to `idle` if found

---

### Releasing Sessions

Mark a session as available for reuse after use.

```rust
session_repo.release_session(session_id).await?;
```

**Parameters:**
- `session_id: i64` - Database ID of the session

**Side Effects:**
- Sets status to `active` (available for reuse)
- Increments `context_used` by 1 (usage counter)
- Updates `last_used_at` to current time

**Usage Pattern:**

```rust
// Acquire
let session = acquire_session(agent_id, min_capacity).await?;

if let Some(session) = session {
    // Use session
    execute_task_with_session(&session).await?;

    // Release for reuse
    release_session(session.id).await?;
}
```

---

### Updating Session Usage

Track token consumption during task execution.

```rust
session_repo.update_usage(
    session_id,
    15000  // tokens_used
).await?;
```

**Parameters:**
- `session_id: i64` - Database ID of the session
- `tokens_used: i64` - Additional tokens consumed (added to `context_used`)

**Side Effects:**
- Increments `context_used` by `tokens_used`
- Updates `last_used_at` to current time

**Example:**

```rust
// Before execution
let session = get_session(session_id).await?;
let available = session.context_capacity - session.context_used;

// Execute task
let tokens_consumed = execute_task_with_context(task, available).await?;

// Update usage
update_usage(session_id, tokens_consumed).await?;
```

---

### Querying Sessions

#### Get by Database ID

```rust
let session = session_repo.get_by_id(123).await?;

if let Some(session) = session {
    println!("Session: {}", session.session_id);
    println!("Capacity: {}/{} tokens",
        session.context_used,
        session.context_capacity
    );
}
```

#### Get by Session ID

```rust
let session = session_repo.get_by_session_id("123-uuid-v4").await?;

if let Some(session) = session {
    println!("Runtime: {}", session.runtime_type);
    println!("Status: {:?}", session.status);
}
```

#### List Sessions by Agent

```rust
use cis_core::task::models::SessionStatus;

// Get all active sessions for an agent
let sessions = session_repo.list_by_agent(
    123,                           // agent_id
    Some(SessionStatus::Active)      // Optional status filter
).await?;

for session in sessions {
    println!("Session {}: {} tokens used",
        session.session_id,
        session.context_used
    );
}

// Get all sessions for an agent (any status)
let all_sessions = session_repo.list_by_agent(
    123,
    None  // No status filter
).await?;
```

**Sorting:**
- Results ordered by `created_at DESC` (newest first)

---

### Expiration Management

#### Manual Expiration

```rust
session_repo.expire_session(session_id).await?;
```

**Parameters:**
- `session_id: i64` - Database ID of the session

**Side Effects:**
- Sets status to `expired`
- Prevents further reuse

#### Cleanup Expired Sessions

```rust
let expired_count = session_repo.cleanup_expired().await?;
println!("Marked {} sessions as expired", expired_count);
```

**Criteria:**
- Status is not already `expired`
- `expires_at < now`

**Use Case:**
- Periodic cleanup job (e.g., every 5 minutes)
- Manual cleanup before session acquisition

#### Delete Expired Sessions

```rust
use std::time::Duration;

let deleted_count = session_repo.delete_expired(
    7  // older_than_days
).await?;

println!("Deleted {} expired sessions", deleted_count);
```

**Criteria:**
- Status is `expired`
- `expires_at < (now - older_than_days * 86400)`

**Use Case:**
- Periodic database maintenance
- Reclaim storage from old sessions

---

### Statistics

#### Count Active Sessions

```rust
let active_count = session_repo.count_active().await?;
println!("Active sessions: {}", active_count);
```

**Criteria:**
- Status is `active`

#### Count Sessions by Agent

```rust
let session_count = session_repo.count_by_agent(123).await?;
println!("Agent 123 has {} sessions", session_count);
```

**Criteria:**
- Counts all sessions regardless of status

---

### Deleting Sessions

#### Single Deletion

```rust
session_repo.delete(session_id).await?;
```

**Use Cases:**
- Manual cleanup
- Test teardown
- User-initiated session clear

---

## Agent Repository

### Registering Agents

```rust
use cis_core::task::session::AgentRepository;

let agent_repo = AgentRepository::new(pool);

// Register a new agent
let agent_id = agent_repo.register(
    "claude",                    // agent_type
    "Claude AI",                // display_name
    &serde_json::json!({        // config
        "model": "claude-3-sonnet",
        "max_tokens": 200000,
        "temperature": 0.7
    }),
    &vec![                       // capabilities
        "code_review".to_string(),
        "module_refactoring".to_string(),
        "documentation".to_string()
    ]
).await?;

println!("Registered agent with ID: {}", agent_id);
```

**Upsert Behavior:**
- If agent_type exists: Update display_name, config, capabilities
- If agent_type doesn't exist: Create new agent
- Always updates `updated_at` timestamp

---

### Querying Agents

#### Get by Type

```rust
let agent = agent_repo.get_by_type("claude").await?;

if let Some(agent) = agent {
    println!("Agent: {}", agent.display_name);
    println!("Enabled: {}", agent.enabled);
    println!("Capabilities: {:?}", agent.capabilities);
}
```

#### List Enabled Agents

```rust
let agents = agent_repo.list_enabled().await?;

for agent in agents {
    println!("{}: {}",
        agent.agent_type,
        agent.display_name
    );
}
```

**Sorting:**
- Ordered by `created_at ASC` (oldest first)

---

### Managing Agent State

#### Enable/Disable Agent

```rust
// Enable agent
agent_repo.set_enabled(agent_id, true).await?;

// Disable agent
agent_repo.set_enabled(agent_id, false).await?;
```

**Side Effects:**
- Updates `enabled` field
- Updates `updated_at` timestamp

**Use Case:**
- Temporarily disable an agent for maintenance
- Enable new agent after configuration

---

## Data Models

### AgentSessionEntity

```rust
pub struct AgentSessionEntity {
    pub id: i64,                        // Database ID (primary key)
    pub session_id: String,              // Unique session identifier
    pub agent_id: i64,                  // Associated agent ID
    pub runtime_type: String,            // Runtime type identifier
    pub status: SessionStatus,            // Current session status
    pub context_capacity: i64,           // Maximum context tokens
    pub context_used: i64,               // Used context tokens
    pub created_at: i64,                 // Creation timestamp
    pub last_used_at: Option<i64>,       // Last usage timestamp
    pub expires_at: i64,                // Expiration timestamp
}
```

**Computed Properties:**

```rust
impl AgentSessionEntity {
    /// Available context capacity
    pub fn available_capacity(&self) -> i64 {
        self.context_capacity - self.context_used
    }

    /// Time until expiration (seconds)
    pub fn ttl_seconds(&self) -> i64 {
        self.expires_at - chrono::Utc::now().timestamp()
    }

    /// Is session expired?
    pub fn is_expired(&self) -> bool {
        self.ttl_seconds() <= 0
    }

    /// Usage percentage
    pub fn usage_percent(&self) -> f64 {
        (self.context_used as f64 / self.context_capacity as f64) * 100.0
    }
}
```

**Usage Examples:**

```rust
let session = get_session(session_id).await?;

// Check available capacity
if session.available_capacity() < 10000 {
    println!("Session nearly full: {}%",
        session.usage_percent()
    );
}

// Check expiration
if session.is_expired() {
    println!("Session expired, create new one");
}

// Time remaining
let ttl = session.ttl_seconds();
println!("Session expires in {} seconds", ttl);
```

---

### AgentEntity

```rust
pub struct AgentEntity {
    pub id: i64,                        // Database ID
    pub agent_type: String,              // Unique agent identifier
    pub display_name: String,            // Human-readable name
    pub enabled: bool,                    // Active flag
    pub config: serde_json::Value,        // Agent configuration
    pub capabilities: Vec<String>,         // Supported capabilities
    pub created_at: i64,                 // Creation timestamp
    pub updated_at: i64,                 // Last update timestamp
}
```

**Example Configurations:**

```json
{
  "claude": {
    "agent_type": "claude",
    "display_name": "Claude AI",
    "enabled": true,
    "config": {
      "model": "claude-3-sonnet",
      "max_tokens": 200000,
      "temperature": 0.7,
      "timeout_secs": 300
    },
    "capabilities": [
      "code_review",
      "module_refactoring",
      "test_writing"
    ]
  }
}
```

---

## Usage Patterns

### Pattern 1: Session Acquisition Loop

```rust
async fn execute_task_with_session(
    session_repo: &SessionRepository,
    agent_id: i64,
    task: &Task
) -> Result<()> {
    // 1. Try to acquire session with sufficient capacity
    let min_capacity = estimate_required_tokens(task);

    let session = loop {
        match session_repo.acquire_session(agent_id, min_capacity).await? {
            Some(session) => break session,
            None => {
                // No suitable session, create new
                let new_id = session_repo.create(
                    agent_id,
                    "claude",
                    200000,
                    60
                ).await?;

                // Retrieve the created session
                break session_repo.get_by_id(new_id).await?.unwrap();
            }
        };
    };

    // 2. Execute task with session
    let tokens_used = execute_task(&session, task).await?;

    // 3. Update usage
    session_repo.update_usage(session.id, tokens_used).await?;

    // 4. Release for reuse
    session_repo.release_session(session.id).await?;

    Ok(())
}
```

---

### Pattern 2: Session Reuse Strategy

```rust
async fn execute_tasks_batch(
    session_repo: &SessionRepository,
    agent_id: i64,
    tasks: Vec<Task>
) -> Result<()> {
    let mut current_session: Option<AgentSessionEntity> = None;

    for task in tasks {
        let required = estimate_required_tokens(&task);

        // Try to reuse current session
        let session = if let Some(ref session) = current_session {
            if session.available_capacity() >= required {
                session.clone()
            } else {
                // Not enough capacity, acquire new
                acquire_or_create(session_repo, agent_id, required).await?
            }
        } else {
            acquire_or_create(session_repo, agent_id, required).await?
        };

        // Execute task
        let used = execute_task(&session, &task).await?;
        session_repo.update_usage(session.id, used).await?;

        // Update current session if still has capacity
        current_session = if session_repo.get_by_id(session.id).await?
            .map(|s| s.available_capacity() >= 10000)  // Minimum threshold
            .unwrap_or(false) {
            Some(session)
        } else {
            None
        };
    }

    Ok(())
}

async fn acquire_or_create(
    repo: &SessionRepository,
    agent_id: i64,
    min_capacity: i64
) -> Result<AgentSessionEntity> {
    match repo.acquire_session(agent_id, min_capacity).await? {
        Some(session) => Ok(session),
        None => {
            let new_id = repo.create(agent_id, "claude", 200000, 60).await?;
            Ok(repo.get_by_id(new_id).await?.unwrap())
        }
    }
}
```

---

### Pattern 3: Periodic Cleanup

```rust
async fn session_cleanup_job(session_repo: &SessionRepository) -> Result<()> {
    loop {
        // 1. Mark expired sessions
        let expired_count = session_repo.cleanup_expired().await?;
        tracing::info!("Marked {} sessions as expired", expired_count);

        // 2. Delete old expired sessions (older than 7 days)
        let deleted_count = session_repo.delete_expired(7).await?;
        tracing::info!("Deleted {} old sessions", deleted_count);

        // 3. Wait before next cleanup (5 minutes)
        tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;
    }
}
```

---

## Performance Optimization

### Session Pool Sizing

**Optimal Capacity:**

```rust
// For typical code review tasks: 50K-100K tokens
let capacity = 100000;

// For long-running conversations: 200K tokens
let capacity = 200000;

// For short tasks: 50K tokens
let capacity = 50000;
```

### TTL Configuration

**Recommended TTL Values:**

| Use Case | TTL | Reasoning |
|----------|-----|-----------|
| Short tasks | 30 minutes | Quick completion |
| Batch processing | 120 minutes | Allow batch reuse |
| Interactive sessions | 60 minutes | Balance reuse & cleanup |

### Capacity Management

**Tracking Usage:**

```rust
let session = get_session(session_id).await?;

// Check usage percentage
let usage_pct = session.usage_percent();

match usage_pct {
    p if p > 90.0 => {
        warn!("Session nearly full: {:.1}%", p);
        // Consider creating new session
    }
    p if p > 75.0 => {
        info!("Session usage: {:.1}%", p);
        // Monitor closely
    }
    _ => {
        debug!("Session usage: {:.1}%", p);
    }
}
```

---

## Best Practices

### DO ✓

1. **Always release sessions after use**
   ```rust
   let session = acquire_session(...).await?;
   let result = execute(&session).await?;
   release_session(session.id).await?;  // ✓
   ```

2. **Use appropriate capacity for task type**
   ```rust
   let capacity = match task.task_type {
       TaskType::CodeReview => 100000,
       TaskType::Documentation => 50000,
       TaskType::ModuleRefactoring => 200000,
   };
   ```

3. **Run periodic cleanup**
   ```rust
   tokio::spawn(async move {
       loop {
           cleanup_expired().await?;
           tokio::time::sleep(Duration::from_secs(300)).await;
       }
   });
   ```

4. **Monitor session statistics**
   ```rust
   let active = count_active().await?;
   let total = count_by_agent(agent_id).await?;
   info!("Active sessions: {}/{}", active, total);
   ```

### DON'T ✗

1. **Don't forget to update token usage**
   ```rust
   execute_task(&session).await?;
   // ✗ Forgot to update usage
   // ✓ update_usage(session.id, tokens).await?;
   ```

2. **Don't use very long TTLs**
   ```rust
   // ✗ 24 hours is too long
   create(agent_id, "claude", 200000, 1440).await?;

   // ✓ 60-120 minutes is reasonable
   create(agent_id, "claude", 200000, 60).await?;
   ```

3. **Don't ignore expired sessions**
   ```rust
   let session = get_session(session_id).await?;

   // ✗ Use without checking expiration
   execute(&session).await?;

   // ✓ Check expiration first
   if session.is_expired() {
       expire_session(session.id).await?;
       // Create new session
   }
   ```

---

## API Reference Summary

### SessionRepository Methods

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `new` | `Arc<DatabasePool>` | `Self` | Create repository |
| `create` | `i64, &str, i64, i64` | `Result<i64>` | Create session |
| `acquire_session` | `i64, i64` | `Result<Option<AgentSessionEntity>>` | Find reusable session |
| `release_session` | `i64` | `Result<()>` | Release for reuse |
| `expire_session` | `i64` | `Result<()>` | Mark as expired |
| `cleanup_expired` | - | `Result<usize>` | Mark all expired |
| `delete_expired` | `i64` | `Result<usize>` | Delete old expired |
| `get_by_id` | `i64` | `Result<Option<AgentSessionEntity>>` | Get by ID |
| `get_by_session_id` | `&str` | `Result<Option<AgentSessionEntity>>` | Get by session_id |
| `list_by_agent` | `i64, Option<SessionStatus>` | `Result<Vec<AgentSessionEntity>>` | List sessions |
| `update_usage` | `i64, i64` | `Result<()>` | Update token usage |
| `delete` | `i64` | `Result<()>` | Delete session |
| `count_active` | - | `Result<i64>` | Count active sessions |
| `count_by_agent` | `i64` | `Result<i64>` | Count agent sessions |

### AgentRepository Methods

| Method | Parameters | Returns | Description |
|--------|------------|----------|-------------|
| `new` | `Arc<DatabasePool>` | `Self` | Create repository |
| `register` | `&str, &str, &Value, &[String]` | `Result<i64>` | Register agent |
| `get_by_type` | `&str` | `Result<Option<AgentEntity>>` | Get by type |
| `list_enabled` | - | `Result<Vec<AgentEntity>>` | List enabled agents |
| `set_enabled` | `i64, bool` | `Result<()>` | Enable/disable agent |

---

**See Also:**
- [Task System API](./task-system.md)
- [DAG Builder API](./dag-builder.md)
- [Task Manager API](./task-manager.md)
