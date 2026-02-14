# CIS Task Management Guide

**Version**: v1.1.6
**Last Updated**: 2026-02-13
**Target Audience**: CIS users, developers, project managers

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Understanding CIS Task System](#understanding-cis-task-system)
3. [Task Lifecycle](#task-lifecycle)
4. [Creating Tasks](#creating-tasks)
5. [Managing Tasks](#managing-tasks)
6. [Task Dependencies](#task-dependencies)
7. [Task Priorities](#task-priorities)
8. [Working with Task Types](#working-with-task-types)
9. [Monitoring and Tracking](#monitoring-and-tracking)
10. [Common Workflows](#common-workflows)
11. [Troubleshooting](#troubleshooting)
12. [Best Practices](#best-practices)

---

## Prerequisites

### System Requirements

- **CIS Installation**: CIS v1.1.5 or later installed
- **Database**: SQLite support (included with CIS)
- **Storage**: Minimum 100MB free space for task data
- **Permissions**: Read/write access to `~/.cis/data/`

### Knowledge Requirements

- Basic familiarity with command-line interface
- Understanding of task management concepts (optional but helpful)
- Knowledge of project workflows (for complex task dependencies)

### Initial Setup

```bash
# Initialize CIS (if not already done)
cis core init

# Verify task system is ready
cis core doctor

# Check database status
cis task list --limit 0
```

---

## Understanding CIS Task System

### What is CIS Task System?

The CIS Task System is a comprehensive task management framework built on SQLite, providing:

- **Persistent Storage**: All tasks stored in SQLite database (`~/.cis/data/tasks.db`)
- **Flexible Metadata**: Rich task attributes including type, priority, dependencies
- **Agent Integration**: Seamless assignment to AI agents and teams
- **DAG Support**: Task dependencies form a Directed Acyclic Graph (DAG)
- **Migration Support**: Import from legacy TOML format

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                      CIS Task System                       │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐      ┌──────────────┐                  │
│  │   CLI        │─────▶│ Task Service │                  │
│  │  Interface   │      │   (Rust)     │                  │
│  └──────────────┘      └──────┬───────┘                  │
│                              │                            │
│                              ▼                            │
│                     ┌─────────────────┐                   │
│                     │  SQLite Database │                   │
│                     │   tasks.db      │                   │
│                     └─────────────────┘                   │
│                              │                            │
│          ┌─────────────────────┼─────────────────────┐     │
│          ▼                     ▼                     ▼     │
│  ┌──────────────┐    ┌──────────────┐    ┌─────────────┐│
│  │   Task       │    │   Agent      │    │    DAG      ││
│  │  Repository  │    │  Assignment  │    │  Scheduler  ││
│  └──────────────┘    └──────────────┘    └─────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Task Attributes

Each task contains:

| Attribute | Type | Description |
|-----------|------|-------------|
| `task_id` | String | Unique task identifier (e.g., "TASK-001") |
| `name` | String | Human-readable task name |
| `task_type` | Enum | Task category (refactoring, injection, etc.) |
| `priority` | Enum | Task urgency (P0-P3, critical-low) |
| `status` | Enum | Current state (pending, running, completed, etc.) |
| `dependencies` | Vec<String> | List of task IDs this task depends on |
| `prompt_template` | String | AI prompt template for execution |
| `context_variables` | JSON | Additional task context |
| `assigned_team_id` | Option<String> | Team assigned to execute task |
| `assigned_agent_id` | Option<String> | Specific agent assigned |
| `estimated_effort_days` | Option<f64> | Estimated effort in person-days |
| `result` | Option<JSON> | Task execution result |
| `error_message` | Option<String> | Error details if failed |

---

## Task Lifecycle

### Task States

```
┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
│ Pending │───▶│Scheduled│───▶│ Running │───▶│Completed│
└─────────┘    └─────────┘    └─────────┘    └─────────┘
     ▲                             │
     │                             ▼
     │                        ┌─────────┐
     └───────────────────────│ Failed  │
              (retry)        └─────────┘
```

### State Descriptions

| State | Description | Transitions To |
|-------|-------------|---------------|
| **Pending** | Task created, waiting to be scheduled | Scheduled, Running, Cancelled |
| **Scheduled** | Task scheduled, waiting for agent | Running, Cancelled |
| **Running** | Task currently executing | Completed, Failed, Cancelled |
| **Completed** | Task finished successfully | - |
| **Failed** | Task execution failed | Pending (retry), Cancelled |
| **Cancelled** | Task cancelled by user | - |
| **Retrying** | Task is being retried after failure | Running, Failed |

### Lifecycle Events

```bash
# Create task (Pending)
cis task create TASK-001 "Fix bug" --type refactoring --priority P0

# Schedule task (Scheduled → Running)
# (Automatic when agent picks up task)

# Update status
cis task update TASK-001 --status running

# Mark complete
cis task update TASK-001 --status completed

# Handle failure
cis task update TASK-001 --status failed --error-message "Timeout"
```

---

## Creating Tasks

### Basic Task Creation

```bash
# Minimal task creation
cis task create TASK-001 "Refactor CLI parser" \
  --type refactoring \
  --priority P1
```

### Complete Task Creation

```bash
cis task create TASK-002 "Implement caching layer" \
  --type injection \
  --priority P0 \
  --dependencies TASK-001 \
  --description "Add Redis caching for API responses" \
  --prompt-template "Implement Redis caching with TTL of 3600 seconds"
```

### Task with Dependencies

```bash
# Create tasks with dependency chain
cis task create TASK-001 "Design database schema" \
  --type documentation \
  --priority P0

cis task create TASK-002 "Implement models" \
  --type injection \
  --priority P1 \
  --dependencies TASK-001

cis task create TASK-003 "Create migrations" \
  --type injection \
  --priority P1 \
  --dependencies TASK-002

cis task create TASK-004 "Write tests" \
  --type test \
  --priority P2 \
  --dependencies TASK-002,TASK-003
```

### Task Types

| Type | Description | Example |
|------|-------------|---------|
| `refactoring` | Code refactoring tasks | "Extract common logic" |
| `injection` | Feature implementation | "Add user authentication" |
| `optimization` | Performance improvements | "Optimize database queries" |
| `review` | Code review tasks | "Review PR #123" |
| `test` | Testing tasks | "Write unit tests" |
| `documentation` | Documentation tasks | "Update API docs" |

### Priority Levels

| Priority | Alias | Usage |
|----------|-------|-------|
| `P0` | `critical` | Urgent, blocks release |
| `P1` | `high` | Important, next milestone |
| `P2` | `medium` | Normal priority |
| `P3` | `low` | Nice to have |

---

## Managing Tasks

### Listing Tasks

```bash
# List all tasks
cis task list

# List pending tasks only
cis task list --status pending

# List P0 tasks
cis task list --priority P0

# List specific type
cis task list --task-type refactoring

# Combine filters
cis task list --status pending --priority P0 --limit 10

# List all tasks including completed
cis task list --all

# Sort by priority
cis task list --sort-by priority --sort-desc
```

### Viewing Task Details

```bash
# View task details
cis task show TASK-001

# Output example:
# Task ID: TASK-001
# Name: Refactor CLI parser
# Type: Refactoring
# Priority: P1 (high)
# Status: Pending
# Dependencies: None
# Created At: 2026-02-13 10:30:00 UTC
# Description: Refactor CLI parser for better error handling
```

### Updating Task Status

```bash
# Update status
cis task update TASK-001 --status running

# Update with error message
cis task update TASK-001 --status failed \
  --error-message "Compilation failed: missing dependency"

# Mark complete
cis task update TASK-001 --status completed
```

### Deleting Tasks

```bash
# Delete a task
cis task delete TASK-001

# Force delete (ignores dependencies)
cis task delete TASK-001 --force
```

### Bulk Operations

```bash
# Update multiple tasks (by priority)
for task_id in $(cis task list --priority P3 --json | jq -r '.[].id'); do
  cis task update "$task_id" --status cancelled
done

# Export tasks for analysis
cis task list --all --json > tasks-backup.json
```

---

## Task Dependencies

### Understanding Dependencies

Dependencies define execution order:

```
TASK-001 (Design)
    │
    ├─▶ TASK-002 (Implementation)
    │       │
    │       ├─▶ TASK-003 (Testing)
    │       │
    │       └─▶ TASK-004 (Documentation)
    │
    └─▶ TASK-005 (Review)
```

### Creating Dependent Tasks

```bash
# Linear dependency chain
cis task create TASK-001 "Design API" --type documentation --priority P0
cis task create TASK-002 "Implement API" --type injection --priority P1 --dependencies TASK-001
cis task create TASK-003 "Test API" --type test --priority P1 --dependencies TASK-002

# Multiple dependencies (fan-in)
cis task create TASK-004 "Integration test" --type test --priority P2 \
  --dependencies TASK-002,TASK-003
```

### Dependency Validation

```bash
# Create invalid dependency (circular)
cis task create TASK-002 "Implement" --dependencies TASK-003
cis task create TASK-003 "Test" --dependencies TASK-002

# CIS will detect and prevent circular dependencies
# Error: Circular dependency detected: TASK-002 -> TASK-003 -> TASK-002
```

### Visualizing Dependencies

```bash
# List tasks with dependencies
cis task list --json | jq '.[] | {id, name, dependencies}'

# Output:
# {
#   "id": "TASK-002",
#   "name": "Implement API",
#   "dependencies": ["TASK-001"]
# }
```

---

## Task Priorities

### Priority Matrix

```
High Impact │  P0  │  P1  │  P2  │
───────────┼──────┼──────┼──────┼
Low Impact │  P1  │  P2  │  P3  │
───────────┼──────┼──────┼──────┼
           │Urgent│Medium│Low   │
                    │Time  │
```

### Priority Assignment Guidelines

| Scenario | Priority | Rationale |
|----------|----------|-----------|
| Production outage | P0 | Immediate impact |
| Security vulnerability | P0 | High risk |
| Breaking build | P0 | Blocks development |
| Feature milestone | P1 | Committed deadline |
| Performance issue | P1 | User experience |
| Bug fix | P2 | Normal maintenance |
| Enhancement | P3 | Backlog item |

### Updating Priorities

```bash
# Escalate priority
cis task update TASK-001 --priority P0

# Downgrade priority
cis task update TASK-001 --priority P3
```

### Priority-Based Execution

CIS DAG Scheduler executes tasks by priority:

```bash
# View execution queue (sorted by priority)
cis task list --status pending --sort-by priority --sort-desc
```

---

## Working with Task Types

### Type-Specific Workflows

#### Refactoring Tasks

```bash
# Create refactoring task
cis task create REFACTOR-001 "Extract authentication logic" \
  --type refactoring \
  --priority P1 \
  --description "Extract common auth code into dedicated module"

# Assign to agent with refactoring capability
cis task update REFACTOR-001 --agent claude
```

#### Injection Tasks

```bash
# Create feature injection task
cis task create INJECT-001 "Add rate limiting" \
  --type injection \
  --priority P0 \
  --dependencies REFACTOR-001 \
  --prompt-template "Implement token bucket rate limiter"
```

#### Optimization Tasks

```bash
# Create optimization task
cis task create OPTIMIZE-001 "Reduce memory usage" \
  --type optimization \
  --priority P1 \
  --description "Profile and reduce memory footprint by 20%"
```

#### Review Tasks

```bash
# Create review task
cis task create REVIEW-001 "Review PR #123" \
  --type review \
  --priority P0 \
  --dependencies INJECT-001
```

#### Test Tasks

```bash
# Create test task
cis task create TEST-001 "Write unit tests for rate limiter" \
  --type test \
  --priority P1 \
  --dependencies INJECT-001
```

#### Documentation Tasks

```bash
# Create documentation task
cis task create DOC-001 "Update API documentation" \
  --type documentation \
  --priority P2 \
  --dependencies INJECT-001,TEST-001
```

---

## Monitoring and Tracking

### Real-Time Monitoring

```bash
# Watch task status (live)
watch -n 5 'cis task list --status running'

# Monitor pending tasks
watch -n 10 'cis task list --status pending --limit 5'
```

### Task Statistics

```bash
# Count tasks by status
cis task list --json | jq 'group_by(.status) | map({status: .[0].status, count: length})'

# Count tasks by priority
cis task list --json | jq 'group_by(.priority) | map({priority: .[0].priority, count: length})'

# Calculate completion rate
total=$(cis task list --all --json | jq 'length')
completed=$(cis task list --status completed --json | jq 'length')
echo "Completion: $((completed * 100 / total))%"
```

### Task Reports

```bash
# Generate daily report
cis task list --all --json > "tasks-$(date +%Y%m%d).json"

# Weekly summary
cis task list --all --json | jq '
  [
    group_by(.status)[],
    {total: length, completed: map(select(.status == "completed")) | length}
  ]
'
```

### Filtering by Time

```bash
# Tasks created today
cis task list --json | jq \
  "select(.created_at | startswith(\"$(date +%Y-%m-%d)\"))"

# Tasks completed this week
cis task list --status completed --json | jq \
  "select(.completed_at >= \"$(date -d '7 days ago' +%Y-%m-%d)\")"
```

---

## Common Workflows

### Workflow 1: Feature Development

```bash
#!/bin/bash
# feature-workflow.sh

FEATURE_NAME="$1"
BASE_ID=$(echo $FEATURE_NAME | tr '[:lower:]' '[:upper:]' | head -c 3)

echo "Creating feature workflow for: $FEATURE_NAME"

# 1. Design task
DESIGN_ID="${BASE_ID}-001"
cis task create $DESIGN_ID "Design $FEATURE_NAME" \
  --type documentation \
  --priority P0

# 2. Implementation task
IMPL_ID="${BASE_ID}-002"
cis task create $IMPL_ID "Implement $FEATURE_NAME" \
  --type injection \
  --priority P1 \
  --dependencies $DESIGN_ID

# 3. Testing task
TEST_ID="${BASE_ID}-003"
cis task create $TEST_ID "Test $FEATURE_NAME" \
  --type test \
  --priority P1 \
  --dependencies $IMPL_ID

# 4. Documentation task
DOC_ID="${BASE_ID}-004"
cis task create $DOC_ID "Document $FEATURE_NAME" \
  --type documentation \
  --priority P2 \
  --dependencies $IMPL_ID,$TEST_ID

# 5. Review task
REVIEW_ID="${BASE_ID}-005"
cis task create $REVIEW_ID "Review $FEATURE_NAME" \
  --type review \
  --priority P1 \
  --dependencies $TEST_ID,$DOC_ID

echo "Created workflow: $DESIGN_ID -> $IMPL_ID -> ($TEST_ID, $DOC_ID) -> $REVIEW_ID"
```

### Workflow 2: Bug Fix

```bash
#!/bin/bash
# bug-fix-workflow.sh

BUG_ID="$1"

# 1. Investigation task
cis task create "${BUG_ID}-INVESTIGATE" "Investigate $BUG_ID" \
  --type review \
  --priority P0

# 2. Fix task
cis task create "${BUG_ID}-FIX" "Fix $BUG_ID" \
  --type refactoring \
  --priority P0 \
  --dependencies "${BUG_ID}-INVESTIGATE"

# 3. Verification task
cis task create "${BUG_ID}-VERIFY" "Verify $BUG_ID fix" \
  --type test \
  --priority P0 \
  --dependencies "${BUG_ID}-FIX"

# 4. Regression test
cis task create "${BUG_ID}-REGRESSION" "Regression test for $BUG_ID" \
  --type test \
  --priority P1 \
  --dependencies "${BUG_ID}-VERIFY"
```

### Workflow 3: Code Review

```bash
#!/bin/bash
# review-workflow.sh

PR_NUMBER="$1"

# 1. Initial review
cis task create "REVIEW-${PR_NUMBER}" "Review PR #$PR_NUMBER" \
  --type review \
  --priority P1

# 2. If changes requested
cis task create "REVIEW-${PR_NUMBER}-FIX" "Address PR #$PR_NUMBER comments" \
  --type refactoring \
  --priority P1 \
  --dependencies "REVIEW-${PR_NUMBER}"

# 3. Re-review
cis task create "REVIEW-${PR_NUMBER}-RECHECK" "Re-review PR #$PR_NUMBER" \
  --type review \
  --priority P1 \
  --dependencies "REVIEW-${PR_NUMBER}-FIX"

# 4. Approve and merge
cis task create "MERGE-${PR_NUMBER}" "Merge PR #$PR_NUMBER" \
  --type injection \
  --priority P2 \
  --dependencies "REVIEW-${PR_NUMBER}-RECHECK"
```

### Workflow 4: Migration from TOML

```bash
# Migrate existing TOML tasks to SQLite
cis migrate run docs/plan/v1.1.6/TASKS_DEFINITIONS.toml --verify

# Verify migration
cis task list --all

# Check migrated tasks
cis task list --json | jq 'select(.metadata.migration_source == "toml")'
```

---

## Troubleshooting

### Common Issues

#### Issue: Task Not Appearing in List

**Symptoms**: Task created but not visible in `cis task list`

**Solutions**:

```bash
# Check if task is completed (hidden by default)
cis task list --all

# Verify task exists
cis task show TASK-001

# Check database status
sqlite3 ~/.cis/data/tasks.db "SELECT * FROM tasks WHERE task_id = 'TASK-001';"
```

#### Issue: Circular Dependency Error

**Symptoms**: Cannot create task due to circular dependency

**Solutions**:

```bash
# Visualize dependencies
cis task list --json | jq '.[] | {id, dependencies}'

# Break circular dependency
cis task delete CIRCULAR-TASK-ID

# Re-create without circular reference
cis task create NEW-ID "Task" --type refactoring --priority P1
```

#### Issue: Task Stuck in "Running" State

**Symptoms**: Task remains in running status indefinitely

**Solutions**:

```bash
# Check task logs
cis task logs TASK-001

# Force update status
cis task update TASK-001 --status failed \
  --error-message "Forcibly marked as failed"

# Retry task
cis task update TASK-001 --status pending
```

#### Issue: Database Locked

**Symptoms**: "database is locked" error

**Solutions**:

```bash
# Check for other processes
lsof ~/.cis/data/tasks.db

# Kill stuck processes
kill -9 $(lsof -t ~/.cis/data/tasks.db)

# Backup and recreate database
cp ~/.cis/data/tasks.db ~/.cis/data/tasks.db.backup
rm ~/.cis/data/tasks.db
cis core init --force
```

#### Issue: Migration Failed

**Symptoms**: TOML migration returns errors

**Solutions**:

```bash
# Validate TOML syntax
toml check docs/plan/v1.1.6/TASKS_DEFINITIONS.toml

# Check migration logs
cis migrate run docs/plan/v1.1.6/TASKS_DEFINITIONS.toml --verbose

# Dry-run migration
cis migrate run docs/plan/v1.1.6/TASKS_DEFINITIONS.toml --dry-run

# Rollback failed migration
cis migrate rollback --before $(date +%s)
```

### Debug Mode

```bash
# Enable debug logging
export RUST_LOG=cis_core=debug,cis_node=debug

# Run command with debug output
cis task list

# Check database directly
sqlite3 ~/.cis/data/tasks.db <<EOF
.headers on
.mode column
SELECT * FROM tasks ORDER BY created_at DESC LIMIT 10;
EOF
```

### Recovery Procedures

#### Recovering Lost Tasks

```bash
# Export all tasks
cis task list --all --json > tasks-recovery.json

# Find missing task
jq '.[] | select(.id == "TASK-001")' tasks-recovery.json

# Recreate from export
jq '.[] | select(.id == "TASK-001")' tasks-recovery.json | \
  jq -r '"cis task create \(.id) \"\(.name)\" --type \(.type) --priority \(.priority)"' | \
  bash
```

#### Fixing Corrupted Database

```bash
# Backup corrupted database
cp ~/.cis/data/tasks.db ~/.cis/data/tasks.db.corrupted

# Dump to SQL
sqlite3 ~/.cis/data/tasks.db.corrupted .dump > tasks-dump.sql

# Clean and restore
rm ~/.cis/data/tasks.db
sqlite3 ~/.cis/data/tasks.db < tasks-dump.sql

# Verify
cis task list --limit 5
```

---

## Best Practices

### Task ID Conventions

```bash
# Feature: FEAT-{NUMBER}-{SLUG}
cis task create FEAT-001-rate-limiter "Add rate limiting"

# Bug: BUG-{NUMBER}-{SLUG}
cis task create BUG-001-auth-leak "Fix authentication leak"

# Refactor: REFACTOR-{NUMBER}-{SLUG}
cis task create REFACTOR-001-parser "Extract parser logic"

# Tech Debt: DEBT-{NUMBER}-{SLUG}
cis task create DEBT-001-upgrade "Upgrade dependencies"
```

### Dependency Design

```bash
# ✅ GOOD: Linear dependencies
TASK-001 (Design) -> TASK-002 (Implement) -> TASK-003 (Test)

# ✅ GOOD: Fan-in dependencies
TASK-001 (Backend) ──┐
                     ├──▶ TASK-003 (Integration)
TASK-002 (Frontend) ┘

# ❌ BAD: Cross-dependencies (avoid)
TASK-001 ──▶ TASK-002 ──▶ TASK-003
   ▲                       │
   └───────────────────────┘
```

### Priority Assignment

```bash
# 1. Assess impact and urgency
# High Impact + Urgent = P0
# Low Impact + Urgent = P1
# High Impact + Not Urgent = P1
# Low Impact + Not Urgent = P2/P3

# 2. Re-evaluate regularly
# Escalate blocked tasks
# Downgrade completed milestones

# 3. Use priority consistently across team
cis task list --priority P0  # Should be < 5 tasks
cis task list --priority P1  # Should be < 10 tasks
```

### Task Granularity

```bash
# ❌ TOO BIG: "Build entire application"
cis task create BIG-001 "Build app"  # Takes months

# ✅ RIGHT SIZE: "Implement user authentication"
cis task create AUTH-001 "Implement auth"  # Takes 1-2 days

# ✅ RIGHT SIZE: "Design authentication schema"
cis task create AUTH-001-DESIGN "Design auth schema"  # Takes 2-4 hours
```

### Documentation

```bash
# Always include description
cis task create TASK-001 "Refactor parser" \
  --description "Extract recursive descent parser into separate module for better testability"

# Include context variables for complex tasks
cis task create TASK-002 "Add caching" \
  --context '{"backend": "redis", "ttl": 3600, "strategy": "write-through"}'

# Reference related code
cis task create TASK-003 "Fix race condition" \
  --description "See issue #123, affects src/concurrency/mod.rs:425"
```

### Automation

```bash
# Create task templates
cat > task-templates.json <<EOF
{
  "feature": {
    "type": "injection",
    "workflow": ["design", "implement", "test", "document"]
  },
  "bug": {
    "type": "refactoring",
    "workflow": ["investigate", "fix", "verify", "regression-test"]
  }
}
EOF

# Use templates with script
cis task create --template feature "Add OAuth2"
```

### Monitoring

```bash
# Daily task review
alias cis-today='cis task list --status pending,running --sort-by priority --sort-desc'

# Weekly report
alias cis-weekly='cis task list --all --json | jq \
  "group_by(.status) | map({status: .[0].status, count: length})"'

# Blocking tasks
alias cis-blockers='cis task list --priority P0 --status pending'
```

### Cleanup

```bash
# Archive completed tasks
cis task list --status completed --json > archive-$(date +%Y%m).json

# Delete old completed tasks (> 90 days)
for id in $(cis task list --status completed --json | \
  jq "select(.completed_at < \"$(date -d '90 days ago' +%Y-%m-%d)\") | .[].id"); do
  cis task delete "$id"
done

# Compact database
sqlite3 ~/.cis/data/tasks.db "VACUUM;"
```

### Integration with CI/CD

```bash
# In CI pipeline
#!/bin/bash
# .github/scripts/cis-task-update.sh

TASK_ID="$1"
STATUS="$2"

# Update task status
cis task update "$TASK_ID" --status "$STATUS"

# If failed, add error message
if [ "$STATUS" = "failed" ]; then
  cis task update "$TASK_ID" --error-message "CI build failed: $CI_BUILD_URL"
fi
```

---

## Advanced Topics

### Custom Task Types

```bash
# Extend task types via metadata
cis task create CUSTOM-001 "Performance audit" \
  --type optimization \
  --metadata '{"audit_type": "memory", "target_reduction": "20%"}'
```

### Task Hooks

```bash
# Pre-execution hook
cis task create TASK-001 "Deploy to staging" \
  --metadata '{"pre_hook": "run-integration-tests"}'

# Post-execution hook
cis task create TASK-002 "Notify team" \
  --metadata '{"post_hook": "send-slack-notification"}'
```

### Task Templates

```bash
# Save template
cis task template save feature \
  --type injection \
  --priority P1 \
  --dependencies "DESIGN-{BASE_ID}"

# Use template
cis task create --template feature "Add OAuth2"
```

---

## Reference

### CLI Commands

```bash
cis task list [options]
cis task create <id> <name> [options]
cis task update <id> [options]
cis task show <id>
cis task delete <id>
cis task logs <id>
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Task not found |
| 3 | Circular dependency |
| 4 | Database error |
| 5 | Invalid input |

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CIS_DATA_DIR` | Data directory | `~/.cis/data` |
| `CIS_TASK_DB` | Task database path | `$CIS_DATA_DIR/tasks.db` |
| `RUST_LOG` | Log level | `info` |

---

## Additional Resources

- [CIS CLI Reference](./cli-reference.md)
- [Migration Guide](./migration-guide.md)
- [Agent Teams Guide](./teams-execution-guide.md)
- [CIS Architecture](../ARCHITECTURE.md)

---

**Last Updated**: 2026-02-13
**For questions or issues**, visit [CIS GitHub](https://github.com/your-org/cis)
