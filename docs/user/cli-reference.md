# CIS CLI Reference Guide

**Version**: v1.1.6
**Last Updated**: 2026-02-13
**Target Audience**: CIS users, developers, system administrators

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [CLI Overview](#cli-overview)
3. [Core Commands](#core-commands)
4. [Task Commands](#task-commands)
5. [Memory Commands](#memory-commands)
6. [Session Commands](#session-commands)
7. [Engine Commands](#engine-commands)
8. [Migration Commands](#migration-commands)
9. [Agent Commands](#agent-commands)
10. [Team Commands](#team-commands)
11. [P2P Commands](#p2p-commands)
12. [Matrix Commands](#matrix-commands)
13. [Configuration](#configuration)
14. [Exit Codes](#exit-codes)
15. [Environment Variables](#environment-variables)
16. [Advanced Usage](#advanced-usage)

---

## Prerequisites

### Installation

```bash
# Install CIS
curl -sSL https://get.cis.dev/install.sh | sh

# Or via cargo
cargo install cis-node

# Verify installation
cis --version
# cis 1.1.6
```

### Initialization

```bash
# Initialize CIS
cis core init

# Initialize with options
cis core init --project --force --provider claude

# Verify setup
cis core status
```

### Shell Completion

```bash
# Generate completion for bash
cis core completion bash > ~/.bash_completion
source ~/.bash_completion

# Generate completion for zsh
cis core completion zsh > ~/.zsh_completion
source ~/.zsh_completion

# Generate completion for fish
cis core completion fish > ~/.config/fish/completions/cis.fish
```

---

## CLI Overview

### Command Structure

```
cis [GLOBAL_OPTIONS] <GROUP> <COMMAND> [COMMAND_OPTIONS] [ARGUMENTS]
```

### Global Options

| Option | Short | Description | Default |
|--------|--------|-------------|---------|
| `--config` | `-c` | Config file path | `~/.cis/config.toml` |
| `--data-dir` | `-d` | Data directory | `~/.cis/data` |
| `--verbose` | `-v` | Verbose output | `false` |
| `--quiet` | `-q` | Suppress output | `false` |
| `--json` | `-j` | JSON output | `false` |
| `--help` | `-h` | Show help | - |
| `--version` | `-V` | Show version | - |

### Command Groups

| Group | Description | Commands |
|-------|-------------|----------|
| `core` | Core functionality | init, status, config, doctor |
| `task` | Task management | list, create, update, show, delete |
| `memory` | Memory operations | get, set, delete, search, vector |
| `session` | Session management | start, stop, list, show, switch |
| `engine` | AI engine control | start, stop, status, list |
| `migrate` | Data migration | run, verify, rollback |
| `agent` | Agent management | list, start, stop, status |
| `team` | Team management | create, delete, status, configure |
| `p2p` | P2P networking | start, stop, status, peers |
| `matrix` | Matrix federation | connect, disconnect, status |

---

## Core Commands

### `cis core init`

Initialize CIS environment.

```bash
# Basic initialization
cis core init

# Initialize project
cis core init --project

# Force overwrite
cis core init --force

# Non-interactive mode
cis core init --non-interactive

# Skip environment checks
cis core init --skip-checks

# Specify provider
cis core init --provider claude
```

**Options**:

| Option | Description |
|--------|-------------|
| `--project` | Initialize as project (not global) |
| `--force` | Overwrite existing configuration |
| `--non-interactive` | Use defaults without prompts |
| `--skip-checks` | Skip environment validation |
| `--provider <provider>` | Default AI provider (claude, opencode, kimi) |

### `cis core status`

Show CIS status and configuration.

```bash
# Basic status
cis core status

# Show paths
cis core status --paths

# JSON output
cis core status --json
```

**Output**:

```
CIS Status
=========
Version: 1.1.6
Installation: /usr/local/bin/cis
Data Directory: /Users/user/.cis/data
Config File: /Users/user/.cis/config.toml

Database
========
Tasks DB: /Users/user/.cis/data/tasks.db (24.5 MB)
Tasks: 156 total (12 running, 45 pending, 99 completed)

Agents
=======
Active: 3
- claude (running)
- opencode (running)
- kimi (running)
```

### `cis core config`

Manage configuration.

```bash
# Get value
cis core config get user.name

# Set value
cis core config set user.name "John Doe"

# List all config
cis core config list

# List as JSON
cis core config list --json

# Open config in editor
cis core config edit
```

### `cis core doctor`

Run diagnostics.

```bash
# Run diagnostics
cis core doctor

# Auto-fix issues
cis core doctor --fix

# Verbose output
cis core doctor --verbose
```

**Checks**:

- Database integrity
- File permissions
- Network connectivity
- Agent status
- Configuration validity
- Disk space

### `cis core completion`

Generate shell completion scripts.

```bash
# Bash
cis core completion bash

# Zsh
cis core completion zsh

# Fish
cis core completion fish

# PowerShell
cis core completion powershell
```

---

## Task Commands

### `cis task list`

List tasks.

```bash
# List all tasks
cis task list

# Filter by status
cis task list --status pending
cis task list --status running
cis task list --status completed
cis task list --status failed

# Filter by priority
cis task list --priority P0
cis task list --priority critical

# Filter by type
cis task list --task-type refactoring
cis task list --task-type injection

# Limit results
cis task list --limit 10

# Show all (including completed)
cis task list --all

# Sort options
cis task list --sort-by priority --sort-desc
cis task list --sort-by created

# JSON output
cis task list --json
```

**Filters**:

| Filter | Values | Description |
|--------|---------|-------------|
| `--status` | pending, running, completed, failed, cancelled | Filter by status |
| `--priority` | P0, P1, P2, P3, critical, high, medium, low | Filter by priority |
| `--task-type` | refactoring, injection, optimization, review, test, documentation | Filter by type |

### `cis task create`

Create new task.

```bash
# Basic task
cis task create TASK-001 "Refactor CLI parser" \
  --type refactoring \
  --priority P1

# With dependencies
cis task create TASK-002 "Implement caching" \
  --type injection \
  --priority P0 \
  --dependencies TASK-001

# With description
cis task create TASK-003 "Add tests" \
  --type test \
  --priority P2 \
  --description "Add unit tests for caching module"

# With prompt template
cis task create TASK-004 "Optimize queries" \
  --type optimization \
  --priority P1 \
  --prompt-template "Analyze and optimize slow queries in database layer"
```

**Options**:

| Option | Description | Required |
|--------|-------------|-----------|
| `task_id` | Unique task identifier | Yes |
| `name` | Task name | Yes |
| `--type` | Task type | Yes |
| `--priority` | Task priority | Yes |
| `--dependencies` | Comma-separated task IDs | No |
| `--prompt-template` | AI prompt template | No |
| `--description` | Task description | No |

### `cis task update`

Update task status.

```bash
# Update status
cis task update TASK-001 --status running

# Mark completed
cis task update TASK-001 --status completed

# Mark failed with error
cis task update TASK-001 --status failed \
  --error-message "Compilation failed: missing dependency"

# Assign to agent
cis task update TASK-001 --agent claude
```

**Status Values**:

| Status | Description |
|--------|-------------|
| `pending` | Task not started |
| `assigned` | Task assigned to agent |
| `running` | Task in progress |
| `completed` | Task finished successfully |
| `failed` | Task execution failed |

### `cis task show`

Show task details.

```bash
# Show task
cis task show TASK-001

# JSON output
cis task show TASK-001 --json
```

**Output**:

```
Task: TASK-001
============
Name: Refactor CLI parser
Type: Refactoring
Priority: P1 (high)
Status: Running

Dependencies
------------
TASK-000 (completed)

Assignment
----------
Agent: claude
Team: development-team

Timeline
--------
Created: 2026-02-13 10:30:00 UTC
Started: 2026-02-13 11:00:00 UTC
Estimated: 4 hours
```

### `cis task delete`

Delete task.

```bash
# Delete task
cis task delete TASK-001

# Force delete (ignores dependencies)
cis task delete TASK-001 --force
```

### `cis task assign`

Assign task to agent.

```bash
# Assign to agent
cis task assign TASK-001 --agent claude

# Assign multiple
cis task assign TASK-001 TASK-002 TASK-003 --agent opencode

# Reassign
cis task reassign TASK-001 --from claude --to kimi
```

---

## Memory Commands

### `cis memory get`

Get memory value.

```bash
# Get memory
cis memory get user.preference.theme

# Output: dark
```

### `cis memory set`

Set memory value.

```bash
# Set in public domain
cis memory set user.preference.theme dark --domain public

# Set in private domain
cis memory set api.key "sk-..." --domain private

# Set with category
cis memory set project.architecture "microservices" \
  --domain public \
  --category context

# Set with semantic index
cis memory set user.preference.editor "vscode" \
  --index
```

**Domains**:

| Domain | Description | Sync |
|--------|-------------|------|
| `public` | Shared across devices | Yes |
| `private` | Local only, encrypted | No |

**Categories**:

| Category | Description |
|----------|-------------|
| `context` | Contextual information |
| `execution` | Execution history |
| `result` | Task results |
| `error` | Error logs |
| `skill` | Skill data |

### `cis memory delete`

Delete memory entry.

```bash
# Delete memory
cis memory delete user.preference.theme
```

### `cis memory search`

Search memory by keyword.

```bash
# Search
cis memory search "theme"

# Limit results
cis memory search "architecture" --limit 10
```

### `cis memory vector`

Semantic search using vector embeddings.

```bash
# Semantic search
cis memory vector "what theme did i prefer"

# With limit
cis memory vector "database configuration" --limit 10

# With threshold
cis memory vector "api setup" --threshold 0.7

# Filter by category
cis memory vector "project decisions" --category context

# Output formats
cis memory vector "caching strategy" --format json
cis memory vector "caching strategy" --format table
```

**Output Formats**:

| Format | Description |
|--------|-------------|
| `plain` | Human-readable text |
| `json` | JSON format |
| `table` | Table format |

### `cis memory list`

List memory keys.

```bash
# List all
cis memory list

# Filter by prefix
cis memory list --prefix user

# Filter by domain
cis memory list --domain public

# Output format
cis memory list --format json
```

### `cis memory stats`

Show memory statistics.

```bash
# General stats
cis memory stats

# By domain
cis memory stats --by-domain

# By category
cis memory stats --by-category
```

**Output**:

```
Memory Statistics
================
Total Entries: 1,234
Public: 987
Private: 247

By Category
-----------
Context: 456
Execution: 321
Result: 234
Error: 123
Skill: 100
```

---

## Session Commands

### `cis session start`

Start new session.

```bash
# Start session
cis session start

# With name
cis session start --name "feature-development"

# With context
cis session start --context project=ecommerce,phase=development
```

### `cis session stop`

Stop current session.

```bash
# Stop session
cis session stop

# Save results
cis session stop --save
```

### `cis session list`

List sessions.

```bash
# List all
cis session list

# Active only
cis session list --active

# With details
cis session list --verbose
```

### `cis session show`

Show session details.

```bash
# Show current
cis session show

# Show specific
cis session show <session-id>

# JSON output
cis session show --json
```

### `cis session switch`

Switch session.

```bash
# Switch to session
cis session switch <session-id>

# Switch by name
cis session switch --name "feature-development"
```

---

## Engine Commands

### `cis engine start`

Start AI engine.

```bash
# Start engine
cis engine start

# Start specific
cis engine start claude

# With configuration
cis engine start --model claude-3-sonnet --timeout 600
```

### `cis engine stop`

Stop AI engine.

```bash
# Stop engine
cis engine stop

# Stop specific
cis engine stop claude

# Force stop
cis engine stop --force
```

### `cis engine status`

Show engine status.

```bash
# All engines
cis engine status

# Specific engine
cis engine status claude

# JSON output
cis engine status --json
```

**Output**:

```
Engine Status
=============
claude: Running
  - Model: claude-3-sonnet
  - Uptime: 2h 34m
  - Tasks: 12
  - Memory: 512MB

opencode: Stopped
kimi: Running
  - Model: glm-4.7-free
  - Uptime: 1h 15m
  - Tasks: 5
  - Memory: 256MB
```

### `cis engine list`

List available engines.

```bash
# List all
cis engine list

# With capabilities
cis engine list --capabilities
```

---

## Migration Commands

### `cis migrate run`

Run migration from TOML to SQLite.

```bash
# Migrate file
cis migrate run ~/.cis/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Migrate directory
cis migrate run ~/.cis/plan/v1.1.6/

# With verification
cis migrate run ~/.cis/plan/v1.1.6/ --verify

# Dry run
cis migrate run ~/.cis/plan/v1.1.6/ --dry-run

# Verbose
cis migrate run ~/.cis/plan/v1.1.6/ --verbose
```

### `cis migrate verify`

Verify migration results.

```bash
# Verify migration
cis migrate verify

# Check specific database
cis migrate verify --database /path/to/tasks.db
```

### `cis migrate rollback`

Rollback migration.

```bash
# Rollback all
cis migrate rollback --before 1678886400

# Rollback in specific database
cis migrate rollback --before 1678886400 \
  --database /path/to/tasks.db
```

---

## Agent Commands

### `cis agent list`

List agents.

```bash
# List all
cis agent list

# With status
cis agent list --status

# JSON output
cis agent list --json
```

### `cis agent start`

Start agent.

```bash
# Start agent
cis agent start claude

# With configuration
cis agent start claude --model claude-3-sonnet

# Background
cis agent start claude --background
```

### `cis agent stop`

Stop agent.

```bash
# Stop agent
cis agent stop claude

# Force stop
cis agent stop claude --force
```

### `cis agent status`

Show agent status.

```bash
# All agents
cis agent status

# Specific agent
cis agent status claude

# JSON output
cis agent status --json
```

### `cis agent logs`

Show agent logs.

```bash
# Show logs
cis agent logs claude

# Follow logs
cis agent logs claude --follow

# Tail lines
cis agent logs claude --tail 50

# Since time
cis agent logs claude --since "1h ago"
```

### `cis agent ping`

Test agent connectivity.

```bash
# Ping agent
cis agent ping claude

# With count
cis agent ping claude --count 5
```

---

## Team Commands

### `cis team create`

Create team.

```bash
# Create team
cis team create development-team

# With ID
cis team create development-team --id team-dev-001

# From template
cis team create --template development my-dev-team
```

### `cis team delete`

Delete team.

```bash
# Delete team
cis team delete development-team

# Force delete
cis team delete development-team --force
```

### `cis team status`

Show team status.

```bash
# Show status
cis team status development-team

# JSON output
cis team status development-team --json
```

### `cis team agents`

Show team agents.

```bash
# List agents
cis team agents development-team

# With details
cis team agents development-team --verbose
```

### `cis team add-member`

Add team member.

```bash
# Add member
cis team add-member development-team \
  --name planner \
  --agent claude \
  --role planner \
  --capabilities planning,analysis
```

### `cis team remove-member`

Remove team member.

```bash
# Remove member
cis team remove-member development-team planner
```

### `cis team configure`

Configure team.

```bash
# Set configuration
cis team configure development-team routing hierarchical

# Get configuration
cis team get-config development-team routing
```

---

## P2P Commands

### `cis p2p start`

Start P2P network.

```bash
# Start P2P
cis p2p start

# With configuration
cis p2p start --listen-port 7677
```

### `cis p2p stop`

Stop P2P network.

```bash
# Stop P2P
cis p2p stop
```

### `cis p2p status`

Show P2P status.

```bash
# Show status
cis p2p status

# JSON output
cis p2p status --json
```

### `cis p2p peers`

Show connected peers.

```bash
# List peers
cis p2p peers

# With details
cis p2p peers --verbose
```

### `cis p2p connect`

Connect to peer.

```bash
# Connect to peer
cis p2p connect /ip4/192.168.1.100/tcp/7677/p2p/12D3KooW...
```

---

## Matrix Commands

### `cis matrix connect`

Connect to Matrix server.

```bash
# Connect
cis matrix connect

# With credentials
cis matrix connect --user @bot:server.com --password secret
```

### `cis matrix disconnect`

Disconnect from Matrix.

```bash
# Disconnect
cis matrix disconnect
```

### `cis matrix status`

Show Matrix status.

```bash
# Show status
cis matrix status

# JSON output
cis matrix status --json
```

---

## Configuration

### Config File Structure

```toml
# ~/.cis/config.toml

[core]
data_dir = "~/.cis/data"
log_level = "info"
max_log_size = "100MB"

[agent]
default_agent = "claude"
timeout = 600
max_retries = 3

[memory]
domain = "public"
enable_vector_search = true
vector_threshold = 0.7

[p2p]
enabled = true
listen_port = 7677
bootstrap_nodes = []

[matrix]
enabled = false
homeserver = "https://matrix.org"
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CIS_DATA_DIR` | Data directory | `~/.cis/data` |
| `CIS_CONFIG_FILE` | Config file path | `~/.cis/config.toml` |
| `CIS_LOG_LEVEL` | Log level | `info` |
| `CIS_DEFAULT_AGENT` | Default agent | `claude` |
| `CIS_TASK_DB` | Task database | `$CIS_DATA_DIR/tasks.db` |
| `RUST_LOG` | Rust logging | `info` |

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Invalid usage |
| 3 | Task not found |
| 4 | Database error |
| 5 | Network error |
| 6 | Agent error |
| 7 | Permission denied |
| 8 | Configuration error |
| 9 | Migration error |
| 10 | Timeout |

---

## Advanced Usage

### Command Aliases

```bash
# Create alias
alias cis-tasks='cis task list --status pending --sort-by priority'

# Use alias
cis-tasks
```

### Command Chaining

```bash
# Chain commands
cis task create TASK-001 "Test" --type test --priority P1 && \
cis task assign TASK-001 --agent claude && \
cis agent status claude
```

### JSON Processing

```bash
# Process JSON output
cis task list --json | jq '.[] | select(.priority == "P0")'

# Extract specific fields
cis task show TASK-001 --json | jq '{id, name, status}'
```

### Batch Operations

```bash
# Batch task creation
for i in {1..10}; do
  cis task create "TASK-$i" "Task $i" --type test --priority P2
done

# Batch status update
cis task list --status pending --json | \
  jq -r '.[].id' | \
  xargs -I {} cis task update {} --status running
```

### Parallel Execution

```bash
# Parallel task execution
parallel cis task run ::: TASK-001 TASK-002 TASK-003

# Parallel migration
find ~/.cis/plan -name "*.toml" | parallel \
  "cis migrate run {} --verify"
```

---

## Tips and Tricks

### Productivity Tips

```bash
# Quick status check
alias cis-status='cis core status && cis task list --limit 5'

# Quick task creation
function cis-quick() {
  cis task create "TASK-$(date +%s)" "$1" --type "${2:-refactoring}" --priority P2
}

# Monitor tasks
watch -n 5 'cis task list --status running'
```

### Debugging Tips

```bash
# Enable debug logging
export RUST_LOG=cis_core=debug,cis_node=debug

# Verbose output
cis task list --verbose

# Trace execution
cis task run TASK-001 --trace
```

### Performance Tips

```bash
# Use JSON for faster processing
cis task list --json | jq ...

# Limit results
cis task list --limit 100

# Cache results
cis task list --json > /tmp/tasks.json
```

---

## Additional Resources

- [Task Management Guide](./task-management-guide.md)
- [Migration Guide](./migration-guide.md)
- [Teams Execution Guide](./teams-execution-guide.md)
- [CIS Architecture](../ARCHITECTURE.md)

---

**Last Updated**: 2026-02-13
**For questions or issues**, visit [CIS GitHub](https://github.com/your-org/cis)
