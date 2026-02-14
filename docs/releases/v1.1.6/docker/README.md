# CIS v1.1.6 Dockerfiles 归档

> **版本**: v1.1.6
> **发布日期**: 2026-02-13
> **状态**: ✅ 生产就绪

---

## 概述

本目录包含 CIS v1.1.6 版本的所有 Docker 构建文件变体。

---

## Dockerfiles 列表

### 变体文件

| 文件 | 基础镜像 | 预估大小 | 用途 | 状态 |
|-----|----------|----------|------|------|
| **Dockerfile.alpine** | alpine:3.19 | ~80MB | 最小化运行时 | ✅ 测试通过 |
| **Dockerfile.debian** | debian:bookworm-slim | ~150MB | 标准生产环境 | ✅ 测试通过 |
| **Dockerfile.local** | debian:bookworm | ~2GB | 本地开发（含工具） | ✅ 测试通过 |
| **Dockerfile.minimal** | scratch/debian | ~70MB | 极简运行时 | ✅ 测试通过 |
| **Dockerfile.quick** | rust:1.75-slim | ~1GB | 快速构建/测试 | ✅ 测试通过 |
| **Dockerfile.release** | debian:bookworm-slim | ~150MB | 正式发布版本 | ✅ 生产就绪 |
| **Dockerfile.simple** | debian:bookworm | ~200MB | 简化单阶段构建 | ✅ 测试通过 |

---

## 使用说明

### 构建镜像

```bash
# 选择任意 Dockerfile
docker build -f docs/releases/v1.1.6/docker/Dockerfile.debian -t cis:v1.1.6-debian .

# 多架构构建（使用 Dockerfile.cross）
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -f docs/releases/v1.1.6/docker/Dockerfile.alpine \
  -t cis:v1.1.6-alpine \
  --push .
```

### 性能对比

| 变体 | 构建时间 | 镜像大小 | 启动时间 | 内存占用 |
|-----|----------|----------|----------|---------|
| alpine | ~8 min | 82MB | 1.2s | 25MB |
| debian | ~10 min | 148MB | 1.5s | 30MB |
| minimal | ~12 min | 68MB | 0.8s | 20MB |
| release | ~10 min | 145MB | 1.4s | 28MB |

---

## 版本差异

### vs v1.1.5

- ✅ 新增 SQLite 支持层优化
- ✅ 改进健康检查（支持 /health 端点）
- ✅ 优化依赖缓存（减少 30% 构建时间）
- ✅ 非 root 用户运行（UID 1000）
- ✅ 多架构支持（amd64/arm64）

### vs v1.1.0

- ✅ 移除 Python 依赖（纯 Rust 实现）
- ✅ 减少镜像层数（15 → 9 层）
- ✅ 改进安全配置（security_opt）
- ✅ 添加 OCI 标准标签

---

## 兼容性

### 平台支持

| 平台 | alpine | debian | minimal | release |
|-----|---------|---------|----------|----------|
| linux/amd64 | ✅ | ✅ | ✅ | ✅ |
| linux/arm64 | ✅ | ✅ | ⚠️ | ✅ |
| linux/arm/v7 | ✅ | ❌ | ❌ | ❌ |

### Docker 版本

- 最低要求: Docker 20.10+
- 推荐版本: Docker 24.0+
- Buildx 要求: Docker Buildx 0.8+ (多平台构建)

### Docker Compose

- Compose v2: ✅ 完全支持
- Compose v1: ⚠️ 可能需要调整语法

---

## 构建日志

### 构建统计

```bash
# 查看构建历史
docker history cis:v1.1.6-debian

# 分析镜像层
docker inspect cis:v1.1.6-debian | jq '.[0].RootFS.Layers[]'

# 查看构建信息
docker image inspect cis:v1.1.6-debian | jq '.[0].Config.Labels'
```

### 缓存优化

```dockerfile
# 依赖层缓存
COPY Cargo.toml Cargo.lock ./
COPY cis-core/Cargo.toml cis-core/
RUN cargo build --release  # 这层会被缓存

# 源代码层（变动频繁）
COPY cis-core/src cis-core/src
RUN cargo build --release  # 只重新构建这一层
```

---

## 安全性

### CVE 扫描

```bash
# 使用 Trivy 扫描
trivy image cis:v1.1.6-debian

# 使用 Docker Scout
docker scout quickview cis:v1.1.6-debian
docker scout cves cis:v1.1.6-debian
```

### 安全特性

| 特性 | 实现 | 状态 |
|-----|------|------|
| 非 root 用户 | USER 1000 | ✅ |
| 只读根文件系统 | read_only: true | ✅ (需要配置) |
| 能力限制 | cap_drop ALL | ✅ |
| 安全选项 | no-new-privileges | ✅ |
| 健康检查 | HEALTHCHECK | ✅ |
| 最小化攻击面 | distroless/slim | ✅ |

---

## 测试

### 集成测试

```bash
# 运行所有 Dockerfile 测试
cd test-network
make test-all-dockerfiles

# 测试特定变体
make test-dockerfile VARIANT=debian
```

### 基准测试

```bash
# 镜像大小基准
docker images --format "table {{.Repository}}:{{.Tag}}\t{{.Size}}"

# 启动时间基准
time docker run --rm cis:v1.1.6-debian --version

# 内存占用基准
docker run --rm cis:v1.1.6-debian &
docker stats --no-stream --format "table {{.Container}}\t{{.MemUsage}}"
```

---

## 故障排查

### 构建失败

```bash
# 清理构建缓存
docker builder prune -a

# 使用 --no-cache 强制重新构建
docker build --no-cache -f Dockerfile.debian -t cis:test .

# 查看构建详情
docker build --progress=plain -f Dockerfile.debian -t cis:test .
```

### 运行时问题

```bash
# 调试模式（运行 shell）
docker run -it --rm --entrypoint bash cis:v1.1.6-debian

# 查看日志
docker logs <container-id>

# 进入运行中的容器
docker exec -it <container-id> bash
```

---

## 发布验证

### 发布前检查清单

- [x] 所有 Dockerfile 测试通过
- [x] 镜像大小符合要求（<200MB）
- [x] CVE 扫描无高危漏洞
- [x] 多平台构建成功（amd64/arm64）
- [x] 健康检查正常工作
- [x] 非 root 用户运行
- [x] 文档完整（README.md）
- [x] 示例 docker-compose.yml 可用

### 发布标签

```bash
# 标记镜像
docker tag cis:v1.1.6-debian your-registry/cis:1.1.6
docker tag cis:v1.1.6-debian your-registry/cis:latest

# 推送到 registry
docker push your-registry/cis:1.1.6
docker push your-registry/cis:latest
```

---

## 相关资源

- [Docker 官方文档](https://docs.docker.com/)
- [Dockerfile 最佳实践](https://docs.docker.com/develop/develop-images/dockerfile_best-practices/)
- [多阶段构建](https://docs.docker.com/develop/develop-images/multistage-build/)
- [安全扫描工具](https://github.com/aquasecurity/trivy)

---

**归档日期**: 2026-02-13
**维护者**: CIS Team
