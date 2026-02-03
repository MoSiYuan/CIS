# CIS 组网方式指南

## 概述

CIS 支持多种组网方式，适应不同场景：

| 方式 | 适用场景 | 特点 | 实现状态 |
|------|---------|------|---------|
| 单机模式 | 个人使用 | 无网络依赖 | ✅ 可用 |
| 局域网 (mDNS) | 团队内网 | 自动发现，零配置 | ✅ 可用 |
| P2P 公网 | 分布式团队 | NAT穿透，去中心化 | ✅ 可用 |
| 混合模式 | 企业部署 | 云端锚点 + 本地节点 | ⚠️ 部分可用 |

---

## 概述

CIS 支持多种组网方式，适应不同场景：

| 方式 | 适用场景 | 特点 |
|------|---------|------|
| 单机模式 | 个人使用 | 无网络依赖 |
| 局域网 (mDNS) | 团队内网 | 自动发现，零配置 |
| P2P 公网 | 分布式团队 | NAT穿透，去中心化 |
| 混合模式 | 企业部署 | 云端锚点 + 本地节点 |

## 单机模式

无需配置，开箱即用。

```bash
cis init --standalone
```

## 局域网组网 (mDNS)

### 自动发现
同一局域网内的 CIS 节点自动发现：

```bash
# 启动时自动广播服务（程序内自动进行）
# 无需手动启动

# 查看发现的节点（API 方式，CLI 待实现）
# cis p2p peers --discovered

# 连接到特定节点（API 方式，CLI 待实现）  
# cis p2p connect node-id
```

> **注意**: 上述 CLI 命令在 v1.0 中尚未实现。如需使用 P2P 功能，需要通过 API 调用。

### 配置

编辑 `~/.cis/config.toml`：

```toml
[p2p]
enabled = true
listen_port = 7677

[p2p.discovery]
# 启用 mDNS
enable_mdns = true
# 服务名称
service_name = "_cis._tcp"
# 广播间隔（秒）
broadcast_interval = 30
```

### 程序化使用

```rust
use cis_core::p2p::P2PNetwork;

// 创建 P2P 网络
let p2p = P2PNetwork::new(
    node_id,
    did,
    "0.0.0.0:7677",
).await?;

// 启动网络
p2p.start().await?;

// 查看发现的节点
let peers = p2p.get_connected_peers().await;

// 广播消息
p2p.broadcast("topic", data).await?;
```

### 防火墙

确保端口开放：
- TCP 7676: CIS 默认端口
- UDP 5353: mDNS 端口

## P2P 公网组网

> **公网组网功能已完全实现**

### 节点发现

#### 方式1: DHT 网络
通过分布式哈希表发现节点：

```toml
[p2p.discovery]
enable_dht = true
bootstrap_nodes = [
    "bootstrap.cis.dev:7676",
    "bootstrap2.cis.dev:7676"
]
```

#### 方式2: Cloud Anchor (预留)
使用 CIS 云锚点服务：

```toml
[p2p.cloud_anchor]
enabled = true
endpoint = "https://anchor.cis.dev"
register_interval = 300
```

#### 方式3: 手动配置

```toml
[p2p.peers]
static_peers = [
    "did:cis:node1@192.168.1.100:7676",
    "did:cis:node2@example.com:7676"
]
```

### NAT 穿透

#### UPnP
自动尝试 UPnP 端口映射：

```toml
[p2p.nat]
enable_upnp = true
```

#### STUN/TURN
配置 STUN/TURN 服务器：

```toml
[p2p.ice]
stun_servers = [
    "stun.l.google.com:19302",
    "stun.cis.dev:3478"
]
turn_servers = [
    { url = "turn:cis.dev:3478", username = "user", credential = "pass" }
]
```

### 连接建立

```bash
# 查看连接状态
cis p2p status

# 手动连接到节点
cis p2p connect did:cis:node2@example.com

# 断开连接
cis p2p disconnect node-id

# 测试连接
cis p2p ping node-id
```

## 混合模式 (推荐)

企业场景推荐配置：

```toml
[p2p]
# 本地局域网发现
[p2p.discovery]
enable_mdns = true

# 云端锚点作为备份
[p2p.cloud_anchor]
enabled = true
endpoint = "https://anchor.company.com"

# 静态配置关键节点
[p2p.peers]
static_peers = [
    "did:cis:backup@backup.company.com:7676"
]

# NAT 穿透
[p2p.nat]
enable_upnp = true
[p2p.ice]
stun_servers = ["stun.company.com:3478"]
```

