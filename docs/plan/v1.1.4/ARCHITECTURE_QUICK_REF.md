# 架构快速参考

> 本文档总结架构评审的关键结论和推荐的抽象接口设计

---

## 架构健康度

```
总体评分: 2.4/5 ⭐⭐☆☆☆

模块内聚:  ⭐⭐⭐☆☆ (3/5)
模块耦合:  ⭐⭐☆☆☆ (2/5)  ⚠️ 需改进
可测试性:  ⭐⭐☆☆☆ (2/5)  ⚠️ 需改进
可配置性:  ⭐⭐☆☆☆ (2/5)  ⚠️ 需改进
可扩展性:  ⭐⭐⭐☆☆ (3/5)
```

---

## 关键问题速查表

### 🔴 立即修复 (P0)

| 问题 | 位置 | 影响 | 修复方案 |
|------|------|------|---------|
| 全局单例 | `p2p/network.rs:21` | 测试困难 | 依赖注入 |
| 跨层调用 | `matrix/bridge.rs` | 循环依赖 | 事件总线 |
| 硬编码端口 | 多处 | 无法配置 | 配置中心 |
| 上帝类 | `matrix/nucleus.rs` | 1432行 | 拆分模块 |

### 🟡 本月修复 (P1)

| 问题 | 位置 | 影响 | 修复方案 |
|------|------|------|---------|
| 工厂类 | `agent/mod.rs` | 违反开闭原则 | 注册表模式 |
| Router 依赖 | `skill/router.rs` | 4个依赖 | 接口抽象 |
| 存储暴露 | `storage/` | 实现泄漏 | Repository 模式 |
| 大文件 | 9个>1000行 | 维护困难 | 拆分模块 |

---

## 推荐抽象接口

### 1. 网络服务接口

```rust
#[async_trait]
pub trait NetworkService: Send + Sync {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()>;
    async fn broadcast(&self, data: &[u8]) -> Result<()>;
    async fn connected_peers(&self) -> Vec<PeerInfo>;
}

// 实现
pub struct P2PNetwork { ... }
pub struct MockNetworkService { ... }  // 测试用
```

**替换位置**: `p2p/network.rs` 全局单例

---

### 2. 存储服务接口

```rust
#[async_trait]
pub trait StorageService: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn query(&self, query: StorageQuery) -> Result<Vec<StorageRecord>>;
}

// 实现
pub struct SqliteStorage { ... }
pub struct MemoryStorage { ... }  // 测试用
```

**替换位置**: `storage/db.rs` 直接暴露 Connection

---

### 3. 事件总线接口

```rust
#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<()>;
    async fn subscribe(&self, topic: &str, handler: Box<dyn EventHandler>) -> Result<Subscription>;
}

pub enum DomainEvent {
    RoomMessage { room_id: String, content: String },
    SkillExecuted { skill_id: String, result: ExecutionResult },
    AgentOnline { node_id: String },
    // ...
}
```

**用于**: 解耦 Matrix ↔ Skill ↔ Agent

---

### 4. Skill 执行接口

```rust
#[async_trait]
pub trait SkillExecutor: Send + Sync {
    async fn execute(&self, skill_id: &str, context: ExecutionContext) -> Result<ExecutionResult>;
    async fn list_skills(&self) -> Result<Vec<SkillInfo>>;
}

pub struct SkillExecutorImpl {
    registry: Arc<SkillRegistry>,
    wasm_runtime: Arc<WasmRuntime>,
}
```

**替换位置**: `skill/manager.rs` 直接调用

---

### 5. Agent Provider 接口

```rust
#[async_trait]
pub trait AgentProvider: Send + Sync {
    async fn execute(&self, task: &Task) -> Result<TaskResult>;
    async fn health_check(&self) -> Result<HealthStatus>;
    fn capabilities(&self) -> Vec<Capability>;
}

// 注册表模式
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn ProviderFactory>>,
}

impl ProviderRegistry {
    pub fn register(&mut self, name: &str, factory: Box<dyn ProviderFactory>);
    pub fn create(&self, name: &str, config: &Config) -> Result<Box<dyn AgentProvider>>;
}
```

**替换位置**: `agent/mod.rs` 工厂类

---

## 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                    Presentation Layer                        │
│                   (cis-node, cis-gui)                       │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Application Layer                          │
│          (NodeService, SkillService, AgentService)          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     Domain Layer                             │
│         (SkillExecutor, EventBus, Federation)               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Infrastructure Layer                        │
│        (P2PNetwork, SqliteStorage, WASMRuntime)             │
└─────────────────────────────────────────────────────────────┘
```

**规则**:
- 上层可以调用下层
- 下层不能调用上层
- 同层之间通过事件总线通信
- 禁止跨层调用

---

## 依赖注入示例

```rust
// 构造时注入依赖
pub struct NodeService {
    network: Arc<dyn NetworkService>,
    storage: Arc<dyn StorageService>,
    event_bus: Arc<dyn EventBus>,
}

impl NodeService {
    pub fn new(
        network: Arc<dyn NetworkService>,
        storage: Arc<dyn StorageService>,
        event_bus: Arc<dyn EventBus>,
    ) -> Self {
        Self { network, storage, event_bus }
    }
}

// 使用容器管理依赖
let container = ServiceContainer::builder()
    .with_network(Arc::new(P2PNetwork::new(config)?))
    .with_storage(Arc::new(SqliteStorage::new(db_path)?))
    .with_event_bus(Arc::new(MemoryEventBus::new()))
    .build()?;

let node_service = NodeService::new(
    container.network(),
    container.storage(),
    container.event_bus(),
);
```

---

## 测试改进

### 之前 (难以测试)

```rust
fn some_function() {
    let p2p = P2PNetwork::global();  // 隐藏依赖,无法 mock
    p2p.broadcast(data)?;
}
```

### 之后 (易于测试)

```rust
async fn some_function(network: Arc<dyn NetworkService>) -> Result<()> {
    network.broadcast(data).await?;
    Ok(())
}

// 测试
#[tokio::test]
async fn test() {
    let mock = Arc::new(MockNetworkService::new());
    mock.expect_broadcast().returning(|_| Ok(()));
    
    some_function(mock.clone()).await.unwrap();
    
    assert!(mock.broadcast_called());
}
```

---

## 重构检查清单

### Phase 1: 配置抽象

- [ ] 创建 `config/` 模块
- [ ] 收集所有硬编码值
- [ ] 替换为配置读取

### Phase 2: 消除全局状态

- [ ] 移除 `P2PNetwork::global()`
- [ ] 实现依赖注入容器
- [ ] 更新所有调用点

### Phase 3: 事件总线

- [ ] 实现 `EventBus` trait
- [ ] Matrix 发布事件
- [ ] Skill 订阅事件

### Phase 4: 拆分大文件

- [ ] `scheduler/mod.rs` (3420行)
- [ ] `vector/storage.rs` (2109行)
- [ ] `matrix/nucleus.rs` (1432行)

### Phase 5: 存储抽象

- [ ] 定义 `StorageService` trait
- [ ] 实现 SQLite 适配器
- [ ] 实现内存适配器 (测试)

---

## 参考文档

- 详细分析: [ARCHITECTURE_REVIEW.md](./ARCHITECTURE_REVIEW.md)
- 重构路线: [ARCHITECTURE_REVIEW.md#四重构路线图](./ARCHITECTURE_REVIEW.md)
- 接口定义: [ARCHITECTURE_REVIEW.md#三抽象接口设计](./ARCHITECTURE_REVIEW.md)

---

*最后更新: 2026-02-10*
