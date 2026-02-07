# CIS 多机部署指南

## 快速开始

### 1. 初始化部署环境

```bash
cd deploy
./scripts/deploy.sh init
```

### 2. 编辑配置文件

```bash
# 协调器配置
vim configs/coordinator.toml

# Worker 配置
vim configs/worker.toml
```

### 3. 部署协调器

```bash
# 本地部署
./scripts/deploy.sh coordinator localhost

# 远程部署
./scripts/deploy.sh coordinator 192.168.1.10
```

### 4. 部署 Worker

```bash
./scripts/deploy.sh worker 192.168.1.11 192.168.1.10
```

## 目录结构

```
deploy/
├── configs/
│   ├── coordinator.toml    # 协调器配置
│   └── worker.toml         # Worker 配置
├── scripts/
│   └── deploy.sh           # 部署脚本
├── docker-compose.yml      # Docker Compose 配置
├── secrets/                # 密钥文件（自动生成）
└── backups/                # 备份目录
```

## 网络要求

| 服务 | 端口 | 协议 | 说明 |
|------|------|------|------|
| Federation API | 7676 | TCP | 节点间 HTTP 通信 |
| P2P Network | 7677 | UDP | QUIC 协议 P2P 通信 |

## 配置说明

### 协调器关键配置

```toml
[p2p]
listen_address = "0.0.0.0:7677"
external_address = "your-public-ip:7677"  # 公网 IP
mdns_enabled = false  # 云服务器关闭 mDNS

[federation]
listen_address = "0.0.0.0:7676"
external_address = "your-domain.com:7676"
```

### Worker 关键配置

```toml
[p2p]
bootstrap_nodes = [
    "/ip4/COORDINATOR_IP/udp/7677/quic-v1"
]

[worker]
worker_type = "cuda"  # cuda | metal | cpu
max_concurrent_tasks = 4
```

## 防火墙配置

```bash
# 协调器节点
ufw allow 7676/tcp
ufw allow 7677/udp

# Worker 节点
ufw allow 7677/udp
```

## 故障排查

```bash
# 检查节点状态
cis-node status

# 查看 P2P 网络状态
cis-node p2p status

# 查看已发现的节点
cis-node p2p peers

# 查看日志
tail -f /var/log/cis/cis-node.log
```
