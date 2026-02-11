# 端口合并快速参考

> 当前状态: 4端口 → 目标: 2端口

---

## 前后对比

### 当前 (4端口)

```
┌─────────┬──────────┬────────────────────────────┐
│  端口   │  协议    │           用途              │
├─────────┼──────────┼────────────────────────────┤
│  6767   │ TCP/WS   │ Matrix Federation          │
│  7676   │ TCP/HTTP │ Matrix Client API          │
│  7677   │ UDP/QUIC │ P2P 传输                   │
│  6768   │ TCP/WS   │ WebSocket 备用             │
└─────────┴──────────┴────────────────────────────┘
```

**问题**:
- 配置复杂 (4个端口)
- 防火墙繁琐 (4条规则)
- 端口编号混乱 (6767/7676/7677)

---

### 合并后 (2端口)

```
┌─────────┬──────────┬────────────────────────────┐
│  端口   │  协议    │           用途              │
├─────────┼──────────┼────────────────────────────┤
│  6767   │ TCP      │ 统一入口 (HTTP/WebSocket)  │
│  7677   │ UDP/QUIC │ P2P 传输                   │
└─────────┴──────────┴────────────────────────────┘
```

**路由区分**:

```
6767 TCP 端口内部路由:
├── /_matrix/client/*     → Matrix Client API
├── /_matrix/federation/* → Matrix Federation API  
├── /_cis/agent/*         → Agent Session (WebSocket)
├── /_cis/skill/*         → Skill Execution
└── /_cis/p2p/ws          → P2P WebSocket Fallback

7677 UDP 端口内部路由 (QUIC ALPN):
├── "cis-p2p/1.0"  → P2P 主协议
├── "cis-dht/1.0"  → DHT 查询
└── "cis-relay/1.0" → 中继服务
```

---

## 配置对比

### 当前配置

```toml
[network]
matrix_client_port = 7676
matrix_federation_port = 6767
agent_session_port = 6767
p2p_quic_port = 7677
p2p_websocket_port = 6768
```

### 合并后配置

```toml
[network]
tcp_port = 6767
udp_port = 7677
```

**简化效果**: 5行 → 2行 (60% 减少)

---

## 实施步骤

### 1. 统一 TCP 服务器 (Week 1-2)

```rust
// 使用 axum 统一路由
let app = Router::new()
    .route("/_matrix/client/*", matrix_client_handler)
    .route("/_matrix/federation/*", matrix_federation_handler)
    .route("/_cis/agent/*", agent_session_handler)
    .route("/_cis/skill/*", skill_handler);

axum::serve(app, "0.0.0.0:6767").await?;
```

### 2. QUIC ALPN 协议 (Week 3)

```rust
// QUIC 协议协商
let server = ServerConfig::builder()
    .with_protocols(&[b"cis-p2p/1.0", b"cis-dht/1.0"])
    .build()?;
```

### 3. 兼容层 (Week 4)

```rust
// 旧端口 301 重定向到新端口
GET http://localhost:7676/_matrix/client/versions
↓ 301 Redirect
GET http://localhost:6767/_matrix/client/versions
```

---

## 防火墙规则对比

### 当前规则

```bash
# 需要开放 4 个端口
iptables -A INPUT -p tcp --dport 6767 -j ACCEPT
iptables -A INPUT -p tcp --dport 7676 -j ACCEPT
iptables -A INPUT -p tcp --dport 6768 -j ACCEPT
iptables -A INPUT -p udp --dport 7677 -j ACCEPT
```

### 合并后规则

```bash
# 只需要开放 2 个端口
iptables -A INPUT -p tcp --dport 6767 -j ACCEPT
iptables -A INPUT -p udp --dport 7677 -j ACCEPT
```

---

## 常见问题

### Q: 性能会有影响吗?

A: 不会。使用 axum 高性能路由， overhead < 1%。反而减少端口监听开销。

### Q: 旧客户端怎么办?

A: 提供兼容层，7676 端口自动 301 重定向到 6767。

### Q: 单一端口会不会成为瓶颈?

A: 内部服务仍是独立的，只是统一入口。可用连接数由系统限制，与端口数量无关。

### Q: WebSocket 和 HTTP 如何共存?

A: 通过 HTTP Upgrade 头部自动协商：
```
GET /_cis/agent/session HTTP/1.1
Upgrade: websocket
Connection: Upgrade
```

---

## 详细文档

- 完整方案: [PORT_CONSOLIDATION_PROPOSAL.md](./PORT_CONSOLIDATION_PROPOSAL.md)
- 架构评审: [ARCHITECTURE_REVIEW.md](./ARCHITECTURE_REVIEW.md)

---

*最后更新: 2026-02-10*
