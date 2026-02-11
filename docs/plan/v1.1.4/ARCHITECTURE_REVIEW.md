# CIS 架构评审报告

> 评审日期: 2026-02-10  
> 评审范围: cis-core, cis-node, cis-gui  
> 文档版本: v1.0.0

---

## 执行摘要

### 架构健康度评分

| 维度 | 评分 | 说明 |
|------|------|------|
| 模块内聚 | ⭐⭐⭐☆☆ (3/5) | 部分模块职责混乱，如 scheduler/mod.rs 3420行 |
| 模块耦合 | ⭐⭐☆☆☆ (2/5) | 多处紧耦合，全局状态管理问题严重 |
| 可测试性 | ⭐⭐☆☆☆ (2/5) | 单例模式、硬编码依赖导致测试困难 |
| 可配置性 | ⭐⭐☆☆☆ (2/5) | 大量硬编码端口、域名、路径 |
| 可扩展性 | ⭐⭐⭐☆☆ (3/5) | Skill 系统相对灵活，但 Agent 扩展受限 |
| **总体** | **2.4/5** | **需要重大改进** |

### 关键问题

1. **全局状态管理混乱** - P2P 使用 static 单例，导致测试困难和隐藏依赖
2. **模块边界模糊** - Matrix、Skill、Agent 之间存在循环依赖
3. **配置硬编码** - 端口、域名分散在代码各处
4. **上帝类** - 多个模块超过1000行，职责过多
5. **缺少抽象层** - 直接依赖具体实现而非接口

---

## 一、严重耦合问题 🔴

### 1.1 全局单例模式 - P2P Network

**位置**: `cis-core/src/p2p/network.rs`

**问题代码**:
```rust
// 全局静态实例
static P2P_INSTANCE: OnceCell<Arc<RwLock<Option<P2PNetwork>>>> = OnceCell::new();

impl P2PNetwork {
    /// 获取全局实例 (反模式!)
    pub fn global() -> Result<Arc<P2PNetwork>> {
        P2P_INSTANCE.get()
            .and_then(|lock| lock.read().ok())
            .and_then(|guard| guard.clone())
            .ok_or_else(|| CisError::p2p("P2P not initialized"))
    }
}
```

**问题分析**:
- 隐藏依赖，调用者不知道依赖了全局状态
- 无法并行测试（测试间会互相干扰）
- 无法 mock，单元测试困难
- 生命周期管理混乱

**改进方案**:
```rust
// 定义抽象接口
#[async_trait]
pub trait NetworkService: Send + Sync {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()>;
    async fn broadcast(&self, data: &[u8]) -> Result<()>;
    async fn connected_peers(&self) -> Vec<PeerInfo>;
}

// 实现依赖注入
pub struct NodeService {
    network: Arc<dyn NetworkService>,  // 依赖接口而非具体类型
}

impl NodeService {
    pub fn new(network: Arc<dyn NetworkService>) -> Self {
        Self { network }
    }
}

// 测试时使用 Mock
#[cfg(test)]
struct MockNetworkService {
    sent_messages: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
}

#[async_trait]
impl NetworkService for MockNetworkService {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()> {
        self.sent_messages.lock().await.push((node_id.to_string(), data.to_vec()));
        Ok(())
    }
    // ...
}
```

---

### 1.2 跨层直接调用 - Matrix Bridge → SkillManager

**位置**: `cis-core/src/matrix/bridge.rs`

**问题代码**:
```rust
impl MatrixBridge {
    pub async fn execute_skill(&self, skill_name: &str, event: &Event) -> Result<Vec<u8>> {
        // 直接创建 SkillManager 实例
        let skill_manager = SkillManager::new()?;  // 紧耦合!
        
        // 直接调用具体方法
        let result = skill_manager.execute(skill_name, event).await?;  // 无抽象层
        
        Ok(result)
    }
}
```

**问题分析**:
- Matrix 层直接依赖 Skill 层，违反分层架构
- 无法单独测试 Matrix 层
- SkillManager 变更会影响 Matrix Bridge

