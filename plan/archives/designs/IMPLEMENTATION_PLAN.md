# CIS DAG Agent Cluster - Implementation Plan

## Current Status Overview

| Phase | Status | Progress | Files |
|-------|--------|----------|-------|
| Phase 1: Core Infrastructure | ✅ Complete | 100% | 7 files, ~3400 lines |
| Phase 2: Matrix Integration | 🔄 Ready | 0% | Design complete |
| Phase 3: Hybrid Mode | ⏳ Planned | 0% | Architecture defined |
| Phase 4: Federation | ⏳ Future | 0% | Conceptual |

---

## Phase 1: Core Infrastructure ✅ COMPLETE

### Deliverables
- [x] `agent/cluster/mod.rs` - Module exports, SessionId
- [x] `agent/cluster/events.rs` - SessionEvent, EventBroadcaster
- [x] `agent/cluster/manager.rs` - SessionManager (global singleton)
- [x] `agent/cluster/session.rs` - AgentSession with PTY
- [x] `agent/cluster/context.rs` - ContextStore (SQLite)
- [x] `agent/cluster/executor.rs` - AgentClusterExecutor
- [x] `agent/cluster/monitor.rs` - SessionMonitor (blockage detection)
- [x] CLI integration - `cis dag sessions/attach/logs/kill/unblock`

### Verification
```bash
# Build check
$ cargo build -p cis-core
$ cargo build -p cis-node

# CLI commands available
$ cis dag --help
$ cis dag sessions --help
$ cis dag attach --help
```

---

## Phase 2: Matrix Integration 🔄 READY (Est: 2-3 days)

### Goal
Add Matrix Room broadcasting capability to DAG execution

### Tasks

#### 2.1 Matrix Event Types (0.5 day)
**File**: `cis-core/src/matrix/events/event_types.rs`

```rust
// Add to MatrixEventType enum
CisDagRunCreated,
CisDagRunStarted,
CisDagRunCompleted,
CisDagRunFailed,
CisDagTaskStatus,
CisDagSessionBlocked,
CisDagSessionRecovered,
```

**Acceptance Criteria**:
- [ ] All new event types serialize/deserialize correctly
- [ ] Event categories properly defined
- [ ] Unit tests pass

#### 2.2 Matrix Broadcaster (1 day)
**File**: `cis-core/src/agent/cluster/matrix_broadcast.rs` (NEW)

```rust
pub struct DagMatrixBroadcaster {
    nucleus: Arc<MatrixNucleus>,
    room_id: RoomId,
}

impl DagMatrixBroadcaster {
    pub async fn broadcast_run_created(&self, run: &DagRun) -> Result<()>;
    pub async fn broadcast_task_status(&self, ...) -> Result<()>;
    pub async fn broadcast_session_blocked(&self, ...) -> Result<()>;
}
```

**Acceptance Criteria**:
- [ ] Can broadcast events to Matrix Room
- [ ] Events are received by Element client
- [ ] Failed broadcasts don't block execution

#### 2.3 SessionManager Integration (0.5 day)
**File**: `cis-core/src/agent/cluster/manager.rs`

```rust
pub struct SessionManager {
    // ... existing fields ...
    matrix_broadcaster: Option<DagMatrixBroadcaster>,
}

// Add methods
impl SessionManager {
    pub fn enable_matrix_broadcast(&mut self, broadcaster: DagMatrixBroadcaster);
}
```

**Acceptance Criteria**:
- [ ] SessionManager optionally broadcasts to Matrix
- [ ] Local execution unchanged when Matrix disabled
- [ ] Events sync to Matrix Room when enabled

#### 2.4 CLI Matrix Commands (0.5 day)
**File**: `cis-node/src/commands/matrix.rs` (NEW)

```rust
pub enum MatrixCommands {
    /// Subscribe to DAG status room
    Subscribe { room_id: String },
    /// List available DAG rooms
    ListRooms,
    /// Create DAG status room
    CreateRoom { name: String },
}
```

**Acceptance Criteria**:
- [ ] `cis matrix subscribe !dag-status:cis.local` works
- [ ] Real-time status updates in terminal
- [ ] Graceful exit on Ctrl+C

**Dependencies**: Phase 1 complete
**Assignee**: TBD
**Due**: Day 3

---

## Phase 3: Hybrid Mode ⏳ PLANNED (Est: 3-4 days)

### Goal
Implement unified executor with local execution + Matrix broadcasting

### Tasks

#### 3.1 UnifiedDagExecutor (1 day)
**File**: `cis-core/src/scheduler/unified_executor.rs` (NEW)

```rust
pub struct UnifiedDagExecutor {
    mode: ExecutionMode,
    local: LocalExecutor,
    matrix_broadcaster: Option<DagMatrixBroadcaster>,
}

pub enum ExecutionMode {
    Local,
    Hybrid { room_id: RoomId },
    Matrix { room_id: RoomId },
}

impl UnifiedDagExecutor {
    pub async fn execute_run(&self, run: &mut DagRun) -> Result<ExecutionReport> {
        match self.mode {
            ExecutionMode::Local => self.local.execute_run(run).await,
            ExecutionMode::Hybrid { .. } => {
                // Local execution + Matrix broadcast
                let result = self.local.execute_run(run).await?;
                self.broadcast_completion(&result).await?;
                Ok(result)
            }
            ExecutionMode::Matrix { .. } => {
                // Pure event-driven (Phase 4)
                todo!()
            }
        }
    }
}
```

