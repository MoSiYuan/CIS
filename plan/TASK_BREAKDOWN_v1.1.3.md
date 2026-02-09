# CIS v1.1.3 细粒度任务拆分计划

> **目标**: 将 CIS 真实实现工作拆分为独立的、可并行的子任务
> **原则**: 每个任务独立可执行，通过明确接口交互，无隐藏上下文依赖

---

## 架构总览

```
┌─────────────────────────────────────────────────────────────────┐
│                        CIS v1.1.3 架构                            │
├─────────────────────────────────────────────────────────────────┤
│  Layer 4: CLI Commands (cis-node/src/commands/)                  │
│     ├─ p2p, matrix, agent, network, session, worker             │
├─────────────────────────────────────────────────────────────────┤
│  Layer 3: Core Services (cis-core/src/)                          │
│     ├─ p2p/, agent/, matrix/, network/, storage/                │
├─────────────────────────────────────────────────────────────────┤
│  Layer 2: Infrastructure (cis-core/src/)                         │
│     ├─ transport/, wasm/, ai/, vector/, scheduler/              │
├─────────────────────────────────────────────────────────────────┤
│  Layer 1: External Dependencies                                  │
│     ├─ mdns-sd, quinn, fastembed, tokio, axum                   │
└─────────────────────────────────────────────────────────────────┘
```

---

## 任务依赖图

```
Phase 1: 基础设施 (Foundation)
┌─────────────────────────────────────────────────────────┐
│  T1.1: mDNS 服务封装                                      │
│     ↓ (提供 DiscoveryService trait)                      │
│  T1.2: QUIC 传输层实现                                     │
│     ↓ (提供 Transport trait)                             │
│  T1.3: PID 文件管理库                                      │
└─────────────────────────────────────────────────────────┘
                           ↓
Phase 2: 核心服务 (Core Services)
┌─────────────────────────────────────────────────────────┐
│  T2.1: P2P Network 状态管理                                │
│     ↓ (使用 T1.1, T1.2)                                  │
│  T2.2: Matrix Server 生命周期管理                          │
│     ↓ (使用 T1.3)                                        │
│  T2.3: Agent 进程检测器                                    │
└─────────────────────────────────────────────────────────┘
                           ↓
Phase 3: CLI 集成 (CLI Integration)
┌─────────────────────────────────────────────────────────┐
│  T3.1: p2p discover 命令 (真实实现)                       │
│  T3.2: p2p connect/disconnect 命令                       │
│  T3.3: matrix start/stop/status 命令                     │
│  T3.4: agent status 命令 (进程检测)                       │
└─────────────────────────────────────────────────────────┘
                           ↓
Phase 4: 高级功能 (Advanced)
┌─────────────────────────────────────────────────────────┐
│  T4.1: DHT 真实操作                                       │
│  T4.2: Federation 事件发送                                │
│  T4.3: Embedding 服务替换                                 │
└─────────────────────────────────────────────────────────┘
```

---

## Phase 1: 基础设施任务

### T1.1: mDNS 服务封装
**优先级**: P0 | **预估时间**: 4h | **依赖**: 无

**任务描述**:
封装 `mdns-sd` 库，提供简洁的 mDNS 服务发现和广播接口。

**输入**:
- 文件: `cis-core/src/p2p/discovery.rs` (已有基础结构)
- crate: `mdns-sd = "0.10"` (已在 Cargo.toml)

**输出规范**:
```rust
// 必须实现的公共接口
pub struct MdnsService {
    daemon: ServiceDaemon,
    service_type: String,
}

impl MdnsService {
    /// 创建并启动 mDNS 服务
    pub fn new(node_id: &str, port: u16, metadata: HashMap<String, String>) -> Result<Self>;
    
    /// 发现同网段服务
    pub fn discover(&self, timeout: Duration) -> Result<Vec<DiscoveredNode>>;
    
    /// 停止服务
    pub fn shutdown(self);
}

pub struct DiscoveredNode {
    pub node_id: String,
    pub address: SocketAddr,
    pub did: String,
    pub metadata: HashMap<String, String>,
}
```

**验收标准**:
1. 两台同一局域网的机器可以互相发现
2. 发现超时后返回空列表（不 panic）
3. 服务停止后资源正确释放
4. 单测覆盖率 > 80%

**测试命令**:
```bash
cargo test --package cis-core mdns -- --nocapture
```

---