**改进方案**:
```rust
// 定义事件总线接口 (中介者模式)
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<()>;
    async fn subscribe(&self, event_type: &str, handler: Box<dyn EventHandler>) -> Result<()>;
}

// 定义 Skill 执行接口
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    async fn execute(&self, skill_name: &str, context: ExecutionContext) -> Result<ExecutionResult>;
}

// Matrix Bridge 只依赖接口
pub struct MatrixBridge {
    event_bus: Arc<dyn EventBus>,
    skill_executor: Arc<dyn SkillExecutor>,  // 依赖注入
}

impl MatrixBridge {
    pub async fn on_room_event(&self, event: RoomEvent) -> Result<()> {
        // 发布事件而非直接调用
        self.event_bus.publish(DomainEvent::RoomMessage {
            room_id: event.room_id,
            content: event.content,
        }).await?;
        
        Ok(())
    }
}

// Skill 模块订阅事件
pub struct SkillEventHandler {
    executor: Arc<dyn SkillExecutor>,
}

#[async_trait]
impl EventHandler for SkillEventHandler {
    async fn handle(&self, event: DomainEvent) -> Result<()> {
        if let DomainEvent::RoomMessage { content, .. } = event {
            if let Some(skill_name) = extract_skill_name(&content) {
                self.executor.execute(&skill_name, context).await?;
            }
        }
        Ok(())
    }
}
```

---

### 1.3 硬编码配置 - 端口号

**位置**: 多处分散

**问题代码**:
```rust
// matrix/mod.rs
pub const MATRIX_PORT: u16 = 6767;

// p2p/network.rs
pub const P2P_PORT: u16 = 7677;

// network/websocket.rs  
pub const WS_PORT: u16 = 6768;

// 还有多处直接使用数字
let addr = format!("127.0.0.1:6767");  // 魔法数字!
```

**问题分析**:
- 端口分散在代码各处，修改困难
- 无法根据环境配置不同端口
- 容易引起冲突

**改进方案**:
```rust
// config/network.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    pub matrix_port: u16,
    pub p2p_port: u16,
    pub websocket_port: u16,
    pub bind_address: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            matrix_port: 6767,
            p2p_port: 7677,
            websocket_port: 6768,
            bind_address: "0.0.0.0".to_string(),
        }
    }
}

// 通过依赖注入传递
pub struct MatrixServer {
    config: NetworkConfig,
}

impl MatrixServer {
    pub fn new(config: NetworkConfig) -> Self {
        Self { config }
    }
    
    pub async fn start(&self) -> Result<()> {
        let addr = format!("{}:{}", self.config.bind_address, self.config.matrix_port);
        // ...
    }
}
```

---

### 1.4 上帝类 - Matrix Nucleus

**位置**: `cis-core/src/matrix/nucleus.rs` (1432行, 64个pub fn)

**问题分析**:
- 单文件1432行，职责过多
- 包含 Room 管理、事件处理、联邦、存储等多个职责
- 修改任何功能都需要修改这个文件
- 代码冲突风险高

**改进方案**:
```rust
// 拆分前: 所有功能在一个文件
pub struct Nucleus {
    rooms: RoomManager,      // Room 管理
    events: EventStore,      // 事件存储
    federation: FederationManager,  // 联邦
    sync: SyncManager,       // 同步
    // ... 更多
}

// 拆分后: 按职责拆分模块
// matrix/room/manager.rs
pub struct RoomManager {
    store: Arc<dyn RoomStore>,
}

// matrix/event/store.rs
pub struct EventStore {
    db: Arc<dyn EventDatabase>,
}

// matrix/federation/manager.rs
pub struct FederationManager {
    client: Arc<dyn FederationClient>,
}

// 通过组合组装
pub struct Nucleus {
    room_manager: Arc<RoomManager>,
    event_store: Arc<EventStore>,
    federation: Arc<FederationManager>,
}
```

---

## 二、中等耦合问题 🟡

### 2.1 工厂类违反开闭原则

**位置**: `cis-core/src/agent/mod.rs`

**问题代码**:
```rust
pub struct AgentProviderFactory;

impl AgentProviderFactory {
    pub fn create(config: &AgentConfig) -> Result<Box<dyn AgentProvider>> {
        match config.agent_type {
            AgentType::Claude => Ok(Box::new(ClaudeProvider::new(config))),
            AgentType::Kimi => Ok(Box::new(KimiProvider::new(config))),
            AgentType::Aider => Ok(Box::new(AiderProvider::new(config))),
            AgentType::OpenCode => Ok(Box::new(OpenCodeProvider::new(config))),
            AgentType::Custom => Err(...),  // 新增类型需要修改这里!
        }
    }
}
```

