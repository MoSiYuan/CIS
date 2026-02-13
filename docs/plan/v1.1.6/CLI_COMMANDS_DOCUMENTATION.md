# CIS CLI Commands - Task, Session, and Engine

> **Version**: v1.1.6
> **Last Updated**: 2026-02-13

---

## Table of Contents

1. [Task Commands](#task-commands)
2. [Session Commands](#session-commands)
3. [Engine Commands](#engine-commands)
4. [Integration with Server API](#integration-with-server-api)
5. [Examples](#examples)

---

## Task Commands

Task management commands for creating, listing, updating, and deleting tasks in the CIS system.

### `cis task list`

List all tasks with optional filtering.

```bash
cis task list [OPTIONS]
```

**Options:**
- `-s, --status <STATUS>` - Filter by status (pending, running, completed, failed)
- `-t, --task-type <TYPE>` - Filter by task type (refactoring, injection, optimization, review, test, documentation)
- `-p, --priority <PRIORITY>` - Filter by priority (P0, P1, P2, P3)
- `-l, --limit <N>` - Limit number of results

**Examples:**

```bash
# List all tasks
cis task list

# List pending P0 tasks
cis task list --status pending --priority P0

# List code review tasks
cis task list --task-type review

# List first 10 high-priority tasks
cis task list --priority P1 --limit 10
```

**Output:**

```
ID    Task ID    Name                Type            Priority  Status     Team
1     TASK-001   Refactor CLI        Module Refactor  P0        Pending    Team-V-CLI
2     TASK-002   Memory optimization  Optimization     P1        Running    Team-V-Memory
```

### `cis task create`

Create a new task.

```bash
cis task create <TASK_ID> <NAME> --type <TYPE> --priority <PRIORITY> [OPTIONS]
```

**Arguments:**
- `TASK_ID` - Unique task identifier (e.g., TASK-001)
- `NAME` - Task name/description

**Required Options:**
- `--type <TYPE>` - Task type (refactoring, injection, optimization, review, test, documentation)
- `--priority <PRIORITY>` - Task priority (P0, P1, P2, P3, critical, high, medium, low)

**Optional Options:**
- `-d, --dependencies <IDS>` - Task dependencies (comma-separated task IDs)
- `--prompt-template <TEMPLATE>` - Custom prompt template
- `--description <DESC>` - Task description

**Examples:**

```bash
# Create a refactoring task
cis task create TASK-001 "Refactor CLI handlers" \
  --type refactoring \
  --priority P0 \
  --description "Refactor CLI command handlers for better modularity"

# Create a task with dependencies
cis task create TASK-002 "Write tests" \
  --type test \
  --priority P1 \
  --dependencies TASK-001

# Create a code review task
cis task create TASK-003 "Review PR #123" \
  --type review \
  --priority P0 \
  --prompt-template "Review this PR for bugs, performance issues, and security vulnerabilities"
```

### `cis task update`

Update task status.

```bash
cis task update <TASK_ID> --status <STATUS> [OPTIONS]
```

**Arguments:**
- `TASK_ID` - Task ID or numeric ID

**Required Options:**
- `--status <STATUS>` - New status (pending, assigned, running, completed, failed)

**Optional Options:**
- `--error-message <MSG>` - Error message (if status is failed)

**Examples:**

```bash
# Mark task as running
cis task update TASK-001 --status running

# Mark task as completed
cis task update TASK-001 --status completed

# Mark task as failed with error
cis task update TASK-001 --status failed --error-message "Compilation failed"
```

### `cis task show`

Show detailed task information.

```bash
cis task show <TASK_ID>
```

**Arguments:**
- `TASK_ID` - Task ID or numeric ID

**Example:**

```bash
cis task show TASK-001
```

**Output:**

```
════════════════════════════════════════ Task Details ═════════════════════════════════════════

Name: Refactor CLI handlers
Task ID: TASK-001
Type: Module Refactoring
Priority: P0
Status: Running
Team: Team-V-CLI
Agent: 1
Description: Refactor CLI command handlers for better modularity
Dependencies: -
Estimated Effort: 3 days
Created: 2026-02-13 10:30:00
Started: 2026-02-13 11:00:00
Completed: -
Duration: -
```

### `cis task delete`

Delete a task.

```bash
cis task delete <TASK_ID>
```

**Arguments:**
- `TASK_ID` - Task ID or numeric ID

**Example:**

```bash
cis task delete TASK-001
```

---

## Session Commands

Session management commands for creating, listing, and managing agent sessions.

### `cis session list`

List all sessions with optional filtering.

```bash
cis session list [OPTIONS]
```

**Options:**
- `-a, --agent-id <ID>` - Filter by agent ID
- `-s, --status <STATUS>` - Filter by status (active, idle, expired, released)

**Examples:**

```bash
# List all sessions
cis session list

# List active sessions for agent 1
cis session list --agent-id 1 --status active
```

**Output:**

```
ID  Session ID              Agent ID  Runtime  Status   Capacity  Used  Created              Expires
1   1-550e8400-e29b-41d4  1         claude   Active   100000    0     2026-02-13 10:00:00  2026-02-13 11:00:00
2   1-550e8400-e29b-41d5  1         claude   Idle     100000    50000 2026-02-13 09:00:00  2026-02-13 10:00:00
```

### `cis session create`

Create a new agent session.

```bash
cis session create <AGENT_TYPE> <RUNTIME> --capacity <TOKENS> --ttl-minutes <MINUTES>
```

**Arguments:**
- `AGENT_TYPE` - Agent type (e.g., claude, opencode)
- `RUNTIME` - Runtime type

**Required Options:**
- `-c, --capacity <TOKENS>` - Context capacity in tokens
- `-t, --ttl-minutes <MINUTES>` - Time-to-live in minutes

**Examples:**

```bash
# Create a Claude session with 100K tokens for 60 minutes
cis session create claude claude --capacity 100000 --ttl-minutes 60

# Create an OpenCode session with 50K tokens for 30 minutes
cis session create opencode opencode --capacity 50000 --ttl-minutes 30
```

### `cis session show`

Show detailed session information.

```bash
cis session show <SESSION_ID>
```

**Arguments:**
- `SESSION_ID` - Session numeric ID

**Example:**

```bash
cis session show 1
```

**Output:**

```
════════════════════════════════════════ Session Details ═════════════════════════════════════════

Session ID: 1-550e8400-e29b-41d4-a716-446655440000
ID: 1
Agent ID: 1
Runtime: claude
Status: Active

Capacity:
• Total: 100000 tokens
• Used: 0 tokens (0.0%)
• Available: 100000 tokens

Timing:
• Created: 2026-02-13 10:00:00
• Last Used: 2026-02-13 10:30:00
• Expires: 2026-02-13 11:00:00 (in 30 minutes)
• TTL: 60 minutes
```

### `cis session acquire`

Acquire an existing session (reuse if available).

```bash
cis session acquire --agent-id <ID> --min-capacity <TOKENS>
```

**Required Options:**
- `-a, --agent-id <ID>` - Agent ID
- `-m, --min-capacity <TOKENS>` - Minimum required capacity

**Example:**

```bash
cis session acquire --agent-id 1 --min-capacity 50000
```

### `cis session release`

Release a session (mark as reusable).

```bash
cis session release <SESSION_ID>
```

**Arguments:**
- `SESSION_ID` - Session numeric ID

**Example:**

```bash
cis session release 1
```

### `cis session cleanup`

Cleanup expired sessions.

```bash
cis session cleanup [OPTIONS]
```

**Options:**
- `-o, --older-than-days <DAYS>` - Delete sessions older than N days (default: 7)

**Example:**

```bash
# Cleanup sessions older than 7 days
cis session cleanup

# Cleanup sessions older than 30 days
cis session cleanup --older-than-days 30
```

---

## Engine Commands

Engine code scanning and injection commands.

### `cis engine scan`

Scan directory for engine code and identify injection locations.

```bash
cis engine scan <DIRECTORY> [OPTIONS]
```

**Arguments:**
- `DIRECTORY` - Directory to scan

**Options:**
- `-e, --engine-type <TYPE>` - Engine type (auto-detected if not specified)
  - Options: unreal, unity, godot, unreal5.7, unreal5.6, unity2022, unity2021, godot4, godot3
- `-o, --output <FILE>` - Output file path (JSON format)

**Examples:**

```bash
# Auto-detect engine and scan
cis engine scan ./MyGame

# Scan specific Unreal project
cis engine scan ./MyUnrealGame --engine-type unreal5.7

# Scan and save results
cis engine scan ./MyGame --output scan-results.json
```

**Output:**

```
════════════════════════════════════════ Engine Scan Results ═════════════════════════════════════════

Engine: Unreal Engine 5.7
Files Scanned: 150
Injection Locations Found: 23

Injection Locations

1. ▸ (90% confidence)
File: MyGame/Source/MyGame/MyActor.cpp
Line: 15
Type: ActorClass
Description: AActor subclass - can inject BeginPlay() logic

2. ▸ (85% confidence)
File: MyGame/Source/MyGame/MyPlayerController.cpp
Line: 23
Type: UFunction
Description: Blueprint callable function - can inject AI logic
```

### `cis engine report`

Generate injection report from scan results.

```bash
cis engine report <SCAN_RESULT> --format <FORMAT>
```

**Arguments:**
- `SCAN_RESULT` - Scan result file (JSON)

**Options:**
- `-f, --format <FORMAT>` - Output format (default: markdown)
  - Options: markdown, json, csv

**Examples:**

```bash
# Generate markdown report
cis engine report scan-results.json --format markdown

# Generate JSON report
cis engine report scan-results.json --format json

# Generate CSV report
cis engine report scan-results.json --format csv
```

### `cis engine list-engines`

List supported engine types.

```bash
cis engine list-engines
```

**Output:**

```
Supported Engine Types

• Unreal5.7 - Unreal Engine 5.7
  Detection: *.uproject, Engine/Source/*.cpp

• Unreal5.6 - Unreal Engine 5.6
  Detection: *.uproject, Engine/Source/*.cpp

• Unity2022 - Unity 2022
  Detection: Assets/, ProjectSettings/

• Unity2021 - Unity 2021
  Detection: Assets/, ProjectSettings/

• Godot4 - Godot Engine 4.x
  Detection: project.godot, *.gd

• Godot3 - Godot Engine 3.x
  Detection: engine.gd, *.gd
```

---

## Integration with Server API

All commands follow the unified Server API pattern:

```rust
use cis_core::server::ServerApi;

async fn execute_command(server: Arc<dyn ServerApi>) -> Result<()> {
    let request = TaskListRequest {
        filters: TaskFilters {
            status: Some(TaskStatus::Pending),
            priority: Some(TaskPriority::P0),
            ..Default::default()
        },
        limit: Some(10),
    };

    let response = server.handle(Box::new(request)).await?;

    match response.status_code() {
        200 => Ok(()),
        _ => Err("Command failed".into()),
    }
}
```

**Benefits:**
- Single implementation for CLI, GUI, and Web API
- Unified error handling and logging
- Consistent behavior across all interfaces
- Automatic request/response validation

---

## Examples

### Complete Task Workflow

```bash
# 1. Create a task
cis task create TASK-001 "Refactor memory module" \
  --type refactoring \
  --priority P0

# 2. List tasks
cis task list --priority P0

# 3. Update task status
cis task update TASK-001 --status running

# 4. Mark as completed
cis task update TASK-001 --status completed

# 5. View task details
cis task show TASK-001

# 6. Delete if needed
cis task delete TASK-001
```

### Session Management Workflow

```bash
# 1. Create a session
cis session create claude claude --capacity 100000 --ttl-minutes 60

# 2. List active sessions
cis session list --status active

# 3. Show session details
cis session show 1

# 4. Release session when done
cis session release 1

# 5. Cleanup old sessions
cis session cleanup --older-than-days 7
```

### Engine Scanning Workflow

```bash
# 1. List supported engines
cis engine list-engines

# 2. Scan a project
cis engine scan ./MyGame --output scan-results.json

# 3. Generate reports in different formats
cis engine report scan-results.json --format markdown > report.md
cis engine report scan-results.json --format json > report.json
cis engine report scan-results.json --format csv > report.csv
```

---

## File Structure

```
cis-node/src/cli/
├── commands/
│   ├── mod.rs
│   ├── task.rs          # Task management command
│   ├── session.rs       # Session management command
│   └── engine.rs        # Engine scanning command
├── groups/
│   ├── mod.rs
│   ├── task.rs          # Task command group
│   ├── session.rs       # Session command group
│   └── engine.rs        # Engine command group
└── handlers/
    ├── mod.rs
    ├── task/mod.rs      # Task command handlers
    ├── session/mod.rs   # Session command handlers
    └── engine/mod.rs    # Engine command handlers
```

---

## Testing

All commands include unit tests. Run tests with:

```bash
# Test all commands
cargo test --package cis-node --lib cli::commands

# Test specific command
cargo test --package cis-node --lib cli::commands::task
cargo test --package cis-node --lib cli::commands::session
cargo test --package cis-node --lib cli::commands::engine
```

---

## Error Handling

All commands use the unified `CommandError` type:

```rust
pub struct CommandError {
    pub message: String,
    pub suggestions: Vec<String>,
    pub exit_code: i32,
    pub source: Option<anyhow::Error>,
}
```

**Example:**

```rust
return Err(CommandError::invalid_argument("priority", &priority)
    .with_suggestions(vec!["P0".to_string(), "P1".to_string()]));
```

---

## Next Steps

1. Implement TaskManager/SessionRepository initialization in CommandContext
2. Add integration tests with mock Server API
3. Create man pages for all commands
4. Add shell completion for all options
5. Implement progress bars for long-running operations
6. Add support for batch operations
7. Create interactive mode for complex commands
