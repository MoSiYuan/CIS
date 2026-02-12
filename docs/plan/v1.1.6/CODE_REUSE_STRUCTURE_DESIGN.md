# CIS 代码复用结构设计文档

> **设计日期**: 2026-02-12
> **版本**: v1.1.6
> **目标**: 定义 CIS 模块拆分和代码复用的标准化结构

---

## 设计目标

### 核心原则

1. **单一职责（SRP）** - 每个模块只负责一个明确的功能
2. **接口隔离（ISP）** - 客户端不应依赖它不需要的接口
3. **依赖倒置（DIP）** - 高层模块不依赖低层模块，都依赖抽象
4. **开闭原则（OCP）** - 对扩展开放，对修改关闭

### 量化目标

| 指标 | 当前值 | 目标值 | 改进 |
|--------|--------|--------|------|
| 平均模块行数 | ~800 行 | <500 行 | -37% |
| 最大模块行数 | 3,439 行 | <1000 行 | -71% |
| 代码重复率 | ~15% | <5% | -67% |
| 测试覆盖率 | ~65% | >85% | +31% |
| 编译时间（增量） | ~120s | <60s | -50% |

---

## 1. 模块化架构设计

### 1.1 分层架构

```
┌─────────────────────────────────────────────────────────────┐
│                   应用层 (Application Layer)              │
│  ┌──────────┐  ┌──────────┐  ┌──────────────┐       │
│  │   CLI     │  │   GUI    │  │  Web API    │       │
│  └─────┬────┘  └─────┬────┘  └──────┬───────┘       │
│        │             │              │                    │
│        └─────────────┴──────────────┘                    │
│                      ▼                                 │
├─────────────────────────────────────────────────────────────┤
│                   服务层 (Service Layer)                  │
│  ┌───────────────────────────────────────────────┐       │
│  │         CIS Server (Unified API)             │       │
│  │  - 认证/授权                               │       │
│  │  - 请求路由                                │       │
│  │  - 响应格式化                              │       │
│  └───────────────────┬───────────────────────────┘       │
│                      │                                    │
│        ┌─────────────┼─────────────┐                    │
│        ▼             ▼             ▼                    │
│  ┌─────────┐  ┌─────────┐  ┌─────────────┐          │
│  │ DAG     │  │ Memory  │  │   P2P      │          │
│  │ Service │  │ Service │  │  Service    │          │
│  └────┬────┘  └────┬────┘  └──────┬──────┘          │
├───────┼────────────┼──────────────┼───────────────────┤
│                   │              │                    │
│        ┌──────────┴──────────────┴────┐             │
│        ▼                             ▼             │
│   ┌─────────────────────────────────────────┐          │
│   │      核心层 (Core Layer)             │          │
│   │  - Event Bus                       │          │
│   │  - Storage Abstraction              │          │
│   │  - Common Traits                  │          │
│   └─────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 模块职责划分

#### 应用层（Application Layer）

**职责**：用户交互和协议适配

- **CLI** - 命令行界面
  - 命令解析
  - 输入验证
  - 输出格式化
  - **关键**：仅调用 Server API，不实现业务逻辑

- **GUI** - 图形界面（Tauri/Electron）
  - 状态管理
  - UI 渲染
  - 用户交互
  - **关键**：仅调用 Server API

- **Web API** - HTTP/WebSocket 接口
  - RESTful API
  - WebSocket 推送
  - **关键**：远程 Agent、小程序访问

#### 服务层（Service Layer）

**职责**：业务逻辑和领域服务

- **DAG Service** - DAG 编排服务
  - DAG 解析和验证
  - 任务调度
  - 执行监控

- **Memory Service** - 记忆服务
  - 存储和检索
  - 语义搜索
  - 命名空间管理

- **P2P Service** - P2P 网络服务
  - 节点发现
  - 数据同步
  - 消息路由

#### 核心层（Core Layer）

**职责**：基础设施和通用能力

- **Event Bus** - 事件总线
  - 事件发布/订阅
  - 异步分发
  - 事件持久化

- **Storage Abstraction** - 存储抽象
  - 键值存储接口
  - 向量存储接口
  - 持久化实现

- **Common Traits** - 通用特性
  - 序列化/反序列化
  - 错误处理
  - 日志记录

---

## 2. 模块通信模式

### 2.1 事件驱动架构

#### 事件定义

```rust
// cis-core/src/events/mod.rs

