# CIS 版本发布文档

**当前版本**: v1.0.0 (Foundation)  
**发布日期**: 2026-02-08  

---

## 📦 版本列表

### 稳定版本

| 版本 | 名称 | 日期 | 状态 | 文档 |
|------|------|------|------|------|
| [v1.0.0](v1.0.0/) | Foundation | 2026-02-08 | ✅ LTS | [详情](v1.0.0/README.md) |

### 开发版本

| 版本 | 预计日期 | 状态 | 主要特性 |
|------|----------|------|----------|
| v1.0.1 | 2026-02 末 | 🚧 开发中 | 稳定性修复 |
| v1.1.0 | 2026 Q2 | 📋 计划中 | GUI 生产化、WASM、IM、P2P |
| v1.2.0 | 2026 Q3 | 📋 计划中 | 移动端、插件市场 |
| v2.0.0 | 2026 Q4 | 📋 计划中 | 完整去中心化 |

---

## 🗂️ v1.0.0 文档结构

```
releases/v1.0.0/
├── README.md              # 版本发布说明
├── VERSION.md             # 版本详细说明
├── CHANGELOG.md           # 变更日志
├── core/                  # 核心机制文档
│   ├── ARCHITECTURE.md    # 系统架构
│   ├── STORAGE.md         # 存储设计 (Matrix 数据库分离)
│   └── MATRIX.md          # Matrix 联邦 (端口分工)
└── archives/              # 归档文档
    ├── kimi_agent.md      # 评估报告
    ├── COMPLETION_ROADMAP.md  # 完善路线图
    └── EXECUTION_PLAN.md      # 执行计划
```

---

## 🔗 快速导航

### 对于用户
- [v1.0.0 发布说明](v1.0.0/README.md) - 快速了解版本
- [v1.0.0 详细说明](v1.0.0/VERSION.md) - 完整版本信息
- [已知问题](v1.0.0/VERSION.md#known-issues) - 查看限制

### 对于开发者
- [系统架构](v1.0.0/core/ARCHITECTURE.md) - 了解整体设计
- [存储设计](v1.0.0/core/STORAGE.md) - 数据库架构
- [完善路线图](v1.0.0/archives/COMPLETION_ROADMAP.md) - 未来规划

### 对于贡献者
- [执行计划](v1.0.0/archives/EXECUTION_PLAN.md) - 可参与的任务
- [评估报告](v1.0.0/archives/kimi_agent.md) - 现状分析

---

## 📊 版本状态概览

### v1.0.0 (当前)

```
完成度: ████████░░ 80%

核心引擎:   ██████████ 90% ✅ 稳定
Matrix 联邦: █████████░ 85% ✅ 稳定
存储系统:   █████████░ 90% ✅ 稳定
GUI 界面:   █████░░░░░ 50% ⚠️ 演示阶段
WASM Skill: ████░░░░░░ 40% ⚠️ 框架待完善
P2P 网络:   ██████░░░░ 60% ⚠️ 部分实现
```

---

## 🚀 使用指南

### 安装最新版本

```bash
# 克隆仓库
git clone https://github.com/opencode/CIS.git
cd CIS

# 切换到 v1.0.0
git checkout v1.0.0

# 编译发布版本
cargo build --release
```

### 运行

```bash
# 查看版本
./target/release/cis --version
# cis 1.0.0

# 获取帮助
./target/release/cis --help
```

---

## 📝 版本选择建议

| 场景 | 推荐版本 | 说明 |
|------|----------|------|
| 生产环境 | v1.0.0 | 核心功能稳定 |
| 功能尝鲜 | develop | 最新功能，不稳定 |
| 贡献代码 | develop | 基于最新分支开发 |

---

## 📞 获取帮助

- **GitHub Issues**: https://github.com/opencode/CIS/issues
- **文档**: https://docs.cis.dev
- **社区**: https://discord.gg/cis

---

**维护者**: CIS Core Team  
**最后更新**: 2026-02-08