### T1.2: QUIC 传输层实现
**优先级**: P0 | **预估时间**: 6h | **依赖**: 无

**任务描述**:
实现基于 QUIC 的 P2P 传输层，支持连接管理和消息传输。

**输入**:
- 文件: `cis-core/src/p2p/transport.rs` (已有基础)
- crate: `quinn = "0.11"` (已配置)

**输出规范**:
```rust
pub struct QuicTransport {
    endpoint: Endpoint,
    connections: Arc<Mutex<HashMap<String, Connection>>>,
}

impl QuicTransport {
    /// 绑定到本地地址
    pub async fn bind(addr: SocketAddr) -> Result<Self>;
    
    /// 连接到远程节点
    pub async fn connect(&self, node_id: &str, addr: SocketAddr) -> Result<Connection>;
    
    /// 断开连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()>;
    
    /// 获取连接列表
    pub fn list_connections(&self) -> Vec<ConnectionInfo>;
    
    /// 发送消息
    pub async fn send(&self, node_id: &str, data: &[u8]) -> Result<()>;
}

pub struct ConnectionInfo {
    pub node_id: String,
    pub address: SocketAddr,
    pub connected_at: Instant,
    pub rtt_ms: u64,
}
```

**验收标准**:
1. 本地回环测试通过 (127.0.0.1:0)
2. 支持并发 100+ 连接
3. 连接断开后能正确清理资源
4. 提供连接状态查询接口

---

### T1.3: PID 文件管理库
**优先级**: P0 | **预估时间**: 3h | **依赖**: 无

**任务描述**:
实现跨平台的 PID 文件管理，用于守护进程管理。

**输出规范**:
```rust
pub struct PidManager {
    pid_file: PathBuf,
}

impl PidManager {
    /// 创建 PID 管理器
    pub fn new(name: &str) -> Self;
    
    /// 写入当前进程 PID
    pub fn write(&self) -> Result<()>;
    
    /// 读取 PID 并检查进程是否存在
    pub fn read(&self) -> Result<Option<u32>>;
    
    /// 检查进程是否运行
    pub fn is_running(&self) -> bool;
    
    /// 发送信号给管理进程
    pub fn signal(&self, sig: Signal) -> Result<()>;
    
    /// 清理 PID 文件
    pub fn cleanup(&self) -> Result<()>;
}

pub enum Signal {
    Term,  // SIGTERM
    Kill,  // SIGKILL
    Hup,   // SIGHUP
}
```

**平台支持**:
- Linux: `/run/user/{uid}/{name}.pid` 或 `~/.local/run/{name}.pid`
- macOS: `~/Library/Run/{name}.pid`

**验收标准**:
1. 写入后能正确读取 PID
2. 进程不存在时返回 None
3. 支持优雅关闭 (SIGTERM) 和强制关闭 (SIGKILL)
4. Drop 时自动清理（可选）

---

## Phase 2: 核心服务任务

### T2.1: P2P Network 状态管理
**优先级**: P1 | **预估时间**: 5h | **依赖**: T1.1, T1.2

**任务描述**:
实现全局 P2P 网络状态管理，支持启动/停止/状态查询。

**输入**:
- 文件: `cis-core/src/p2p/mod.rs`
- 依赖任务: T1.1 (MdnsService), T1.2 (QuicTransport)

**输出规范**:
```rust
pub static P2P_NETWORK: OnceCell<RwLock<Option<Arc<P2PNetwork>>>> = OnceCell::new();

pub struct P2PNetwork {
    mdns: MdnsService,
    transport: QuicTransport,
    peer_manager: PeerManager,
    local_node: NodeInfo,
}

impl P2PNetwork {
    /// 全局单例获取
    pub async fn global() -> Option<Arc<Self>>;
    
    /// 初始化并启动
    pub async fn start(config: P2PConfig) -> Result<Arc<Self>>;
    
    /// 停止并清理
    pub async fn stop() -> Result<()>;
    
    /// 获取运行状态
    pub fn status() -> P2PStatus;
    
    /// 获取发现的节点列表
    pub async fn discovered_peers(&self) -> Vec<PeerInfo>;
    
    /// 连接到指定节点
    pub async fn connect(&self, addr: &str) -> Result<()>;
    
    /// 断开连接
    pub async fn disconnect(&self, node_id: &str) -> Result<()>;
}

pub struct P2PStatus {
    pub running: bool,
    pub listen_addr: Option<SocketAddr>,
    pub connected_peers: usize,
    pub discovered_peers: usize,
}
```