use async_trait::async_trait;
use serde::{Serialize, Deserialize};

/// 事件 trait
pub trait Event: Send + Sync {
    /// 事件类型标识
    fn event_type(&self) -> &'static str;

    /// 事件负载（序列化）
    fn payload(&self) -> Result<Vec<u8>>;

    /// 事件时间戳
    fn timestamp(&self) -> DateTime<Utc>;
}

/// 事件处理器
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// 处理事件
    async fn handle(&self, event: Box<dyn Event>) -> Result<()>;

    /// 处理器名称
    fn name(&self) -> &str;
}
```

#### 事件总线实现

```rust
// cis-core/src/events/bus.rs

use tokio::sync::broadcast;
use std::sync::Arc;
use std::collections::HashMap;

pub struct EventBus {
    /// 通道发送器（每个事件类型一个）
    senders: Arc<RwLock<HashMap<String, broadcast::Sender<Box<dyn Event>>>>>,

    /// 通道容量
    channel_capacity: usize,
}

impl EventBus {
    pub fn new(channel_capacity: usize) -> Self {
        Self {
            senders: Arc::new(RwLock::new(HashMap::new())),
            channel_capacity,
        }
    }

    /// 发布事件
    pub async fn publish(&self, event: Box<dyn Event>) -> Result<()> {
        let event_type = event.event_type();
        let mut senders = self.senders.write().await;

        let sender = senders
            .entry(event_type.to_string())
            .or_insert_with(|| broadcast::channel(self.channel_capacity).0);

        // 发送到所有订阅者
        sender.send(event)
            .map_err(|e| CisError::event(format!("Failed to publish event: {}", e)))?;

        Ok(())
    }

    /// 订阅事件
    pub async fn subscribe(
        &self,
        event_type: &str,
    ) -> broadcast::Receiver<Box<dyn Event>> {
        let mut senders = self.senders.write().await;

        let sender = senders
            .entry(event_type.to_string())
            .or_insert_with(|| broadcast::channel(self.channel_capacity).0);

        sender.subscribe()
    }
}
```

#### 事件使用示例

```rust
// 定义事件
#[derive(Serialize, Deserialize)]
pub struct TaskCompletedEvent {
    pub task_id: String,
    pub result: TaskResult,
    pub timestamp: DateTime<Utc>,
}

impl Event for TaskCompletedEvent {
    fn event_type(&self) -> &'static str {
        "task.completed"
    }

    fn payload(&self) -> Result<Vec<u8>> {
        bincode::serialize(&self)
            .map_err(|e| CisError::serialization(format!("Failed to serialize event: {}", e)))
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
}

// 发布事件
let event = TaskCompletedEvent {
    task_id: "task-123".to_string(),
    result: TaskResult::Success,
    timestamp: Utc::now(),
};
event_bus.publish(Box::new(event)).await?;

// 订阅事件
let mut receiver = event_bus.subscribe("task.completed").await;
tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        handler.handle(event).await?;
    }
    Ok::<(), Error>(())
});
```

### 2.2 依赖注入模式

#### 服务定位器（Service Locator）

```rust
// cis-core/src/services/mod.rs

use std::sync::Arc;
use std::collections::HashMap;
use async_trait::async_trait;

/// 服务 trait（标记服务接口）
pub trait Service: Send + Sync {
    fn service_name(&self) -> &str;
}

/// 服务容器
pub struct ServiceContainer {
    services: Arc<RwLock<HashMap<String, Arc<dyn Service>>>>,
}

impl ServiceContainer {
    pub fn new() -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册服务
    pub async fn register<T: Service + 'static>(&self, service: Arc<T>) {
        let name = service.service_name().to_string();
        let mut services = self.services.write().await;
        services.insert(name, service);
    }

    /// 获取服务
    pub async fn get<T: Service + 'static>(&self, name: &str) -> Result<Arc<T>> {
        let services = self.services.read().await;

        let service = services
            .get(name)
            .ok_or_else(|| CisError::service_not_found(name))?;

        // 尝试 downcast
        service
            .clone()
            .downcast::<T>()
            .map_err(|_| CisError::service_type_mismatch(name))
    }
}

