# CIS 部署文档

本文档介绍 CIS 的各种部署方式，包括单机、集群和容器化部署。

## 部署方式概览

| 方式 | 适用场景 | 复杂度 | 扩展性 |
|------|----------|--------|--------|
| [单机部署](./standalone.md) | 个人使用、开发测试 | 低 | 无 |
| [Docker 部署](./docker.md) | 小型团队、快速部署 | 低 | 中 |
| [集群部署](./cluster.md) | 企业级、高可用 | 中 | 高 |
| [Kubernetes](./kubernetes.md) | 云原生、自动扩缩容 | 高 | 极高 |

## 快速选择

### 个人用户

推荐 **单机部署** 或 **Docker 部署**：

```bash
# 单机
curl -fsSL .../install.sh | bash
cis init && cis node start

# Docker
docker-compose up -d
```

### 小型团队

推荐 **Docker Compose** 部署：

```bash
git clone https://github.com/MoSiYuan/CIS.git
cd CIS
docker-compose up -d
```

### 企业用户

推荐 **Kubernetes** 部署：

```bash
kubectl apply -f deploy/kubernetes/
```

## 部署要求

### 系统要求

| 资源 | 最低 | 推荐 |
|------|------|------|
| CPU | 1 核 | 2 核+ |
| 内存 | 512 MB | 2 GB+ |
| 存储 | 10 GB | 100 GB+ SSD |
| 网络 | 10 Mbps | 100 Mbps+ |

### 端口要求

| 端口 | 协议 | 用途 | 必需 |
|------|------|------|------|
| 7676 | TCP | Federation API | 是 |
| 7677 | UDP | P2P QUIC | 是 |
| 6767 | TCP | WebSocket | 可选 |
| 80/443 | TCP | HTTP/HTTPS | 可选 |

### 操作系统

- **Linux**: Ubuntu 20.04+, CentOS 8+, Debian 11+
- **macOS**: 11.0+
- **Windows**: Windows 10/Server 2019+ (WSL2 推荐)

## 部署检查清单

### 部署前

- [ ] 确认系统满足要求
- [ ] 准备备份策略
- [ ] 配置防火墙规则
- [ ] 准备 SSL 证书（生产环境）

### 部署中

- [ ] 初始化 CIS
- [ ] 配置 AI Provider
- [ ] 设置 ACL
- [ ] 配置备份

### 部署后

- [ ] 验证服务状态
- [ ] 测试网络连接
- [ ] 配置监控告警
- [ ] 更新 DNS（如需公网访问）

## 安全最佳实践

### 网络安全

```toml
[network]
# 使用白名单模式
mode = "whitelist"

# 启用 TLS
tls_enabled = true
tls_cert = "/etc/cis/cert.pem"
tls_key = "/etc/cis/key.pem"
```

### 数据安全

```toml
[storage]
# 启用加密
encryption = true

# 定期备份
backup_enabled = true
backup_interval = 86400  # 每天
```

### 访问控制

```bash
# 只允许特定节点
cis network allow did:cis:trusted-node --reason "生产节点"
cis network mode whitelist
```

## 监控和日志

### 日志配置

```toml
[logging]
level = "info"
file = "/var/log/cis/cis.log"
max_size = "100MB"
max_files = 7
```

### 监控指标

```bash
# 查看指标
curl http://localhost:7676/metrics

# Prometheus 格式
# cis_node_peers_total 5
# cis_memory_entries_total 1234
```

## 故障恢复

### 备份恢复

```bash
# 备份
cis backup create --output /backup/cis-$(date +%Y%m%d).tar.gz

# 恢复
cis backup restore /backup/cis-20240101.tar.gz
```

### 灾难恢复

1. **节点故障**: 使用助记词在新硬件恢复
2. **数据损坏**: 从备份恢复
3. **网络分区**: 等待自动恢复或手动同步

## 升级指南

### 滚动升级（集群）

```bash
# 1. 升级一个节点
kubectl set image deployment/cis cis=mosiyuan/cis:v1.2.0 --selector=node=node-1

# 2. 验证
kubectl rollout status deployment/cis

# 3. 升级其他节点
kubectl set image deployment/cis cis=mosiyuan/cis:v1.2.0
```

### 停机升级（单机）

```bash
# 1. 备份
cis backup create

# 2. 停止服务
cis node stop

# 3. 升级
curl -fsSL .../install.sh | bash

# 4. 启动
cis node start

# 5. 验证
cis node status
```

## 性能调优

### 数据库优化

```toml
[storage.sqlite]
# WAL 模式
journal_mode = "WAL"

# 缓存大小
cache_size = 10000  # 页面数

# 同步模式
synchronous = "NORMAL"
```

### 网络优化

```toml
[p2p]
# 并发连接数
max_connections = 100

# 缓冲区大小
recv_buffer = "4MB"
send_buffer = "4MB"
```

## 故障排除

### 常见问题

| 问题 | 可能原因 | 解决方法 |
|------|----------|----------|
| 端口被占用 | 其他服务使用 | 修改端口配置 |
| 权限不足 | 非 root 用户 | 修改目录权限 |
| 内存不足 | 系统资源限制 | 增加内存或限制缓存 |
| 网络不通 | 防火墙配置 | 开放端口 |

### 调试命令

```bash
# 检查端口
netstat -tlnp | grep cis

# 检查进程
ps aux | grep cis-node

# 查看日志
tail -f /var/log/cis/cis.log

# 网络测试
cis network ping <peer-id>
```

## 参考

- [单机部署指南](./standalone.md)
- [Docker 部署指南](./docker.md)
- [Kubernetes 部署指南](./kubernetes.md)
- [集群部署指南](./cluster.md)
- [安全配置](../user/security.md)
- [性能优化](../user/performance.md)
