# CIS v1.1.2 Release Notes

**发布日期**: 2026-02-09  
**版本**: 1.1.2  
**状态**: 稳定版

---

## 🐛 问题修复

### CLI Provider 调用格式修复
修复了所有 AI Agent Provider 的 CLI 调用格式问题：

| Provider | 修复前 | 修复后 |
|---------|-------|-------|
| Claude | `claude -- prompt` | `claude prompt` |
| Kimi | `kimi chat --no-stream -- prompt` | `kimi chat --no-stream prompt` |
| OpenCode | `opencode run --format json -- prompt` | `opencode run --format json prompt` |

**问题原因**: `--` 分隔符导致 prompt 被错误解析，造成 "no prompt provided" 错误。

### 编译配置优化
- **默认启用所有功能**: `cargo build --release` 开箱即用
- 默认功能: `["encryption", "vector", "p2p"]`
- 修复了 network.rs 中的 borrow checker 错误

---

## 🚀 快速开始

### 安装

```bash
# 克隆仓库
git clone https://github.com/MoSiYuan/CIS.git
cd CIS

# 构建（默认启用全部功能）
cargo build --release --package cis-node

# 安装
cp target/release/cis-node ~/.local/bin/cis
```

### 初始化与启动

```bash
# 初始化
cis init

# 启动节点
cis node start
```

### 多主机组网

**Coordinator 节点:**
```bash
cis node start --role coordinator
```

**Worker 节点:**
```bash
# 配置引导节点后启动
cis node start --role worker
```

---

## 📋 版本信息

| 组件 | 版本 |
|-----|------|
| cis-core | 1.1.2 |
| cis-node | 1.1.2 |
| cis-skill-sdk | 1.1.2 |
| cis-gui | 1.1.2 |

---

## 📚 相关文档

- [安装指南](INSTALL.md)
- [更新日志](CHANGELOG.md)
- [快速开始](START.md)

---

## ⬆️ 升级说明

从 v1.1.1 升级:
```bash
git pull origin main
cargo build --release --package cis-node
cp target/release/cis-node ~/.local/bin/cis
```

从 v1.1.0 升级:
```bash
# 备份配置
cp -r ~/.config/cis ~/.config/cis.backup

# 拉取更新
git pull origin main

# 重新构建
cargo build --release --package cis-node
cp target/release/cis-node ~/.local/bin/cis

# 重新初始化
cis init --force
```
