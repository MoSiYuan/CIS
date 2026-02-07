# CIS 文档中心

欢迎来到 CIS (Cluster of Independent Systems) 文档中心！

## 📚 文档导航

### 快速开始

新用户从这里开始：

| 文档 | 说明 |
|------|------|
| [安装指南](getting-started/installation.md) | 各种安装方式详解 |
| [快速开始](getting-started/quickstart.md) | 5 分钟上手指南 |
| [配置说明](getting-started/configuration.md) | 配置文件详解 |

### 用户指南

日常使用参考：

| 文档 | 说明 |
|------|------|
| [CLI 命令参考](USAGE.md) | 完整命令列表 |
| [故障排除](TROUBLESHOOTING.md) | 常见问题解决方案 |
| [Shell 别名](../scripts/shell-aliases.sh) | 快捷命令别名 |

### 开发文档

开发者参考：

| 文档 | 说明 |
|------|------|
| [架构设计](ARCHITECTURE.md) | 系统架构概述 |
| [开发指南](SKILL_DEVELOPMENT.md) | Skill 开发教程 |
| [API 文档](api/README.md) | Rust API 文档 |
| [Matrix 联邦](MATRIX_FEDERATION_IMPROVEMENT_PLAN.md) | 网络协议设计 |

### 贡献指南

参与项目：

| 文档 | 说明 |
|------|------|
| [贡献指南](../CONTRIBUTING.md) | 如何贡献代码 |
| [代码规范](contributing/code-style.md) | 编码规范 |
| [提交规范](contributing/commit-convention.md) | Git 提交规范 |

---

## 🚀 5 分钟快速开始

```bash
# 1. 安装 CIS
curl -fsSL https://raw.githubusercontent.com/MoSiYuan/CIS/main/scripts/install/install.sh | bash

# 2. 初始化
cis init

# 3. 检查环境
cis doctor

# 4. 开始使用
cis skill list
cis skill do "分析今天的代码提交"
```

---

## 📖 按场景查找文档

### 我是新用户
1. 阅读 [安装指南](getting-started/installation.md)
2. 跟随 [快速开始](getting-started/quickstart.md)
3. 遇到问题查看 [故障排除](TROUBLESHOOTING.md)

### 我想开发 Skill
1. 阅读 [架构设计](ARCHITECTURE.md) 了解 Skill 系统
2. 学习 [开发指南](SKILL_DEVELOPMENT.md)
3. 参考 [示例 Skill](../skills/)

### 我想参与开发
1. 阅读 [贡献指南](../CONTRIBUTING.md)
2. 设置 [开发环境](../CONTRIBUTING.md#开发环境设置)
3. 查看 [代码规范](contributing/code-style.md)

### 我想部署多节点
1. 阅读 [网络配置](NETWORKING.md)
2. 了解 [Matrix 联邦](MATRIX_FEDERATION_IMPROVEMENT_PLAN.md)
3. 查看 [P2P 实现](P2P_IMPLEMENTATION_COMPLETE.md)

---

## 📋 文档状态

| 文档 | 状态 | 最后更新 |
|------|------|----------|
| 安装指南 | ✅ 完整 | 2026-02-07 |
| 快速开始 | ✅ 完整 | 2026-02-07 |
| 故障排除 | ✅ 完整 | 2026-02-07 |
| API 文档 | ⚠️ 部分 | 2026-02-01 |
| 架构设计 | ✅ 完整 | 2026-02-05 |

---

## 💬 获取帮助

- **GitHub Issues**: [提交问题](https://github.com/MoSiYuan/CIS/issues)
- **文档反馈**: 文档中有错误？请提交 Issue 或 PR

---

**提示**: 使用 `cis <command> --help` 查看任何命令的详细帮助。
