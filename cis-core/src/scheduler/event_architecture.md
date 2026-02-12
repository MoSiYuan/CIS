# Event-Driven Architecture for CIS Scheduler

> **Version**: 1.0
> **Created**: 2026-02-12
> **Team**: Team E (Event-Driven Scheduler)

---

## Overview

This document describes the event-driven architecture designed to replace the polling-based scheduling mechanism in CIS v1.1.6. The event-driven approach reduces CPU usage by 30%+ and decreases DAG scheduling latency from 500ms+ to <100ms.

---

## Motivation

### Current Problems (Polling-Based)

The existing implementation in `multi_agent_executor.rs` uses hardcoded polling:

```rust
// Lines 256-274 in multi_agent_executor.rs
if ready_tasks.is_empty() {
    // Check if completed
    let is_completed = { /* ... */ };

    if is_completed {
        break;
    }

    // ❌ PROBLEM: Wastes CPU cycles
    tokio::time::sleep(Duration::from_millis(100)).await;
    continue;
}

// ❌ PROBLEM: Another polling point
if available_slots == 0 {
    tokio::time::sleep(Duration::from_millis(100)).await;
    continue;
}
```

**Issues**:
1. **High Latency**: 100ms polling interval = average 50ms response delay
2. **CPU Waste**: Continuous wake-ups even when no work exists
3. **Poor Scalability**: More DAG runs = more polling loops
4. **Energy Inefficiency**: Prevents CPU power saving

### Event-Driven Solution

Replace polling with reactive notifications:
- Tasks notify when ready → Immediate wake-up
- Task completion notification → No polling needed
- Agent availability notification → Efficient scheduling

---

## Architecture Design

### Core Components

```
┌─────────────────────────────────────────────────────────────────┐
│                    Event-Driven Scheduler                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────┐         ┌──────────────────┐             │
│  │  ReadyNotify    │         │ CompletionNotify  │             │
│  │  (tokio::Notify)│         │  (broadcast)      │             │
│  └────────┬────────┘         └────────┬─────────┘             │
│           │                           │                         │
│           │ notify_ready()            │ task_completed()        │
│           ▼                           ▼                         │
│  ┌──────────────────────────────────────────────────┐           │
│  │        EventDrivenScheduler (select! loop)       │           │
│  └──────────────────────┬───────────────────────────┘           │
│                         │                                       │
│           ┌─────────────┼─────────────┐                         │
│           ▼             ▼             ▼                         │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐               │
│  │EventHandler │ │EventHandler │ │EventHandler │               │
│  │  ReadyTask  │ │ Completion  │ │   Error     │               │
│  └─────────────┘ └─────────────┘ └─────────────┘               │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Event Types

```rust
/// Scheduler event types
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    /// Task is ready to execute (dependencies satisfied)
    TaskReady {
        run_id: String,
        task_id: String,
    },

    /// Task completed execution
    TaskCompleted {
        run_id: String,
        task_id: String,
        result: TaskExecutionResult,
    },

    /// Task failed
    TaskFailed {
        run_id: String,
        task_id: String,
        error: String,
    },

    /// Agent became available
    AgentAvailable {
        agent_id: String,
        runtime_type: RuntimeType,
    },

    /// DAG run created
    DagCreated {
        run_id: String,
    },

    /// DAG run completed
    DagCompleted {
        run_id: String,
        status: DagRunStatus,
    },
}
```

---

## Notification Mechanisms

### 1. ReadyNotify (tokio::sync::Notify)

**Purpose**: Wake up scheduler when tasks become ready

```rust
use tokio::sync::Notify;

pub struct ReadyNotify {
    notify: Arc<Notify>,
}

impl ReadyNotify {
    /// Called by DAG when task dependencies are satisfied
    pub fn notify_ready(&self) {
        self.notify.notify_one(); // Wake up one waiter
        self.notify.notify_waiters(); // Or wake up all
    }

    /// Wait for next ready task
    pub async fn wait_for_ready(&self) {
        self.notify.notified().await;
    }
}
```

**Usage**:
```rust
// In task completion handler
fn on_task_complete(&self, task_id: String) {
    // Update DAG
    self.dag.mark_completed(task_id);

    // Check which tasks become ready
    let newly_ready = self.dag.get_newly_ready_tasks();

    if !newly_ready.is_empty() {
        // Notify scheduler immediately
        self.ready_notify.notify_ready();
    }
}
```

### 2. CompletionNotifier (tokio::sync::broadcast)

**Purpose**: Broadcast task completion to multiple listeners

```rust
use tokio::sync::broadcast;

