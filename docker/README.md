# CIS Docker 部署指南

> **CIS v1.1.6** - Cluster of Independent Systems
> **最后更新**: 2026-02-13

---

## 目录

- [快速开始](#快速开始)
- [Dockerfiles 说明](#dockerfiles-说明)
- [Docker Compose](#docker-compose)
- [构建镜像](#构建镜像)
- [运行容器](#运行容器)
- [生产部署](#生产部署)
- [故障排查](#故障排查)

---

## 快速开始

### 1. 使用 Docker Compose（推荐）

```bash
# 进入 docker 目录
cd docker

# 启动所有服务
docker-compose up -d

# 查看日志
docker-compose logs -f cis-node

# 停止服务
docker-compose down
```

### 2. 手动构建和运行

```bash
# 构建镜像
cd docker
docker build -f Dockerfile -t cis-node:latest ..

# 运行容器
docker run -d \
  --name cis-node \
  -p 7676:7676 \
  -p 7677:7677/udp \
  -v cis-data:/var/lib/cis/data \
  cis-node:latest
```

---

## Dockerfiles 说明

### 主 Dockerfiles

| 文件 | 用途 | 基础镜像 | 大小 | 说明 |
|-----|------|----------|------|------|
| **Dockerfile** | 生产环境 | debian:bookworm-slim | ~150MB | 多阶段构建，优化体积 |
| **Dockerfile.dev** | 开发环境 | rust:1.75-slim-bookworm | ~2GB | 包含开发工具，热重载 |
| **Dockerfile.cross** | 跨平台构建 | alpine:3.19 + rust:nightly | ~80MB | 支持 amd64/arm64 |

### Dockerfile 特性

#### 生产环境 (Dockerfile)

```dockerfile
# 多阶段构建
Stage 1: Builder (rust:1.75-slim)
  - 优化依赖缓存
  - 分层构建利用 Docker cache
  - strip 减小二进制体积

Stage 2: Runtime (debian:bookworm-slim)
  - 非 root 用户运行 (UID 1000)
  - 健康检查
  - 标准文件系统布局 (FHS)
```

**特点**:
- ✅ 最小镜像体积 (~150MB)
- ✅ 安全 (非 root 用户)
- ✅ 符合 OCI 规范
- ✅ 健康检查和信号处理

#### 开发环境 (Dockerfile.dev)

```dockerfile
# 包含开发工具
- cargo-watch (热重载)
- gdb/lldb (调试)
- ripgrep/fd (工具)
- git (版本控制)
```

**特点**:
- 🔧 开发工具齐全
- 🔄 支持热重载
- 📦 包含源代码
- 🐛 调试符号完整

#### 跨平台构建 (Dockerfile.cross)

```dockerfile
# 多架构支持
--platform=$BUILDPLATFORM
- linux/amd64
- linux/arm64
- linux/arm/v7
```

**特点**:
- 🌐 支持多平台
- ⚡ Alpine 基础镜像
- 🔗 静态链接 (musl)
- 📦 最小运行时依赖

---

## Docker Compose

### 服务架构

```
┌─────────────────────────────────────────────────────┐
│                 Docker Network (cis-network)          │
├─────────────────────────────────────────────────────┤
│                                                     │
│  ┌──────────────┐    ┌──────────────┐              │
│  │   cis-node   │◀──▶│   cis-gui     │              │
│  │   :7676      │    │   :3000       │              │
│  │   :7677/udp  │    └──────────────┘              │
│  └──────────────┘                                      │
│        │                                              │
│        ▼                                              │
│  ┌──────────────┐                                      │
│  │ cis-matrix-  │                                      │
│  │   bridge     │                                      │
│  │   :8080      │                                      │
│  └──────────────┘                                      │
└─────────────────────────────────────────────────────┘
```

### 配置文件

| 文件 | 场景 | 服务 |
|-----|------|------|
| **docker-compose.yml** | 生产/测试 | cis-node, cis-gui, cis-matrix-bridge |
| **docker-compose.dev.yml** | 开发 | 挂载源代码，热重载 |

### 组合使用

```bash
# 基础生产环境
docker-compose -f docker-compose.yml up -d

# 开发环境（覆盖配置）
docker-compose -f docker-compose.yml -f docker-compose.dev.yml up -d

# 仅启动特定服务
docker-compose up -d cis-node
docker-compose up -d cis-gui
```

---

## 构建镜像

### 标准构建

```bash
# 构建生产镜像
docker build -f docker/Dockerfile -t cis-node:latest .

# 指定版本
docker build -f docker/Dockerfile -t cis-node:1.1.6 .

# 构建参数
docker build \
  -f docker/Dockerfile \
  --build-arg RUST_VERSION=1.75 \
  --build-arg CIS_VERSION=1.1.6 \
  -t cis-node:1.1.6 .
```

### 跨平台构建

```bash
# 使用 buildx（支持多平台）
docker buildx create --use

# 构建多架构镜像
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -f docker/Dockerfile.cross \
  -t your-registry/cis:latest \
  --push .

# 本地构建（模拟其他平台）
docker buildx build \
  --platform linux/arm64 \
  -f docker/Dockerfile.cross \
  -t cis-arm64:latest \
  --load .
```

### 开发构建

```bash
# 使用开发 Dockerfile（包含调试工具）
docker build -f docker/Dockerfile.dev -t cis-dev:latest .

# 运行开发容器（挂载源代码）
docker run -it --rm \
  -v $(pwd):/app \
  -w /app \
  cis-dev:latest \
  bash
```

---

## 运行容器

### 环境变量

| 变量 | 默认值 | 说明 |
|-----|--------|------|
| `RUST_LOG` | info | 日志级别 (error/warn/info/debug/trace) |
| `CIS_VERSION` | 1.1.6 | CIS 版本号 |
| `CIS_DATA_DIR` | /var/lib/cis/data | 数据目录 |
| `CIS_LOG_DIR` | /var/log/cis | 日志目录 |
| `CIS_CONFIG_DIR` | /etc/cis | 配置目录 |

### 数据卷

| 卷 | 用途 |
|-----|------|
| `cis-data` | 持久化数据（SQLite、向量索引） |
| `cis-logs` | 日志文件 |
| `cis-bridge-data` | Matrix Bridge 数据 |

### 端口映射

| 端口 | 协议 | 服务 |
|-----|------|------|
| 7676 | TCP | Federation API (HTTP) |
| 7677 | UDP | P2P QUIC 传输 |
| 6767 | TCP | Matrix Federation + Agent Session |
| 3000 | TCP | GUI (如果启动) |
| 8080 | TCP | Matrix Bridge (如果启动) |

### 示例命令

```bash
# 完整配置
docker run -d \
  --name cis-node \
  --restart unless-stopped \
  -p 7676:7676 \
  -p 7677:7677/udp \
  -v cis-data:/var/lib/cis/data \
  -v cis-logs:/var/log/cis \
  -e RUST_LOG=debug \
  -e CIS_VERSION=1.1.6 \
  --health-cmd "curl -fs http://localhost:7676/health || exit 1" \
  --health-interval 30s \
  --health-timeout 10s \
  --health-retries 3 \
  cis-node:latest

# 挂载自定义配置
docker run -d \
  --name cis-node \
  -p 7676:7676 \
  -v $(pwd)/config.toml:/etc/cis/config.toml:ro \
  -v cis-data:/var/lib/cis/data \
  cis-node:latest

# 开发模式（挂载源代码）
docker run -it --rm \
  -v $(pwd)/cis-core:/app/cis-core \
  -v $(pwd)/cis-node:/app/cis-node \
  -w /app \
  cis-dev:latest \
  bash
```

---

## 生产部署

### Kubernetes

使用 `docker/k8s/` 目录下的 manifests:

```bash
# 创建 namespace
kubectl create namespace cis

# 部署 CIS Node
kubectl apply -f docker/k8s/cis-node.yaml -n cis

# 部署 Service
kubectl apply -f docker/k8s/service.yaml -n cis

# 部署 Ingress
kubectl apply -f docker/k8s/ingress.yaml -n cis

# 检查状态
kubectl get pods -n cis
kubectl logs -f deployment/cis-node -n cis
```

### Docker Swarm

```bash
# 初始化 Swarm
docker swarm init

# 部署 Stack
docker stack deploy -c docker-compose.yml cis

# 扩展服务
docker service scale cis_cis-node=3

# 查看服务
docker service ls
docker service ps cis_cis-node
```

### 安全加固

```yaml
# docker-compose.yml 安全配置
services:
  cis-node:
    # 非 root 用户
    user: "1000:1000"

    # 只读文件系统（除了数据目录）
    read_only: true
    tmpfs:
      - /tmp

    # 资源限制
    deploy:
      resources:
        limits:
          cpus: '2.0'
          memory: 2G
        reservations:
          cpus: '0.5'
          memory: 512M

    # 能力（减少权限）
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE

    # 安全选项
    security_opt:
      - no-new-privileges:true
```

---

## 故障排查

### 容器启动失败

```bash
# 查看容器日志
docker logs cis-node

# 查看最近的容器
docker ps -a

# 检查健康状态
docker inspect --format='{{.State.Health.Status}}' cis-node
```

### 网络问题

```bash
# 测试端口是否监听
docker exec cis-node netstat -tlnp

# 从容器内测试外部连接
docker exec cis-node curl -fs https://www.google.com

# 检查防火墙规则
iptables -L DOCKER-USER
```

### 数据持久化

```bash
# 查看卷
docker volume ls

# 检查卷内容
docker run --rm -v cis-data:/data alpine ls -la /data

# 备份卷
docker run --rm -v cis-data:/data -v $(pwd):/backup \
  alpine tar czf /backup/cis-data-backup.tar.gz -C /data .

# 恢复卷
docker run --rm -v cis-data:/data -v $(pwd):/backup \
  alpine tar xzf /backup/cis-data-backup.tar.gz -C /data
```

### 性能调优

```bash
# 查看资源使用
docker stats cis-node

# 限制内存使用
docker update -m 1g --memory-swap 1g cis-node

# 限制 CPU 使用
docker update --cpus="1.5" cis-node

# 查看容器大小
docker images cis-node
docker system df
```

### 常见错误

| 错误 | 原因 | 解决方案 |
|-----|------|---------|
| `permission denied` | 数据目录权限 | `chown -R 1000:1000 ./data` |
| `port already allocated` | 端口冲突 | `docker ps` 检查占用，修改端口映射 |
| `no space left on device` | 磁盘空间不足 | `docker system prune -a` 清理 |
| `health check failed` | 服务未启动或端口错误 | 检查 `RUST_LOG=debug` 日志 |
| `cannot connect to network` | 防火墙/网络配置 | 检查 `--network` 和防火墙规则 |

---

## 更多信息

- [CIS 官方文档](../../docs/)
- [v1.1.6 发布说明](../../docs/releases/v1.1.6/RELEASE_NOTES.md)
- [存储设计文档](../../docs/plan/v1.1.6/TASK_STORAGE_SQLITE_DESIGN.md)
- [GitHub Issues](https://github.com/your-org/CIS/issues)

---

**最后更新**: 2026-02-13