**改进方案**:
```rust
// 使用注册表模式
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ProviderFactory>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, name: &str, factory: Box<dyn ProviderFactory>) {
        self.providers.insert(name.to_string(), factory);
    }
    
    pub fn create(&self, name: &str, config: &Config) -> Result<Box<dyn AgentProvider>> {
        self.providers.get(name)
            .ok_or_else(|| Error::unknown_provider(name))?
            .create(config)
    }
}

// 使用
lazy_static! {
    static ref REGISTRY: RwLock<ProviderRegistry> = RwLock::new(ProviderRegistry::new());
}

// 注册新 Provider (开闭原则: 扩展而非修改)
pub fn register_providers() {
    let mut reg = REGISTRY.write().unwrap();
    reg.register("claude", Box::new(ClaudeProviderFactory));
    reg.register("kimi", Box::new(KimiProviderFactory));
    // 第三方可以注册自己的 Provider
}
```

---

### 2.2 Skill Router 依赖过多

**位置**: `cis-core/src/skill/router.rs` (1287行)

**问题代码**:
```rust
pub struct SkillRouter {
    vector_storage: Arc<VectorStorage>,      // 直接依赖
    skill_manager: Arc<SkillManager>,        // 直接依赖
    db_manager: Arc<DbManager>,              // 直接依赖
    embedding_service: Arc<EmbeddingService>, // 直接依赖
    config: RouterConfig,
}
```

**问题分析**:
- 依赖4个具体类型
- 构造复杂，测试困难
- 任何依赖变更都需修改 Router

**改进方案**:
```rust
// 定义 Skill 查找接口
#[async_trait]
pub trait SkillRepository: Send + Sync {
    async fn find_by_intent(&self, intent: &Intent) -> Result<Vec<SkillMatch>>;
    async fn find_by_name(&self, name: &str) -> Result<Option<Skill>>;
}

// 定义向量检索接口
#[async_trait]
pub trait VectorSearch: Send + Sync {
    async fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>>;
}

// Skill Router 只依赖接口
pub struct SkillRouter {
    skill_repo: Arc<dyn SkillRepository>,
    vector_search: Arc<dyn VectorSearch>,
}

// 实现适配器
pub struct VectorStorageAdapter {
    storage: Arc<VectorStorage>,
}

#[async_trait]
impl VectorSearch for VectorStorageAdapter {
    async fn search(&self, query: &str, top_k: usize) -> Result<Vec<SearchResult>> {
        self.storage.search(query, top_k).await
    }
}
```

---

### 2.3 存储层直接暴露实现

**位置**: `cis-core/src/storage/` 多处

**问题代码**:
```rust
// 直接暴露 rusqlite 类型
pub fn get_connection() -> Result<Connection> {  // 返回具体类型!
    CONNECTION_POOL.get()
}

// 直接操作 SQL
pub fn save_event(event: &Event) -> Result<()> {
    let conn = get_connection()?;
    conn.execute(
        "INSERT INTO events (id, type, content) VALUES (?1, ?2, ?3)",  // SQL 硬编码
        params![event.id, event.event_type, event.content],
    )?;
    Ok(())
}
```

**改进方案**:
```rust
// 定义存储接口
#[async_trait]
pub trait EventStore: Send + Sync {
    async fn save(&self, event: &Event) -> Result<()>;
    async fn get(&self, id: &str) -> Result<Option<Event>>;
    async fn query(&self, filter: EventFilter) -> Result<Vec<Event>>;
}

// SQLite 实现细节封装
pub struct SqliteEventStore {
    pool: ConnectionPool,  // 不暴露 Connection
}

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn save(&self, event: &Event) -> Result<()> {
        // SQL 细节封装在此
        let conn = self.pool.get().await?;
        conn.execute(...).await?;
        Ok(())
    }
}

// 使用依赖注入
pub struct EventService {
    store: Arc<dyn EventStore>,  // 可以是 SQLite、Postgres、Memory
}
```

---

## 三、抽象接口设计

### 3.1 核心领域接口