/// 简化的服务获取宏
macro_rules! get_service {
    ($container:expr, $type:ty) => {{
        $container
            .get::<$type>($type::SERVICE_NAME)
            .await?
    }};
}
```

#### 服务定义示例

```rust
// cis-core/src/memory/service.rs

use async_trait::async_trait;

#[async_trait]
pub trait MemoryService: Service {
    /// 获取记忆
    async fn get(&self, key: &str) -> Result<Option<MemoryItem>>;

    /// 设置记忆
    async fn set(&self, key: &str, value: Vec<u8>, domain: MemoryDomain) -> Result<()>;

    /// 语义搜索
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>>;
}

impl Service for dyn MemoryService {
    fn service_name(&self) -> &str {
        "memory"
    }
}

// 实现示例
pub struct SqliteMemoryService {
    db: Arc<SqliteConnection>,
}

#[async_trait]
impl MemoryService for SqliteMemoryService {
    async fn get(&self, key: &str) -> Result<Option<MemoryItem>> {
        // 实现细节...
    }

    async fn set(&self, key: &str, value: Vec<u8>, domain: MemoryDomain) -> Result<()> {
        // 实现细节...
    }

    async fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryItem>> {
        // 实现细节...
    }
}

impl Service for SqliteMemoryService {
    fn service_name(&self) -> &str {
        "memory"
    }
}
```

---

## 3. 核心模块拆分设计

### 3.1 Scheduler 模块拆分

#### 当前问题
- **scheduler/mod.rs** (3,439 行) - 包含整个 DAG 调度系统
- 职责过多：DAG 定义、验证、执行、持久化、通知

#### 目标结构

```
cis-core/src/scheduler/
├── mod.rs                    # 模块导出（~100 行）
│
├── core/                     # 核心 DAG 定义（~300 行）
│   ├── mod.rs
│   ├── dag.rs               # TaskDag, UnifiedDag
│   ├── task.rs              # 任务定义
│   ├── validator.rs         # DAG 验证（循环检测）
│   └── topo_sort.rs         # 拓扑排序
│
├── execution/               # 执行器（~400 行）
│   ├── mod.rs
│   ├── executor.rs          # Executor trait
│   ├── local.rs            # LocalExecutor
│   ├── multi_agent.rs       # MultiAgentExecutor
│   └── skill.rs           # SkillExecutor
│
├── events/                 # 事件系统（~300 行）
│   ├── mod.rs
│   ├── bus.rs              # 事件总线
│   ├── dispatcher.rs       # 事件分发
│   └── types.rs           # 事件类型
│
├── persistence/            # 持久化（~250 行）
│   ├── mod.rs
│   ├── store.rs           # 持久化存储
│   └── recovery.rs        # 状态恢复
│
├── notification/          # 通知系统（~250 行）
│   ├── mod.rs
│   ├── notifier.rs        # 通知器
│   └── channels.rs        # 通知通道
│
└── converters/           # 转换器（已存在，~826 行）
    └── mod.rs
```

#### 核心接口

```rust
// cis-core/src/scheduler/core/dag.rs

/// DAG trait（支持多种实现）
pub trait Dag: Send + Sync {
    /// 验证 DAG
    fn validate(&self) -> Result<()>;

    /// 获取所有任务
    fn tasks(&self) -> Vec<&Task>;

    /// 获取依赖关系
    fn dependencies(&self, task_id: &str) -> Vec<&str>;

    /// 拓扑排序
    fn topological_sort(&self) -> Result<Vec<String>>;
}

/// Executor trait
#[async_trait]
pub trait Executor: Send + Sync {
    /// 执行 DAG
    async fn execute(&self, dag: Box<dyn Dag>) -> Result<ExecutionReport>;

    /// 取消执行
    async fn cancel(&self, execution_id: &str) -> Result<()>;