**验收标准**:
1. 多次调用 start 返回相同实例
2. stop 后资源完全释放
3. 状态信息实时准确
4. 支持并发访问（线程安全）

---

### T2.2: Matrix Server 生命周期管理
**优先级**: P1 | **预估时间**: 4h | **依赖**: T1.3

**任务描述**:
实现 Matrix Server 的真实启动、停止和状态检测。

**输入**:
- 文件: `cis-core/src/matrix/server.rs`
- 依赖任务: T1.3 (PidManager)

**输出规范**:
```rust
pub struct MatrixServerManager {
    pid_manager: PidManager,
    config: MatrixConfig,
}

impl MatrixServerManager {
    pub fn new(config: MatrixConfig) -> Self;
    
    /// 启动 Matrix 服务（阻塞直到启动成功或失败）
    pub async fn start(&self) -> Result<ServerHandle>;
    
    /// 停止服务
    pub async fn stop(&self) -> Result<()>;
    
    /// 获取状态
    pub fn status(&self) -> ServerStatus;
    
    /// 重启服务
    pub async fn restart(&self) -> Result<ServerHandle>;
}

pub struct ServerStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub listen_addr: SocketAddr,
    pub uptime_secs: Option<u64>,
}

pub struct ServerHandle {
    pub pid: u32,
    pub port: u16,
    pub shutdown: Sender<()>,
}
```

**验收标准**:
1. start 后 PID 文件正确写入
2. status 能准确检测运行状态
3. stop 发送 SIGTERM，超时后 SIGKILL
4. 端口被占用时返回明确错误

---

### T2.3: Agent 进程检测器
**优先级**: P1 | **预估时间**: 4h | **依赖**: 无

**任务描述**:
实现真实的 Agent 进程检测，支持 Claude/OpenCode。

**输入**:
- 文件: `cis-core/src/agent/persistent/`

**输出规范**:
```rust
pub struct AgentProcessDetector;

impl AgentProcessDetector {
    /// 检测指定类型的 Agent 进程
    pub fn detect(agent_type: AgentType) -> Vec<AgentProcessInfo>;
    
    /// 通过 PID 检查 Agent 是否运行
    pub fn is_running(pid: u32) -> bool;
    
    /// 获取 Agent 的活跃会话
    pub fn get_sessions(agent_type: AgentType) -> Vec<AgentSession>;
    
    /// 通过端口检测服务
    pub fn check_port(addr: SocketAddr) -> bool;
}

pub struct AgentProcessInfo {
    pub pid: u32,
    pub agent_type: AgentType,
    pub command: String,
    pub working_dir: PathBuf,
    pub start_time: SystemTime,
    pub port: Option<u16>,
}

pub enum AgentType {
    Claude,
    OpenCode,
    Kimi,
}
```

**实现要求**:
- macOS: 使用 `ps` 命令或 `sysinfo` crate
- Linux: 读取 `/proc/{pid}/cmdline`
- 通过进程名和命令行参数识别 Agent 类型

**验收标准**:
1. 能正确识别运行中的 claude/opencode 进程
2. 返回准确的 PID、启动时间、工作目录
3. 对非 Agent 进程返回空列表
4. 单测模拟不同进程场景

---

## Phase 3: CLI 集成任务

### T3.1: p2p discover 命令真实实现
**优先级**: P1 | **预估时间**: 3h | **依赖**: T2.1

**任务描述**:
替换 `cis-node/src/commands/p2p.rs` 中的模拟发现代码。

**输入**:
- 文件: `cis-node/src/commands/p2p.rs:298-347`
- 依赖任务: T2.1 (P2PNetwork)

**实现要求**:
```rust
async fn discover_nodes(timeout_secs: u64, verbose: bool) -> Result<()> {
    // 获取全局 P2P 网络实例
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    
    // 触发发现
    let peers = network.discovered_peers().await;
    
    // 显示结果（不再有硬编码节点）
    println!("Discovered {} nodes:", peers.len());
    for peer in peers {
        println!("  • {}", peer.node_id);
        println!("    Address: {}", peer.address);
    }
    
    Ok(())
}
```

**验收标准**:
1. 无硬编码的 node-abc123/node-def456
2. 真实发现同网段节点
3. 超时后正确返回
4. 网络未启动时给出明确错误

---

### T3.2: p2p connect/disconnect 命令
**优先级**: P1 | **预估时间**: 3h | **依赖**: T2.1

