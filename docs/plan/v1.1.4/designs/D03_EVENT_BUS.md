# D03: 事件总线设计 (Phase 3)

> 任务: P0-1 架构重构 Phase 3  
> 负责人: 开发 A  
> 工期: Week 3-4 (8天)  
> 状态: 设计中  
> 依赖: D02 全局状态消除

---

## 目标

解耦 Matrix、Skill、Agent 之间的直接调用，通过事件总线通信。

---

## 当前问题

```rust
// ❌ 跨层直接调用 - matrix/bridge.rs
impl MatrixBridge {
    pub async fn on_room_event(&self, event: RoomEvent) -> Result<()> {
        // 直接创建 SkillManager 实例
        let skill_manager = SkillManager::new()?;  // 紧耦合!
        
        // 直接调用
        skill_manager.execute(event.skill_name, event).await?;  // 无抽象层
        
        Ok(())
    }
}

// 问题:
// 1. Matrix 层直接依赖 Skill 层
// 2. 无法单独测试 Matrix 层
// 3. SkillManager 变更会影响 Matrix Bridge
// 4. 循环依赖风险
```

---

## 设计方案

### 领域事件定义

```rust
// events/domain.rs

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// 所有领域事件的基 trait
pub trait DomainEvent: Send + Sync {
    fn event_id(&self) -> &str;
    fn event_type(&self) -> &str;
    fn timestamp(&self) -> DateTime<Utc>;
    fn to_json(&self) -> Result<String>;
}

/// 房间消息事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomMessageEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub room_id: String,
    pub sender: String,
    pub content: MessageContent,
}

impl DomainEvent for RoomMessageEvent {
    fn event_id(&self) -> &str { &self.event_id }
    fn event_type(&self) -> &str { "room.message" }
    fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}

/// Skill 执行请求事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillExecuteEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub skill_name: String,
    pub method: String,
    pub params: Vec<u8>,
    pub requester: String,
    pub context: ExecutionContext,
}

impl DomainEvent for SkillExecuteEvent {
    fn event_id(&self) -> &str { &self.event_id }
    fn event_type(&self) -> &str { "skill.execute" }
    fn timestamp(&self) -> DateTime<Utc> { self.timestamp }
}

/// Skill 执行完成事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCompletedEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub original_event_id: String,
    pub skill_name: String,
    pub result: ExecutionResult,
}

/// Agent 在线状态事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentOnlineEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub node_id: String,
    pub agent_id: String,
    pub capabilities: Vec<Capability>,
}

/// 联邦任务事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FederationTaskEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub task_id: String,
    pub from_node: String,
    pub to_node: String,
    pub task: Task,
}

/// 事件包装器 - 用于序列化
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventWrapper {
    RoomMessage(RoomMessageEvent),
    SkillExecute(SkillExecuteEvent),
    SkillCompleted(SkillCompletedEvent),
    AgentOnline(AgentOnlineEvent),
    FederationTask(FederationTaskEvent),
    // ... 更多事件
}
```

---

### 事件总线接口

```rust
// event_bus/mod.rs

#[async_trait]
pub trait EventBus: Send + Sync {
    /// 发布事件
    async fn publish<E: DomainEvent>(&self, event: E) -> Result<()>;
    
    /// 订阅特定类型的事件
    async fn subscribe<F, E>(&self, handler: F) -> Result<Subscription>
    where
        F: EventHandler<E>,
        E: DomainEvent;
    
    /// 订阅特定 topic
    async fn subscribe_topic(&self, topic: &str, handler: Box<dyn Fn(EventWrapper)>) -> Result<Subscription>;
}

/// 事件处理器 trait
#[async_trait]
pub trait EventHandler<E: DomainEvent>: Send + Sync {
    async fn handle(&self, event: E) -> Result<()>;
}

/// 订阅句柄 - 用于取消订阅
pub struct Subscription {
    id: String,
}

impl Subscription {
    pub fn cancel(&self) -> Result<()> {
        // 取消订阅
        Ok(())
    }
}
```