    /// 查询状态
    async fn status(&self, execution_id: &str) -> Result<ExecutionStatus>;
}
```

### 3.2 P2P 模块拆分

#### 当前问题
- **p2p/transport_secure.rs** (1,536 行) - 混合了加密、认证、传输
- **p2p/network.rs** (1,097 行) - 混合了连接、消息、事件
- **p2p/nat.rs** (936 行) - STUN/UPnP/TURN 混在一起

#### 目标结构

```
cis-core/src/p2p/
├── mod.rs                     # 模块导出（~150 行）
│
├── transport/                # 传输层（~600 行）
│   ├── mod.rs
│   ├── secure.rs           # 安全传输（精简）
│   ├── crypto.rs          # 加密原语
│   ├── handshake.rs       # 握手协议
│   └── quic.rs           # QUIC 实现
│
├── nat/                     # NAT 穿透（~400 行）
│   ├── mod.rs
│   ├── stun.rs            # STUN 协议
│   ├── upnp.rs            # UPnP 协议
│   ├── turn.rs            # TURN 协议
│   └── detector.rs        # NAT 类型检测
│
├── discovery/              # 节点发现（~300 行）
│   ├── mod.rs
│   ├── mdns.rs           # mDNS 发现
│   ├── dht.rs            # DHT 发现
│   └── bootstrap.rs       # Bootstrap 节点
│
├── connection/             # 连接管理（~350 行）
│   ├── mod.rs
│   ├── manager.rs         # 连接管理器
│   ├── pool.rs           # 连接池
│   └── health.rs         # 健康检查
│
├── messaging/             # 消息系统（~300 行）
│   ├── mod.rs
│   ├── router.rs         # 消息路由
│   ├── codec.rs          # 消息编解码
│   └── queue.rs         # 消息队列
│
├── sync/                 # P2P 同步（~400 行）
│   ├── mod.rs
│   ├── protocol.rs       # 同步协议
│   ├── crdt.rs          # CRDT 实现
│   └── conflict.rs      # 冲突解决
│
└── config/               # P2P 配置（~300 行）
    ├── mod.rs
    ├── discovery.rs      # 发现配置
    ├── transport.rs      # 传输配置
    └── nat.rs           # NAT 配置
```

### 3.3 Config 模块拆分

#### 当前问题
- **config/loader.rs** (1,034 行) - 所有加载逻辑混在一起
- 各个子配置文件（p2p.rs 916 行, security.rs 648 行等）职责不清

#### 目标结构

```
cis-core/src/config/
├── mod.rs                     # 模块导出（~100 行）
│
├── loader/                 # 加载器（~400 行）
│   ├── mod.rs
│   ├── file.rs            # 文件加载
│   ├── parser.rs          # TOML/YAML 解析
│   ├── validator.rs       # 配置验证
│   └── defaults.rs       # 默认值
│
├── source/                 # 配置源（~300 行）
│   ├── mod.rs
│   ├── file.rs            # 文件源
│   ├── env.rs            # 环境变量源
│   └── cli.rs            # CLI 参数源
│
├── merge/                  # 配置合并（~200 行）
│   ├── mod.rs
│   └── strategy.rs        # 合并策略
│
├── p2p/                    # P2P 配置（~400 行）
│   ├── mod.rs
│   ├── discovery.rs       # 发现配置
│   ├── dht.rs            # DHT 配置
│   └── transport.rs      # 传输配置
│
├── security/               # 安全配置（~300 行）
│   ├── mod.rs
│   ├── encryption.rs     # 加密配置
│   ├── keys.rs          # 密钥配置
│   └── acl.rs           # ACL 配置
│
└── wasm/                    # WASM 配置（~300 行）
    ├── mod.rs
    ├── sandbox.rs        # 沙箱配置
    ├── limits.rs         # 资源限制
    └── permissions.rs   # 权限配置
