# 多设备同步示例

这个示例展示如何在多台设备间同步 CIS 数据，解决跨设备幻觉问题。

## 场景

- 💻 工作站（高性能，主力开发）
- 📱 笔记本（移动办公）
- 🖥️ 服务器（24/7 运行）

## 核心问题

**跨设备幻觉**: 同一用户在不同设备使用独立 Agent 时，由于上下文窗口差异导致的回答不一致。

**CIS 解决方案**:
- 硬件绑定的本地记忆
- P2P 联邦同步
- 记忆内联打包

## 目录结构

```
multi-device-sync/
├── README.md
├── configs/
│   ├── workstation.toml
│   ├── laptop.toml
│   └── server.toml
├── dags/
│   ├── sync-all.dag.toml
│   └── backup.dag.toml
└── scripts/
    ├── setup-device.sh
    └── verify-sync.sh
```

## 快速开始

### 1. 第一台设备（工作站）

```bash
# 安装 CIS
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash

# 初始化（记录助记词！）
cis init
# 保存显示的 24 个助记词

# 配置为工作站
cp configs/workstation.toml ~/.cis/config.toml

# 启动节点
cis node start
```

### 2. 第二台设备（笔记本）

```bash
# 安装 CIS
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash

# 使用相同助记词恢复
cis init --recover
# 输入工作站生成的助记词

# 配置为笔记本
cp configs/laptop.toml ~/.cis/config.toml

# 启动节点
cis node start
```

### 3. 添加设备到白名单

在工作站上：

```bash
# 查看笔记本的 DID
cis network list --pending

# 添加到白名单
cis network allow did:cis:laptop-xxxxx --reason "我的笔记本"
```

在笔记本上：

```bash
# 查看工作站的 DID
cis network list --pending

# 添加到白名单
cis network allow did:cis:workstation-xxxxx --reason "我的工作站"
```

### 4. 验证同步

```bash
# 在工作站上创建记忆
cis skill do "记住：我的数据库密码是 secret123"

# 在笔记本上查询
cis memory search "数据库密码"

# 应该返回相同的答案
```

## 配置说明

### workstation.toml

```toml
[node]
name = "workstation"
role = "coordinator"  # 作为协调节点

[p2p]
enabled = true
listen_address = "0.0.0.0:7677"

[federation]
enabled = true
listen_address = "0.0.0.0:7676"

[sync]
auto_sync = true
sync_interval = 300  # 5 分钟
conflict_resolution = "timestamp"  # 时间戳优先

[storage]
path = "/var/lib/cis/data"
backup_enabled = true
backup_interval = 3600  # 每小时备份
```

### laptop.toml

```toml
[node]
name = "laptop"
role = "worker"

[p2p]
enabled = true
listen_address = "0.0.0.0:7677"
bootstrap_peers = [
    "/ip4/192.168.1.100/udp/7677/quic-v1/p2p/WORKSTATION_PEER_ID"
]

[federation]
enabled = true

[sync]
auto_sync = true
sync_on_connect = true
```

### server.toml

```toml
[node]
name = "server"
role = "replica"  # 作为备份节点

[p2p]
enabled = true
listen_address = "0.0.0.0:7677"

[sync]
auto_sync = true
sync_interval = 60  # 每分钟同步

[backup]
enabled = true
retention_days = 30
```

## 同步策略

### 自动同步 DAG

```toml
# dags/sync-all.dag.toml
[dag]
name = "sync-all"
description = "同步所有设备"
schedule = "*/5 * * * *"  # 每 5 分钟

[step.discover]
command = "cis network discover"

[step.sync-peers]
command = "cis network sync --all-peers --strategy merge"
depends_on = ["discover"]

[step.verify]
command = "./scripts/verify-sync.sh"
depends_on = ["sync-peers"]
```

### 备份 DAG

```toml
# dags/backup.dag.toml
[dag]
name = "backup"
description = "备份到服务器"
schedule = "0 * * * *"  # 每小时

[step.export]
command = "cis memory export --format json > /tmp/backup.json"

[step.compress]
command = "gzip /tmp/backup.json"
depends_on = ["export"]

[step.transfer]
command = "rsync /tmp/backup.json.gz server:/backups/cis/"
depends_on = ["compress"]
```

## 网络配置

### 局域网发现（mDNS）