**任务描述**:
实现真实的节点连接和断开。

**输入**:
- 文件: `cis-node/src/commands/p2p.rs:380-458`

**实现要求**:
```rust
async fn connect_node(address: &str, node_id: Option<&str>) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    
    network.connect(address).await?;
    println!("✅ Connected to {}", address);
    Ok(())
}

async fn disconnect_node(node_id: &str) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P network not started"))?;
    
    network.disconnect(node_id).await?;
    println!("✅ Disconnected from {}", node_id);
    Ok(())
}
```

**验收标准**:
1. 连接真实建立 QUIC 连接
2. 断开后连接资源释放
3. 连接失败时返回具体错误（网络不可达、拒绝连接等）
4. 重复连接处理（幂等或报错）

---

### T3.3: matrix start/stop/status 命令
**优先级**: P1 | **预估时间**: 4h | **依赖**: T2.2

**任务描述**:
替换 Matrix 命令的 TODO 和模拟实现。

**输入**:
- 文件: `cis-node/src/commands/matrix.rs`
- 依赖任务: T2.2 (MatrixServerManager)

**实现要求**:
```rust
// start_matrix_server 函数修改
async fn start_matrix_server(port: u16, daemon: bool, launch: bool) -> Result<()> {
    let manager = MatrixServerManager::new(MatrixConfig { port, .. });
    
    match manager.status() {
        ServerStatus { running: true, .. } => {
            println!("⚠️  Matrix server already running");
            return Ok(());
        }
        _ => {}
    }
    
    let handle = manager.start().await?;
    println!("✅ Matrix server started on port {}", port);
    
    if daemon {
        // 后台模式：直接返回
        return Ok(());
    }
    
    // 前台模式：等待 shutdown 信号
    handle.shutdown.await?;
    Ok(())
}

// stop_matrix_server 函数修改
async fn stop_matrix_server() -> Result<()> {
    let manager = MatrixServerManager::default();
    manager.stop().await?;
    println!("✅ Matrix server stopped");
    Ok(())
}

// show_matrix_status 函数修改
async fn show_matrix_status() -> Result<()> {
    let manager = MatrixServerManager::default();
    let status = manager.status();
    
    println!("Status: {}", if status.running { "🟢 Running" } else { "🔴 Stopped" });
    if let Some(pid) = status.pid {
        println!("PID: {}", pid);
    }
    println!("Port: {}", status.listen_addr.port());
    Ok(())
}
```

**验收标准**:
1. start 后进程真实启动，PID 文件写入
2. stop 发送信号终止进程
3. status 显示真实状态（不是 "Unknown"）
4. 重复 start 给出提示不崩溃

---

### T3.4: agent status 命令（进程检测）
**优先级**: P2 | **预估时间**: 3h | **依赖**: T2.3

**任务描述**:
实现 Agent 状态的真实检测。

**输入**:
- 文件: `cis-node/src/commands/agent.rs` (如存在) 或新增
- 依赖任务: T2.3 (AgentProcessDetector)

**输出规范**:
```bash
$ cis agent status

📊 Agent Status
═══════════════

Claude:
  🟢 Running (PID: 12345)
  📁 Working dir: /Users/xxx/.cis/agents/claude-xxx
  ⏱️  Started: 2026-02-09 10:00:00
  
OpenCode:
  🔴 Not running
  💡 Start with: cis agent start opencode

Kimi:
  🟡 Stale (PID: 12340, process not found)
  ⚠️  Clean up recommended
```

**验收标准**:
1. 显示真实运行的 Agent
2. 僵尸进程（PID 存在但进程已死）标记为 stale
3. 提供清理命令入口
4. 统计信息准确（启动时间、工作目录）

---

## Phase 4: 高级功能任务

### T4.1: DHT 真实操作
**优先级**: P2 | **预估时间**: 6h | **依赖**: T2.1

**任务描述**:
实现 DHT put/get/find_node 的真实操作。

**输入**:
- 文件: `cis-core/src/p2p/dht.rs`, `cis-node/src/commands/p2p.rs:760-840`