**Acceptance Criteria**:
- [ ] Supports Local, Hybrid, Matrix modes
- [ ] Mode switching at runtime
- [ ] Backward compatible with existing code

#### 3.2 CLI Integration (0.5 day)
**File**: `cis-node/src/commands/dag.rs`

Update existing execute command:
```rust
DagCommands::Execute {
    run_id: Option<String>,
    mode: ExecutionMode,  // NEW
    room_id: Option<String>,  // NEW
}
```

**Acceptance Criteria**:
- [ ] `cis dag execute --local` works (default)
- [ ] `cis dag execute --hybrid --room !dag:cis.local` works
- [ ] `cis dag execute --matrix --room !dag:cis.local` works

#### 3.3 Configuration (0.5 day)
**File**: `cis-core/src/config.rs`

```toml
[dag]
default_mode = "hybrid"

[dag.matrix]
enabled = true
auto_create_room = true
broadcast_interval_ms = 500
```

**Acceptance Criteria**:
- [ ] Config file drives default mode
- [ ] Environment variable override
- [ ] CLI flags override config

#### 3.4 GUI/Web Dashboard (1-2 days)
**File**: `cis-gui/src/dag_dashboard.rs` (NEW)

```rust
pub struct DagDashboard {
    room_subscriber: MatrixRoomSubscriber,
    active_runs: Vec<DagRunStatus>,
}

impl DagDashboard {
    pub fn ui(&mut self, ctx: &egui::Context) {
        // Show real-time DAG status
        // Subscribe to Matrix Room events
    }
}
```

**Acceptance Criteria**:
- [ ] GUI shows active DAG runs
- [ ] Real-time task status updates
- [ ] Click to view session output

**Dependencies**: Phase 2 complete
**Assignee**: TBD
**Due**: Day 7

---

## Phase 4: Federation (FUTURE) (Est: 5-7 days)

### Goal
Cross-node distributed DAG execution with task distribution

### Tasks

#### 4.1 Matrix Task Assignment
**File**: `cis-core/src/agent/cluster/matrix_executor.rs` (NEW)

```rust
pub struct MatrixDagExecutor {
    room_id: RoomId,
    task_queue: mpsc::Receiver<MatrixDagTask>,
    is_coordinator: bool,
}

impl MatrixDagExecutor {
    pub async fn listen_and_execute(&self) -> Result<()> {
        // Join room
        // Listen for cis.dag.task.assigned events
        // Claim tasks assigned to this node
        // Execute and broadcast results
    }
    
    pub async fn assign_task(&self, task: DagTask, target_node: &str) -> Result<()> {
        // Coordinator broadcasts task assignment
    }
}
```

#### 4.2 Task Distribution Algorithm
**File**: `cis-core/src/scheduler/task_scheduler.rs` (NEW)

```rust
pub trait TaskScheduler {
    fn select_node(&self, task: &DagTask, available_nodes: &[NodeInfo]) -> Option<String>;
}

pub struct LoadBalancingScheduler;
impl TaskScheduler for LoadBalancingScheduler {
    fn select_node(&self, task: &DagTask, nodes: &[NodeInfo]) -> Option<String> {
        // Select node with lowest load
        // Consider agent type availability
        // Consider data locality
    }
}
```

#### 4.3 Fault Tolerance
- Node failure detection (heartbeat timeout)
- Task retry on other nodes
- State recovery from Matrix Room history

#### 4.4 Security
- DID-based node authentication
- Task authorization (which node can execute what)
- Encrypted room for sensitive DAGs

**Dependencies**: Phase 3 complete
**Priority**: Low (advanced feature)

---

## Quick Start Guide

### Current (Phase 1)
```bash
# Build and run
$ cargo build --release
$ ./target/release/cis dag run example-dag.toml
$ ./target/release/cis dag execute --run-id <id>
```

### After Phase 2
```bash
# Enable Matrix broadcasting
$ cis dag execute --use-agent --room !dag-status:cis.local dag.toml

# Subscribe from another terminal
$ cis matrix subscribe !dag-status:cis.local
```

### After Phase 3
```bash
# Hybrid mode (recommended)
$ cis dag execute --hybrid --room !dag-status:cis.local dag.toml

# Or set in config
echo '[dag]
default_mode = "hybrid"
room_id = "!dag-status:cis.local"' >> ~/.cis/config.toml
```

---

## Resource Allocation

| Phase | Developer Days | Review Days | Testing Days | Total |
|-------|---------------|-------------|--------------|-------|
| Phase 1 | 5 | 1 | 2 | 8 ✅ |
| Phase 2 | 2 | 0.5 | 1 | 3.5 🔄 |
| Phase 3 | 3 | 1 | 1.5 | 5.5 ⏳ |
| Phase 4 | 5 | 2 | 3 | 10 ⏳ |
| **Total** | **15** | **4.5** | **7.5** | **27** |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Matrix connection instability | Medium | Medium | Local queue + retry |
| Event storm (too many broadcasts) | Low | High | Batch + sampling |
| Backward compatibility break | Low | High | Comprehensive tests |
| Performance regression | Medium | Medium | Benchmarks + profiling |

---

## Next Actions (Immediate)

1. **Review Phase 1 code** (1 hour)
   - Code review of agent/cluster module
   - Identify refactoring needs

2. **Setup Matrix test environment** (2 hours)
   - Start local Matrix homeserver (Synapse/Dendrite)
   - Create test rooms
   - Verify event broadcasting

3. **Begin Phase 2.1** (0.5 day)
   - Add MatrixEventType variants
   - Write unit tests

**Ready to start Phase 2?** (y/n)