默认启用，设备在同一局域网自动发现：

```toml
[discovery]
mdns_enabled = true
```

### 公网同步

如果设备不在同一网络：

```toml
[p2p]
# 使用中继服务器
relay_addresses = [
    "/dns4/cis-relay.example.com/udp/7677/quic-v1"
]

# 或配置端口转发
external_address = "/ip4/YOUR_PUBLIC_IP/udp/7677/quic-v1"
```

### VPN/WireGuard

```toml
[p2p]
# 使用 VPN 地址
listen_address = "10.0.0.2:7677"
```

## 冲突解决

### 策略选项

```toml
[sync]
# 时间戳优先（默认）
conflict_resolution = "timestamp"

# 设备优先级
conflict_resolution = "priority"
device_priority = ["workstation", "server", "laptop"]

# 手动解决
conflict_resolution = "manual"
```

### 手动解决冲突

```bash
# 查看冲突
cis sync conflicts

# 选择版本
cis sync resolve --id conflict-xxx --keep local
# 或
cis sync resolve --id conflict-xxx --keep remote
```

## 安全考虑

### 网络 ACL

```bash
# 查看当前连接
cis network list

# 拒绝未知设备
cis network mode whitelist

# 审计模式（记录但不拒绝）
cis network mode quarantine
```

### 加密传输

所有 P2P 通信自动使用 TLS + QUIC 加密，无需额外配置。

### 设备丢失处理

如果设备丢失：

```bash
# 从其他设备撤销访问
cis network deny did:cis:laptop-xxxxx --reason "设备丢失"

# 新设备使用助记词恢复
# 丢失设备上的数据因硬件绑定无法访问
```

## 故障排除

### 设备无法发现

```bash
# 检查网络连接
ping <other-device-ip>

# 检查端口开放
telnet <other-device-ip> 7677

# 手动添加对等节点
cis peer add /ip4/<ip>/udp/7677/quic-v1/p2p/<peer-id>
```

### 同步失败

```bash
# 查看同步状态
cis sync status

# 强制重新同步
cis sync reset
cis sync start

# 查看详细日志
cis logs --follow | grep sync
```

### 数据不一致

```bash
# 验证数据完整性
cis doctor --check-sync

# 修复不一致
cis sync repair
```

## 性能优化

### 增量同步

```toml
[sync]
incremental = true
batch_size = 1000
```

### 压缩传输

```toml
[p2p]
compression = true
compression_level = 6
```

### 带宽限制

```toml
[sync]
max_bandwidth = "10MB/s"
```

## 监控

### 同步指标

```bash
# 查看同步统计
cis sync stats

# 查看网络流量
cis network stats
```

### 告警

```toml
# dags/sync-monitor.dag.toml
[dag]
name = "sync-monitor"
schedule = "*/1 * * * *"

[step.check-latency]
command = "cis network ping-all | grep -q 'timeout' && exit 1"

[step.check-sync]
command = "cis sync status | grep -q 'behind' && exit 1"
depends_on = ["check-latency"]

[step.alert]
command = "cis skill do '发送同步异常告警'"
on_failure = true
```

## 最佳实践

1. **始终备份助记词**: 这是恢复数据的唯一方式
2. **至少一个常驻节点**: 建议服务器 24/7 运行
3. **定期检查同步状态**: 使用 `cis doctor`
4. **合理设置 ACL**: 不要开放给不信任的设备
5. **监控网络流量**: 避免意外的带宽消耗

## 示例场景

### 场景 1: 办公室 -> 家里

```bash
# 在办公室
cis skill do "今天完成了 Feature X 的开发"

# 回家继续工作
cis memory search "Feature X"
# 获得完整上下文
```

### 场景 2: 旅行时断网

```bash
# 旅行前
cis sync force  # 强制完整同步

# 旅行中（离线）
cis memory search "..."  # 本地查询，无需网络

# 回家后
cis sync  # 自动合并变更
```

### 场景 3: 团队协作

```bash
# 共享项目记忆（只读）
cis memory share --project my-project --readonly

# 团队成员订阅
cis memory subscribe --from did:cis:teammate-xxxxx
```

## 参考

- [网络配置](../../docs/network-configuration.md)
- [P2P 同步](../../docs/p2p-sync.md)
- [安全最佳实践](../../docs/security-best-practices.md)