**输出规范**:
```rust
// DHT 命令实现
async fn dht_put(key: &str, value: &str) -> Result<()> {
    let network = P2PNetwork::global()
        .await
        .ok_or_else(|| anyhow!("P2P not started"))?;
    
    network.dht_put(key, value).await?;
    println!("✅ Stored '{}' in DHT", key);
    Ok(())
}

async fn dht_get(key: &str) -> Result<Option<String>> {
    let network = P2PNetwork::global().await?;
    match network.dht_get(key).await? {
        Some(value) => {
            println!("{}: {}", key, value);
            Ok(Some(value))
        }
        None => {
            println!("Key '{}' not found in DHT", key);
            Ok(None)
        }
    }
}
```

**验收标准**:
1. put 后 get 能获取相同值
2. 跨节点数据可检索
3. 路由表维护正确
4. 节点离线后数据仍可用（冗余存储）

---

### T4.2: Federation 事件发送
**优先级**: P2 | **预估时间**: 5h | **依赖**: T2.2

**任务描述**:
实现 Agent Federation 的真实 Matrix 事件发送。

**输入**:
- 文件: `cis-core/src/agent/federation/agent.rs:192,271,293`

**输出规范**:
```rust
impl FederationClient {
    /// 发送心跳事件到 Matrix Room
    pub async fn send_heartbeat(&self) -> Result<()>;
    
    /// 发送任务请求
    pub async fn send_task_request(&self, task: &TaskRequest) -> Result<String>;
    
    /// 订阅 Room 事件
    pub async fn subscribe_events(&self, callback: impl Fn(FederationEvent)) -> Result<()>;
}
```

**验收标准**:
1. 心跳事件真实发送到 Matrix Room
2. 其他节点能收到并处理
3. 断线后自动重连
4. 消息顺序保证

---

### T4.3: Embedding 服务替换
**优先级**: P2 | **预估时间**: 4h | **依赖**: 无

**任务描述**:
替换所有 mock embedding 实现，使用真实 fastembed。

**输入**:
- 文件: 
  - `cis-core/src/memory/service.rs:929`
  - `cis-core/src/task/vector.rs:415-421`
  - `cis-core/src/vector/storage.rs:1876-1882`
- crate: `fastembed = "4.0"` (已配置)

**输出规范**:
```rust
// 删除所有 mock embedding，统一使用
pub struct EmbeddingService {
    model: TextEmbedding,
}

impl EmbeddingService {
    pub async fn new() -> Result<Self> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::NomicEmbedTextV15)
        )?;
        Ok(Self { model })
    }
    
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.model.embed(vec![text], None)?;
        Ok(embeddings[0].clone())
    }
}
```

**验收标准**:
1. 相同文本生成相同向量
2. 相似文本向量距离近
3. 批处理性能达标（>100 texts/sec）
4. 模型自动下载（首次使用）

---

## 任务分配策略

### 并行组 1 (无依赖)
- T1.1: mDNS 服务封装
- T1.2: QUIC 传输层
- T1.3: PID 文件管理
- T2.3: Agent 进程检测

### 并行组 2 (依赖组 1)
- T2.1: P2P Network 状态管理 (需 T1.1, T1.2)
- T2.2: Matrix Server 生命周期 (需 T1.3)

### 并行组 3 (依赖组 2)
- T3.1: p2p discover 命令
- T3.2: p2p connect/disconnect
- T3.3: matrix start/stop/status
- T3.4: agent status

### 并行组 4 (依赖组 2, 3)
- T4.1: DHT 真实操作
- T4.2: Federation 事件发送
- T4.3: Embedding 服务替换

---

## Agent 分配建议

| Agent | 任务 | 技能要求 |
|-------|------|---------|
| A | T1.1, T3.1 | 网络编程、mDNS |
| B | T1.2, T4.1 | QUIC、P2P 协议 |
| C | T1.3, T2.2, T3.3 | 系统编程、进程管理 |
| D | T2.1, T3.2 | Rust async、架构设计 |
| E | T2.3, T3.4 | 系统信息、进程检测 |
| F | T4.2, T4.3 | Matrix 协议、机器学习 |

---

## 验收流程

1. **单元测试**: 每个任务必须包含单测 (>80% 覆盖)
2. **集成测试**: 同组任务完成后联合测试
3. **端到端测试**: 完整链路验证
4. **代码审查**: 接口符合规范，无 mock/stub

---

## 文档模板

每个任务必须输出:
```
tasks/T{编号}/
├── README.md          # 任务说明、接口定义
├── IMPLEMENTATION.md  # 实现细节
├── tests/             # 单元测试
│   ├── mod.rs
│   └── integration.rs
└── EXAMPLES.md        # 使用示例
```
