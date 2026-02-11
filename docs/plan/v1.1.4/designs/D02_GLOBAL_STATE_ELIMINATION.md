# D02: 全局状态消除 (Phase 2)

> 任务: P0-1 架构重构 Phase 2  
> 负责人: 开发 A  
> 工期: Week 2-3 (7天)  
> 状态: 设计中  
> 依赖: D01 配置抽象

---

## 目标

消除所有 `static` 单例，改为依赖注入模式。

---

## 当前问题

```rust
// ❌ 全局单例 - p2p/network.rs
static P2P_INSTANCE: OnceCell<Arc<RwLock<Option<P2PNetwork>>>> = OnceCell::new();

impl P2PNetwork {
    pub fn global() -> Result<Arc<P2PNetwork>> {  // 反模式!
        P2P_INSTANCE.get()
            .and_then(|lock| lock.read().ok())
            .and_then(|guard| guard.clone())
            .ok_or_else(|| CisError::p2p("P2P not initialized"))
    }
}

// 使用 - 隐藏依赖
fn some_function() {
    let p2p = P2PNetwork::global().unwrap();  // 难以测试!
    p2p.broadcast(data);
}
```

**问题**:
- 隐藏依赖，难以追踪
- 无法并行测试
- 无法 mock
- 生命周期管理混乱

---

## 设计方案

### 抽象接口定义

```rust
// traits/network.rs

#[async_trait]
pub trait NetworkService: Send + Sync {
    /// 发送消息到指定节点
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()>;
    
    /// 广播消息到所有节点
    async fn broadcast(&self, data: &[u8]) -> Result<()>;
    
    /// 获取已连接节点列表
    async fn connected_peers(&self) -> Vec<PeerInfo>;
    
    /// 发现新节点
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>>;
    
    /// 启动网络服务
    async fn start(&self) -> Result<()>;
    
    /// 停止网络服务
    async fn stop(&self) -> Result<()>;
}

// traits/storage.rs

#[async_trait]
pub trait StorageService: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn put(&self, key: &str, value: &[u8]) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<()>;
    async fn query(&self, query: StorageQuery) -> Result<Vec<StorageRecord>>;
}

// traits/event_bus.rs

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: DomainEvent) -> Result<()>;
    async fn subscribe(&self, topic: &str, handler: Box<dyn EventHandler>) -> Result<Subscription>;
}

pub trait EventHandler: Send + Sync {
    fn handle(&self, event: DomainEvent) -> Result<()>;
}
```

---

### 依赖注入容器

```rust
// container.rs

use std::sync::Arc;

/// 服务容器 - 管理所有依赖
pub struct ServiceContainer {
    config: Arc<Config>,
    network: Arc<dyn NetworkService>,
    storage: Arc<dyn StorageService>,
    event_bus: Arc<dyn EventBus>,
}

impl ServiceContainer {
    /// 创建容器（生产环境）
    pub async fn production(config: Config) -> Result<Self> {
        let config = Arc::new(config);
        
        // 创建基础设施
        let storage = Arc::new(SqliteStorage::new(&config.storage).await?);
        let event_bus = Arc::new(MemoryEventBus::new());
        let network = Arc::new(P2PNetwork::new(&config.p2p, event_bus.clone()).await?);
        
        Ok(Self {
            config,
            network,
            storage,
            event_bus,
        })
    }
    
    /// 创建容器（测试环境）
    pub fn test() -> TestContainerBuilder {
        TestContainerBuilder::new()
    }
    
    // Getters
    pub fn config(&self) -> Arc<Config> {
        Arc::clone(&self.config)
    }
    
    pub fn network(&self) -> Arc<dyn NetworkService> {
        Arc::clone(&self.network)
    }
    
    pub fn storage(&self) -> Arc<dyn StorageService> {
        Arc::clone(&self.storage)
    }
    
    pub fn event_bus(&self) -> Arc<dyn EventBus> {
        Arc::clone(&self.event_bus)
    }
}

/// 测试容器构建器
pub struct TestContainerBuilder {
    config: Option<Config>,
    network: Option<Arc<dyn NetworkService>>,
    storage: Option<Arc<dyn StorageService>>,
    event_bus: Option<Arc<dyn EventBus>>,
}

impl TestContainerBuilder {
    fn new() -> Self {
        Self {
            config: None,
            network: None,
            storage: None,
            event_bus: None,
        }
    }
    
    pub fn with_network(mut self, network: Arc<dyn NetworkService>) -> Self {
        self.network = Some(network);
        self
    }
    
    pub fn with_storage(mut self, storage: Arc<dyn StorageService>) -> Self {
        self.storage = Some(storage);
        self
    }
    
    pub fn with_event_bus(mut self, event_bus: Arc<dyn EventBus>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }
    
    pub fn build(self) -> ServiceContainer {
        ServiceContainer {
            config: Arc::new(self.config.unwrap_or_default()),
            network: self.network.unwrap_or_else(|| Arc::new(MockNetworkService::new())),
            storage: self.storage.unwrap_or_else(|| Arc::new(MockStorageService::new())),
            event_bus: self.event_bus.unwrap_or_else(|| Arc::new(MockEventBus::new())),
        }
    }
}
```

---

### 服务层改造

