# Persistent AgentSession Usage Example

This example demonstrates how to use the new persistence mode features of `AgentSession`.

## Basic Usage

```rust
use cis_core::agent::cluster::{AgentSession, SessionId};
use cis_core::agent::cluster::events::EventBroadcaster;
use cis_core::agent::AgentType;
use std::path::PathBuf;

// Create a persistent session
let event_broadcaster = EventBroadcaster::new(100);
let mut session = AgentSession::new(
    SessionId::new("run-1", "task-1"),
    AgentType::OpenCode,
    PathBuf::from("/workspace"),
    "Initial prompt".to_string(),
    "".to_string(),
    event_broadcaster,
    10000,
);

// Enable persistent mode
session.set_persistent(true);
session.set_max_idle_secs(3600); // Auto-destroy after 1 hour of idle

// Start the session
session.start(80, 24).await.unwrap();
```

## Sending Commands and Waiting for Output

```rust
// Send a command to the persistent session
session.send_input_async("ls -la").await.unwrap();

// Wait for specific output pattern
let output = session.wait_for_output("total", Duration::from_secs(10)).await.unwrap();
println!("Output: {}", output);

// Or get current output at any time
let current_output = session.get_output_content().await;
println!("Current output: {}", current_output);
```

## Task Completion and Idle State

```rust
// When task completes successfully, persistent session goes to Idle state
session.mark_completed("Task finished", 0).await;

// Check if session can accept new tasks
if session.can_accept_task().await {
    // Send new task
    session.send_input_async("next command").await.unwrap();
}
```

## Pause and Resume

```rust
// Pause the session
session.mark_paused().await;

// ... do something else ...

// Resume the session
session.resume().await.unwrap();
```

## Auto-Destroy Check

```rust
// Check if session should be auto-destroyed
if session.should_auto_destroy().await {
    session.shutdown("Idle timeout").await.unwrap();
}
```

## Session State Helpers

```rust
// Check session state
if session.is_active().await {
    println!("Session is active and can be reused");
}

if session.is_terminal().await {
    println!("Session is in terminal state");
}

if session.can_accept_task().await {
    println!("Session can accept new tasks");
}
```

## Non-Persistent Mode (Default)

```rust
// Create a non-persistent session (default behavior)
let mut session = AgentSession::new(
    SessionId::new("run-1", "task-1"),
    AgentType::OpenCode,
    PathBuf::from("/workspace"),
    "prompt".to_string(),
    "".to_string(),
    event_broadcaster,
    10000,
);
// persistent is false by default

// When completed, should_auto_destroy returns true
session.mark_completed("done", 0).await;
assert!(session.should_auto_destroy().await); // true for non-persistent
```

## New SessionState Variants

The `SessionState` enum now includes:

- `Idle` - Session is idle and waiting for new tasks (persistent mode)
- `Paused` - Session is paused and waiting to be resumed
- `Killed` - Session has been killed

State helper methods:
- `is_active()` - Returns true for `Idle` and `RunningDetached`
- `can_accept_task()` - Returns true only for `Idle`
- `is_terminal()` - Returns true for `Completed`, `Failed`, and `Killed`
