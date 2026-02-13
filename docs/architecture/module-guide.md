# CIS Module Organization Guide v1.1.6

> **Version**: 1.1.6
> **Last Updated**: 2026-02-13
> **Target Audience**: Developers, Contributors, System Architects

---

## Table of Contents

1. [Module Architecture Overview](#module-architecture-overview)
2. [Core Foundation Modules](#core-foundation-modules)
3. [Memory & Intelligence Modules](#memory--intelligence-modules)
4. [Orchestration Modules](#orchestration-modules)
5. [Skills & Agents Modules](#skills--agents-modules)
6. [Networking Modules](#networking-modules)
7. [Storage & Data Modules](#storage--data-modules)
8. [Identity & Security Modules](#identity--security-modules)
9. [Project & Context Modules](#project--context-modules)
10. [Service Layer Modules](#service-layer-modules)
11. [Module Interaction Patterns](#module-interaction-patterns)
12. [Adding New Modules](#adding-new-modules)

---

## Module Architecture Overview

### Module Categories

CIS is organized into logical categories, each with specific responsibilities:

```
cis-core/
├── Core Foundation       # Basic building blocks
├── Memory & Intelligence # Data storage and retrieval
├── Orchestration        # Task execution and scheduling
├── Skills & Agents      # Capabilities and AI integration
├── Networking          # P2P communication
├── Storage & Data      # Persistent storage
├── Identity & Security  # Authentication and authorization
├── Project & Context    # Project-level features
└── Service Layer       # Business logic and API
```

### Dependency Graph

```
┌─────────────────────────────────────────────────────────────┐
│                    Service Layer                          │
│  (Depends on all other modules)                          │
└─────────────────────────────────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────┴─────────────────────────────────┐
│                  Orchestration                           │
│  scheduler/, decision/, task/                            │
└─────────────────────────────────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────┴─────────────────────────────────┐
│        Skills & Agents │ Memory & Intelligence             │
│  skill/, agent/        │  memory/, vector/, cache/        │
└─────────────────────────┴─────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────┴─────────────────────────────────┐
│       Networking │ Storage & Data │ Identity & Security      │
│  p2p/, network/       │  storage/, telemetry/  │  identity/ │
└─────────────────────────┴───────────────────────────────────┘
                          ▲
                          │
┌─────────────────────────┴─────────────────────────────────┐
│                 Core Foundation                          │
│  types/, error/, config/, sandbox/, events/              │
└─────────────────────────────────────────────────────────────┘
```

### Module Principles

1. **Single Responsibility**: Each module has one clear purpose
2. **Dependency Inversion**: Modules depend on abstractions (traits)
3. **Interface Segregation**: Small, focused interfaces
4. **Open/Closed**: Open for extension, closed for modification
5. **Fail-Safe**: Graceful degradation on errors

---

## Core Foundation Modules

### types/ - Core Data Structures

**Purpose**: Fundamental data types used across the system

**Key Types**:
```rust
// Memory domain classification
pub enum MemoryDomain {
    Private,  // Encrypted, never synced
    Public,   // Clear text, can be synced
}

// Memory categorization
pub enum MemoryCategory {
    Context,      // Conversation context
    Preference,   // User preferences
    Result,       // Query results
    System,       // System data
}

// Task execution levels
pub enum TaskLevel {
    Mechanical { retry: usize },           // Auto-execute
    Recommended { timeout: secs, default_action: String },
    Confirmed,                           // Manual approval
    Arbitrated { stakeholders: Vec<String> }, // Multi-party vote
}

// Memory item
pub struct MemoryItem {
    pub key: String,
    pub value: Vec<u8>,
    pub domain: MemoryDomain,
    pub category: MemoryCategory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

**Responsibilities**:
- Define core domain types
- Provide type-safe enums for configuration
- Implement serialization/deserialization
- Document type invariants

**Dependencies**: None (foundation module)

---

### error/ - Unified Error Handling

**Purpose**: Centralized error definitions and handling

**Key Types**:
```rust
pub enum CisError {
    // Memory errors
    MemoryNotFound(String),
    MemoryIndexFailed(String),

    // Network errors
    NetworkConnectionFailed(String),
    P2PDiscoveryFailed(String),

    // Skill errors
    SkillLoadFailed(String),
    SkillExecutionFailed(String),

    // Task errors
    TaskExecutionFailed(String),
    TaskValidationFailed(String),

    // Storage errors
    DatabaseError(String),
    IoError(String),

    // Generic errors
    InvalidInput(String),
    Unauthorized(String),
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, CisError>;
```

**Responsibilities**:
- Define all error types
- Provide error context and chain errors
- Implement error conversion from external libraries
- Format errors for user display

**Dependencies**: types

---

### config/ - Configuration Management

**Purpose**: Unified configuration loading and validation

**Key Structures**:
```rust
pub struct CisConfig {
    pub core: CoreConfig,
    pub memory: MemoryConfig,
    pub p2p: P2PConfig,
    pub agent: AgentConfig,
    pub skills: SkillsConfig,
}

pub struct CoreConfig {
    pub data_dir: PathBuf,
    pub log_level: LogLevel,
    pub max_concurrent_tasks: usize,
}

pub struct MemoryConfig {
    pub data_dir: PathBuf,
    pub hot_weeks: usize,
    pub total_weeks: usize,
    pub max_index_entries: usize,
}
```

**Responsibilities**:
- Load configuration from TOML files
- Validate configuration values
- Provide configuration to other modules
- Handle configuration overrides (env vars, CLI args)

**Dependencies**: types, error

**Configuration Files**:
- `~/.cis/config.toml` - Global configuration
- `.cis/project.toml` - Project configuration
- Environment variables: `CIS_*`

---

### sandbox/ - Security Boundaries

**Purpose**: Path and capability validation for security

**Key Functions**:
```rust
pub fn validate_path(path: &Path) -> Result<PathBuf> {
    // Ensure path is within allowed boundaries
    // Resolve symlinks
    // Check for path traversal attacks
}

pub fn validate_capability(cap: &Capability) -> Result<()> {
    // Check if capability is granted
    // Validate parameters
}
```

**Responsibilities**:
- Validate file system paths
- Check skill capabilities
- Prevent privilege escalation
- Enforce security boundaries

**Dependencies**: types, error

---

### events/ - Domain Events

**Purpose**: Decoupled communication via event bus

**Key Types**:
```rust
pub enum DomainEvent {
    MemoryChanged(MemoryChangedEvent),
    TaskCompleted(TaskCompletedEvent),
    SkillLoaded(SkillLoadedEvent),
    PeerConnected(PeerConnectedEvent),
}

pub trait EventBus {
    async fn publish(&self, event: DomainEvent) -> Result<()>;
    async fn subscribe(&self, event_type: Type) -> EventBusStream;
}
```

**Responsibilities**:
- Define domain events
- Implement event bus
- Provide event streaming
- Handle event subscriptions

**Dependencies**: types, error

---

## Memory & Intelligence Modules

### memory/ - Memory V2 Architecture

**Purpose**: Hybrid memory storage with precision indexing

**Module Structure**:
```
memory/
├── mod.rs              # Public API
├── log_memory.rs       # Weekly log storage
├── vector_index.rs     # Precision vector index
├── archiver.rs         # 54-week rotation
├── memory_service.rs   # Unified API
└── error.rs           # Memory-specific errors
```

**Key Components**:

#### LogMemory
```rust
pub struct LogMemory {
    db: Arc<Mutex<SqliteConnection>>,
    current_week: String,
    write_buffer: Arc<Mutex<Vec<LogEntry>>>,
}

impl LogMemory {
    pub async fn append(&self, entry: LogEntry) -> Result<()>;
    pub async fn get(&self, key: &str) -> Result<Option<LogEntry>>;
    pub async fn query_week(&self, week_id: &str) -> Result<Vec<LogEntry>>;
    pub async fn increment_access(&self, key: &str) -> Result<()>;
}
```

**Responsibilities**:
- Store all memory entries (complete logs)
- Organize by week (54-week rotation)
- Provide exact key queries
- Track access statistics
- Batch write optimization

#### VectorIndex
```rust
pub struct VectorIndex {
    hnsw: Hnsw<Vec<f32>>,
    strategy: IndexStrategy,
    max_entries: usize,
}

impl VectorIndex {
    pub async fn index_entry(&mut self, entry: &LogEntry) -> Result<bool>;
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<String>>;
    fn classify_entry(&self, entry: &LogEntry) -> IndexType;
    fn should_index(&self, index_type: &IndexType) -> bool;
}
```

**Responsibilities**:
- Selective indexing (~10% of data)
- Semantic search with embeddings
- Weight-based ranking
- LRU eviction when full

#### WeekArchiver
```rust
pub struct WeekArchiver {
    config: ArchiveConfig,
    db_dir: PathBuf,
}

impl WeekArchiver {
    pub async fn archive_current_week(&self) -> Result<String>;
    pub async fn cleanup_old_archives(&self) -> Result<CleanupReport>;
    pub fn list_week_dbs(&self) -> Result<Vec<String>>;
}
```

**Responsibilities**:
- Weekly rotation (Sunday 23:59)
- Compress old databases
- Maintain 54-week window
- Automatic cleanup

**Dependencies**: storage, vector, types, error

---

### vector/ - Semantic Search Engine

**Purpose**: Vector embeddings and similarity search

**Key Components**:
```rust
pub struct EmbeddingService {
    model: EmbeddingModel,
    cache: Arc<Mutex<LruCache<String, Vec<f32>>>>,
}

impl EmbeddingService {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>>;
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
}

pub struct VectorSearch {
    index: Hnsw<Vec<f32>>,
}

impl VectorSearch {
    pub fn search(&self, query: &[f32], limit: usize) -> Result<Vec<SearchResult>>;
}
```

**Responsibilities**:
- Generate text embeddings (768-dimensional)
- Manage embedding cache
- Provide HNSW-based similarity search
- Handle batch embedding

**Dependencies**: types, error

**Supported Models**:
- all-MiniLM-L6-v2 (default, English)
- paraphrase-multilingual-MiniLM-L12-v2 (multilingual)

---

### cache/ - LRU Cache Layer

**Purpose**: In-memory caching for frequently accessed data

**Key Types**:
```rust
pub struct MemoryCache {
    cache: Arc<Mutex<LruCache<String, CacheEntry>>>,
    max_size: usize,
    ttl: Duration,
}

pub struct CacheEntry {
    pub data: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub access_count: u32,
}

impl MemoryCache {
    pub async fn get(&self, key: &str) -> Option<Vec<u8>>;
    pub async fn set(&self, key: String, data: Vec<u8>);
    pub async fn invalidate(&self, key: &str);
    pub async fn clear(&self);
}
```

**Responsibilities**:
- Cache memory queries
- LRU eviction policy
- TTL-based expiration
- Cache statistics

**Dependencies**: types, error

---

## Orchestration Modules

### scheduler/ - DAG Task Scheduler

**Purpose**: Directed acyclic graph task orchestration

**Module Structure**:
```
scheduler/
├── mod.rs              # Public API
├── dag.rs             # DAG data structures
├── executor.rs        # Task execution engine
├── local_executor.rs  # Local task executor
├── persistence.rs     # State persistence
└── error.rs          # Scheduler errors
```

**Key Types**:
```rust
pub struct TaskDag {
    pub tasks: Vec<Task>,
    pub edges: Vec<(TaskId, TaskId)>,
    pub policy: ExecutionPolicy,
}

pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub skill: String,
    pub level: TaskLevel,
    pub deps: Vec<TaskId>,
    pub params: Value,
}

pub enum ExecutionPolicy {
    AllSuccess,    // All tasks must succeed
    BestEffort,    // Continue on failure
    ContinueOn,    // Continue on specific failures
}
```

**Responsibilities**:
- DAG validation (no cycles)
- Topological sorting
- Parallel execution of independent tasks
- State persistence and recovery
- Rollback on failure

**Dependencies**: types, error, decision, storage

---

### decision/ - Four-Tier Decision System

**Purpose**: Multi-level task approval mechanism

**Module Structure**:
```
decision/
├── mod.rs              # Public API
├── levels.rs          # Decision level implementations
│   ├── mechanical.rs
│   ├── recommended.rs
│   ├── confirmed.rs
│   └── arbitrated.rs
├── countdown.rs       # Countdown timer for recommended
├── confirmation.rs    # User confirmation UI
└── arbitration.rs    # Multi-party voting
```

**Decision Levels**:

#### Mechanical (Automatic)
```rust
pub struct MechanicalLevel {
    pub retry: usize,
}

impl DecisionLevel for MechanicalLevel {
    async fn execute(&self, task: Task) -> Result<TaskResult> {
        for attempt in 0..self.retry {
            match execute_task(&task).await {
                Ok(result) => return Ok(result),
                Err(e) if attempt < self.retry - 1 => {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}
```

#### Recommended (Countdown)
```rust
pub struct RecommendedLevel {
    pub timeout: Duration,
    pub default_action: String,
}

impl DecisionLevel for RecommendedLevel {
    async fn execute(&self, task: Task) -> Result<TaskResult> {
        println!("Recommended action: {}", task.name);
        println!("Will execute in {:?} (Ctrl+C to cancel)...", self.timeout);

        tokio::select! {
            _ = tokio::time::sleep(self.timeout) => {
                execute_task(&task).await
            }
            _ = tokio::signal::ctrl_c() => {
                Err(CisError::CancelledByUser)
            }
        }
    }
}
```

#### Confirmed (Manual Approval)
```rust
pub struct ConfirmedLevel;

impl DecisionLevel for ConfirmedLevel {
    async fn execute(&self, task: Task) -> Result<TaskResult> {
        println!("Please confirm: {}", task.description);
        print!("Execute? [y/N] ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            execute_task(&task).await
        } else {
            Err(CisError::CancelledByUser)
        }
    }
}
```

#### Arbitrated (Multi-Party Vote)
```rust
pub struct ArbitratedLevel {
    pub stakeholders: Vec<String>,
    pub quorum: usize,
}

impl DecisionLevel for ArbitratedLevel {
    async fn execute(&self, task: Task) -> Result<TaskResult> {
        let votes = request_votes(&self.stakeholders, &task).await?;

        if votes.iter().filter(|v| v.approved).count() >= self.quorum {
            execute_task(&task).await
        } else {
            Err(CisError::InsufficientVotes)
        }
    }
}
```

**Responsibilities**:
- Implement all four decision levels
- Handle user interactions
- Manage countdown timers
- Coordinate multi-party voting

**Dependencies**: types, error, events

---

### task/ - Task Management

**Purpose**: Task lifecycle and state management

**Key Types**:
```rust
pub struct TaskManager {
    db: Arc<Mutex<SqliteConnection>>,
    executor: Arc<TaskExecutor>,
}

pub struct Task {
    pub id: TaskId,
    pub name: String,
    pub description: String,
    pub status: TaskStatus,
    pub result: Option<TaskResult>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}
```

**Responsibilities**:
- Create and track tasks
- Store task history
- Query tasks by status
- Purge old tasks

**Dependencies**: types, error, storage

---

## Skills & Agents Modules

### skill/ - Skill Management System

**Purpose**: Hot-swappable capability modules

**Module Structure**:
```
skill/
├── mod.rs              # Public API
├── registry.rs        # Skill registry
├── router.rs          # Skill request router
├── executor.rs        # Skill execution engine
├── project_registry.rs # Project-local skills
├── chain.rs           # Skill chaining
├── semantics.rs       # Semantic skill discovery
└── types.rs          # Skill types
```

**Key Types**:
```rust
pub struct Skill {
    pub name: String,
    pub version: String,
    pub description: String,
    pub skill_type: SkillType,
    pub permissions: Vec<Permission>,
    pub path: PathBuf,
}

pub enum SkillType {
    Native,     // Native binary
    Wasm,       // WASM module
    Dag,        // DAG workflow
}

pub struct SkillManifest {
    pub skill: SkillMetadata,
    pub permissions: Permissions,
    pub dependencies: Vec<String>,
    pub environment: HashMap<String, String>,
}
```

#### SkillRegistry
```rust
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
    watchers: Vec<FileSystemWatcher>,
}

impl SkillRegistry {
    pub async fn discover(&mut self, paths: &[PathBuf]) -> Result<usize> {
        // Scan directories for skills
        // Load skill.toml files
        // Validate manifests
    }

    pub async fn load(&mut self, name: &str) -> Result<Arc<dyn SkillExecutor>> {
        // Load skill binary/WASM
        // Initialize executor
        // Return handle
    }

    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }
}
```

#### SkillRouter
```rust
pub struct SkillRouter {
    registry: Arc<SkillRegistry>,
    project_registry: Arc<ProjectRegistry>,
}

impl SkillRouter {
    pub async fn route(&self, request: SkillRequest) -> Result<SkillResponse> {
        // Check project-local skills first
        if let Some(project) = self.project_registry.current_project() {
            if let Some(skill) = project.get_skill(&request.skill_name) {
                return self.execute_skill(skill, request).await;
            }
        }

        // Fall back to global skills
        let skill = self.registry.get(&request.skill_name)
            .ok_or_else(|| CisError::SkillNotFound(request.skill_name))?;

        self.execute_skill(skill, request).await
    }
}
```

**Responsibilities**:
- Discover and register skills
- Route requests to appropriate skills
- Manage skill lifecycle (load/unload)
- Handle skill permissions

**Dependencies**: types, error, sandbox, wasm, project

---

### agent/ - AI Provider Abstraction

**Purpose**: Abstract interface for multiple AI providers

**Module Structure**:
```
agent/
├── mod.rs              # Public API
├── providers/          # Provider implementations
│   ├── mod.rs
│   ├── claude.rs      # Anthropic Claude
│   ├── kimi.rs        # Moonshot AI Kimi
│   ├── opencode.rs    # OpenCode GLM
│   └── aider.rs       # Aider
├── cluster/           # Agent cluster management
│   ├── mod.rs
│   ├── mod.rs
│   ├── context.rs     # Cluster context
│   ├── executor.rs    # Cluster executor
│   └── monitor.rs     # Cluster monitoring
└── types.rs          # Agent types
```

**Key Traits**:
```rust
#[async_trait]
pub trait AgentProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn execute(&self, req: AgentRequest) -> Result<AgentResponse>;
    async fn execute_stream(&self, req: AgentRequest, tx: mpsc::Sender<String>);

    fn capabilities(&self) -> AgentCapabilities;
}

pub struct AgentRequest {
    pub prompt: String,
    pub context: AgentContext,
    pub skills: Vec<String>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
}

pub struct AgentResponse {
    pub content: String,
    pub tokens_used: usize,
    pub model: String,
    pub duration: Duration,
}
```

**Responsibilities**:
- Provide unified API for all AI providers
- Handle provider-specific features
- Manage API keys and authentication
- Stream responses
- Track usage and costs

**Dependencies**: types, error, config

---

### wasm/ - WASM Runtime

**Purpose**: Sandboxed execution of WASM skills

**Key Types**:
```rust
pub struct WasmRuntime {
    engine: wasmi::Engine,
    modules: HashMap<String, wasmi::Module>,
}

pub struct WasmSkill {
    module: wasmi::Module,
    instance: wasmi::Instance,
    memory: wasmi::Memory,
}

impl WasmRuntime {
    pub fn load_module(&mut self, wasm_bytes: &[u8]) -> Result<String> {
        // Validate WASM magic number
        // Compile module
        // Store for instantiation
    }

    pub fn instantiate(&self, module_name: &str) -> Result<WasmSkill> {
        // Create instance
        // Link CIS host functions
        // Return handle
    }
}
```

**Host Functions**:
```rust
// Functions exposed to WASM skills
extern "C" {
    fn cis_log(level: u32, message_ptr: u32, message_len: u32);
    fn cis_memory_get(key_ptr: u32, key_len: u32) -> u32;
    fn cis_memory_set(key_ptr: u32, key_len: u32, value_ptr: u32, value_len: u32);
    fn cis_schedule_task(task_ptr: u32, task_len: u32);
}
```

**Responsibilities**:
- Load and validate WASM modules
- Provide host function interface
- Enforce capability security
- Limit resource usage (memory, CPU)

**Dependencies**: types, error, sandbox

---

## Networking Modules

### p2p/ - Peer-to-Peer Networking

**Purpose**: Decentralized P2P communication

**Module Structure**:
```
p2p/
├── mod.rs              # Public API
├── swarm.rs           # libp2p swarm management
├── behavior.rs        # P2P behavior implementation
├── discovery.rs       # Peer discovery
├── nat.rs            # NAT traversal
├── crdt.rs           # CRDT-based data sync
└── types.rs          # P2P types
```

**Key Types**:
```rust
pub struct P2PNetwork {
    swarm: Swarm<CisBehavior>,
    peer_id: PeerId,
    did: DID,
}

pub struct CisBehavior {
    // Custom P2P behavior for CIS
}

pub enum P2PEvent {
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
    MessageReceived(PeerId, Vec<u8>),
    DiscoveryFound(PeerId),
}
```

**Responsibilities**:
- Manage P2P swarm
- Handle peer discovery (mDNS, DHT)
- NAT traversal
- Encrypt communication (Noise protocol)
- Sync public memory

**Dependencies**: types, error, identity, events

---

### network/ - Network Session Management

**Purpose**: High-level network session and ACL management

**Module Structure**:
```
network/
├── mod.rs              # Public API
├── session_manager.rs # Session lifecycle
├── agent_session.rs   # Agent-specific sessions
├── acl.rs            # Access control lists
├── acl_module/       # ACL implementation
└── did_verify.rs     # DID verification
```

**Key Types**:
```rust
pub struct SessionManager {
    sessions: HashMap<SessionId, Session>,
    acl: Arc<AclManager>,
}

pub struct Session {
    pub id: SessionId,
    pub peer_id: PeerId,
    pub did: DID,
    pub capabilities: Vec<Capability>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

pub struct AclManager {
    rules: Vec<AclRule>,
}

pub struct AclRule {
    pub principal: Principal,
    pub resource: Resource,
    pub action: Action,
    pub effect: Effect,
}
```

**Responsibilities**:
- Manage network sessions
- Enforce ACL rules
- Verify DIDs
- Handle session lifecycle

**Dependencies**: types, error, identity, storage

---

### matrix/ - Matrix Federation Gateway

**Purpose**: Integration with Matrix decentralized messaging

**Module Structure**:
```
matrix/
├── mod.rs              # Public API
├── routes/           # HTTP routes
│   ├── auth.rs
│   ├── register.rs
│   ├── room.rs
│   └── discovery.rs
├── events/           # Matrix events
├── sync/             # Sync engine
├── websocket/        # WebSocket tunnel
├── cloud/            # Cloud tunnel for NAT
└── nucleus.rs        # Core Matrix client
```

**Key Types**:
```rust
pub struct MatrixGateway {
    client: matrix_sdk::Client,
    syncer: SyncEngine,
    tunnel: WsTunnel,
}

pub enum MatrixEvent {
    Message(RoomId, String),
    MemorySync(String, Vec<u8>),
    TaskRequest(TaskId, Task),
}
```

**Responsibilities**:
- Connect to Matrix homeserver
- Sync public memory
- Handle federation
- Tunnel through cloud for NAT traversal

**Dependencies**: types, error, memory, p2p

---

## Storage & Data Modules

### storage/ - Unified Storage Layer

**Purpose**: Cross-platform storage with core/skill isolation

**Module Structure**:
```
storage/
├── mod.rs              # Public API
├── connection.rs      # Database connection pooling
├── db.rs             # Main database
├── memory_db.rs      # Memory-specific database
├── federation_db.rs  # Federation database
├── conversation_db.rs # Conversation storage
├── room_manager.rs   # Matrix room management
├── pool.rs           # Connection pool
├── wal.rs            # Write-ahead logging
├── paths.rs          # Cross-platform paths
└── safety.rs         # Safe database operations
```

**Key Types**:
```rust
pub struct StorageManager {
    core_db: Arc<Mutex<Connection>>,
    skill_dbs: HashMap<String, Arc<Mutex<Connection>>>,
    pool: ConnectionPool,
}

pub struct ConnectionPool {
    max_connections: usize,
    connections: Vec<Connection>,
}
```

**Responsibilities**:
- Manage database connections
- Provide connection pooling
- Handle write-ahead logging
- Ensure data isolation between core and skills
- Cross-platform path resolution

**Dependencies**: types, error

---

### telemetry/ - Request Logging

**Purpose**: Comprehensive request logging and observability

**Key Types**:
```rust
pub struct RequestLogger {
    db: SqliteConnection,
}

pub struct RequestLog {
    pub id: RequestId,
    pub timestamp: DateTime<Utc>,
    pub method: String,
    pub path: String,
    pub duration_ms: u64,
    pub status_code: u16,
    pub stages: Vec<RequestStage>,
}

pub struct RequestStage {
    pub name: String,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub metadata: HashMap<String, Value>,
}
```

**Responsibilities**:
- Log all requests
- Track request stages
- Provide performance metrics
- Cleanup old logs

**Dependencies**: types, error, storage

---

## Identity & Security Modules

### identity/ - DID Management

**Purpose**: Decentralized identity with hardware binding

**Key Types**:
```rust
pub struct DIDManager {
    keypair: Keypair,
    hardware_id: HardwareId,
    did: DID,
}

pub struct DID {
    pub method: String,
    pub id: String,
    pub public_key: Vec<u8>,
}

pub struct HardwareId {
    pub cpu_id: String,
    pub mac_addresses: Vec<String>,
    pub disk_id: String,
}
```

**Responsibilities**:
- Generate DID keypairs
- Bind DID to hardware
- Sign and verify messages
- Manage DID documents

**Dependencies**: types, error

---

### decision/ - Decision Framework

**Purpose**: Configuration and execution of decision levels

**Responsibilities**:
- Provide decision-level configuration
- Execute decision workflows
- Track decision history
- Handle decision callbacks

**Dependencies**: types, error, events

---

## Project & Context Modules

### project/ - Project-Level Configuration

**Purpose**: Per-project configuration and local skills

**Key Types**:
```rust
pub struct ProjectConfig {
    pub name: String,
    pub id: String,
    pub root_dir: PathBuf,
    pub ai: AiConfig,
    pub skills: Vec<ProjectSkill>,
    pub memory: MemoryConfig,
}

pub struct ProjectSession {
    pub project: ProjectConfig,
    pub agent: Box<dyn AgentProvider>,
    pub memory: Arc<MemoryService>,
}

impl ProjectSession {
    pub async fn start(&self) -> Result<()> {
        // Send project context to agent
        // Load project-local skills
        // Setup memory namespace
    }
}
```

**Responsibilities**:
- Detect project from current directory
- Load project.toml configuration
- Manage project-local skills
- Setup project memory namespace
- Initialize project session

**Dependencies**: types, error, memory, agent, skill

---

### intent/ - Intent Parsing

**Purpose**: Natural language intent recognition

**Key Types**:
```rust
pub struct IntentParser {
    patterns: Vec<IntentPattern>,
    model: Option<NlpModel>,
}

pub struct Intent {
    pub action: String,
    pub entities: HashMap<String, String>,
    pub confidence: f32,
}
```

**Responsibilities**:
- Parse user input into intents
- Extract entities (parameters)
- Route to appropriate handlers
- Learn from corrections

**Dependencies**: types, error

---

### conversation/ - Conversation Management

**Purpose**: Multi-turn conversation context

**Key Types**:
```rust
pub struct ConversationManager {
    conversations: HashMap<ConversationId, Conversation>,
}

pub struct Conversation {
    pub id: ConversationId,
    pub messages: Vec<Message>,
    pub context: ConversationContext,
}

pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

pub enum MessageRole {
    User,
    Assistant,
    System,
}
```

**Responsibilities**:
- Track conversation history
- Manage conversation context
- Provide conversation summarization
- Handle context window limits

**Dependencies**: types, error, memory

---

## Service Layer Modules

### service/ - Business Logic Layer

**Purpose**: Unified data access for CLI, GUI, and API

**Module Structure**:
```
service/
├── mod.rs              # Public API
├── memory_service.rs  # Memory business logic
├── task_service.rs    # Task management
├── skill_service.rs   # Skill operations
├── node_service.rs    # Node management
├── dag_service.rs     # DAG operations
└── project_service.rs # Project management
```

**Key Pattern**:
```rust
pub struct MemoryService {
    log_memory: Arc<LogMemory>,
    vector_index: Arc<RwLock<VectorIndex>>,
    archiver: Arc<WeekArchiver>,
    cache: Arc<MemoryCache>,
}

#[async_trait]
pub trait IMemoryService: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<MemoryItem>>;
    async fn set(&self, key: &str, value: Vec<u8>, domain: MemoryDomain, category: MemoryCategory) -> Result<()>;
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>>;
    async fn delete(&self, key: &str) -> Result<()>;
}
```

**Responsibilities**:
- Implement business logic
- Coordinate multiple core modules
- Provide high-level API
- Handle transactions and rollbacks

**Dependencies**: All core modules

---

### cli/ - AI-Native CLI Framework

**Purpose**: Command-line interface with AI assistance

**Key Components**:
```rust
pub struct CisCli {
    config: Arc<CisConfig>,
    services: Arc<ServiceContainer>,
}

impl CisCli {
    pub async fn run(&self) -> Result<()> {
        // Parse commands
        // Execute actions
        // Display results
    }

    pub async fn ai_assist(&self, query: &str) -> Result<String> {
        // Use AI to understand complex queries
        // Generate commands
        // Execute and report
    }
}
```

**Responsibilities**:
- Parse CLI commands
- Provide AI-assisted command generation
- Format output
- Handle interactive sessions

**Dependencies**: service, config

---

### container/ - Dependency Injection Container

**Purpose**: Manage service lifecycles and dependencies

**Key Types**:
```rust
pub struct ServiceContainer {
    services: HashMap<TypeId, Box<dyn Any>>,
    singletons: HashMap<TypeId, Box<dyn Any>>,
}

impl ServiceContainer {
    pub fn register<T: 'static>(&mut self, service: T) {
        // Register service
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        // Retrieve service
    }

    pub async fn initialize_all(&self) -> Result<()> {
        // Initialize all services
    }
}
```

**Responsibilities**:
- Register services
- Resolve dependencies
- Manage singleton instances
- Handle service initialization

**Dependencies**: All modules (depends on everything)

---

## Module Interaction Patterns

### Memory Access Pattern

```
Service Layer
    │
    ├─▶ Check Cache (cache/)
    │       └─▶ Hit → Return
    │       └─▶ Miss
    │
    ├─▶ Query LogMemory (memory/)
    │       └─▶ Found → Update Cache → Return
    │       └─▶ Not Found
    │
    ├─▶ Search VectorIndex (vector/)
    │       └─▶ Results → Fetch from LogMemory → Return
    │       └─▶ No Results
    │
    └─▶ Return None
```

### Task Execution Pattern

```
DAG Service
    │
    ├─▶ Load DAG (scheduler/)
    │       └─▶ Validate
    │       └─▶ Topological Sort
    │
    ├─▶ For Each Task (decision/)
    │       │
    │       ├─▶ Evaluate Level
    │       │       ├─▶ Mechanical → Auto-execute
    │       │       ├─▶ Recommended → Countdown
    │       │       ├─▶ Confirmed → Prompt User
    │       │       └─▶ Arbitrated → Request Votes
    │       │
    │       ├─▶ Execute Task (skill/ or agent/)
    │       │       └─▶ Call Skill
    │       │       └─▶ Call Agent Provider
    │       │
    │       └─▶ Handle Result
    │               ├─▶ Success → Continue
    │               └─▶ Failure → Retry/Rollback
    │
    └─▶ Persist State (storage/)
```

### Skill Invocation Pattern

```
CLI/API Request
    │
    ├─▶ Parse Intent (intent/)
    │       └─▶ Extract Skill Name
    │       └─▶ Extract Parameters
    │
    ├─▶ Route Skill (skill/router.rs)
    │       │
    │       ├─▶ Check Project Skills (project/)
    │       │       └─▶ Found → Load
    │       │       └─▶ Not Found
    │       │
    │       └─▶ Check Global Skills (skill/registry.rs)
    │               └─▶ Found → Load
    │               └─▶ Not Found → Error
    │
    ├─▶ Load Skill (skill/executor.rs)
    │       ├─▶ Native → Fork Process
    │       ├─▶ WASM → Instantiate in Runtime
    │       └─▶ DAG → Execute via Scheduler
    │
    ├─▶ Execute with Sandbox (sandbox/)
    │       └─▶ Validate Paths
    │       └─▶ Check Capabilities
    │
    └─▶ Return Result
```

### P2P Sync Pattern

```
Network Event
    │
    ├─▶ Peer Connected (p2p/)
    │       └─▶ Exchange DIDs (identity/)
    │       └─▶ Verify Permissions (network/acl.rs)
    │       └─▶ Start Sync
    │
    ├─▶ Memory Sync (memory/)
    │       ├─▶ Get Changed Public Memory
    │       ├─▶ Send to Peers
    │       └─▶ Receive from Peers
    │               └─▶ Merge (CRDT) (p2p/crdt.rs)
    │
    └─▶ Handle Conflicts
            ├─▶ Last-Write-Wins
            ├─▶ Manual Resolution
            └─▶ CRDT Merge
```

---

## Adding New Modules

### Step 1: Define Module Purpose

Before creating a new module, answer:
1. What problem does it solve?
2. What are its responsibilities?
3. What are its dependencies?
4. What is its public API?

### Step 2: Create Module Structure

```bash
cd cis-core/src
mkdir new_module
touch new_module/mod.rs
touch new_module/types.rs
touch new_module/error.rs
```

### Step 3: Implement Core Types

```rust
// new_module/types.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewType {
    pub id: String,
    pub data: Vec<u8>,
}
```

### Step 4: Define Errors

```rust
// new_module/error.rs
use cis_core::error::{CisError, Result};

#[derive(Debug, thiserror::Error)]
pub enum NewModuleError {
    #[error("Failed to process: {0}")]
    ProcessingFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl From<NewModuleError> for CisError {
    fn from(err: NewModuleError) -> Self {
        CisError::InternalError(err.to_string())
    }
}
```

### Step 5: Implement Public API

```rust
// new_module/mod.rs
pub mod types;
pub mod error;

use types::NewType;
use error::{NewModuleError, Result};

pub struct NewModule {
    config: ModuleConfig,
}

impl NewModule {
    pub fn new(config: ModuleConfig) -> Self {
        Self { config }
    }

    pub async fn process(&self, input: &str) -> Result<NewType> {
        // Implementation
    }
}
```

### Step 6: Register with Container

```rust
// container/mod.rs
use crate::new_module::NewModule;

impl ServiceContainer {
    pub fn register_new_module(&mut self, config: ModuleConfig) {
        let module = NewModule::new(config);
        self.register(module);
    }
}
```

### Step 7: Write Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_module() {
        let config = ModuleConfig::default();
        let module = NewModule::new(config);

        let result = module.process("test").await;
        assert!(result.is_ok());
    }
}
```

### Step 8: Add Documentation

```rust
//! # New Module
//!
//! This module provides functionality for...
//!
//! ## Example
//!
//! ```rust
//! use cis_core::new_module::NewModule;
//!
//! let module = NewModule::new(config);
//! let result = module.process("input").await?;
//! ```
```

### Step 9: Update lib.rs

```rust
// cis-core/src/lib.rs
pub mod new_module;
```

---

## Module Best Practices

### 1. Keep Modules Focused

- Each module should have a single, well-defined purpose
- If a module does too many things, split it
- Use sub-modules for organization

### 2. Use Traits for Abstraction

```rust
#[async_trait]
pub trait ModuleTrait: Send + Sync {
    async fn process(&self, input: &str) -> Result<String>;
}
```

### 3. Handle Errors Gracefully

```rust
pub async fn process(&self, input: &str) -> Result<String> {
    self.validate(input)?;

    match self.internal_process(input).await {
        Ok(result) => Ok(result),
        Err(e) => {
            tracing::error!("Processing failed: {}", e);
            Err(e.into())
        }
    }
}
```

### 4. Use Dependency Injection

```rust
pub struct MyModule {
    dependency: Arc<dyn DependencyTrait>,
}

impl MyModule {
    pub fn new(dependency: Arc<dyn DependencyTrait>) -> Self {
        Self { dependency }
    }
}
```

### 5. Emit Events

```rust
use cis_core::events::{EventBus, DomainEvent};

pub async fn process(&self, input: &str) -> Result<String> {
    let result = self.internal_process(input).await?;

    self.event_bus.publish(DomainEvent::ProcessingComplete {
        input: input.to_string(),
        result: result.clone(),
    }).await?;

    Ok(result)
}
```

### 6. Write Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_module() -> MyModule {
        // Setup test dependencies
        MyModule::new(test_config())
    }

    #[tokio::test]
    async fn test_happy_path() {
        let module = setup_test_module();
        let result = module.process("valid input").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_error_handling() {
        let module = setup_test_module();
        let result = module.process("invalid input").await;
        assert!(result.is_err());
    }
}
```

### 7. Document Public APIs

```rust
/// Processes the input string and returns a result.
///
/// # Arguments
///
/// * `input` - The input string to process
///
/// # Returns
///
/// * `Ok(String)` - The processed result
/// * `Err(CisError)` - If processing fails
///
/// # Example
///
/// ```no_run
/// use cis_core::my_module::MyModule;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let module = MyModule::new(default_config());
/// let result = module.process("hello").await?;
/// # Ok(())
/// # }
/// ```
pub async fn process(&self, input: &str) -> Result<String> {
    // Implementation
}
```

---

## Conclusion

The CIS module architecture is designed to be:
- **Modular**: Clear separation of concerns
- **Extensible**: Easy to add new functionality
- **Maintainable**: Well-organized and documented
- **Testable**: Each module can be tested independently
- **Performant**: Minimal overhead and efficient data flow

When adding new modules or modifying existing ones, follow the established patterns and principles to maintain consistency and quality across the codebase.

---

**Document Version**: 1.0
**Last Updated**: 2026-02-13
**Authors**: CIS Architecture Team