```rust
// 修改前 - 使用全局单例
pub struct NodeService;

impl NodeService {
    pub async fn broadcast_status(&self) -> Result<()> {
        let p2p = P2PNetwork::global()?;  // ❌ 隐藏依赖
        p2p.broadcast(b"status:online").await?;
        Ok(())
    }
}

// 修改后 - 依赖注入
pub struct NodeService {
    network: Arc<dyn NetworkService>,
}

impl NodeService {
    pub fn new(network: Arc<dyn NetworkService>) -> Self {
        Self { network }
    }
    
    pub async fn broadcast_status(&self) -> Result<()> {
        self.network.broadcast(b"status:online").await?;  // ✅ 显式依赖
        Ok(())
    }
}

// 使用
let container = ServiceContainer::production(config).await?;
let node_service = NodeService::new(container.network());
node_service.broadcast_status().await?;
```

---

### Mock 实现

```rust
// mocks/network.rs

use std::sync::{Arc, Mutex};

pub struct MockNetworkService {
    sent_messages: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    should_fail: bool,
}

impl MockNetworkService {
    pub fn new() -> Self {
        Self {
            sent_messages: Arc::new(Mutex::new(Vec::new())),
            should_fail: false,
        }
    }
    
    pub fn set_should_fail(&mut self, fail: bool) {
        self.should_fail = fail;
    }
    
    pub fn sent_messages(&self) -> Vec<(String, Vec<u8>)> {
        self.sent_messages.lock().unwrap().clone()
    }
}

#[async_trait]
impl NetworkService for MockNetworkService {
    async fn send_to(&self, node_id: &str, data: &[u8]) -> Result<()> {
        if self.should_fail {
            return Err(Error::network("mock failure"));
        }
        self.sent_messages.lock().unwrap().push((
            node_id.to_string(),
            data.to_vec(),
        ));
        Ok(())
    }
    
    async fn broadcast(&self, data: &[u8]) -> Result<()> {
        if self.should_fail {
            return Err(Error::network("mock failure"));
        }
        // 记录广播
        Ok(())
    }
    
    async fn connected_peers(&self) -> Vec<PeerInfo> {
        vec![]
    }
    
    async fn discover_peers(&self) -> Result<Vec<PeerInfo>> {
        Ok(vec![])
    }
    
    async fn start(&self) -> Result<()> {
        Ok(())
    }
    
    async fn stop(&self) -> Result<()> {
        Ok(())
    }
}
```

---

## 测试改进

### 修改前（难以测试）

```rust
#[test]
fn test_with_global() {
    // 无法测试，因为 P2PNetwork::global() 是全局状态
    // 测试之间会互相干扰
}
```

### 修改后（易于测试）

```rust
#[tokio::test]
async fn test_node_service() {
    // 创建 mock
    let mock_network = Arc::new(MockNetworkService::new());
    
    // 注入依赖
    let service = NodeService::new(mock_network.clone());
    
    // 执行测试
    service.broadcast_status().await.unwrap();
    
    // 验证行为
    let messages = mock_network.sent_messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].1, b"status:online");
}

#[tokio::test]
async fn test_node_service_failure() {
    let mut mock = MockNetworkService::new();
    mock.set_should_fail(true);
    let mock = Arc::new(mock);
    
    let service = NodeService::new(mock);
    
    // 验证错误处理
    let result = service.broadcast_status().await;
    assert!(result.is_err());
}
```

---

## 迁移计划

### 阶段 A: 提取接口 (Week 2, Day 1-3)

```rust
// 1. 为 P2PNetwork 创建 trait
#[async_trait]
pub trait NetworkService: Send + Sync { ... }

// 2. 为现有实现实现 trait
impl NetworkService for P2PNetwork { ... }

// 3. 创建 mock 实现
pub struct MockNetworkService { ... }
impl NetworkService for MockNetworkService { ... }
```

### 阶段 B: 创建容器 (Week 2, Day 4-5)

```rust
// 创建 ServiceContainer
// 创建 TestContainerBuilder
```

### 阶段 C: 改造服务层 (Week 3, Day 1-5)

逐个改造服务：
1. NodeService
2. SkillService
3. AgentService
4. ...

每个服务：
1. 添加构造函数参数
2. 移除全局单例调用
3. 更新测试

---

## 移除的全局状态清单

| 全局状态 | 位置 | 替代方案 | 状态 |
|---------|------|---------|------|
| `P2P_INSTANCE` | `p2p/network.rs` | `Arc<dyn NetworkService>` | 待移除 |
| `DB_POOL` | `storage/db.rs` | `Arc<dyn StorageService>` | 待移除 |
| `CONFIG` | `config/mod.rs` | 显式传递 | 待移除 |

---

## 任务清单

- [ ] 定义 `NetworkService` trait
- [ ] 定义 `StorageService` trait
- [ ] 定义 `EventBus` trait
- [ ] 为 `P2PNetwork` 实现 `NetworkService`
- [ ] 为 `SqliteStorage` 实现 `StorageService`
- [ ] 创建 `MockNetworkService`
- [ ] 创建 `MockStorageService`
- [ ] 创建 `ServiceContainer`
- [ ] 创建 `TestContainerBuilder`
- [ ] 改造 `NodeService`
- [ ] 改造 `SkillService`
- [ ] 改造 `AgentService`
- [ ] 移除 `P2PNetwork::global()`
- [ ] 更新所有测试

---

## 验收标准

```bash
# 测试 1: 所有测试通过
cargo test --all

# 测试 2: 无全局单例
# 搜索 static 关键字，确认无业务逻辑单例
grep -r "static.*OnceCell" src/ | grep -v test | wc -l
# 预期: 0

# 测试 3: 测试并行执行
cargo test --all -- --test-threads=8
# 预期: 全部通过（无全局状态冲突）
```

---

## 依赖

- D01 配置抽象

---

*设计创建日期: 2026-02-10*
