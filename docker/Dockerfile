# CIS v1.1.0 (Cluster of Independent Systems) - DAG Executor
# Multi-stage build for production deployment
# 
# 优化说明:
# - 使用 Rust 1.75 slim 镜像作为 builder
# - 使用 distroless 或 debian slim 作为 runtime
# - 分阶段构建以减小最终镜像体积
# - 支持缓存层优化

# ==================== Builder Stage ====================
FROM rust:1.75-slim-bookworm AS builder

WORKDIR /build

# 安装构建依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# 创建依赖缓存层
# 先复制 Cargo 文件以利用 Docker 缓存
COPY Cargo.toml Cargo.lock ./
COPY cis-core/Cargo.toml cis-core/
COPY cis-node/Cargo.toml cis-node/
COPY cis-gui/Cargo.toml cis-gui/
COPY skills/dag-executor/Cargo.toml skills/dag-executor/

# 创建虚拟 main.rs 以构建依赖
RUN mkdir -p cis-core/src cis-node/src cis-gui/src skills/dag-executor/src && \
    echo "fn main() {}" > cis-node/src/main.rs && \
    echo "pub fn lib() {}" > cis-core/src/lib.rs && \
    echo "pub fn lib() {}" > cis-gui/src/lib.rs && \
    echo "pub fn lib() {}" > skills/dag-executor/src/lib.rs

# 预编译依赖（利用缓存层）
RUN cargo build --release -p cis-node 2>/dev/null || true

# 复制实际源代码
COPY cis-core/src cis-core/src
COPY cis-node/src cis-node/src
COPY cis-gui/src cis-gui/src
COPY skills/dag-executor/src skills/dag-executor/src

# 更新文件时间戳以触发重新构建
RUN touch cis-node/src/main.rs

# 构建 release 二进制文件
RUN cargo build --release -p cis-node --bin cis-node && \
    cargo build --release -p cis-node --bin cis-cli 2>/dev/null || true

# 压缩二进制文件
RUN strip /build/target/release/cis-node || true && \
    if [ -f /build/target/release/cis-cli ]; then strip /build/target/release/cis-cli; fi

# ==================== Runtime Stage ====================
# 使用 debian slim 作为基础镜像
FROM debian:bookworm-slim AS runtime

WORKDIR /app

# 安装运行时依赖
# v1.1.0: 优化依赖安装，减少层数
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libsqlite3-0 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean \
    && apt-get autoremove -y

# 创建必要的目录
# v1.1.0: 使用标准 Linux 目录结构
RUN mkdir -p /var/lib/cis/data \
    /var/log/cis \
    /etc/cis \
    /tmp/cis

# 复制二进制文件
COPY --from=builder /build/target/release/cis-node /usr/local/bin/cis-node
COPY --from=builder /build/target/release/cis-cli /usr/local/bin/cis-cli 2>/dev/null || true

# 设置默认配置文件
# v1.1.0: 使用嵌入式默认配置
RUN echo '[node]' > /etc/cis/config.toml && \
    echo 'id = ""' >> /etc/cis/config.toml && \
    echo 'name = "cis-node"' >> /etc/cis/config.toml && \
    echo 'role = "coordinator"' >> /etc/cis/config.toml && \
    echo '' >> /etc/cis/config.toml && \
    echo '[p2p]' >> /etc/cis/config.toml && \
    echo 'enabled = true' >> /etc/cis/config.toml && \
    echo 'listen_address = "0.0.0.0:7677"' >> /etc/cis/config.toml && \
    echo '' >> /etc/cis/config.toml && \
    echo '[federation]' >> /etc/cis/config.toml && \
    echo 'enabled = true' >> /etc/cis/config.toml && \
    echo 'listen_address = "0.0.0.0:7676"' >> /etc/cis/config.toml

# 创建非 root 用户
# v1.1.0: 使用固定的 UID/GID 便于 Kubernetes 安全上下文
RUN groupadd -r -g 1000 cis && \
    useradd -r -u 1000 -g cis cis && \
    chown -R cis:cis /var/lib/cis /var/log/cis /etc/cis /tmp/cis

# 暴露端口
# 7676: Federation API (HTTP)
# 7677: P2P QUIC 传输
# 6767: Matrix Federation + Agent Session (WebSocket)
EXPOSE 7676 7677/udp 6767

# 健康检查
# v1.1.0: 优化健康检查参数
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -fs http://localhost:7676/health || exit 1

# 设置环境变量
ENV RUST_LOG=info \
    CIS_VERSION=1.1.0 \
    CIS_DATA_DIR=/var/lib/cis/data \
    CIS_LOG_DIR=/var/log/cis \
    CIS_CONFIG_DIR=/etc/cis

# 切换到非 root 用户
USER cis

# 默认命令
ENTRYPOINT ["cis-node"]
CMD ["daemon", "--config", "/etc/cis/config.toml"]

# ==================== GPU Runtime Stage (Optional) ====================
# 如需 GPU 支持，取消下面注释并使用 nvidia/cuda 基础镜像
# FROM nvidia/cuda:12.0-runtime-ubuntu22.04 AS runtime-gpu
# 
# WORKDIR /app
# 
# RUN apt-get update && apt-get install -y --no-install-recommends \
#     ca-certificates \
#     libsqlite3-0 \
#     curl \
#     && rm -rf /var/lib/apt/lists/*
# 
# COPY --from=builder /build/target/release/cis-node /usr/local/bin/cis-node
# 
# ENV NVIDIA_VISIBLE_DEVICES=all \
#     NVIDIA_DRIVER_CAPABILITIES=compute,utility
# 
# EXPOSE 7676 7677/udp 6767
# 
# HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
#     CMD curl -fs http://localhost:7676/health || exit 1
# 
# USER cis
# ENTRYPOINT ["cis-node"]
# CMD ["daemon", "--config", "/etc/cis/config.toml"]
