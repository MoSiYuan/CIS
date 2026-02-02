# CIS 架构设计文档 v1.0

## 概述

CIS (Cluster of Independent Systems) 是基于 AgentFlow 项目派生的硬件绑定型分布式系统，遵循第一性原理和奥卡姆剃刀原则。

## 核心设计原则

### 1. 第一性原理
- **记忆主权**：用户数据本地加密存储，永不上传云端
- **硬件锚定**：DID 与硬件指纹强绑定，确保身份真实性
- **P2P 联邦**：无中心协调器，节点间直接通信

### 2. 奥卡姆剃刀
- **最小依赖**：核心功能不依赖外部服务
- **模块化**：涌现功能作为 Skill 开发，不侵入核心
- **简化抽象**：避免过度设计，保持代码简洁

## 模块架构

```
┌─────────────────────────────────────────────────────────────┐
│                        CIS 架构                              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  表现层      │  │  控制层      │  │  物理层      │      │
│  │ (可选 Skill) │  │  (Core)      │  │  (Core)      │      │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤      │
│  │ • Dashboard  │  │ • Task DAG   │  │ • SQLite     │      │
│  │ • REST API   │  │ • Executor   │  │ • File Sys   │      │
│  │ • Webhook    │  │ • Scheduler  │  │ • P2P (QUIC) │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              核心 (cis-core)                         │   │
│  │  • types      - 核心数据结构                        │   │
│  │  • sandbox    - 安全沙箱                            │   │
│  │  • scheduler  - DAG 任务调度                        │   │
│  │  • error      - 统一错误处理                        │   │
│  │  • (future)   - memory, executor, p2p, identity     │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │          Skills (涌现功能，可选)                     │   │
│  │  • skill-dev-workflow - Skill 开发工作流 (核心)     │   │
│  │  • cis-api-skill       - REST API                   │   │
│  │  • cis-dashboard-skill - Web UI                     │   │
│  │  • cis-webhook-skill   - Webhook 集成               │   │
│  │  • cis-monitor-skill   - 资源监控                   │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## 当前实现状态

### ✅ Phase 0: 基础设施 (已完成)
- [x] 项目结构 (cis-core, cis-node, skills)
- [x] Cargo.toml 配置
- [x] 核心类型定义 (types.rs)
- [x] 统一错误处理 (error.rs)
- [x] 安全沙箱 (sandbox/mod.rs) - 100% 继承 AgentFlow
- [x] DAG 调度器 (scheduler/mod.rs) - 100% 继承 AgentFlow

### 🔄 Phase 1: 核心迁移 (进行中)
- [ ] 记忆系统 (memory/) - Kimi 负责
- [ ] 任务执行器 (executor/) - 从 AgentFlow 迁移
- [ ] 提示词构建器 (executor/prompt_builder.rs)
- [ ] Claude CLI 集成

### 📋 Phase 2: CIS 特有功能 (规划中)
- [ ] 硬件指纹模块 (hardware/fingerprint.rs)
- [ ] DID 管理 (identity/did.rs)
- [ ] P2P 通信 (p2p/) - libp2p + QUIC
- [ ] Merkle DAG 同步 (sync/)

### 🎯 Phase 3: Skills 开发 (优先级最高)
- [x] **skill-dev-workflow** - 默认 Skill 开发工作流 (Meta-Skill)
- [ ] cis-api-skill
- [ ] cis-dashboard-skill
- [ ] cis-webhook-skill

## 技术栈

### 核心依赖
```toml
tokio = "1.35"          # 异步运行时
rusqlite = "0.30"       # 数据库 (未来加密)
serde = "1.0"           # 序列化
thiserror = "1.0"       # 错误处理
tracing = "0.1"         # 日志
```

### 可选特性 (feature flags)
```toml
[features]
default = []
encryption = ["sqlx", "ed25519-dalek", "chacha20poly1305"]
p2p = ["prost", "tonic", "libp2p"]
full = ["encryption", "p2p"]
```

## 设计决策

### 1. 为什么分 cis-core 和 cis-node?
- **cis-core**: 库 (library)，可被嵌入为 Skill
- **cis-node**: 可执行文件，实际运行的节点

### 2. 为什么使用 feature flags?
- 允许用户按需启用功能 (如加密、P2P)
- 减少默认依赖，保持核心简洁
- 支持最小化部署 (如无 P2P 的单机版)

### 3. 为什么涌现功能都是 Skills?
- 遵循奥卡姆剃刀原则
- 核心保持最小化
- 用户可自定义开发
- 便于社区贡献

## 下一步工作

### 当前优先级: **skill-dev-workflow** Meta-Skill

这是最重要的 Skill，因为它定义了如何开发其他 Skills。包含：

1. **用户需求分析** - 理解用户真实需求
2. **需求数据整理与文档整合** - 结构化需求
3. **基于需求与现状的功能设计** - 设计方案
4. **执行开发** - 编写代码
5. **验收测试** - 验证功能
6. **给出总结报告** - 文档化

---

**文档版本**: v1.0
**最后更新**: 2026-02-02
**作者**: CIS Team