---

### 内存事件总线实现

```rust
// event_bus/memory.rs

use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};

pub struct MemoryEventBus {
    /// topic -> 订阅者列表
    subscribers: Arc<RwLock<HashMap<String, Vec<mpsc::UnboundedSender<EventWrapper>>>>>,
    /// 事件历史 (用于重放)
    history: Arc<RwLock<Vec<(DateTime<Utc>, EventWrapper)>>>,
}

impl MemoryEventBus {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl EventBus for MemoryEventBus {
    async fn publish<E: DomainEvent>(&self, event: E) -> Result<()> {
        // 包装事件
        let wrapper = match event.event_type() {
            "room.message" => EventWrapper::RoomMessage(
                serde_json::from_str(&event.to_json()?)?
            ),
            "skill.execute" => EventWrapper::SkillExecute(
                serde_json::from_str(&event.to_json()?)?
            ),
            // ... 更多映射
            _ => return Err(Error::unknown_event_type(event.event_type())),
        };
        
        // 记录历史
        self.history.write().await.push((Utc::now(), wrapper.clone()));
        
        // 分发给订阅者
        let topic = event.event_type();
        let subscribers = self.subscribers.read().await;
        
        if let Some(subs) = subscribers.get(topic) {
            for sender in subs {
                let _ = sender.send(wrapper.clone());
            }
        }
        
        Ok(())
    }
    
    async fn subscribe<F, E>(&self, handler: F) -> Result<Subscription>
    where
        F: EventHandler<E>,
        E: DomainEvent,
    {
        let topic = E::default().event_type().to_string();
        let (tx, mut rx) = mpsc::unbounded_channel();
        
        // 添加到订阅列表
        self.subscribers
            .write()
            .await
            .entry(topic)
            .or_insert_with(Vec::new)
            .push(tx);
        
        // 启动处理任务
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                // 反序列化并处理
                if let Ok(concrete) = serde_json::from_str::<E>(
                    &serde_json::to_string(&event).unwrap()
                ) {
                    let _ = handler.handle(concrete).await;
                }
            }
        });
        
        Ok(Subscription { id: uuid::Uuid::new_v4().to_string() })
    }
}
```

---

### 改造 Matrix Bridge

```rust
// 修改前 - 直接调用
pub struct MatrixBridge {
    // ...
}

impl MatrixBridge {
    pub async fn on_room_event(&self, event: RoomEvent) -> Result<()> {
        let skill_manager = SkillManager::new()?;
        skill_manager.execute(event.skill_name, event).await?;
        Ok(())
    }
}

// 修改后 - 事件驱动
pub struct MatrixBridge {
    event_bus: Arc<dyn EventBus>,
}

impl MatrixBridge {
    pub fn new(event_bus: Arc<dyn EventBus>) -> Self {
        Self { event_bus }
    }
    
    pub async fn on_room_event(&self, event: RoomEvent) -> Result<()> {
        // 发布事件，而非直接调用
        self.event_bus.publish(RoomMessageEvent {
            event_id: event.id,
            timestamp: Utc::now(),
            room_id: event.room_id,
            sender: event.sender,
            content: event.content,
        }).await?;
        
        Ok(())
    }
}
```

### Skill 模块订阅事件

