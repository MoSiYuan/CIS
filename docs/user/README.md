# CIS 用户指南

欢迎来到 CIS 用户指南！这里提供从入门到精通的完整文档。

## 目录

### 快速开始
- [安装 CIS](./installation.md) - 多种安装方式详解
- [5 分钟上手指南](./quickstart.md) - 快速体验 CIS
- [基础概念](./concepts.md) - 理解 CIS 的核心概念

### 核心功能
- [节点管理](./node-management.md) - 启动、配置和管理节点
- [DAG 工作流](./dag-workflow.md) - 创建和运行工作流
- [记忆管理](./memory-management.md) - 存储和检索信息
- [网络配置](./network-configuration.md) - P2P 网络和对等节点

### 进阶主题
- [安全配置](./security.md) - DID、ACL 和加密
- [备份与恢复](./backup-restore.md) - 数据保护策略
- [故障排除](./troubleshooting.md) - 常见问题解决
- [性能优化](./performance.md) - 调优指南

### 集成
- [Shell 集成](../../packaging/shell/README.md) - Bash/Zsh/Fish 集成
- [VS Code 插件](../../packaging/vscode-cis/README.md) - 编辑器集成
- [Matrix Bridge](../../packaging/matrix-appservice/README.md) - 消息集成

## 快速参考

### 常用命令

```bash
# 初始化
cis init

# 启动节点
cis node start

# 查看状态
cis node status

# 运行 DAG
cis dag run <name>

# 搜索记忆
cis memory search <query>

# 查看帮助
cis --help
cis <command> --help
```

### 配置文件位置

| 平台 | 路径 |
|------|------|
| Linux | `~/.cis/config.toml` |
| macOS | `~/.cis/config.toml` |
| Windows | `%USERPROFILE%\.cis\config.toml` |

### 数据存储位置

| 平台 | 路径 |
|------|------|
| Linux | `~/.cis/data/` |
| macOS | `~/.cis/data/` |
| Windows | `%USERPROFILE%\.cis\data\` |

### 获取帮助

- 📖 [完整文档](./README.md)
- 🐛 [问题反馈](https://github.com/MoSiYuan/CIS/issues)
- 💬 [讨论社区](https://github.com/MoSiYuan/CIS/discussions)

## 下一步

1. 阅读 [安装指南](./installation.md) 安装 CIS
2. 完成 [快速开始](./quickstart.md) 教程
3. 探索 [示例项目](../../examples/)