```

---

## 4. 跨模块复用策略

### 4.1 共享工具库

```
cis-core/src/utils/
├── mod.rs                    # 工具导出
│
├── crypto/                 # 加密工具
│   ├── mod.rs
│   ├── cipher.rs         # 对称加密
│   ├── asymmetric.rs     # 非对称加密
│   └── hash.rs          # 哈希函数
│
├── serialization/        # 序列化工具
│   ├── mod.rs
│   ├── binary.rs         # 二进制序列化
│   └── json.rs          # JSON 序列化
│
├── validation/           # 验证工具
│   ├── mod.rs
│   ├── schema.rs         # Schema 验证
│   └── constraints.rs   # 约束验证
│
├── async_utils/        # 异步工具
│   ├── mod.rs
│   ├── timeout.rs       # 超时控制
│   ├── cancellation.rs # 取消令牌
│   └── retry.rs         # 重试逻辑
│
└── metrics/             # 指标收集
    ├── mod.rs
    ├── counter.rs       # 计数器
    ├── histogram.rs     # 直方图
    └── gauge.rs         # 仪表
```

### 4.2 错误处理统一

```rust
// cis-core/src/error/mod.rs

use thiserror::Error;

/// 统一的 CIS 错误类型
#[derive(Error, Debug)]
pub enum CisError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Service type mismatch: {0}")]
    ServiceTypeMismatch(String),
}

/// 便利宏
macro_rules! impl_error {
    ($name:ident, $variant:ident) => {
        impl From<$name> for CisError {
            fn from(e: $name) -> Self {
                CisError::$variant(e.to_string())
            }
        }
    };
}
```

### 4.3 日志标准化

```rust
// cis-core/src/logging/mod.rs

use tracing::{info, warn, error, debug};

/// 结构化日志宏
macro_rules! log_event {
    ($level:ident, $event_type:expr, $data:expr) => {{
        tracing::$level!(
            event_type = $event_type,
            ?data,
            "CIS event"
        );
    }};
}

/// 性能日志宏
macro_rules! log_perf {
    ($operation:expr, $duration:expr) => {{
        tracing::info!(
            operation = $operation,
            duration_ms = $duration.as_millis(),
            "Performance metric"
        );
    }};
}
```

---

## 5. Server API 设计

### 5.1 统一 Server 接口

```rust
// cis-core/src/server/mod.rs

use async_trait::async_trait;
use serde::{Serialize, Deserialize};

/// 请求 trait
pub trait Request: Send + Sync {
    fn request_type(&self) -> &'static str;
}

/// 响应 trait
pub trait Response: Send + Sync {
    fn status_code(&self) -> u16;
}

/// Server API trait
#[async_trait]
pub trait ServerApi: Send + Sync {
    /// 处理请求
    async fn handle(&self, request: Box<dyn Request>) -> Result<Box<dyn Response>>;

    /// 健康检查
    async fn health_check(&self) -> Result<HealthStatus>;

    /// 关闭服务器
    async fn shutdown(&self) -> Result<()>;
}
```

### 5.2 CLI/GUI/远程 API 使用

```rust
// CLI 示例（仅调用 Server API）
pub async fn handle_init(ctx: &CliContext, args: InitArgs) -> Result<()> {
    let request = InitProjectRequest {
        path: args.path,
        name: args.name,
        force: args.force,
    };

    let response = ctx
        .server
        .handle(Box::new(request))
        .await?;

    match response.status_code() {
        200 => println!("✓ Project initialized"),
        409 => println!("✗ Project already exists"),
        _ => println!("✗ Failed to initialize"),
    }

    Ok(())
}

// GUI 示例（仅调用 Server API）
#[tauri::command]
pub async fn gui_init_project(
    server: State<'_, Arc<dyn ServerApi>>,
    path: String,
    name: String,
) -> Result<String, String> {
    let request = InitProjectRequest { path, name };
    let response = server
        .handle(Box::new(request))
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::to_string(&response).unwrap())
}

