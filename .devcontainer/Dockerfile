# CIS Dev Container Dockerfile
# 用于 VS Code Dev Containers

FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

# 安装系统依赖
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    git \
    ca-certificates \
    curl \
    vim \
    htop \
    tree \
    jq \
    # 网络工具
    net-tools \
    iputils-ping \
    dnsutils \
    telnet \
    # 调试工具
    gdb \
    lldb \
    strace \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# 安装 cargo 工具
RUN cargo install cargo-watch cargo-expand cargo-edit cargo-audit

# 设置环境变量
ENV RUST_LOG=debug
ENV RUST_BACKTRACE=1
ENV CARGO_TARGET_DIR=/app/target

# 创建工作目录
WORKDIR /app

# 确保 vscode 用户有权限
RUN mkdir -p /app/.cis /app/target && \
    chown -R vscode:vscode /app

USER vscode
