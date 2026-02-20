# Migration Guide: v1.1.x to v1.2.0

This guide covers migrating from CIS v1.1.x to v1.2.0.

## Overview

v1.2.0 introduces major architectural changes:

1. **Module Refactoring**: Core functionality split into cis-common workspace crates
2. **ZeroClaw Compatibility**: New adapter layer for ZeroClaw integration
3. **Builder Pattern**: New Runtime builder API

## Breaking Changes

### 1. Module Path Changes

```rust
// v1.1.x
use cis_core::types::TaskLevel;
use cis_core::storage::StorageService;

// v1.2.0 (recommended)
use cis_types::TaskLevel;
use cis_storage::StorageService;

// v1.2.0 (backward compatible - will show deprecation warning)
use cis_core::types::TaskLevel;
use cis_core::storage::StorageService;
```

### 2. Runtime Initialization

```rust
// v1.1.x
let core = CISCore::new(config).await?;

// v1.2.0
let runtime = Runtime::builder()
    .with_storage(RocksDbStorage::new("./data")?)
    .with_memory(CISMemory::new(...))
    .with_scheduler(CISScheduler::new(...))
    .build()?;
```

### 3. Memory Operations

```rust
// v1.1.x
use cis_core::memory::MemoryService;
let entries = memory.get("key").await?;

// v1.2.0
use cis_memory::MemoryService;
let entries = memory.get("key").await?;
```

### 4. Scheduler Usage

```rust
// v1.1.x
use cis_core::scheduler::DagScheduler;
let scheduler = DagScheduler::new();

// v1.2.0
use cis_scheduler::DagScheduler;
let scheduler = DagScheduler::new();
```

## Backward Compatibility

v1.2.0 provides re-exports for backward compatibility:

```rust
// This still works but shows deprecation warning
use cis_core::types::{Task, TaskLevel, TaskStatus};

// Recommended: use the new paths
use cis_types::{Task, TaskLevel, TaskStatus};
```

## ZeroClaw Integration

To use CIS as ZeroClaw backend:

```rust
use cis_core::Runtime;
use cis_core::zeroclaw::{ZeroclawMemoryAdapter, ZeroclawSchedulerAdapter};

let runtime = Runtime::builder()
    .with_storage(RocksDbStorage::new("./data")?)
    .with_memory(CISMemory::new(...))
    .build()?;

let memory_adapter = ZeroclawMemoryAdapter::new(runtime.memory().clone());
let scheduler_adapter = ZeroclawSchedulerAdapter::new(runtime.scheduler().clone());
```

## Feature Flags

```toml
# v1.1.x
cis-core = { version = "1.1.5", features = ["encryption", "vector", "p2p"] }

# v1.2.0
cis-core = { version = "1.2.0", features = ["encryption", "vector", "p2p"] }

# New: Use individual cis-common crates directly
cis-types = "1.2.0"
cis-storage = "1.2.0"
cis-memory = "1.2.0"
```

## Migration Steps

1. **Update Cargo.toml**
   - Change version to 1.2.0
   - Add new dependencies if needed

2. **Update Import Statements**
   - Replace `cis_core::` imports with specific crate imports
   - Or keep existing imports (backward compatible)

3. **Update Runtime Initialization**
   - If using Runtime builder pattern, update configuration

4. **Test Verification**
   - Run existing tests to ensure compatibility
   - Check for deprecation warnings

5. **Update ZeroClaw Integration** (optional)
   - If using ZeroClaw, update to use new adapter layer

## Deprecation Timeline

- **v1.2.0**: Backward compatible, deprecation warnings
- **v1.3.0**: Re-exports may be removed
- **v2.0.0**: Old imports removed

## Support

For issues or questions:
- GitHub Issues: https://github.com/cis-projects/cis/issues
- Discord: https://discord.gg/cis
