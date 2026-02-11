# CIS 端口合并提案

> 提案日期: 2026-02-10  
> 目标版本: v1.1.4 或 v1.2.0  
> 优先级: P1

---

## 执行摘要

将当前 **4个端口** 合并为 **2个端口**，通过接口路径区分不同服务类型：

| 当前方案 | 端口数 | 配置复杂度 | 防火墙规则 |
|---------|--------|-----------|-----------|
| 4端口分离 | 4 | 高 | 4条规则 |
| **2端口合并** (提案) | **2** | **低** | **2条规则** |

---

## 当前端口使用情况

### 端口清单

| 端口 | 协议 | 用途 | 现状 |
|------|------|------|------|
| **6767** | TCP/WebSocket | Matrix Federation + Agent Session | 活跃使用 |
| **7676** | TCP/HTTP | Matrix Client-Server API | 活跃使用 |
| **7677** | UDP/QUIC | P2P 传输 | 活跃使用 |
| **6768** | TCP/WebSocket | WebSocket 联邦 (备用) | 部分代码中 |

### 问题分析

1. **配置复杂** - 用户需要配置4个端口
2. **防火墙繁琐** - 需要开放4个端口
3. **端口分散** - 6767/7676/7677 无规律，易混淆
4. **资源浪费** - 3个 TCP 端口可以合并

---

## 合并方案

### 目标架构

```
┌─────────────────────────────────────────────────────────────┐
│                    单一 TCP 端口 (6767)                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  /_matrix/client/*    →  Matrix Client-Server API          │
│  /_matrix/federation/* →  Matrix Federation API            │
│  /_cis/agent/*        →  Agent Session API                 │
│  /_cis/skill/*        →  Skill Execution API               │
│  /_cis/p2p/ws         →  P2P WebSocket Fallback            │
│                                                             │
│  Upgrade: WebSocket → 用于实时通信 (Agent Session)          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼ HTTP/TCP 统一入口

┌─────────────────────────────────────────────────────────────┐
│                    单一 UDP 端口 (7677)                      │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  QUIC 协议多路复用:                                          │
│    Stream 0x01: P2P 控制通道                                │
│    Stream 0x02: P2P 数据传输                                │
│    Stream 0x03: DHT 查询                                    │
│                                                             │
│  或基于 Connection ID 区分不同服务                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 详细设计

### 1. TCP 端口合并 (6767)

#### 路由设计

```rust
// unified_server.rs
use axum::{
    routing::{get, post, put, delete},
    Router,
};

pub fn create_unified_router() -> Router {
    Router::new()
        // Matrix Client-Server API
        .route("/_matrix/client/versions", get(matrix::client_versions))
        .route("/_matrix/client/v3/login", post(matrix::login))
        .route("/_matrix/client/v3/sync", get(matrix::sync))
        .route("/_matrix/client/v3/rooms/:room_id/join", post(matrix::join_room))
        // ... 更多 Matrix C-S API
        
        // Matrix Federation API
        .route("/_matrix/federation/v1/version", get(matrix::federation_version))
        .route("/_matrix/federation/v1/send/:txn_id", put(matrix::federation_send))
        .route("/_matrix/key/v2/server", get(matrix::server_key))
        // ... 更多 Federation API
        
        // CIS Agent API (WebSocket Upgrade)
        .route("/_cis/agent/session", get(agent::handle_session))
        .route("/_cis/agent/:agent_id/attach", get(agent::attach_session))
        
        // CIS Skill API
        .route("/_cis/skill/:skill_id/execute", post(skill::execute))
        .route("/_cis/skill/:skill_id/status", get(skill::status))
        
        // P2P WebSocket Fallback
        .route("/_cis/p2p/relay", get(p2p::websocket_relay))
        
        // 健康检查
        .route("/health", get(health_check))
        .route("/ready", get(readiness_check))
}