```rust
// traits/mod.rs

/// 网络服务抽象
#[async_trait]
pub trait NetworkService: Send + Sync {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()>;
    async fn broadcast(&self, data: &[u8]) -> Result<()>;
    async fn connected_peers(&self) -> Vec<PeerInfo>;
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;
}

/// 存储服务抽象
#[async_trait]
pub trait StorageService: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn query(&self, query: StorageQuery) -> Result<Vec<StorageRecord>>;
}

/// 事件总线抽象
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<()>;
    async fn subscribe(&self, topic: &str, handler: Box<dyn EventHandler>) -> Result<Subscription>;
}

/// Skill 执行抽象
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    async fn execute(&self, skill_id: &str, context: ExecutionContext) -> Result<ExecutionResult>;
    async fn list_skills(&self) -> Result<Vec<SkillInfo>>;
}

/// Agent Provider 抽象
#[async_trait]
pub trait AgentProvider: Send + Sync {
    async fn execute(&self, task: &Task) -> Result<TaskResult>;
    async fn health_check(&self) -> Result<HealthStatus>;
    fn capabilities(&self) -> Vec<Capability>;
}
```

---

### 3.2 分层架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐       │
│  │   cis-node   │  │   cis-gui    │  │    HTTP API  │       │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘       │
└─────────┼─────────────────┼─────────────────┼───────────────┘
          │                 │                 │
┌─────────┼─────────────────┼─────────────────┼───────────────┐
│         │   Application Layer              │               │
│  ┌──────▼───────┐  ┌──────────────┐  ┌────▼────────┐        │
│  │  NodeService │  │  SkillService│  │AgentService │        │
│  └──────┬───────┘  └──────┬───────┘  └─────┬───────┘        │
└─────────┼─────────────────┼────────────────┼────────────────┘
          │                 │                │
┌─────────┼─────────────────┼────────────────┼────────────────┐
│         │   Domain Layer                   │               │
│  ┌──────▼───────┐  ┌──────────────┐  ┌────▼────────┐        │
│  │ SkillExecutor│  │  EventBus    │  │ Federation  │        │
│  └──────┬───────┘  └──────┬───────┘  └─────┬───────┘        │
└─────────┼─────────────────┼────────────────┼────────────────┘
          │                 │                │
┌─────────┼─────────────────┼────────────────┼────────────────┐
│         │   Infrastructure Layer           │               │
│  ┌──────▼───────┐  ┌──────────────┐  ┌────▼────────┐        │
│  │ P2PNetwork   │  │ SqliteStore  │  │WASMRuntime  │        │
│  └──────────────┘  └──────────────┘  └─────────────┘        │
└─────────────────────────────────────────────────────────────┘

依赖方向: 上层 → 下层 (通过接口)
禁止跨层调用: Presentation 不能直接调用 Infrastructure
```

---

### 3.3 依赖注入容器

```rust
// container.rs

pub struct ServiceContainer {
    network: Arc<dyn NetworkService>,
    storage: Arc<dyn StorageService>,
    event_bus: Arc<dyn EventBus>,
    skill_executor: Arc<dyn SkillExecutor>,
}

impl ServiceContainer {
    pub fn builder() -> ContainerBuilder {
        ContainerBuilder::new()
    }
    
    // Getters
    pub fn network(&self) -> Arc<dyn NetworkService> {
        Arc::clone(&self.network)
    }
    
    pub fn storage(&self) -> Arc<dyn StorageService> {
        Arc::clone(&self.storage)
    }
    
    // ...
}

pub struct ContainerBuilder {
    network: Option<Arc<dyn NetworkService>>,
    storage: Option<Arc<dyn StorageService>>,
    // ...
}

impl ContainerBuilder {
    pub fn with_network(mut self, network: Arc<dyn NetworkService>) -> Self {
        self.network = Some(network);
        self
    }
    
    pub fn with_storage(mut self, storage: Arc<dyn StorageService>) -> Self {
        self.storage = Some(storage);
        self
    }
    
    pub fn build(self) -> Result<ServiceContainer> {
        Ok(ServiceContainer {
            network: self.network.ok_or_else(|| Error::missing("network"))?,
            storage: self.storage.ok_or_else(|| Error::missing("storage"))?,
            // ...
        })
    }
}

// 使用
let container = ServiceContainer::builder()
    .with_network(Arc::new(P2PNetwork::new(config)?))
    .with_storage(Arc::new(SqliteStorage::new(db_path)?))
    .build()?;