```rust
// skill/event_handler.rs

pub struct SkillEventHandler {
    executor: Arc<dyn SkillExecutor>,
}

impl SkillEventHandler {
    pub fn new(executor: Arc<dyn SkillExecutor>) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl EventHandler<RoomMessageEvent> for SkillEventHandler {
    async fn handle(&self, event: RoomMessageEvent) -> Result<()> {
        // 检查消息是否触发 Skill
        if let Some(skill_name) = self.extract_skill_name(&event.content) {
            // 发布 Skill 执行事件
            self.event_bus.publish(SkillExecuteEvent {
                event_id: uuid::Uuid::new_v4().to_string(),
                timestamp: Utc::now(),
                skill_name: skill_name.to_string(),
                method: "execute".to_string(),
                params: serde_json::to_vec(&event.content)?,
                requester: event.sender.clone(),
                context: ExecutionContext::from_room(&event),
            }).await?;
        }
        
        Ok(())
    }
}

#[async_trait]
impl EventHandler<SkillExecuteEvent> for SkillEventHandler {
    async fn handle(&self, event: SkillExecuteEvent) -> Result<()> {
        // 执行 Skill
        let result = self.executor.execute(
            &event.skill_name,
            &event.method,
            &event.params
        ).await?;
        
        // 发布完成事件
        self.event_bus.publish(SkillCompletedEvent {
            event_id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            original_event_id: event.event_id,
            skill_name: event.skill_name,
            result,
        }).await?;
        
        Ok(())
    }
}
```

---

## 事件流图

```
┌─────────────────────────────────────────────────────────────┐
│                        事件流                                │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐                                          │
│  │  Matrix Room │                                          │
│  └──────┬───────┘                                          │
│         │ RoomMessageEvent                                  │
│         ▼                                                  │
│  ┌──────────────────┐                                      │
│  │   Event Bus      │ ◄───────────────────────────────────┐│
│  └──────┬───────────┘                                    ││
│         │                                                  ││
│    ┌────┴────┬────────────┬──────────┐                    ││
│    │         │            │          │                    ││
│    ▼         ▼            ▼          ▼                    ││
│ ┌──────┐ ┌──────┐ ┌──────────┐ ┌──────────┐              ││
│ │Skill │ │Agent │ │ Federation│ │  Logging │              ││
│ └──┬───┘ └──┬───┘ └────┬─────┘ └──────────┘              ││
│    │        │          │                                  ││
│    │        │          │ FederationTaskEvent              ││
│    │        │          └──────────────────────────────────┼┘
│    │        │                                             │
│    ▼        ▼                                             │
│ SkillCompletedEvent                                       │
│    │                                                      │
│    └──────────────────────────────────────────────────────►│
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 事件路由表

| 事件类型 | 发布者 | 订阅者 | 说明 |
|---------|--------|--------|------|
| `room.message` | Matrix Bridge | Skill Handler | 触发 Skill 执行 |
| `skill.execute` | Skill Handler | Skill Executor | 执行具体 Skill |
| `skill.completed` | Skill Executor | Matrix Bridge, Logger | 返回结果 |
| `agent.online` | Agent | Federation | 节点发现 |
| `federation.task` | Federation | Agent Executor | 远程任务 |

---

## 任务清单

- [ ] 定义所有领域事件
- [ ] 实现 `EventBus` trait
- [ ] 实现 `MemoryEventBus`
- [ ] 改造 `MatrixBridge`
- [ ] 实现 `SkillEventHandler`
- [ ] 实现 `AgentEventHandler`
- [ ] 实现 `FederationEventHandler`
- [ ] 移除直接调用
- [ ] 更新测试

---

## 验收标准

```rust
// 测试 1: 事件发布和订阅
#[tokio::test]
async fn test_event_pub_sub() {
    let bus = Arc::new(MemoryEventBus::new());
    
    let received = Arc::new(AtomicBool::new(false));
    let received_clone = received.clone();
    
    // 订阅
    bus.subscribe(move |event: RoomMessageEvent| {
        received_clone.store(true, Ordering::SeqCst);
        async { Ok(()) }
    }).await.unwrap();
    
    // 发布
    bus.publish(RoomMessageEvent { ... }).await.unwrap();
    
    // 验证
    sleep(Duration::from_millis(100)).await;
    assert!(received.load(Ordering::SeqCst));
}

// 测试 2: 无直接调用
# 搜索直接调用模式
grep -r "SkillManager::new()" src/ | grep -v test | wc -l
# 预期: 0
```

---

## 依赖

- D02 全局状态消除

---

*设计创建日期: 2026-02-10*
