# 🚀 CIS v1.1.2 快速开始指南

## 安装

### 方式一：自动安装脚本（推荐）

```bash
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash
```

### 方式二：从源码构建

```bash
# 克隆仓库
git clone https://github.com/MoSiYuan/CIS.git
cd CIS

# 构建（默认启用所有功能：vector, p2p, encryption）
cargo build --release --package cis-node

# 安装
cp target/release/cis-node ~/.local/bin/cis
```

## 初始化

```bash
# 交互式初始化
cis init

# 或使用快速初始化
cis init --non-interactive --provider claude
```

## 启动节点

### 单节点模式
```bash
cis node start
```

### 多主机组网

**Coordinator (协调节点):**
```bash
# 1. 获取本机 DID
cis node info

# 2. 启动
cis node start --role coordinator
```

**Worker (工作节点):**
```bash
# 1. 配置引导节点（编辑 ~/.config/cis/config.yaml）
# network:
#   p2p:
#     bootstrap_nodes:
#       - "/ip4/<COORDINATOR_IP>/tcp/7677"

# 2. 启动
cis node start --role worker
```

## 验证连接

```bash
# 查看节点状态
cis node status

# 查看对等节点
cis network peers

# 测试连通性
cis network ping <对方节点DID>
```

## 使用 Agent

```bash
# 执行单任务
cis agent execute "分析当前目录的代码结构"

# 使用 DAG 执行复杂任务
cis dag run examples/dag-code-review.yaml
```

## 更多信息

- [安装指南](INSTALL.md) - 详细安装和配置说明
- [CHANGELOG](CHANGELOG.md) - 版本更新日志
- [API 文档](docs/API.md) - 开发接口文档
