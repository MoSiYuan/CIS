# P2P 网络配置指南

本文档介绍如何配置 CIS 的 P2P 网络功能。

## 目录

- [快速开始](#快速开始)
- [基础配置](#基础配置)
- [高级配置](#高级配置)
- [节点发现](#节点发现)
- [安全设置](#安全设置)
- [故障排查](#故障排查)

## 快速开始

### 1. 初始化 P2P 网络

```bash
# 启动 P2P 网络（基础模式）
cis p2p start

# 启动 P2P 网络（完整功能）
cis p2p start --dht --nat-traversal
```

### 2. 发现节点

```bash
# 在局域网发现节点
cis p2p discover

# 查看已发现的节点
cis p2p peers
```

### 3. 连接节点

```bash
# 连接到特定节点
cis p2p connect did:cis:node@192.168.1.100:7677

# 或仅使用地址
cis p2p connect 192.168.1.100:7677 --node-id node-abc123
```

## 基础配置

### 配置文件位置

P2P 配置存储在 CIS 配置文件中：

- **全局配置**: `~/.config/cis/config.toml`
- **项目配置**: `.cis/config.toml`

### 基础配置示例

```toml
[p2p]
enabled = true
listen_port = 7677
enable_dht = true
enable_nat_traversal = true
external_address = "auto"  # 或手动指定 "1.2.3.4:7677"

[p2p.bootstrap]
nodes = [
    "bootstrap.cis.dev:6767",
    "bootstrap2.cis.dev:6767"
]

[p2p.stun]
servers = [
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302"
]

[p2p.limits]
max_peers = 50
max_connections_per_peer = 5
connection_timeout_secs = 30
```

### 配置项说明

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `enabled` | `true` | 是否启用 P2P 网络 |
| `listen_port` | `7677` | P2P 监听端口 |
| `enable_dht` | `true` | 启用 DHT 节点发现 |
| `enable_nat_traversal` | `true` | 启用 NAT 穿透 |
| `external_address` | `"auto"` | 外部地址（用于公网访问）|
| `max_peers` | `50` | 最大对等节点数 |

## 高级配置

### DHT 配置

```toml
[p2p.dht]
# 监听地址
listen_addr = "0.0.0.0:7678"

# 节点公告间隔（秒）
announce_interval_secs = 300

# 节点超时时间（秒）
node_timeout_secs = 600

# Kademlia k 值（桶大小）
k = 20

# 并发查询数（alpha）
alpha = 3

# 数据复制因子
replication_factor = 3
```

### NAT 穿透配置

```toml
[p2p.nat]
# UPnP 端口映射
enable_upnp = true

# STUN 服务器
stun_servers = [
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302"
]

# TURN 服务器（中继）
[[p2p.nat.turn]]
server = "turn.example.com:3478"
username = "user"
credential = "pass"
realm = "example.com"
```

### QUIC 传输配置

```toml
[p2p.quic]
# 最大并发流数
max_concurrent_streams = 100

# 空闲超时（秒）
idle_timeout_secs = 60

# 握手超时（秒）
handshake_timeout_secs = 10

# 证书配置
cert_path = "/path/to/cert.pem"
key_path = "/path/to/key.pem"
```

## 节点发现

### mDNS 局域网发现

mDNS 自动在局域网发现 CIS 节点，无需配置。

```bash
# 查看发现的节点
cis p2p peers

# 手动触发发现
cis p2p discover --timeout 30
```

### DHT 广域网发现

```bash
# 添加 bootstrap 节点
cis p2p dht add-bootstrap bootstrap.cis.dev:6767

# 查看 bootstrap 节点列表
cis p2p dht list-bootstrap

# 手动查找节点
cis p2p dht find-node <node-id>
```

### 静态节点配置

在配置文件中预定义节点：

```toml
[[p2p.static_peers]]
node_id = "peer-abc123"
did = "did:cis:abc123"
address = "192.168.1.100:7677"
capabilities = ["memory_sync", "skill_invoke"]

[[p2p.static_peers]]
node_id = "peer-def456"
did = "did:cis:def456"
address = "peer.example.com:7677"
capabilities = ["memory_sync"]
```

## 安全设置

### TLS 证书配置

```toml
[p2p.tls]
# 使用自签名证书（默认）
use_self_signed = true

# 或使用自定义证书
cert_path = "/path/to/cert.pem"
key_path = "/path/to/key.pem"
ca_path = "/path/to/ca.pem"
```

### 访问控制

```toml
[p2p.acl]
# 允许的节点（白名单）
allowed_nodes = [
    "did:cis:trusted-node-1",
    "did:cis:trusted-node-2"
]

# 拒绝的节点（黑名单）
blocked_nodes = [
    "did:cis:malicious-node"
]

# 默认策略: allow 或 deny
default_policy = "allow"
```

### 连接限制

```toml
[p2p.rate_limit]
# 每节点最大连接数
max_connections_per_node = 5

# 连接速率限制（每分钟）
connection_rate_per_min = 10

# 消息速率限制（每秒）
message_rate_per_sec = 100
```

## 故障排查

### 检查 P2P 状态

```bash
# 查看完整状态
cis p2p status

# 网络诊断
cis p2p diagnose

# 仅检查 NAT
cis p2p diagnose --check nat

# 仅检查端口
cis p2p diagnose --check port
```

### 测试连接

```bash
# Ping 节点
cis p2p ping <node-id>

# 测试 NAT 穿透
cis p2p hole-punch --detect-only

# 测试到特定节点的打洞
cis p2p hole-punch --target 192.168.1.100:7677
```

### 常见问题

#### 节点无法发现

1. 检查防火墙设置
2. 确认 mDNS 服务运行正常
3. 尝试手动添加节点

#### DHT 连接失败

1. 检查 bootstrap 节点配置
2. 验证网络连通性
3. 查看 DHT 路由表状态

#### NAT 穿透失败

参见 [NAT 穿透故障排查](./nat-troubleshooting.md)

## 最佳实践

### 生产环境配置

```toml
[p2p]
enabled = true
listen_port = 7677
enable_dht = true
enable_nat_traversal = true
max_peers = 100

[p2p.dht]
announce_interval_secs = 300
node_timeout_secs = 900
k = 20
alpha = 3
replication_factor = 5

[p2p.nat]
enable_upnp = true
stun_servers = [
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
    "stun3.l.google.com:19302",
    "stun4.l.google.com:19302"
]

[p2p.quic]
max_concurrent_streams = 200
idle_timeout_secs = 120
```

### 开发环境配置

```toml
[p2p]
enabled = true
listen_port = 7677
enable_dht = false
enable_nat_traversal = false
max_peers = 10

[p2p.static_peers]
# 仅连接本地测试节点
[[p2p.static_peers]]
node_id = "local-test"
address = "127.0.0.1:7678"
```

## 监控和日志

### 启用详细日志

```bash
RUST_LOG=cis_core::p2p=debug cis p2p start
```

### 监控指标

```bash
# 查看 P2P 指标（如支持）
cis p2p metrics

# 导出指标到文件
cis p2p metrics --output metrics.json
```

### 日志分析

关键日志模式：

```
# 节点发现
[INFO] Discovered peer: node-xxx

# 连接建立
[INFO] Connected to peer: node-xxx @ address

# DHT 公告
[INFO] Announcing node xxx to DHT

# NAT 穿透
[INFO] External address discovered: xxx
[WARN] NAT traversal failed

# 错误
[ERROR] Failed to connect to peer: xxx
```
