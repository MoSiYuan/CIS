**CIS（独联体）技术架构设计文档 v1.0**
**基于 Matrix 协议内核 + WebSocket 联邦 + SQLite 统一存储**

---

## 1. 架构哲学与核心约束

**设计原则：**
- **主权隔离**：每个节点是独立主权实体，数据所有权绝对归属本地
- **零外部依赖**：纯 Rust 实现，单二进制文件，无 Docker，无 PostgreSQL/MySQL 等外部服务
- **随时关机耐受**：支持非优雅断电，重启后状态自恢复，数据零丢失
- **轻量联邦**：节点间通过 WebSocket 长连接形成"独联体"，云端仅作信标，不转发数据

---

## 2. 核心组件架构

### 2.1 Matrix 内核层（CIS-Matrix-Core）

**定位**：Matrix 协议作为 CIS 的**内部神经束**，非外部 IM 工具。

**实现方式**：
- **非部署 Synapse**：嵌入式轻量级 Matrix 协议栈，仅实现核心子集（Client-Server Sync + 简化 Federation）
- **DID 身份映射**：Matrix User ID (`@user:homeserver`) 映射为 CIS 的 DID (`did:cis:{node_id}:{pub_key_short}`)
- **Room 语义重构**：
  - `Room` = Skill 命名空间 / 公域记忆通道
  - `Room ID` = `!{skill_name}:{node_id}.cis.local`
  - `Event Type` = 强类型 Skill 消息（如 `io.cis.git.push`, `io.cis.task.deploy`）

**Rust 模块**：
```rust
pub struct MatrixNucleus {
    store: Arc<CISStorage>,          // 复用 SQLite 统一存储
    crypto: OlmMachine,              // 端到端加密（matrix-sdk-crypto）
    event_bus: broadcast::Sender<MatrixEvent>,
}
```

### 2.2 网络联邦层（WebSocket Federation）

**拓扑结构**：**星型轮询 + P2P 长连接**

**云端锚点（Cloud Anchor）职责**：
- 仅维护**节点登记簿**：`{did, node_id, last_seen, public_endpoint_hint}`
- **不转发消息**，仅协助 NAT 打洞和初始握手

**连接生命周期**：

1. **轮询发现**（每 30 秒）：
   ```rust
   // 查询云端锚点获取在线节点列表
   let peers = anchor.heartbeat().await;
   ```

2. **WebSocket 打洞**：
   - 并行向锚点请求对端 Endpoint
   - 同时发起 UDP 打洞 + WebSocket 握手（`tokio-tungstenite`）
   - **双端认证**：Noise 协议握手 + DID 签名验证

3. **长连接维持**：
   - 成功后建立 `WebSocketStream`，纳入 `active_tunnels` 池
   - Matrix Federation PDU 通过 WebSocket **二进制帧**传输（Protobuf 序列化）
   - **心跳保活**：应用层 ping/pong（5 秒间隔），超时自动移除

**优势**：
- 延迟 < 50ms（相比 HTTPS 轮询降低 90%）
- 支持服务端推送（节点状态变更即时同步）
- 断线自动重连，Matrix Room State Resolution 保证分区恢复一致性

### 2.3 统一存储层（SQLite 主权化）

**原则**：CIS 所有静态化存储统一使用 SQLite，保持技术栈整洁，单文件即节点主权。

**数据库组织**：
```
~/.cis/
├── node.db              # 主数据库（Matrix、DID、网络状态、公域记忆）
├── skills/
│   └── {skill_name}.db  # Skill 分离库（大数据量时 ATTACH）
└── wal/
    └── node.db-wal      # WAL 文件（随时关机安全）
```

**关键配置（针对随时关机优化）**：
```sql
PRAGMA journal_mode = WAL;           -- 写前日志，读写不互斥
PRAGMA synchronous = NORMAL;         -- 每秒 fsync，平衡性能与安全
PRAGMA wal_autocheckpoint = 1000;    -- 自动合并点
PRAGMA journal_size_limit = 100MB;   -- 防 WAL 无限增长
```

**核心 Schema**：

```sql
-- Matrix 事件总线（存储于 node.db）
CREATE TABLE matrix_events (
    event_id TEXT PRIMARY KEY,
    room_id TEXT NOT NULL,           -- 对应 Skill/记忆域
    sender TEXT NOT NULL,            -- DID
    event_type TEXT NOT NULL,        -- io.cis.*
    content_json TEXT,               -- JSONB 格式
    received_at INTEGER,
    federate INTEGER DEFAULT 0       -- 0=私域(不联邦), 1=公域
) WITHOUT ROWID;

-- DID 身份与信任网络
CREATE TABLE did_trust (
    trustor TEXT,
    trustee TEXT,
    trust_level INTEGER CHECK(trust_level IN (0,1,2)), -- 0=黑名单,1=读,2=写
    PRIMARY KEY (trustor, trustee)
);

-- 网络节点状态（WebSocket 联邦视图）
CREATE TABLE network_peers (
    node_id TEXT PRIMARY KEY,
    endpoint_ws TEXT,
    status INTEGER,                  -- 0=离线,1=在线,2=打洞中
    last_seen INTEGER,
    rtt_ms INTEGER                   -- 网络延迟（最优路径选择）
);

-- 断线同步队列（记忆补全机制）
CREATE TABLE pending_sync (
    target_node TEXT,
    room_id TEXT,
    since_event_id TEXT,             -- 从哪开始追消息
    priority INTEGER                 -- 0=公域优先,1=私域按需
);
```