// 远程 API 示例（仅调用 Server API）
pub async fn api_init_project(
    State(server): State<Arc<dyn ServerApi>>,
    Json(request): Json<InitProjectRequest>,
) -> Result<Json<InitProjectResponse>, StatusCode> {
    let response = server
        .handle(Box::new(request))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(*response.downcast::<InitProjectResponse>().unwrap()))
}
```

---

## 6. 实施路线图

### 阶段 1: 基础设施（Week 1-2）

**目标**：建立模块化基础设施

| 任务 | 工作量 | 负责团队 | 优先级 |
|------|--------|----------|--------|
| 实现事件总线 | 3 人日 | Team Q | P0 |
| 实现服务容器 | 2 人日 | Team Q | P0 |
| 统一错误处理 | 2 人日 | Team Q | P0 |
| 标准化日志 | 1 人日 | Team Q | P0 |

### 阶段 2: 核心拆分（Week 3-5）

**目标**：拆分最关键模块

| 任务 | 工作量 | 负责团队 | 优先级 |
|------|--------|----------|--------|
| scheduler 拆分 | 10-15 人日 | Team Q | P0 |
| config/loader 拆分 | 3-5 人日 | Team R | P0 |
| p2p/transport 拆分 | 5-7 人日 | Team S | P1 |

### 阶段 3: 次要拆分（Week 6-7）

**目标**：拆分其余模块

| 任务 | 工作量 | 负责团队 | 优先级 |
|------|--------|----------|--------|
| skill/chain 拆分 | 4-6 人日 | Team T | P1 |
| skill/router 拆分 | 4-6 人日 | Team T | P1 |
| skill/manager 拆分 | 3-4 人日 | Team T | P2 |

### 阶段 4: 验收和优化（Week 8）

**目标**：全面测试和文档

| 任务 | 工作量 | 负责团队 |
|------|--------|----------|
| 单元测试迁移 | 5-8 人日 | 全员 |
| 集成测试 | 3-5 人日 | QA |
| 性能基准 | 2-3 人日 | 性能团队 |
| 文档更新 | 2-3 人日 | 技术写作 |

---

## 7. 成功指标

### 代码质量

| 指标 | 当前 | 目标 | 测量方式 |
|------|------|------|----------|
| 平均模块行数 | ~800 行 | <500 行 | `tokei` |
| 圈复杂度 | ~15 | <10 | `cargo-geiger` |
| 代码重复率 | ~15% | <5% | `cargo-detect-dupes` |
| 测试覆盖率 | ~65% | >85% | `tarpaulin` |

### 性能

| 指标 | 当前 | 目标 | 测量方式 |
|------|------|------|----------|
| 增量编译时间 | ~120s | <60s | `cargo build --timings` |
| 完整编译时间 | ~480s | <300s | `hyperfine` |
| 单元测试时间 | ~180s | <120s | `cargo test --timings` |
| 内存占用 | ~150MB | <200MB | `heaptrack` |

### 开发效率

| 指标 | 当前 | 目标 | 测量方式 |
|------|------|------|----------|
| 新功能开发周期 | 5-7 天 | 3-5 天 | JIRA 统计 |
| Bug 修复时间 | 2-3 天 | 1-2 天 | JIRA 统计 |
| 代码审查时间 | 4-6 小时 | 2-3 小时 | GitHub 统计 |
| 文档完整性 | ~60% | >90% | 文档覆盖率工具 |

---

## 8. 风险缓解

### 技术风险

| 风险 | 缓解措施 | 负责人 |
|------|----------|--------|
| 拆分后性能下降 | 性能基准测试对比 | 性能团队 |
| 循环依赖 | 使用 `cargo-depends` 检测 | 架构师 |
| API 破坏 | 版本化 API + 兼容层 | API 设计师 |
| 测试不足 | 强制 80% 覆盖率门禁 | QA |

### 业务风险

| 风险 | 缓解措施 | 负责人 |
|------|----------|--------|
| 开发周期延长 | 分阶段上线，保留旧代码 | PM |
| 团队适应困难 | 培训 + 代码示例 | 技术主管 |
| 文档滞后 | 并行编写文档 | 技术写作 |

---

## 9. 下一步行动

### 立即执行（今天）

1. ✅ 完成代码复用结构设计文档
2. ⏳ 优化 CLI 引导文档
3. ⏳ 制定详细执行计划

### 本周执行

1. 启动基础设施团队（Team Q）
   - 实现事件总线
   - 实现服务容器
   - 统一错误处理

2. 优化 CLI 文档
   - 更新架构说明
   - 强调 Server API 优先
   - 添加最佳实践

3. 启动 scheduler 拆分（Team Q）
   - 创建 scheduler/core/
   - 创建 scheduler/execution/
   - 创建 scheduler/events/

---

**文档版本**: 1.0
**设计完成日期**: 2026-02-12
**作者**: CIS Architecture Team
**审核状态**: 待审核
