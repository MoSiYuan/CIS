# CIS 安装指南

本文档介绍在不同平台上安装 CIS 的方法。

## 系统要求

### 最低要求
- **操作系统**: Linux, macOS, Windows
- **内存**: 512 MB RAM
- **磁盘**: 100 MB 可用空间
- **网络**: 可选的 P2P 网络端口

### 推荐配置
- **内存**: 2 GB RAM
- **磁盘**: 1 GB SSD
- **网络**: 稳定的互联网连接（用于 AI Provider）

## 安装方式

### 方式一：自动安装脚本（推荐）

#### macOS / Linux

```bash
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash
```

#### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.ps1 | iex
```

### 方式二：Homebrew（macOS/Linux）

```bash
# 添加 Tap
brew tap mosiyuan/cis

# 安装 CIS
brew install cis

# 启用 Shell 集成
echo 'source $(brew --prefix)/share/cis/cis.bash' >> ~/.bashrc  # Bash
echo 'source $(brew --prefix)/share/cis/cis.zsh' >> ~/.zshrc    # Zsh
```

### 方式三：预编译二进制

#### macOS

```bash
# Intel
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-macos-x86_64.tar.gz | tar xz
sudo mv cis-node cis-cli /usr/local/bin/

# Apple Silicon
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-macos-arm64.tar.gz | tar xz
sudo mv cis-node cis-cli /usr/local/bin/
```

#### Linux

```bash
# x86_64
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-linux-x86_64.tar.gz | tar xz
sudo mv cis-node cis-cli /usr/local/bin/

# ARM64
curl -fsSL https://github.com/MoSiYuan/CIS/releases/latest/download/cis-linux-arm64.tar.gz | tar xz
sudo mv cis-node cis-cli /usr/local/bin/
```

#### Windows

```powershell
# 下载并解压
Invoke-WebRequest -Uri "https://github.com/MoSiYuan/CIS/releases/latest/download/cis-windows-x86_64.zip" -OutFile "cis.zip"
Expand-Archive -Path "cis.zip" -DestinationPath "C:\Program Files\CIS"

# 添加到 PATH
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\Program Files\CIS", "User")
```

### 方式四：从源码构建

#### 前置要求

- Rust 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- pkg-config
- libssl-dev
- libsqlite3-dev

#### 构建步骤

```bash
# 克隆仓库
git clone https://github.com/MoSiYuan/CIS.git
cd CIS

# 构建 Release 版本
cargo build --release

# 安装二进制文件
sudo cp target/release/cis-node /usr/local/bin/
sudo cp target/release/cis-cli /usr/local/bin/

# 可选：安装 Shell 补全
mkdir -p ~/.local/share/cis
cp packaging/shell/cis.bash ~/.local/share/cis/
echo 'source ~/.local/share/cis/cis.bash' >> ~/.bashrc
```

### 方式五：Docker

```bash
# 拉取镜像
docker pull mosiyuan/cis:latest

# 运行
docker run -d \
  -p 7676:7676 \
  -p 7677:7677/udp \
  -p 6767:6767 \
  -v cis-data:/var/lib/cis/data \
  mosiyuan/cis:latest
```

或使用 Docker Compose:

```bash
docker-compose up -d
```

## 验证安装

```bash
# 检查版本
cis-node --version
cis-cli --version

# 初始化（首次运行）
cis init

# 检查状态
cis node status
```

## 升级

### Homebrew

```bash
brew upgrade cis
```

### 脚本安装

```bash
# 重新运行安装脚本
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash
```

### 手动升级

```bash
# 下载新版本
# ...（参考上面的安装步骤）

# 数据会自动保留
# 配置文件位于 ~/.cis/
```

## 卸载

### Homebrew

```bash
brew uninstall cis
brew untap mosiyuan/cis
```

### 手动卸载

```bash
# 删除二进制文件
sudo rm /usr/local/bin/cis-node /usr/local/bin/cis-cli

# 删除数据（注意：这会删除所有数据！）
rm -rf ~/.cis

# 删除 Shell 集成配置
# 编辑 ~/.bashrc 或 ~/.zshrc，删除 CIS 相关行
```

## 平台特定说明

### macOS

#### 安全提示

首次运行可能遇到安全警告：

```
"cis-node" cannot be opened because the developer cannot be verified.
```

解决方法：

1. **系统设置** -> **隐私与安全** -> **安全性**
2. 点击 "仍然允许"
3. 重新运行命令

或终端执行：

```bash
xattr -dr com.apple.quarantine /usr/local/bin/cis-node
xattr -dr com.apple.quarantine /usr/local/bin/cis-cli
```

### Linux

#### systemd 服务

创建服务文件：

```bash
sudo tee /etc/systemd/system/cis.service > /dev/null <<EOF
[Unit]
Description=CIS Node
After=network.target

[Service]
Type=simple
User=$USER
ExecStart=/usr/local/bin/cis-node daemon
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl daemon-reload
sudo systemctl enable cis
sudo systemctl start cis
```

### Windows

#### 防火墙配置

CIS 需要以下端口：

- 7676/tcp - Federation API
- 7677/udp - P2P QUIC
- 6767/tcp - WebSocket

Windows Defender 防火墙可能会阻止这些端口，需要手动允许。

## 故障排除

### 命令未找到

```bash
# 检查 PATH
echo $PATH

# 添加 PATH
export PATH="$PATH:/usr/local/bin"
```

### 权限错误

```bash
# 修复权限
chmod +x /usr/local/bin/cis-node
chmod +x /usr/local/bin/cis-cli

# 或重新安装
sudo cp target/release/cis-node /usr/local/bin/
```

### 依赖缺失

#### Ubuntu/Debian

```bash
sudo apt-get install -y libssl-dev libsqlite3-dev pkg-config
```

#### CentOS/RHEL/Fedora

```bash
sudo dnf install -y openssl-devel sqlite-devel pkgconfig
```

#### macOS

```bash
brew install openssl sqlite pkg-config
```

## 下一步

- [快速开始](./quickstart.md) - 学习基础使用
- [配置指南](./configuration.md) - 了解配置选项
- [网络设置](./network-configuration.md) - 配置 P2P 网络