## 安全连接

### TLS 加密
所有 P2P 连接使用 TLS 1.3：
- 证书: 基于 DID 的自签名证书
- 密钥交换: X25519
- 认证: DID + 签名验证

### 访问控制

```toml
[p2p.acl]
# 白名单模式
whitelist = [
    "did:cis:trusted-node-1",
    "did:cis:trusted-node-2"
]

# 黑名单
blacklist = [
    "did:cis:untrusted-node"
]
```

## 数据同步

### 同步策略

```toml
[p2p.sync]
# 自动同步间隔（秒）
interval = 60

# 批量大小
batch_size = 100

# 冲突解决策略: lww | vector_clock | crdt
conflict_resolution = "crdt"

# 公域记忆同步
[p2p.sync.public_memory]
enabled = true
priority = "high"
```

### 程序化触发同步

```rust
use cis_core::p2p::sync::MemorySyncManager;

// 创建同步管理器
let sync = MemorySyncManager::new(
    memory_service,
    vector_storage,
    p2p_network,
    node_id,
);

// 启动同步管理器
sync.start().await?;

// 广播更新
sync.broadcast_update(key, value, category).await?;

// 同步到特定节点
sync.sync_with_node(peer_id).await?;
```

### 手动触发同步

```bash

# 同步所有数据
cis p2p sync

# 同步特定节点
cis p2p sync --node node-id

# 强制完整同步
cis p2p sync --full

# 查看同步状态
cis p2p sync-status
```

## 网络拓扑

### 星型拓扑
适合小型团队，一个中心节点：

```
    [中心节点]
    /    |    \
[节点1] [节点2] [节点3]
```

### 网状拓扑
完全去中心化，所有节点互联：

```
[节点1] ←→ [节点2]
   ↑ ↘     ↗ ↑
   ←→ [节点3] ←→
```

### 混合拓扑
大型组织推荐，分层结构：

```
[总部锚点]
    |
[区域节点1] ←→ [区域节点2]
   |                 |
[子节点]          [子节点]
```

## 监控与诊断

### 网络状态 (API 方式)

```rust
// 查看已连接节点
let peers = p2p.get_connected_peers().await;
for peer in peers {
    println!("Node: {}, Address: {}", peer.node_id, peer.address);
}

// 查看特定节点
let peer = p2p.get_peer("node-id").await?;
```

### CLI 命令

```bash

# 查看网络拓扑
cis p2p topology

# 查看连接详情
cis p2p connections --verbose

# 查看数据传输统计
cis p2p stats

# 网络延迟测试
cis p2p latency-test
```

### 故障排除

#### 节点发现失败
```bash
# 检查环境
cis doctor

# 检查端口占用
lsof -i :7677  # macOS/Linux
netstat -an | findstr 7677  # Windows
```

#### 调试信息

启用 P2P 调试日志：

```bash
RUST_LOG=cis_core::p2p=debug cis status
```

## 性能优化

### 带宽控制

```toml
[p2p.bandwidth]
# 上传限制 (KB/s)
upload_limit = 1024

# 下载限制 (KB/s)
download_limit = 2048

# 同步时段限制
[p2p.bandwidth.off_peak]
start = "02:00"
end = "06:00"
upload_limit = 5120
download_limit = 5120
```

### 连接池

```toml
[p2p.connection_pool]
# 最大并发连接
max_connections = 50

# 空闲超时（秒）
idle_timeout = 300

# 连接保活间隔
keepalive_interval = 30
```

## 示例场景

### 场景1: 家庭网络
```toml
# 简单配置，仅 mDNS
[p2p.discovery]
enable_mdns = true
enable_dht = false
```

### 场景2: 小型办公室
```toml
[p2p.discovery]
enable_mdns = true

[p2p.peers]
static_peers = [
    "did:cis:server@192.168.1.10:7676"
]
```

### 场景3: 分布式团队
```toml
[p2p.discovery]
enable_mdns = true
enable_dht = true

[p2p.cloud_anchor]
enabled = true

[p2p.ice]
stun_servers = ["stun.cis.dev:3478"]
```

### 场景4: 企业内网
```toml
[p2p]
# 关闭公网发现
[p2p.discovery]
enable_mdns = true
enable_dht = false

[p2p.peers]
static_peers = [
    "did:cis:hub1@10.0.1.10:7676",
    "did:cis:hub2@10.0.2.10:7676"
]

[p2p.acl]
whitelist_only = true
whitelist = [
    "did:cis:corp-node-*"
]
```