#[derive(Clone, Debug)]
pub struct TaskCompletion {
    pub run_id: String,
    pub task_id: String,
    pub success: bool,
    pub output: String,
}

pub struct CompletionNotifier {
    tx: broadcast::Sender<TaskCompletion>,
}

impl CompletionNotifier {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1000); // Channel size
        Self { tx }
    }

    /// Send completion notification
    pub fn notify_completion(&self, completion: TaskCompletion) -> Result<()> {
        self.tx.send(completion)?;
        Ok(())
    }

    /// Subscribe to completions
    pub fn subscribe(&self) -> broadcast::Receiver<TaskCompletion> {
        self.tx.subscribe()
    }
}
```

**Usage**:
```rust
// In task executor
let result = execute_task(task).await?;

// Notify completion
completion_notifier.notify_completion(TaskCompletion {
    run_id: run_id.clone(),
    task_id: task_id.clone(),
    success: result.success,
    output: result.output,
})?;
```

### 3. ErrorNotifier (tokio::sync::broadcast)

**Purpose**: Broadcast errors to error handlers

```rust
#[derive(Clone, Debug)]
pub struct TaskError {
    pub run_id: String,
    pub task_id: String,
    pub error: String,
    pub severity: ErrorSeverity,
}

pub enum ErrorSeverity {
    Warning,   // Continue execution
    Error,     // May affect dependent tasks
    Critical,  // Abort entire DAG
}
```

---

## Event-Driven Scheduler

### Main Event Loop

```rust
use tokio::select;

pub struct EventDrivenScheduler {
    ready_notify: Arc<ReadyNotify>,
    completion_notifier: CompletionNotifier,
    error_notifier: ErrorNotifier,
    // ... other fields
}

impl EventDrivenScheduler {
    pub async fn run(&self) -> Result<()> {
        let mut completion_rx = self.completion_notifier.subscribe();
        let mut error_rx = self.error_notifier.subscribe();

        loop {
            select! {
                // Event 1: Task ready notification
                _ = self.ready_notify.wait_for_ready() => {
                    self.handle_ready_tasks().await?;
                }

                // Event 2: Task completion
                result = completion_rx.recv() => {
                    match result {
                        Ok(completion) => {
                            self.handle_completion(completion).await?;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Completion channel lagged, missed {} messages", n);
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break; // Channel closed
                        }
                    }
                }

                // Event 3: Error notification
                result = error_rx.recv() => {
                    if let Ok(error) = result {
                        self.handle_error(error).await?;
                    }
                }

                // Event 4: Periodic health check
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    self.health_check().await?;
                }
            }
        }

        Ok(())
    }
}
```

### Event Handlers

```rust
impl EventDrivenScheduler {
    /// Handle task ready event
    async fn handle_ready_tasks(&self) -> Result<()> {
        // Get all ready tasks (non-blocking)
        let ready_tasks = self.get_all_ready_tasks().await?;

        info!("Processing {} ready tasks", ready_tasks.len());

        for (run_id, task_id) in ready_tasks {
            self.schedule_task(run_id, task_id).await?;
        }

        Ok(())
    }

    /// Handle task completion event
    async fn handle_completion(&self, completion: TaskCompletion) -> Result<()> {
        info!(
            "Task {} in run {} completed: {}",
            completion.task_id,
            completion.run_id,
            completion.success
        );

        // Update DAG state
        self.update_task_state(
            &completion.run_id,
            &completion.task_id,
            completion.success,
        ).await?;

        // Check if dependent tasks become ready
        let newly_ready = self.check_dependent_tasks(
            &completion.run_id,
            &completion.task_id,
        ).await?;

        // If new tasks ready, they will auto-notify via ReadyNotify

        // Check if DAG completed
        if self.is_dag_completed(&completion.run_id).await? {
            self.finalize_dag(&completion.run_id).await?;
        }

        Ok(())
    }

    /// Handle error event
    async fn handle_error(&self, error: TaskError) -> Result<()> {
        match error.severity {
            ErrorSeverity::Warning => {
                warn!("Warning in task {}: {}", error.task_id, error.error);
            }
            ErrorSeverity::Error => {
                // Mark task as failed, continue DAG
                self.mark_task_failed(&error.run_id, &error.task_id, &error.error).await?;
            }
            ErrorSeverity::Critical => {
                // Abort entire DAG
                self.abort_dag(&error.run_id, &error.error).await?;
            }
        }
        Ok(())
    }
}
```

---

## Integration with Existing Code

### Backward Compatibility

Keep the old polling implementation as fallback:

```rust
pub enum SchedulingMode {
    EventDriven,
    Polling,  // Fallback for compatibility
}