// WebSocket 升级处理
async fn handle_websocket_upgrade(
    Path(service): Path<String>,
    ws: WebSocketUpgrade,
    req: Request,
) -> Response {
    match service.as_str() {
        "agent" => ws.on_upgrade(agent::handle_socket),
        "federation" => ws.on_upgrade(federation::handle_socket),
        "p2p" => ws.on_upgrade(p2p::handle_websocket),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}
```

#### 协议升级协商

```rust
// 根据 Path 自动判断协议
GET /_matrix/client/v3/sync      → HTTP/JSON 响应
GET /_matrix/federation/v1/send  → HTTP/JSON 响应
GET /_cis/agent/session          → WebSocket Upgrade (实时)
```

---

### 2. UDP 端口合并 (7677)

#### QUIC 多路复用

```rust
// quic_multiplexer.rs
use quinn::{Endpoint, Incoming, Connection};

pub struct QuicMultiplexer {
    endpoint: Endpoint,
    handlers: HashMap<u8, Box<dyn StreamHandler>>,
}

impl QuicMultiplexer {
    /// 注册流处理器
    pub fn register_handler(&mut self, stream_type: u8, handler: Box<dyn StreamHandler>) {
        self.handlers.insert(stream_type, handler);
    }
    
    /// 处理新连接
    pub async fn handle_connection(&self, conn: Connection) -> Result<()> {
        // 握手时交换支持的流类型
        let (mut send, mut recv) = conn.open_bi().await?;
        
        // 读取客户端声明的流类型
        let mut buf = [0u8; 1];
        recv.read_exact(&mut buf).await?;
        let stream_type = buf[0];
        
        // 路由到对应处理器
        if let Some(handler) = self.handlers.get(&stream_type) {
            handler.handle(conn).await?;
        }
        
        Ok(())
    }
}

// 流类型定义
pub const STREAM_P2P_CONTROL: u8 = 0x01;
pub const STREAM_P2P_DATA: u8 = 0x02;
pub const STREAM_DHT_QUERY: u8 = 0x03;

// 使用示例
let mut mux = QuicMultiplexer::new(endpoint);
mux.register_handler(STREAM_P2P_CONTROL, Box::new(P2PControlHandler::new()));
mux.register_handler(STREAM_P2P_DATA, Box::new(P2PDataHandler::new()));
mux.register_handler(STREAM_DHT_QUERY, Box::new(DhtQueryHandler::new()));
```

#### 替代方案: ALPN 协议协商

```rust
// 使用 QUIC 的 ALPN 进行协议协商
let server_config = ServerConfig::builder()
    .with_protocols(&[
        b"cis-p2p/1.0",      // P2P 主协议
        b"cis-dht/1.0",      // DHT 查询
        b"cis-relay/1.0",    // 中继服务
    ])
    .build()?;

// 根据 ALPN 协议名路由
match connection.alpn_protocol() {
    Some(b"cis-p2p/1.0") => handle_p2p_connection(connection).await,
    Some(b"cis-dht/1.0") => handle_dht_connection(connection).await,
    _ => Err(Error::unsupported_protocol()),
}
```

---

## 配置简化

### 当前配置 (复杂)

```toml
[network]
matrix_client_port = 7676      # HTTP API
matrix_federation_port = 6767  # WebSocket Federation
agent_session_port = 6767      # 与 Federation 共享
p2p_quic_port = 7677           # QUIC
p2p_websocket_port = 6768      # WebSocket 备用
```

### 合并后配置 (简洁)

```toml
[network]
# 单一 TCP 端口 (Matrix + Agent + Federation)
tcp_port = 6767
bind_address = "0.0.0.0"

# 单一 UDP 端口 (P2P QUIC)
udp_port = 7677

# 可选: 启用/禁用特定服务
[features]
matrix_api = true
agent_session = true
federation = true
p2p = true
```

---

## 兼容性处理

### 向后兼容层

```rust
// 为旧客户端提供兼容
pub struct CompatibilityLayer {
    inner: Arc<UnifiedServer>,
}

impl CompatibilityLayer {
    /// 启动兼容服务器
    pub async fn start(&self) -> Result<()> {
        // 如果配置中启用了兼容模式，监听旧端口并重定向
        if self.config.compatibility_mode {
            // 7676 → 6767 重定向
            tokio::spawn(self.redirect_7676_to_6767());
            
            // 6768 → 6767 重定向
            tokio::spawn(self.redirect_6768_to_6767());
        }
        
        Ok(())
    }
    
    /// HTTP 301 重定向
    async fn redirect_7676_to_6767(&self) -> Result<()> {
        let app = Router::new()
            .fallback(|req: Request| async move {
                let new_uri = format!("http://localhost:6767{}", req.uri().path());
                Redirect::permanent(&new_uri)
            });
        
        axum::serve(tokio::net::TcpListener::bind("0.0.0.0:7676").await?, app).await?;
        Ok(())
    }
}
```

### 客户端自动发现

```rust
// .well-known/cis 配置
{
  "m.homeserver": {
    "base_url": "http://localhost:6767",
    "server_name": "localhost"
  },
  "cis.services": {
    "matrix_client": "/_matrix/client",
    "matrix_federation": "/_matrix/federation",
    "agent_session": "/_cis/agent",
    "p2p_websocket": "/_cis/p2p/ws"
  }
}
```

---

## 实现路线图

### Phase 1: 统一服务器框架 (Week 1)

- [ ] 创建 `unified_server` 模块
- [ ] 实现基础路由分发
- [ ] 集成 WebSocket 升级支持

### Phase 2: 服务迁移 (Week 2-3)

- [ ] 迁移 Matrix Client API → `/_matrix/client/*`
- [ ] 迁移 Matrix Federation API → `/_matrix/federation/*`
- [ ] 迁移 Agent Session → `/_cis/agent/*`
- [ ] 迁移 P2P WebSocket → `/_cis/p2p/*`

### Phase 3: P2P QUIC 多路复用 (Week 4)

- [ ] 实现 QUIC ALPN 协商
- [ ] 或实现 Stream 类型多路复用
- [ ] 迁移 P2P 控制流
- [ ] 迁移 DHT 查询流

### Phase 4: 兼容层 (Week 5)

- [ ] 实现端口重定向
- [ ] 更新 .well-known 配置
- [ ] 测试向后兼容

### Phase 5: 文档更新 (Week 6)

- [ ] 更新安装文档
- [ ] 更新配置文档
- [ ] 更新防火墙配置指南

---

## 收益分析

### 部署简化

| 场景 | 当前 | 合并后 |
|------|------|--------|
| Docker 端口映射 | `-p 6767:6767 -p 7676:7676 -p 7677:7677` | `-p 6767:6767 -p 7677:7677` |
| 防火墙规则 | 3条 | 2条 |
| 配置文件行数 | ~20行 | ~5行 |

### 运维简化

- **监控**: 只需要监控 2 个端口的健康状态
- **日志**: 统一入口，便于关联分析
- **故障排查**: 减少端口冲突问题

### 安全性

- **攻击面**: 减少 25% 的开放端口
- **配置错误**: 降低端口配置错误概率
- **审计**: 统一入口便于审计日志

---

## 风险评估

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|---------|
| 旧客户端不兼容 | 中 | 高 | 提供兼容层 + 301 重定向 |
| 性能瓶颈 | 低 | 中 | 使用 axum 高性能路由 |
| 单点故障 | 低 | 高 | 服务隔离，单一端口故障不影响内部 |
| 协议冲突 | 低 | 中 | 严格的 Path 前缀区分 |

---

## 相关文档

- [架构评审报告](./ARCHITECTURE_REVIEW.md)
- [简化实现分析](./SIMPLIFIED_IMPLEMENTATION_ANALYSIS.md)

---

*提案创建日期: 2026-02-10*  
*状态: 评审中*
