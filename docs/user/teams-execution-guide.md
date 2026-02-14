# CIS Agent Teams Execution Guide

**Version**: v1.1.6
**Last Updated**: 2026-02-13
**Target Audience**: CIS users, system architects, DevOps engineers

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Understanding Agent Teams](#understanding-agent-teams)
3. [Team Architecture](#team-architecture)
4. [Creating Teams](#creating-teams)
5. [Team Communication](#team-communication)
6. [Task Distribution](#task-distribution)
7. [Monitoring Teams](#monitoring-teams)
8. [Advanced Patterns](#advanced-patterns)
9. [Troubleshooting](#troubleshooting)
10. [Best Practices](#best-practices)

---

## Prerequisites

### System Requirements

- **CIS Version**: v1.1.6 or later
- **Network**: P2P or Matrix Federation enabled (for multi-node teams)
- **Agents**: At least 2 AI agent runtimes configured
- **Memory**: Minimum 2GB RAM per agent
- **Database**: SQLite with team support enabled

### Configuration Requirements

```bash
# Verify prerequisites
cis core status --agent

# Check available agents
cis agent list

# Verify network status
cis p2p status
```

### Knowledge Requirements

- Understanding of AI agent capabilities
- Familiarity with task delegation
- Knowledge of distributed systems (helpful)
- Team coordination concepts

---

## Understanding Agent Teams

### What are Agent Teams?

CIS Agent Teams enable multiple AI agents to collaborate on complex tasks:

```
┌─────────────────────────────────────────────────────────────┐
│                   Agent Team Coordination                   │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐      ┌──────────────┐                  │
│  │   Claude     │      │  OpenCode    │                  │
│  │  (Planning)  │◄────►│ (Execution)  │                  │
│  └──────────────┘      └──────────────┘                  │
│          ▲                     ▲                          │
│          │                     │                          │
│          └─────────┬─────────┘                          │
│                    │                                    │
│            ┌───────────────┐                             │
│            │ Team Manager  │                             │
│            │  (CIS Core)  │                             │
│            └───────────────┘                             │
└─────────────────────────────────────────────────────────────┘
```

### Benefits of Agent Teams

| Aspect | Single Agent | Agent Teams |
|--------|-------------|-------------|
| **Task Complexity** | Limited | High complexity |
| **Specialization** | Generalist | Domain experts |
| **Parallelism** | Sequential | Parallel execution |
| **Resilience** | Single point of failure | Fault-tolerant |
| **Scalability** | Limited | Highly scalable |
| **Cost** | Lower | Higher |

### Use Cases

1. **Code Development**: Planner agent (Claude) + Coder agent (OpenCode)
2. **Testing**: Test generator + Test executor + Analyzer
3. **Documentation**: Writer + Reviewer + Publisher
4. **Research**: Searcher + Analyzer + Synthesizer
5. **Operations**: Monitor + Responder + Reporter

---

## Team Architecture

### Team Models

#### Model 1: Hierarchical Team

```
┌─────────────────────────────────────┐
│         Team Leader                │
│         (Claude)                  │
│    - Task decomposition            │
│    - Result aggregation           │
└────────┬────────────────────────┬───┘
         │                     │
    ┌────┴─────┐         ┌───┴─────┐
    │  Worker  │         │  Worker  │
    │(OpenCode)│         │  (Kimi)  │
    └──────────┘         └──────────┘
```

**Use Case**: Complex task requiring coordination

#### Model 2: Peer Team

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│    Claude    │    │  OpenCode    │    │    Kimi     │
│   (Planner)  │◄──►│  (Coder)     │◄──►│ (Reviewer)  │
└──────────────┘    └──────────────┘    └──────────────┘
         ▲                     ▲                     ▲
         └─────────────────────┴─────────────────────┘
                          │
                   ┌───────────────┐
                   │ Shared Task   │
                   │    Queue      │
                   └───────────────┘
```

**Use Case**: Collaborative problem solving

#### Model 3: Pipeline Team

```
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│   Generator  │───▶│  Executor   │───▶│  Validator  │
│   (Claude)   │    │ (OpenCode)   │    │   (Kimi)    │
└──────────────┘    └──────────────┘    └──────────────┘
```

**Use Case**: Sequential task processing

### Team Member Roles

| Role | Responsibility | Example Agents |
|------|---------------|----------------|
| **Planner** | Task decomposition, dependency analysis | Claude, GPT-4 |
| **Executor** | Code generation, task execution | OpenCode, Kimi |
| **Reviewer** | Quality assurance, validation | Claude, Kimi |
| **Monitor** | Progress tracking, error detection | Custom agents |
| **Reporter** | Result aggregation, reporting | Any agent |

---

## Creating Teams

### Method 1: Configuration File

```toml
# ~/.cis/config/teams.toml

[[teams]]
name = "development-team"
id = "team-dev-001"

# Team members
[[teams.members]]
name = "planner"
agent = "claude"
role = "planner"
capabilities = ["planning", "analysis"]

[[teams.members]]
name = "coder"
agent = "opencode"
role = "executor"
capabilities = ["coding", "testing"]

[[teams.members]]
name = "reviewer"
agent = "kimi"
role = "reviewer"
capabilities = ["review", "validation"]

# Team configuration
[teams.config]
max_concurrent_tasks = 3
coordination_mode = "hierarchical"
leader = "planner"

# Task routing
[teams.routing]
rules = [
  {match = "type:planning", assign_to = "planner"},
  {match = "type:coding", assign_to = "coder"},
  {match = "type:review", assign_to = "reviewer"}
]
```

### Method 2: CLI Commands

```bash
# Create team
cis team create development-team \
  --id team-dev-001 \
  --max-concurrent 3 \
  --coordination hierarchical

# Add members
cis team add-member development-team \
  --name planner \
  --agent claude \
  --role planner \
  --capabilities planning,analysis

cis team add-member development-team \
  --name coder \
  --agent opencode \
  --role executor \
  --capabilities coding,testing

cis team add-member development-team \
  --name reviewer \
  --agent kimi \
  --role reviewer \
  --capabilities review,validation

# Set team leader
cis team set-leader development-team planner
```

### Method 3: Programmatic Creation

```rust
use cis_core::agent::team::{Team, TeamMember, TeamConfig};

// Create team
let mut team = Team::new(
    "development-team",
    "team-dev-001"
);

// Add members
team.add_member(TeamMember {
    name: "planner".to_string(),
    agent: "claude".to_string(),
    role: "planner".to_string(),
    capabilities: vec!["planning".to_string(), "analysis".to_string()],
});

team.add_member(TeamMember {
    name: "coder".to_string(),
    agent: "opencode".to_string(),
    role: "executor".to_string(),
    capabilities: vec!["coding".to_string(), "testing".to_string()],
});

// Configure team
team.set_config(TeamConfig {
    max_concurrent_tasks: 3,
    coordination_mode: CoordinationMode::Hierarchical,
    leader: Some("planner".to_string()),
});

// Save team
team.save().await?;
```

### Team Templates

```bash
# Pre-configured templates
cis team create --template development my-dev-team
cis team create --template research my-research-team
cis team create --template testing my-test-team

# Available templates
cis team template list
# - development: Planner + Coder + Reviewer
# - research: Searcher + Analyzer + Synthesizer
# - testing: Generator + Executor + Validator
# - operations: Monitor + Responder + Reporter
```

---

## Team Communication

### Communication Channels

CIS supports multiple communication patterns:

#### 1. Shared Task Queue

```bash
# Tasks automatically routed to team
cis task create TASK-001 "Implement feature" \
  --team development-team \
  --type coding

# Task routed to "coder" agent based on rules
```

#### 2. Direct Agent Communication

```bash
# Agent A sends message to Agent B
cis agent send-message planner coder \
  --content "Task analysis complete, ready for implementation"
```

#### 3. Broadcast Communication

```bash
# Team leader broadcasts to all members
cis team broadcast development-team \
  --from planner \
  --content "Project deadline extended to 2026-02-20"
```

#### 4. Matrix Federation

```bash
# Multi-node team communication
cis matrix send-message --room #development-team \
  --content "Code review completed"

# Agents on different nodes communicate via Matrix
```

### Communication Patterns

#### Pattern 1: Request-Response

```
Planner          Coder
   │   ───────▶   │
   │   Request     │
   │               │
   │   ◀────────   │
   │   Response    │
```

**Use Case**: Task assignment and result reporting

#### Pattern 2: Publish-Subscribe

```
Monitor  ──▶  Event Bus  ◀──  Reporter
   │                            │
   └─────────────┬──────────────┘
                 │
             Analyzer
```

**Use Case**: Event-driven coordination

#### Pattern 3: Fan-Out/Fan-In

```
           ┌─────────┐
Planner ──▶│ Worker1 │──┐
           ├─────────┤  │
           │ Worker2 │──┼──▶ Aggregator
           ├─────────┤  │
           │ Worker3 │──┘
           └─────────┘
```

**Use Case**: Parallel task execution

### Message Formats

#### Task Assignment Message

```json
{
  "type": "task_assignment",
  "task_id": "TASK-001",
  "from": "planner",
  "to": "coder",
  "payload": {
    "name": "Implement caching",
    "type": "coding",
    "priority": "P0",
    "context": {
      "spec": "Add Redis caching",
      "deadline": "2026-02-15"
    }
  },
  "timestamp": "2026-02-13T10:30:00Z"
}
```

#### Progress Update Message

```json
{
  "type": "progress_update",
  "task_id": "TASK-001",
  "from": "coder",
  "to": "planner",
  "payload": {
    "status": "in_progress",
    "progress": 0.6,
    "message": "Cache implementation 60% complete"
  },
  "timestamp": "2026-02-13T11:00:00Z"
}
```

#### Result Message

```json
{
  "type": "result",
  "task_id": "TASK-001",
  "from": "coder",
  "to": "planner",
  "payload": {
    "status": "completed",
    "result": {
      "files_modified": ["src/cache.rs"],
      "lines_added": 150
    }
  },
  "timestamp": "2026-02-13T12:00:00Z"
}
```

---

## Task Distribution

### Automatic Routing

```bash
# Configure routing rules
cis team configure routing development-team <<EOF
rules:
  - match: "type:planning"
    assign_to: "planner"
  - match: "type:coding"
    assign_to: "coder"
  - match: "type:review"
    assign_to: "reviewer"
EOF

# Tasks automatically routed
cis task create TASK-001 "Design API" \
  --team development-team \
  --type planning \
  # Routed to "planner" agent

cis task create TASK-002 "Implement API" \
  --team development-team \
  --type coding \
  # Routed to "coder" agent
```

### Manual Assignment

```bash
# Manually assign task to specific agent
cis task assign TASK-001 --agent claude

# Assign multiple tasks
cis task assign TASK-001 TASK-002 TASK-003 --agent opencode

# Reassign task
cis task reassign TASK-001 --from claude --to kimi
```

### Load Balancing

```bash
# Enable load balancing
cis team configure load-balancing development-team \
  --strategy round-robin

# Available strategies:
# - round-robin: Distribute evenly
# - least-busy: Assign to least loaded agent
# - capability: Match agent capabilities
# - priority: Route to highest-priority available agent

# Check agent load
cis team load development-team
# Output:
# planner: 2 tasks (queued: 1)
# coder: 5 tasks (queued: 3)
# reviewer: 1 task (queued: 0)
```

### Dependency Handling

```bash
# Tasks with dependencies automatically ordered
cis task create TASK-001 "Design" --team development-team
cis task create TASK-002 "Implement" --team development-team --dependencies TASK-001
cis task create TASK-003 "Review" --team development-team --dependencies TASK-002

# Team coordinator ensures execution order
# TASK-001 (planner) → TASK-002 (coder) → TASK-003 (reviewer)
```

---

## Monitoring Teams

### Team Status

```bash
# Check overall team status
cis team status development-team

# Output:
# Team: development-team (team-dev-001)
# Status: Active
# Members: 3 online
# Tasks: 12 total (5 running, 4 pending, 3 completed)
# Leader: planner (claude)
```

### Agent Status

```bash
# Check individual agent status
cis team agents development-team

# Output:
# ┌─────────┬──────────┬─────────┬─────────┬──────────┐
# │ Agent   │ Status   │ Tasks   │ Load    │ Health   │
# ├─────────┼──────────┼─────────┼─────────┼──────────┤
# │ planner │ running  │ 2       │ 20%     │ healthy  │
# │ coder   │ running  │ 5       │ 80%     │ warning  │
# │ reviewer│ running  │ 1       │ 10%     │ healthy  │
# └─────────┴──────────┴─────────┴─────────┴──────────┘
```

### Task Progress

```bash
# Monitor team task progress
cis team tasks development-team --status running

# Output:
# ┌──────────┬─────────────┬──────────┬────────────┐
# │ Task ID  │ Assigned To │ Progress │ Status     │
# ├──────────┼─────────────┼──────────┼────────────┤
# │ TASK-001 │ coder       │ 60%      │ in_progress│
# │ TASK-002 │ planner     │ 100%     │ completed  │
# │ TASK-003 │ reviewer    │ 30%      │ in_progress│
# └──────────┴─────────────┴──────────┴────────────┘
```

### Performance Metrics

```bash
# Get team performance metrics
cis team metrics development-team

# Output:
# Average Task Completion Time: 2.5 hours
# Throughput: 4.8 tasks/hour
# Success Rate: 95.2%
# Agent Utilization:
#   - planner: 25%
#   - coder: 85%
#   - reviewer: 40%
```

### Real-Time Monitoring

```bash
# Watch team activity
watch -n 5 'cis team status development-team'

# Monitor agent logs
cis team logs development-team --agent coder --follow

# Task completion notifications
cis team watch development-team \
  --events task_completed,task_failed
```

---

## Advanced Patterns

### Pattern 1: Hierarchical Task Decomposition

```bash
# Level 1: High-level planning
cis task create PROJECT-001 "Build e-commerce platform" \
  --team dev-team \
  --type planning

# Planner decomposes into sub-tasks:
# - TASK-002: Design database (coder)
# - TASK-003: Implement API (coder)
# - TASK-004: Create frontend (coder)
# - TASK-005: Integration testing (tester)
# - TASK-006: Documentation (writer)

# Agents work on sub-tasks in parallel
```

### Pattern 2: Peer Review Loop

```bash
# Initial implementation
cis task create TASK-001 "Implement auth" \
  --team dev-team \
  --type coding \
  --assign-to coder

# Automatic review
cis task create TASK-002 "Review auth implementation" \
  --team dev-team \
  --type review \
  --dependencies TASK-001 \
  --assign-to reviewer

# If review fails, task returns to coder
# (automated via workflow rules)
```

### Pattern 3: Specialist Teams

```bash
# Create specialist teams
cis team create security-team --id team-sec-001
cis team create performance-team --id team-perf-001
cis team create ui-team --id team-ui-001

# Route tasks to specialists
cis task create TASK-001 "Security audit" \
  --team security-team

cis task create TASK-002 "Performance optimization" \
  --team performance-team

cis task create TASK-003 "UI redesign" \
  --team ui-team
```

### Pattern 4: Multi-Stage Pipeline

```bash
# Define pipeline stages
cis team configure pipeline dev-team <<EOF
stages:
  - name: planning
    agent: planner
    output: design_doc
  - name: implementation
    agent: coder
    input: design_doc
    output: source_code
  - name: review
    agent: reviewer
    input: source_code
    output: review_report
EOF

# Execute pipeline
cis team execute-pipeline dev-team --input "Build auth system"
```

### Pattern 5: Dynamic Team Formation

```bash
# Agents form temporary teams for specific tasks
cis task create TASK-001 "Emergency bug fix" \
  --priority P0 \
  --requirements "planner,coder,reviewer" \
  --dynamic-team

# CIS automatically forms team with available agents
# Team disbands after task completion
```

---

## Troubleshooting

### Issue: Agent Not Responding

**Symptoms**: Tasks stuck, no progress updates

**Diagnosis**:

```bash
# Check agent status
cis team agents development-team

# Check agent logs
cis agent logs claude --tail 50

# Test agent connectivity
cis agent ping claude
```

**Solutions**:

```bash
# Restart stuck agent
cis agent restart claude

# If unresponsive, kill and restart
cis agent kill claude --force
cis agent start claude

# Reassign tasks from dead agent
cis team reassign-tasks development-team --from claude --to opencode
```

### Issue: Task Not Assigned

**Symptoms**: Task in pending state, no agent assigned

**Diagnosis**:

```bash
# Check task details
cis task show TASK-001

# Check routing rules
cis team get-config development-team --key routing

# Check agent capabilities
cis agent capabilities opencode
```

**Solutions**:

```bash
# Manual assignment
cis task assign TASK-001 --agent claude

# Update routing rules
cis team configure routing development-team <<EOF
rules:
  - match: "type:*"
    assign_to: "any_available"
EOF

# Add missing capabilities
cis agent add-capability opencode planning
```

### Issue: Team Communication Failure

**Symptoms**: Agents can't exchange messages

**Diagnosis**:

```bash
# Check network status
cis p2p status

# Check Matrix connectivity
cis matrix status

# Test communication
cis team broadcast development-team --content "Test message"
```

**Solutions**:

```bash
# Restart P2P network
cis p2p restart

# Reconnect to Matrix
cis matrix reconnect

# Use alternative communication
cis team configure communication development-team --mode direct
```

### Issue: Load Imbalance

**Symptoms**: One agent overloaded, others idle

**Diagnosis**:

```bash
# Check agent load
cis team load development-team

# View task distribution
cis team tasks development-team --json | jq 'group_by(.assigned_agent) | map({agent: .[0].assigned_agent, count: length})'
```

**Solutions**:

```bash
# Rebalance tasks
cis team rebalance development-team

# Update load balancing strategy
cis team configure load-balancing development-team --strategy least-busy

# Scale team (add more members)
cis team add-member development-team --name coder2 --agent opencode
```

### Issue: Circular Dependencies

**Symptoms**: Tasks waiting on each other

**Diagnosis**:

```bash
# Visualize dependencies
cis task list --json | jq '[.[] | {id, dependencies}]'

# Detect cycles
sqlite3 ~/.cis/data/tasks.db <<EOF
WITH RECURSIVE dep_chain(task_id, path) AS (
  SELECT task_id, task_id FROM tasks
  UNION ALL
  SELECT t.task_id, dc.path || ' -> ' || t.task_id
  FROM tasks t
  JOIN dep_chain dc ON t.task_id IN (
    SELECT value FROM json_each(dc.dependencies)
  )
  WHERE dc.path NOT LIKE '%' || t.task_id || '%'
)
SELECT * FROM dep_chain WHERE path LIKE '% -> %';
EOF
```

**Solutions**:

```bash
# Remove circular dependency
cis task update TASK-002 --dependencies ""

# Reorganize tasks
cis task reorganize --break-cycles

# Manual intervention
cis task delete TASK-003
```

---

## Best Practices

### 1. Team Composition

```bash
# ✅ GOOD: Balanced team with complementary skills
planner (claude) + coder (opencode) + reviewer (kimi)

# ❌ BAD: All agents with same capabilities
coder1 (opencode) + coder2 (opencode) + coder3 (opencode)

# ✅ GOOD: Role-based specialization
planner + coder + tester + reviewer + documenter

# ❌ BAD: Too many specialists (coordination overhead)
10+ specialized agents for simple tasks
```

### 2. Task Granularity

```bash
# ✅ RIGHT SIZE: Tasks take 1-4 hours
cis task create TASK-001 "Implement user registration" \
  --team dev-team

# ❌ TOO BIG: Tasks take days
cis task create TASK-002 "Build entire application" \
  --team dev-team

# ❌ TOO SMALL: Tasks take minutes
cis task create TASK-003 "Add semicolon" \
  --team dev-team

# Decompose large tasks into sub-tasks
# Combine related small tasks
```

### 3. Communication Minimization

```bash
# ❌ BAD: Excessive messaging
for i in {1..100}; do
  cis agent send-message planner coder --content "Update $i"
done

# ✅ GOOD: Batched updates
cis agent send-message planner coder --content "$(cat updates.txt)"
```

### 4. Error Handling

```bash
# Configure automatic retry
cis team configure error-handling development-team <<EOF
retry_policy:
  max_attempts: 3
  backoff: exponential
  on_failure: notify_leader

escalation:
  - after: 2 attempts
    notify: team_leader
  - after: 3 attempts
    escalate_to: human
EOF
```

### 5. Monitoring and Alerting

```bash
# Set up alerts
cis team configure alerts development-team <<EOF
alerts:
  - condition: "task_failure_rate > 0.1"
    action: notify_admin
  - condition: "agent_load > 0.9"
    action: scale_team
  - condition: "task_queue_size > 50"
    action: add_agents
EOF
```

### 6. Documentation

```bash
# Document team responsibilities
cat > ~/.cis/config/teams/development-team.md <<EOF
# Development Team

**Purpose**: Implement and review features

**Members**:
- planner (claude): Task decomposition, design
- coder (opencode): Implementation, unit tests
- reviewer (kimi): Code review, quality assurance

**Workflow**:
1. planner receives feature request
2. planner decomposes into tasks
3. coder implements tasks
4. reviewer validates implementation
5. planner aggregates results

**Escalation**: Contact team-lead@company.com for blockers
EOF
```

### 7. Regular Maintenance

```bash
# Weekly team review
alias cis-team-review='cis team metrics development-team && \
  cis team agents development-team && \
  cis team rebalance development-team'

# Monthly team optimization
cis team optimize development-team \
  --remove-idle-members \
  --rebalance-capabilities
```

---

## Reference

### Team Commands

```bash
cis team create <name> [options]
cis team delete <name>
cis team list
cis team status <name>
cis team agents <name>
cis team tasks <name> [options]
cis team configure <name> <key> <value>
cis team add-member <team> [options]
cis team remove-member <team> <member>
cis team set-leader <team> <member>
cis team broadcast <team> [options]
cis team metrics <team>
cis team logs <team> [options]
cis team rebalance <team>
```

### Team Configuration

```toml
[team]
name = "development-team"
id = "team-dev-001"
max_concurrent_tasks = 3
coordination_mode = "hierarchical"
leader = "planner"

[team.routing]
strategy = "capability"
rules = [...]

[team.communication]
mode = "matrix"
max_message_size = "1MB"
timeout = 300

[team.monitoring]
enable_metrics = true
log_level = "info"
alert_threshold = 0.9
```

### Agent Roles

| Role | Capabilities | Examples |
|------|-------------|----------|
| Planner | analysis, design, decomposition | Claude, GPT-4 |
| Executor | coding, testing, implementation | OpenCode, Kimi |
| Reviewer | validation, verification, QA | Any agent |
| Monitor | tracking, alerting, reporting | Custom agents |
| Orchestrator | coordination, aggregation | Claude |

---

## Additional Resources

- [Task Management Guide](./task-management-guide.md)
- [CIS CLI Reference](./cli-reference.md)
- [CIS Architecture](../ARCHITECTURE.md)
- [Agent Teams Analysis](../releases/v1.1.3/AGENT_TEAMS_INTEGRATION_ANALYSIS.md)

---

**Last Updated**: 2026-02-13
**For questions or issues**, visit [CIS GitHub](https://github.com/your-org/cis)
