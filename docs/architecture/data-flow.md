# CIS Data Flow Architecture v1.1.6

> **Version**: 1.1.6
> **Last Updated**: 2026-02-13
> **Target Audience**: Developers, System Architects, DevOps Engineers

---

## Table of Contents

1. [Data Flow Overview](#data-flow-overview)
2. [Memory Operations Flow](#memory-operations-flow)
3. [Task Execution Flow](#task-execution-flow)
4. [Skill Invocation Flow](#skill-invocation-flow)
5. [Agent Integration Flow](#agent-integration-flow)
6. [P2P Synchronization Flow](#p2p-synchronization-flow)
7. [Project Session Flow](#project-session-flow)
8. [Matrix Federation Flow](#matrix-federation-flow)
9. [Error Handling Flow](#error-handling-flow)
10. [Performance Optimization Patterns](#performance-optimization-patterns)

---

## Data Flow Overview

### System-Wide Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                      Entry Points                            │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐│
│  │   CLI    │  │   GUI    │  │   API    │  │  Skill   ││
│  └─────┬────┘  └─────┬────┘  └─────┬────┘  └─────┬────┘│
└────────┼───────────────┼───────────────┼───────────────┼────┘
         │               │               │               │
         ▼               ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Service Layer                             │
│  ┌────────────┐ ┌────────────┐ ┌────────────┐           │
│  │  Memory    │ │   Task     │ │  Skill     │           │
│  │ Service    │ │  Service   │ │  Service   │           │
│  └──────┬─────┘ └──────┬─────┘ └──────┬─────┘           │
└─────────┼────────────────┼─────────────────┼──────────────────┘
          │                │                 │
          ▼                ▼                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                   Core Engine Layer                          │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐         │
│  │  Memory    │  │ Scheduler  │  │  Skill     │         │
│  │   V2      │  │   DAG      │  │  Router    │         │
│  └──────┬─────┘  └──────┬─────┘  └──────┬─────┘         │
└─────────┼─────────────────┼─────────────────┼──────────────────┘
          │                 │                 │
          ▼                 ▼                 ▼
┌─────────────────────────────────────────────────────────────────┐
│                  Storage & Infrastructure                     │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐         │
│  │  SQLite    │  │  WASM      │  │  P2P       │         │
│  │ (Weekly DB)│  │  Runtime   │  │  Network   │         │
│  └────────────┘  └────────────┘  └────────────┘         │
└─────────────────────────────────────────────────────────────────┘
```

### Data Classification

CIS handles several types of data, each with different flow characteristics:

| Data Type | Storage | Sync | Retention | Example |
|-----------|---------|-------|-----------|---------|
| **Private Memory** | Encrypted SQLite | Never | 54 weeks | API keys, personal data |
| **Public Memory** | Clear SQLite | P2P | 54 weeks | Preferences, configs |
| **Task State** | SQLite + Memory | Never | Until completion | DAG execution state |
| **Conversation** | SQLite + Memory | Never | Per session | Chat history |
| **Skill Metadata** | Filesystem | Never | Indefinite | skill.toml |
| **Vector Index** | HNSW (Memory) | Never | Dynamic | Semantic embeddings |
| **Peer Data** | In-memory + DHT | P2P | Session-based | Connected peers |

---

## Memory Operations Flow

### Memory Write Flow

```
User Action: "Remember my API key is abc123"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Service Layer: MemoryService.set()     │
│    - Parse memory key                      │
│    - Determine domain (Private/Public)       │
│    - Determine category                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Classify Entry                       │
│    - Key pattern matching                 │
│    - Content analysis                    │
│    - Access frequency check               │
│    Result: IndexType::Sensitive          │
└─────────────────┬───────────────────────┘
                  │
                  ├──────────────────────────┐
                  ▼                          ▼
┌──────────────────────────┐  ┌──────────────────────────┐
│ 3a. LogMemory.append() │  │ 3b. VectorIndex.check()│
│    - Create LogEntry    │  │    - Check if should    │
│    - Add to buffer     │  │      index             │
│    - Batch write (100) │  │    Result: NO (sensitive)│
└────────────┬───────────┘  └──────────────────────────┘
             │
             ▼
┌─────────────────────────────────────────────┐
│ 4. Write to Weekly DB                    │
│    - memory-2026-W06.db                 │
│    - INSERT INTO log_entries              │
│    - Update access_count = 0             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Check Index Policy                  │
│    - Sensitive data → Skip vectorization │
│    - Only index if allowed_types allows  │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Emit Event                          │
│    - DomainEvent::MemorySet              │
│    - Notify subscribers                 │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 7. Return Confirmation                  │
│    "Memory saved to private domain"      │
└───────────────────────────────────────────┘
```

### Memory Read Flow

```
User Request: "Get my API key"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Service Layer: MemoryService.get()     │
│    - Key: "user/api-key"                 │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Check Cache                         │
│    - LRU cache lookup                    │
│    Result: Cache miss                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Query LogMemory                     │
│    - SELECT * FROM log_entries          │
│      WHERE key = "user/api-key"          │
│    - Exact match query                   │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 4a. Found          │  │ 4b. Not Found    │
│    - Decrypt (if    │  │    - Return None  │
│      private)       │  │                  │
│    - Update cache  │  └──────────────────┘
│    - Increment     │
│      access_count  │
└─────────┬──────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 5. Emit Event                          │
│    - DomainEvent::MemoryAccess           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Return Result                       │
│    Some(MemoryItem {                     │
│      key: "user/api-key",                │
│      value: b"abc123",                   │
│      domain: Private,                    │
│    })                                   │
└───────────────────────────────────────────┘
```

### Memory Search Flow (Hybrid)

```
User Query: "What are my project preferences?"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Service Layer: MemoryService.search() │
│    - Query: "project preferences"         │
│    - Limit: 10                          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. VectorIndex.search()                │
│    - Create query embedding              │
│    - HNSW search (nearest neighbors)    │
│    - Returns: [log_id_1, log_id_2]    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Batch Fetch from LogMemory           │
│    - SELECT * FROM log_entries          │
│      WHERE id IN (log_id_1, log_id_2)    │
│    - Fetch full entries                │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Re-rank by Weight                   │
│    - Calculate relevance score            │
│    - Apply access_count boost           │
│    - Apply recency boost                │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Update Access Statistics             │
│    - Increment access_count for all     │
│      fetched entries                    │
│    - Update last_accessed_at           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Return Sorted Results                │
│    [                                     │
│      MemoryItem { key: "project/theme" }, │
│      MemoryItem { key: "project/lang" },  │
│      ...                                  │
│    ]                                     │
└───────────────────────────────────────────┘
```

### Weekly Archive Flow

```
Sunday 23:59
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. WeekArchiver Trigger                 │
│    - Scheduled task fires                 │
│    - Current week: 2026-W06             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Create New Weekly DB                │
│    - Close memory-2026-W06.db          │
│    - Create memory-2026-W07.db         │
│    - Migrate schema                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Compress Old DB                     │
│    - gzip memory-2026-W06.db            │
│    - Move to archives/                  │
│    - archives/2026-W06.db.gz          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Cleanup Vector Index                │
│    - Remove vectors from archived week   │
│    - Rebuild HNSW index              │
│    - Save to index/hnsw_index.db       │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Apply 54-Week Retention            │
│    - Check archive count                │
│    - If > 54, delete oldest           │
│    - archives/2025-W01.db.gz deleted  │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Report Completion                   │
│    - Archived: memory-2026-W06.db     │
│    - Compressed to: 45KB              │
│    - Index entries removed: 127        │
│    - Archives deleted: 1               │
└───────────────────────────────────────────┘
```

---

## Task Execution Flow

### DAG Loading Flow

```
User Request: "Execute deployment workflow"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Load DAG Definition                 │
│    - From .cis/dags/deploy.toml         │
│    - Parse TOML                        │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Validate DAG                       │
│    - Check for cycles                  │
│    - Validate dependencies              │
│    - Check skill existence             │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 3a. Valid          │  │ 3b. Invalid     │
│    - Build task     │  │    - Return     │
│      graph        │  │      error      │
│    - Topological  │  │    - Show       │
│      sort        │  │      cycle      │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 4. Schedule Tasks                     │
│    - Group by dependency levels         │
│    - Identify parallel tasks           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Begin Execution                    │
│    → [Next Flow: Task Execution]       │
└───────────────────────────────────────────┘
```

### Task Execution Flow (Per Task)

```
Task Ready for Execution
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Evaluate Task Level                │
│    - Read task.level                   │
│    - Determine decision strategy         │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Execute Decision Logic              │
│                                          │
│    Level → Strategy:                     │
│    Mechanical ──────▶ Auto-execute        │
│    Recommended ────▶ Countdown (5s)      │
│    Confirmed ──────▶ Prompt user         │
│    Arbitrated ────▶ Request votes       │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Execute Task                       │
│    - Route to skill or agent           │
│    - Pass parameters                   │
│    - Monitor execution                │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 4a. Success        │  │ 4b. Failure      │
│    - Save result   │  │    - Check retry │
│    - Update state │  │      count      │
│    - Continue    │  │    - Rollback   │
│      next tasks   │  │      (if DAG)   │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 5. Persist State                      │
│    - Update task status                │
│    - Save to checkpoint               │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Emit Event                         │
│    - DomainEvent::TaskCompleted         │
│    - Notify subscribers               │
└───────────────────────────────────────────┘
```

### Parallel Task Execution Flow

```
DAG with Independent Tasks
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Dependency Resolution                │
│    Tasks: [A, B, C, D]                   │
│    Deps: A→C, B→C, C→D                  │
│                                          │
│    Level 1: [A, B] (independent)          │
│    Level 2: [C] (depends on A, B)         │
│    Level 3: [D] (depends on C)            │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Execute Level 1 in Parallel        │
│    ┌─────────┐     ┌─────────┐           │
│    │ Task A  │     │ Task B  │           │
│    │ Running │     │ Running │           │
│    └────┬────┘     └────┬────┘           │
│         │               │                 │
│         ▼               ▼                 │
│    [Complete]     [Complete]             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Verify Dependencies Met            │
│    - Check A completed ✓                │
│    - Check B completed ✓                │
│    - All deps for C satisfied           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Execute Level 2                    │
│    ┌─────────┐                           │
│    │ Task C  │                           │
│    │ Running │                           │
│    └────┬────┘                           │
│         │                                 │
│         ▼                                 │
│    [Complete]                             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Execute Level 3                    │
│    ┌─────────┐                           │
│    │ Task D  │                           │
│    │ Running │                           │
│    └────┬────┘                           │
│         │                                 │
│         ▼                                 │
│    [Complete]                             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. DAG Complete                       │
│    - All tasks finished                  │
│    - Return final result                │
└───────────────────────────────────────────┘
```

---

## Skill Invocation Flow

### Skill Discovery Flow

```
CIS Startup
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Scan Skill Directories              │
│    Paths: [                              │
│      ~/.cis/skills/,                     │
│      /usr/local/lib/cis/skills/,         │
│      ./.cis/skills/ (project)           │
│    ]                                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Read skill.toml for Each           │
│    - Validate metadata                   │
│    - Check permissions                  │
│    - Load dependencies                 │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 3a. Valid Skill    │  │ 3b. Invalid     │
│    - Add to       │  │    - Log warning │
│      registry     │  │    - Skip        │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 4. Initialize Skill Router             │
│    - Build skill map                    │
│    - Load project skills (if in project)│
│    - Ready for invocation               │
└───────────────────────────────────────────┘
```

### Skill Execution Flow (Native)

```
User Request: "Run linter on src/main.rs"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Intent Parsing                    │
│    - Extract: action="run", skill="linter"│
│    - Extract: args=["src/main.rs"]       │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Skill Router: Route Request        │
│    - Check project skills first           │
│    - Fall back to global skills          │
│    Found: linter (native)               │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Validate Permissions              │
│    - Check filesystem permission         │
│    - Validate path                     │
│    - Check command permission          │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 4a. Allowed        │  │ 4b. Denied       │
│    - Continue      │  │    - Return      │
│                    │  │      error       │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 5. Execute Native Skill              │
│    - Fork process                      │
│    - Set environment variables          │
│    - Pass stdin: {"args": ["src/main.rs"]}│
│    - Read stdout                       │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Monitor Execution                 │
│    - Check timeout (30s default)        │
│    - Monitor resource usage              │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 7a. Success        │  │ 7b. Timeout/Error│
│    - Parse output  │  │    - Kill       │
│    - Return result │  │      process     │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 8. Emit Event                        │
│    - DomainEvent::SkillExecuted          │
│    - Skill: linter                     │
│    - Duration: 2.3s                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 9. Return Result to User             │
│    {                                      │
│      "success": true,                     │
│      "output": "No issues found",          │
│      "exit_code": 0                      │
│    }                                      │
└───────────────────────────────────────────┘
```

### Skill Execution Flow (WASM)

```
User Request: "Run WASM skill"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Load WASM Module                  │
│    - Read .wasm file                   │
│    - Validate magic number              │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Compile to WASM Instance           │
│    - Parse WASM binary                 │
│    - Validate functions                │
│    - Allocate memory                  │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Link Host Functions               │
│    - cis_log()                        │
│    - cis_memory_get()                 │
│    - cis_memory_set()                 │
│    - cis_schedule_task()              │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Invoke Skill Function             │
│    - Call "handle" export             │
│    - Pass parameters as JSON           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Execute in Sandbox               │
│    - Enforce memory limits             │
│    - Enforce CPU limits              │
│    - Trap on invalid operations       │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 6a. Success        │  │ 6b. Trap/Error  │
│    - Read result   │  │    - Capture    │
│      from memory   │  │      error      │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 7. Cleanup                           │
│    - Deallocate memory                 │
│    - Drop instance                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 8. Return Result                     │
│    {                                      │
│      "success": true,                     │
│      "result": {...}                      │
│    }                                      │
└───────────────────────────────────────────┘
```

---

## Agent Integration Flow

### CIS → Agent (CIS Calls AI Provider)

```
Task requires AI: "Summarize these logs"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Determine Agent Provider           │
│    - Check task.agent (if specified)    │
│    - Fall back to config.default_agent  │
│    Selected: claude                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Build Agent Request               │
│    AgentRequest {                        │
│      prompt: "Summarize these logs...", │
│      context: AgentContext {           │
│        work_dir: Some("/project"),     │
│        memory_access: vec!["project/"],│
│        project_config: Some(...),      │
│      },                               │
│      skills: vec!["memory-search"],   │
│      max_tokens: Some(4096),         │
│      temperature: Some(0.7),         │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Call Claude Provider              │
│    claude::AgentProvider::execute()     │
│    - Build HTTP request                │
│    - Add authentication               │
│    - POST to Anthropic API           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Stream Response (if enabled)       │
│    - Read SSE events                   │
│    - Parse delta chunks                │
│    - Accumulate content               │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Parse Agent Response              │
│    AgentResponse {                       │
│      content: "The logs show...",       │
│      tokens_used: 1234,                │
│      model: "claude-3-sonnet",         │
│      duration: 2.3s,                  │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Process Result                   │
│    - Extract summary                    │
│    - Store to memory (if requested)    │
│    - Update usage statistics           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 7. Return to Task                   │
│    - Pass result to next task          │
│    - Continue DAG execution            │
└───────────────────────────────────────────┘
```

### Agent → CIS (AI Provider Calls CIS)

```
Claude needs memory: "Check user preferences"
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Agent Generates Command            │
│    Claude decides:                       │
│    "I need to check user preferences,   │
│     I'll call CIS memory service"       │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Agent Executes CIS CLI            │
│    $ cis memory get user/preference/theme │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. CLI Invokes Service Layer         │
│    - cli/main.rs                        │
│    - Parses command                    │
│    - Calls MemoryService               │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. MemoryService Executes           │
│    - Check cache                        │
│    - Query LogMemory                  │
│    - Return result                    │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Result Returns to Agent          │
│    CLI output: "dark"                   │
│    Agent reads output                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Agent Uses Result                 │
│    "User prefers dark theme.             │
│     I'll format output accordingly..."   │
└───────────────────────────────────────────┘
```

---

## P2P Synchronization Flow

### Peer Discovery Flow

```
CIS Node Starts
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Initialize P2P Swarm             │
│    - Generate keypair                   │
│    - Create DID                        │
│    - Bind to port 7677               │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Start Discovery Protocols          │
│    - mDNS (local LAN)                  │
│    - DHT (global network)              │
│    - Bootstrap nodes                  │
└─────────────────┬───────────────────────┘
                  │
                  ├──────────────────────┐
                  ▼                      ▼
┌──────────────────────┐    ┌──────────────────────┐
│ 3a. mDNS Discovery │    │ 3b. DHT Discovery │
│    - Broadcast on   │    │    - Query DHT    │
│      local network │    │    - Find peers   │
│    - Discover:     │    │    - Get addrs    │
│      192.168.1.3  │    └─────┬────────────┘
└─────────┬──────────┘          │
          │                     │
          └──────────┬─────────┘
                     ▼
┌─────────────────────────────────────────────┐
│ 4. Connect to Peers                 │
│    - Dial peer addresses                │
│    - Perform DID handshake            │
│    - Establish secure channel (Noise)  │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Emit PeerConnected Event           │
│    - Notify subscribers               │
│    - Update peer list                │
└───────────────────────────────────────────┘
```

### Public Memory Sync Flow

```
Memory Changed (Public Domain)
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Detect Change                     │
│    - MemoryService.set() called          │
│    - Domain: Public                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Create Sync Marker                │
│    SyncMarker {                          │
│      key: "project/config",            │
│      version: 123,                    │
│      timestamp: now,                  │
│      sync_peers: [peer1, peer2],     │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Queue for Sync                   │
│    - Add to sync_queue                 │
│    - Mark as pending                  │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Sync Loop (Every 5 min)          │
│    - Get pending sync markers          │
│    - For each marker:                  │
│      For each connected peer:          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Send to Peers                    │
│    p2p.sync_memory(key, peers)         │
│    - Serialize memory item             │
│    - Send via P2P protocol          │
│    - Wait for ACK                    │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 6a. Success        │  │ 6b. Failure     │
│    - Mark synced   │  │    - Retry later│
│    - Remove from   │  │    - Keep in    │
│      queue       │  │      queue      │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 7. Peer Receives Sync              │
│    - Deserialize memory item           │
│    - Check version (CRDT)          │
│    - Merge if newer                │
│    - Send ACK                      │
└───────────────────────────────────────────┘
```

### CRDT Merge Flow (Conflict Resolution)

```
Concurrent Updates to Same Key
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Peer A Updates                    │
│    key: "project/config"                │
│    value: v1, version: 10             │
│    synced to peers                    │
└─────────────────┬───────────────────────┘
                  │
                  │ (Meanwhile)
                  │
┌─────────────────┴───────────────────────┐
│ 2. Peer B Updates (Offline)         │
│    key: "project/config"                │
│    value: v2, version: 11             │
│    not synced (network partition)      │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Peer B Reconnects                │
│    - Discovers Peer A's update         │
│    - Has conflicting version           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. CRDT Merge Logic                 │
│    - Compare versions                   │
│    - Version 11 > Version 10          │
│    - Peer B's version wins            │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Apply Merge                      │
│    - Update value to v2                │
│    - Update version to 11             │
│    - Mark as resolved                 │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Sync Resolved Version            │
│    - Broadcast v2 to all peers        │
│    - Ensure convergence             │
└───────────────────────────────────────────┘
```

---

## Project Session Flow

### Project Detection and Initialization

```
User Runs CIS Command in Project Directory
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Check Current Directory           │
│    pwd: /Users/dev/my-project           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Search for Project Config         │
│    - Check .cis/project.toml            │
│    Found: project.toml                 │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Load Project Configuration        │
│    ProjectConfig {                      │
│      name: "my-project",               │
│      id: "proj-abc-123",              │
│      ai: AiConfig {...},               │
│      skills: [...],                   │
│      memory: MemoryConfig {...},        │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Create Project Session            │
│    ProjectSession {                     │
│      project: ProjectConfig,            │
│      agent: ClaudeProvider,            │
│      memory: MemoryService,            │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Initialize Session                │
│    a. Load project-local skills         │
│    b. Setup memory namespace          │
│       "project/my-project"             │
│    c. Configure agent with guide       │
│    d. Check shared keys              │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Ready for Project Operations       │
│    - Skills loaded                      │
│    - Memory namespace active            │
│    - Agent context configured          │
└───────────────────────────────────────────┘
```

### Project Skill Routing Flow

```
User Invokes Skill in Project
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Skill Router Receives Request       │
│    skill_name: "custom-linter"          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Check Project Skills First        │
│    if let Some(project) = current_project() │
│    - Search .cis/skills/               │
│    - Found: custom-linter             │
└─────────────────┬───────────────────────┘
                  │
                  ├────────────────┐
                  ▼                ▼
┌──────────────────────┐  ┌──────────────────┐
│ 3a. Found in       │  │ 3b. Not Found   │
│    Project        │  │    - Check      │
│    - Load project│  │      global      │
│      skill       │  │      skills     │
└─────────┬──────────┘  └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 4. Load Project Skill                │
│    - Read skill.toml                   │
│    - Validate permissions              │
│    - Prepare execution environment     │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Execute with Project Context       │
│    - Set CWD to project root           │
│    - Pass project config              │
│    - Execute skill                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Return Result                    │
│    - Skill output                      │
│    - Project-specific context           │
└───────────────────────────────────────────┘
```

---

## Matrix Federation Flow

### Matrix Client Connection Flow

```
CIS Starts Matrix Gateway
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Load Matrix Config                │
│    homeserver: "matrix.cis.io"         │
│    username: "@cis-node:matrix.cis.io"  │
│    password: "***"                      │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Connect to Homeserver             │
│    - Resolve homeserver address         │
│    - Establish WebSocket connection     │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Authenticate                     │
│    - Send login request                │
│    - Receive access token             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Initial Sync                     │
│    - Sync rooms                       │
│    - Sync messages                    │
│    - Sync member list                 │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Join Federation Room              │
│    - Room: #cis-federation             │
│    - Subscribe to events              │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Setup Event Handlers              │
│    - On message: Process             │
│    - On memory sync: Update memory   │
│    - On task request: Execute task  │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 7. Start Background Sync             │
│    - Sync loop every 30s               │
│    - Process incoming events           │
│    - Send outgoing updates           │
└───────────────────────────────────────────┘
```

### Memory Sync via Matrix Flow

```
Public Memory Updated Locally
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Detect Public Memory Change        │
│    - MemoryService.set() called          │
│    - Domain: Public                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Queue Matrix Event               │
│    MatrixEvent::MemorySync {            │
│      key: "project/config",            │
│      value: [...],                    │
│      version: 123,                   │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Send to Federation Room          │
│    - Serialize event                   │
│    - m.room.message                    │
│    - Send via Matrix client           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Other Nodes Receive Event         │
│    - Parse message                     │
│    - Validate sender (DID)            │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Apply Memory Update              │
│    - Check version (CRDT)             │
│    - Merge if newer                  │
│    - Update local LogMemory          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Emit Sync Complete Event          │
│    - Notify subscribers               │
│    - Update sync status              │
└───────────────────────────────────────────┘
```

---

## Error Handling Flow

### Error Propagation Flow

```
Error Occurs in Module
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Module Detects Error              │
│    - Operation fails                   │
│    - Return Err(ModuleError)           │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Convert to CisError              │
│    impl From<ModuleError> for CisError   │
│    - Wrap with context                │
│    - Preserve error chain             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Propagate to Service Layer       │
│    - Service receives CisError          │
│    - Decide recovery strategy         │
└─────────────────┬───────────────────────┘
                  │
                  ├──────────────────────┐
                  ▼                      ▼
┌──────────────────────┐    ┌──────────────────┐
│ 4a. Retryable      │    │ 4b. Fatal       │
│    - Log warning    │    │    - Log error   │
│    - Retry with     │    │    - Return to   │
│      backoff      │    │      user       │
└─────────┬──────────┘    └──────────────────┘
          │
          ▼
┌─────────────────────────────────────────────┐
│ 5. Retry Logic                      │
│    for attempt in 0..max_retries {      │
│      match operation() {                │
│        Ok(result) => return Ok(result), │
│        Err(e) if attempt < max - 1 =>  │
│          tokio::time::sleep(backoff).await,│
│        Err(e) => return Err(e),         │
│      }                                 │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Emit Error Event                 │
│    DomainEvent::Error {                │
│      error: CisError,                  │
│      context: ErrorContext,            │
│    }                                   │
└───────────────────────────────────────────┘
```

### Rollback Flow (DAG Execution)

```
Task Fails in DAG
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Detect Failure                    │
│    - Task execution failed               │
│    - Error not recoverable             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Check Rollback Policy            │
│    DAG policy: "rollback_on_failure"   │
│    - Has rollback handler?             │
│    Yes → Execute rollback             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Identify Completed Tasks          │
│    - Query task state                  │
│    - Find completed tasks in DAG        │
│    - Reverse topological order         │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Execute Rollbacks                 │
│    for task in completed_tasks.reverse() │
│    {                                   │
│      if let Some(rollback) = task.rollback │
│      {                                 │
│        rollback.execute().await?;         │
│      }                                 │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Restore State                     │
│    - Revert memory changes             │
│    - Delete created files             │
│    - Restore checkpoints             │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 6. Mark DAG as Failed               │
│    - Update DAG status                │
│    - Record failure reason            │
│    - Notify subscribers             │
└───────────────────────────────────────────┘
```

---

## Performance Optimization Patterns

### Caching Strategy

```
┌─────────────────────────────────────────────┐
│            Multi-Layer Cache             │
├─────────────────────────────────────────────┤
│                                          │
│  ┌──────────────────────────────────────┐ │
│  │  L1: In-Memory LRU Cache         │ │
│  │  - Hot data (< 1000 entries)      │ │
│  │  - Sub-millisecond access        │ │
│  └──────────┬─────────────────────────┘ │
│             │ Miss                     │
│             ▼                          │
│  ┌──────────────────────────────────────┐ │
│  │  L2: Vector Index (HNSW)        │ │
│  │  - Semantic search               │ │
│  │  - ~50ms access                 │ │
│  └──────────┬─────────────────────────┘ │
│             │ Miss                     │
│             ▼                          │
│  ┌──────────────────────────────────────┐ │
│  │  L3: Weekly DB (SQLite)         │ │
│  │  - Exact match                  │ │
│  │  - ~10ms access                │ │
│  └──────────┬─────────────────────────┘ │
│             │ Miss                     │
│             ▼                          │
│  ┌──────────────────────────────────────┐ │
│  │  L4: Archive (Compressed)       │ │
│  │  - Decompress & query            │ │
│  │  - ~100ms access               │ │
│  └──────────────────────────────────────┘ │
│                                          │
└─────────────────────────────────────────────┘
```

### Batch Write Optimization

```
Memory Write Operations
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Client Calls set()                │
│    for i in 0..1000 {                   │
│      memory.set(key, value).await          │
│    }                                     │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Buffer in Memory                 │
│    write_buffer.push(entry)             │
│    - Don't write immediately          │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Flush Conditions                  │
│    Flush when:                          │
│    - Buffer size >= 100               │
│    - OR 5 seconds elapsed              │
│    - OR explicit flush() called        │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Batch Write to DB                │
│    BEGIN TRANSACTION                    │
│    for entry in buffer {                │
│      INSERT INTO log_entries ...        │
│    }                                   │
│    COMMIT                              │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 5. Clear Buffer                     │
│    buffer.clear()                      │
└───────────────────────────────────────────┘
```

### Parallel Execution Pattern

```
Independent Tasks
                    │
                    ▼
┌─────────────────────────────────────────────┐
│ 1. Identify Parallel Tasks           │
│    Tasks: [A, B, C, D]                 │
│    Deps: A→C, B→C, C→D                │
│    Parallel groups:                     │
│    - Group 1: [A, B] (no deps)         │
│    - Group 2: [C] (depends on A,B)   │
│    - Group 3: [D] (depends on C)      │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 2. Execute Group 1 in Parallel     │
│    tokio::join! {                       │
│      task_a.execute(),                  │
│      task_b.execute(),                  │
│    }                                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 3. Wait for All to Complete          │
│    - Check results                      │
│    - Handle failures                   │
└─────────────────┬───────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────┐
│ 4. Proceed to Next Group            │
│    - All dependencies satisfied         │
│    - Execute Group 2                  │
└───────────────────────────────────────────┘
```

---

## Conclusion

The CIS data flow architecture is designed for:

- **Performance**: Multi-layer caching, batch operations, parallel execution
- **Reliability**: Error propagation, retry logic, rollback mechanisms
- **Scalability**: Weekly rotation, selective indexing, P2P distribution
- **Consistency**: CRDT-based sync, version vectors, transactional updates

Understanding these data flows is essential for:
- Debugging issues
- Optimizing performance
- Adding new features
- Extending the system

---

**Document Version**: 1.0
**Last Updated**: 2026-02-13
**Authors**: CIS Architecture Team