**Skill 存储规范**：
- Skill 可创建独立 `.db` 文件，但需通过 `ATTACH DATABASE` 挂载到主连接
- 支持跨 Skill JOIN 查询（如 Git Skill 关联 Matrix 事件）
- 备份：直接文件复制（SQLite 热读特性）或使用 `Backup` API 在线备份

---

## 3. 数据流与控制流

### 3.1 服务发现流程

```
节点启动
  ↓
轮询云端锚点（HTTPS/TLS，仅初始阶段）
  ↓
获取在线节点列表 + 自身公网映射
  ↓
对未连接节点发起 WebSocket 打洞
  ↓
DID 双向认证（Noise + Ed25519 签名）
  ↓
建立长连接 → 注册到 active_tunnels
  ↓
同步 Matrix Room 状态（补全离线期间消息）
```

### 3.2 Skill 通信流程（以 Git Skill 为例）

**本地 Git 操作**：
```bash
git commit -m "update"
git push cis://node1/repo
```

**CIS 内部流转**：
1. **Git Skill** 拦截 push，提取元数据（commit hash, ref, diff stats）
2. **构造 Matrix Event**：
   ```json
   {
     "type": "io.cis.git.push",
     "content": {
       "repo": "voxel-engine",
       "ref": "refs/heads/main",
       "commit": "abc123...",
       "objects": ["hash1", "hash2"]
     }
   }
   ```
3. **Matrix Core** 写入 `node.db`：
   - 插入 `matrix_events` 表（本地 Room）
   - 若 Room 标记 `federate=1`，通过 WebSocket 广播给在线 peers
4. **对象存储**：实际 blob 通过对象存储 Skill 处理，Matrix 仅存引用

**跨节点同步**：
- 对端节点 WebSocket 接收到 Event，写入本地 `matrix_events`（相同 Room ID）
- Git Skill 监听到新 Event，拉取对象（如需要），更新本地工作区

---

## 4. Skill 架构规范

**Skill 即 Matrix Room 的视图**：

```rust
pub trait Skill {
    // Skill 命名空间对应 Matrix Room ID
    fn room_id(&self) -> &str;
    
    // 初始化：创建/加入 Room，注册事件处理器
    async fn init(&mut self, matrix: Arc<MatrixNucleus>) -> Result<()>;
    
    // 处理 Matrix Event（通过 SQLite 订阅或内存通道）
    async fn on_event(&self, event: MatrixEvent) -> Result<()>;
}

// Git Skill 示例
pub struct GitSkill {
    storage: Arc<CISStorage>,
    repo_path: PathBuf,
}

impl Skill for GitSkill {
    fn room_id(&self) -> &str {
        "!git-voxelengine:node1.cis.local"
    }
    
    async fn init(&mut self, matrix: Arc<MatrixNucleus>) {
        // 创建本地 Room（federate=false 默认私域）
        matrix.create_room(RoomOptions {
            room_id: self.room_id(),
            federate: false,  // 可动态切换为 true 开启联邦
        }).await;
        
        // 监听 io.cis.git.* 事件
        matrix.register_handler("io.cis.git.push", |e| self.handle_push(e)).await;
    }
}
```

**涌现式扩展**：
- **动态发现**：新 Skill 上线 = 加入对应 Matrix Room，其他节点通过 WebSocket 联邦立即感知
- **权限继承**：Matrix Power Level 映射为 CIS 权限（0=普通成员，50=管理员，100=节点主权者）
- **现场编写**：Agent 可动态创建临时 Room 作为"沙盒频道"，测试新 Skill 通信协议

---

## 5. 部署与运维

### 5.1 零 Docker 部署

**单二进制启动**：
```bash
./cis-node --init --node-name "munin-macmini"
# 生成 ~/.cis/node.db（SQLite 自动创建）
# 生成 DID 密钥对
# 启动 WebSocket 监听（随机端口或指定 8484）
```

**关机耐受机制**：
- **日常运行**：依赖 SQLite WAL 模式，随时断电不丢数据（最多丢失 1 秒内未提交事务）
- **优雅关机**（可选）：发送 `SIGTERM` 时触发 `PRAGMA wal_checkpoint(TRUNCATE)`，合并 WAL 文件
- **重启恢复**：启动时自动检查 WAL 文件，存在未合并日志则自动恢复

### 5.2 节点迁移

**主权便携性**：
```bash
# 冷备份（节点随时关机后复制）
cp ~/.cis/node.db ~/.cis/backups/node.db.backup
scp ~/.cis/node.db new-server:~/.cis/

# 新节点启动（DID 不变，身份延续）
./cis-node --restore ~/.cis/node.db
# 自动重连联邦，同步离线期间消息
```

---

## 6. 技术栈总结

| 组件         | 技术选型                       | 理由                                                |
| ------------ | ------------------------------ | --------------------------------------------------- |
| **通信协议** | Matrix 子集（嵌入式）          | 成熟联邦协议，Room=Skill 命名空间，E2E 加密原生支持 |
| **传输层**   | WebSocket (WSS)                | 长连接低延迟，Rust 原生支持，穿透 NAT               |
| **发现服务** | 云端锚点（轻量 HTTP）          | 仅登记 Endpoint，不转发数据，可自托管               |
| **存储引擎** | SQLite 3.40+                   | 单文件主权，WAL 模式支持随时关机，零配置            |
| **序列化**   | Protobuf（网络）+ JSON（存储） | 高效与可读性平衡                                    |
| **身份认证** | DID (Ed25519)                  | 自托管身份，与 Matrix User ID 映射                  |

---

**此架构确保 CIS 节点作为"数字主权体"的绝对独立性**：单文件即可运行，单数据库即可迁移，长连接自主维护，Matrix 协议提供强大的扩展性而不引入外部依赖。