pub struct MultiAgentDagExecutor {
    mode: SchedulingMode,
    event_scheduler: Option<EventDrivenScheduler>,
    // ... existing fields
}

impl MultiAgentDagExecutor {
    pub async fn execute(&self, run_id: &str) -> Result<MultiAgentExecutionReport> {
        match self.mode {
            SchedulingMode::EventDriven => {
                self.execute_event_driven(run_id).await
            }
            SchedulingMode::Polling => {
                self.execute_polling(run_id).await  // Old implementation
            }
        }
    }
}
```

### Migration Strategy

**Phase 1**: Implement event-driven alongside polling (feature flag)
**Phase 2**: Test event-driven with feature flag
**Phase 3**: Make event-driven default
**Phase 4**: Remove polling code (future version)

---

## Performance Targets

| Metric | Current (Polling) | Target (Event-Driven) | Improvement |
|--------|-------------------|----------------------|-------------|
| DAG Scheduling Latency | 500ms+ | <100ms | 5x faster |
| CPU Usage (Idle) | 5-10% | <1% | 10x reduction |
| CPU Usage (Load) | 30-40% | 20-30% | 30% reduction |
| Task Dispatch Delay | 50ms avg | <1ms | 50x faster |

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ready_notify() {
        let notify = ReadyNotify::new();

        // Spawn waiter
        let handle = tokio::spawn({
            let notify = notify.clone();
            async move {
                notify.wait_for_ready().await;
            }
        });

        // Notify
        notify.notify_ready();

        // Should complete immediately
        assert!(tokio::time::timeout(
            Duration::from_millis(10),
            handle
        ).await.is_ok());
    }

    #[tokio::test]
    async fn test_completion_notifier() {
        let notifier = CompletionNotifier::new();
        let mut rx = notifier.subscribe();

        let completion = TaskCompletion {
            run_id: "test-run".to_string(),
            task_id: "task-1".to_string(),
            success: true,
            output: "done".to_string(),
        };

        // Send
        notifier.notify_completion(completion.clone()).unwrap();

        // Receive
        let received = tokio::time::timeout(
            Duration::from_millis(100),
            rx.recv()
        ).await.unwrap().unwrap();

        assert_eq!(received.task_id, completion.task_id);
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_event_driven_execution() {
    // Create DAG
    let dag = create_test_dag();

    // Create event-driven executor
    let executor = EventDrivenExecutor::new();

    // Execute
    let report = executor.execute(dag).await.unwrap();

    // Verify
    assert_eq!(report.completed, 3);
    assert_eq!(report.failed, 0);
}

#[tokio::test]
async fn test_latency() {
    let executor = EventDrivenExecutor::new();

    let start = std::time::Instant::now();
    executor.execute(create_test_dag()).await.unwrap();
    let latency = start.elapsed();

    assert!(latency < Duration::from_millis(100));
}
```

### Benchmark Tests

See `benches/scheduler_comparison.rs` (to be created in P1-1.5)

---

## Implementation Files

| File | Purpose |
|------|---------|
| `notify.rs` | Notification primitives (ReadyNotify, CompletionNotifier, ErrorNotifier) |
| `event_driven.rs` | Event-driven scheduler implementation |
| `event_handler.rs` | Event processing logic |
| `multi_agent_executor.rs` | Updated to support both modes |

---

## Next Steps

1. ✅ **P1-1.1**: Design event architecture (THIS FILE)
2. ⏳ **P1-1.2**: Implement `notify.rs`
3. ⏳ **P1-1.3**: Implement `event_driven.rs`
4. ⏳ **P1-1.4**: Update `multi_agent_executor.rs`
5. ⏳ **P1-1.5**: Benchmark comparison

---

## References

- tokio::sync::Notify: https://docs.rs/tokio/latest/tokio/sync/struct.Notify.html
- tokio::sync::broadcast: https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html
- tokio::select!: https://docs.rs/tokio/latest/tokio/macro.select.html
- Code Review: `docs/user/code-review-execution-layer.md`
- Solution: `docs/plan/v1.1.6/SOLUTION.md`

---

**Document Owner**: Team E
**Status**: Ready for Implementation
**Last Updated**: 2026-02-12