let node_service = NodeService::new(container.network(), container.storage());
```

---

## 四、重构路线图

### Phase 1: 配置抽象 (Week 1)

**目标**: 消除所有硬编码配置

- [ ] 创建 `config/` 模块
- [ ] 收集所有硬编码端口、域名、路径
- [ ] 实现配置加载和验证
- [ ] 替换所有硬编码值

**涉及文件**:
- `config/network.rs` (新建)
- `config/storage.rs` (新建)
- `matrix/mod.rs`
- `p2p/network.rs`

---

### Phase 2: 全局状态消除 (Week 2)

**目标**: 消除 static 单例，改为依赖注入

- [ ] 移除 `P2PNetwork::global()`
- [ ] 移除其他 static 状态
- [ ] 实现依赖注入容器
- [ ] 更新所有调用点

**涉及文件**:
- `p2p/network.rs`
- `p2p/mod.rs`
- `lib.rs` (初始化逻辑)

---

### Phase 3: 事件总线引入 (Week 3-4)

**目标**: 解耦 Matrix ↔ Skill ↔ Agent 依赖

- [ ] 设计 `EventBus` 接口
- [ ] 实现内存事件总线
- [ ] 实现 Matrix 事件发布
- [ ] Skill 改为订阅模式
- [ ] 移除直接调用

**涉及文件**:
- `traits/event_bus.rs` (新建)
- `matrix/bridge.rs`
- `skill/manager.rs`
- `agent/federation/`

---

### Phase 4: 大文件拆分 (Week 5-6)

**目标**: 拆分超过1000行的文件

| 原文件 | 拆分后 |
|--------|--------|
| `scheduler/mod.rs` (3420行) | `scheduler/dag/mod.rs`, `scheduler/executor.rs`, `scheduler/monitor.rs` |
| `vector/storage.rs` (2109行) | `vector/storage/core.rs`, `vector/index.rs`, `vector/search.rs` |
| `matrix/nucleus.rs` (1432行) | `matrix/room/manager.rs`, `matrix/event/store.rs`, `matrix/federation/manager.rs` |

---

### Phase 5: 存储层抽象 (Week 7-8)

**目标**: 存储实现细节不暴露

- [ ] 定义 `StorageService` trait
- [ ] 实现 SQLite 适配器
- [ ] 重构所有存储调用
- [ ] 添加内存存储实现 (测试用)

---

## 五、测试改进

### 当前问题

```rust
// 难以测试的代码
fn test_current() {
    // 无法 mock P2P，因为是全局单例
    let result = some_function();  // 会调用 P2P::global()
}
```

### 改进后

```rust
// 易于测试的代码
#[tokio::test]
async fn test_with_mock() {
    let mock_network = Arc::new(MockNetworkService::new());
    let mock_storage = Arc::new(MockStorageService::new());
    
    let service = NodeService::new(mock_network.clone(), mock_storage.clone());
    
    // 预设 mock 行为
    mock_network.expect_send_to().returning(|_, _| Ok(()));
    
    // 执行测试
    service.send_message("node1", "hello").await.unwrap();
    
    // 验证
    assert_eq!(mock_network.sent_messages().len(), 1);
}
```

---

## 六、总结

### 主要问题

1. **全局状态** - P2P 单例导致测试困难
2. **跨层调用** - Matrix 直接调用 Skill
3. **硬编码** - 端口、域名分散
4. **上帝类** - 多个文件超过1000行
5. **缺少抽象** - 依赖具体实现

### 改进收益

| 维度 | 改进前 | 改进后 |
|------|--------|--------|
| 可测试性 | ⭐⭐☆☆☆ | ⭐⭐⭐⭐☆ |
| 可维护性 | ⭐⭐☆☆☆ | ⭐⭐⭐⭐☆ |
| 可扩展性 | ⭐⭐⭐☆☆ | ⭐⭐⭐⭐⭐ |
| 模块化 | ⭐⭐☆☆☆ | ⭐⭐⭐⭐☆ |

### 优先级建议

1. **🔴 P0 (立即)**: 消除全局单例、配置抽象
2. **🟡 P1 (本月)**: 事件总线、大文件拆分
3. **🟢 P2 (下月)**: 存储抽象、完善测试

---

*报告创建日期: 2026-02-10*  
*下次评审日期: 重构 Phase 2 完成后*
