# CIS v1.1.4 版本计划

## 概述

v1.1.4 是一个专注于**技术债务清理**和**核心功能完善**的版本。本版本将解决 v1.1.3 中发现的 70+ 处简化实现，确保 CIS 的核心功能稳定可用。

## 核心目标 (v1.1.4 最终范围)

基于 [SCOPE_FINAL.md](./SCOPE_FINAL.md) 确认：

### P0 - 必须完成 (6项, 12周)
1. **架构重构 Phase 1-3** - 配置抽象 + 全局状态消除 + 事件总线
2. **安全基线建立** - 威胁模型 + 加固清单
3. **WASM 基础执行** - 加载 → 执行 → 清理
4. **P2P 传输加密** - Noise Protocol 集成
5. **WASM 沙箱审计** - 资源限制 + 逃逸测试
6. **测试框架搭建** - CI/CD + Mock 框架

### P1 - 尽量完成 (8项)
- P2P 连接处理循环、Agent 联邦事件订阅、远程任务处理
- 输入验证框架、命令白名单、测试用例编写
- DAG Skill 执行、安全加固实施

### 延后到 v1.1.5
- 架构重构 Phase 4-5、端口合并、P2P DHT 完整实现
- NAT Relay 打洞、Worker 监控、终端 Resize

## 文档结构

```
v1.1.4/
├── README.md                          # 本文件
├── ROADMAP.md                         # 版本路线图
├── SCOPE_FINAL.md                     # 最终范围确认
├── SIMPLIFIED_IMPLEMENTATION_ANALYSIS.md  # 简化实现分析报告
├── ARCHITECTURE_REVIEW.md             # 架构评审与耦合分析
├── ARCHITECTURE_QUICK_REF.md          # 架构快速参考
├── PORT_CONSOLIDATION_PROPOSAL.md     # 端口合并提案 (P2)
├── PORT_CONSOLIDATION_QUICK.md        # 端口合并快速参考
├── REVIEW_CLAUDE.md                   # Claude 评审报告
├── SECURITY_ASSESSMENT.md             # 安全评估与加固计划
├── TESTING_STRATEGY.md                # 测试策略
├── TEAM_REVIEW_AGENDA.md              # 团队评审议程
├── TASK_BREAKDOWN.md                  # 详细任务分解与分配
└── designs/                           # 设计文档目录
    ├── D01_CONFIG_ABSTRACTION.md      # D01: 配置抽象设计
    ├── D02_GLOBAL_STATE_ELIMINATION.md # D02: 全局状态消除
    ├── D03_EVENT_BUS.md               # D03: 事件总线设计
    ├── D04_WASM_EXECUTION.md          # D04: WASM 执行设计
    ├── D05_P2P_ENCRYPTION.md          # D05: P2P 加密设计
    └── .gitkeep
```

## 快速导航

### 范围与计划
- **最终范围确认** → [SCOPE_FINAL.md](./SCOPE_FINAL.md) ⭐ 必读
- **版本路线图** → [ROADMAP.md](./ROADMAP.md)
- **任务分解与分配** → [TASK_BREAKDOWN.md](./TASK_BREAKDOWN.md) ⭐ 任务分配

### 设计文档 (P0)
- **D01: 配置抽象** → [designs/D01_CONFIG_ABSTRACTION.md](./designs/D01_CONFIG_ABSTRACTION.md) (Week 1-2)
- **D02: 全局状态消除** → [designs/D02_GLOBAL_STATE_ELIMINATION.md](./designs/D02_GLOBAL_STATE_ELIMINATION.md) (Week 2-3)
- **D03: 事件总线** → [designs/D03_EVENT_BUS.md](./designs/D03_EVENT_BUS.md) (Week 3-4)
- **D04: WASM 执行** → [designs/D04_WASM_EXECUTION.md](./designs/D04_WASM_EXECUTION.md) (Week 5-6)
- **D05: P2P 加密** → [designs/D05_P2P_ENCRYPTION.md](./designs/D05_P2P_ENCRYPTION.md) (Week 5-6)

### 问题分析
- **功能问题清单** → [SIMPLIFIED_IMPLEMENTATION_ANALYSIS.md](./SIMPLIFIED_IMPLEMENTATION_ANALYSIS.md)
- **架构耦合分析** → [ARCHITECTURE_REVIEW.md](./ARCHITECTURE_REVIEW.md)
- **架构快速参考** → [ARCHITECTURE_QUICK_REF.md](./ARCHITECTURE_QUICK_REF.md)

### 评审报告
- **Claude 评审报告** → [REVIEW_CLAUDE.md](./REVIEW_CLAUDE.md)
- **安全评估** → [SECURITY_ASSESSMENT.md](./SECURITY_ASSESSMENT.md)
- **测试策略** → [TESTING_STRATEGY.md](./TESTING_STRATEGY.md)
- **团队评审议程** → [TEAM_REVIEW_AGENDA.md](./TEAM_REVIEW_AGENDA.md)

## 关键统计数据

| 指标 | 数值 |
|------|------|
| 简化实现总数 | 70+ |
| 🔴 P0 优先级 | 4 项 |
| 🟡 P1 优先级 | 10 项 |
| 🟢 P2 优先级 | 其余 |
| 预计工期 | 4-6 周 |

## 问题分布

```
P2P 网络       ████████████████████  15处
Agent 联邦     ██████████████        10处
CLI 命令       ████████████████████████  25处
调度器         ██████                 4处
WASM 执行      █████                  4处
其他           ████████████           12处
```

## 开发原则

1. **优先修复 P0** - 阻塞性功能优先
2. **保持向后兼容** - 不破坏现有 API
3. **测试驱动** - 每个修复都需配套测试
4. **文档同步** - 代码变更同步更新文档

## 参与贡献

欢迎参与 v1.1.4 开发！请：

1. 阅读 [REVIEW_CLAUDE.md](./REVIEW_CLAUDE.md) 了解外部评审意见
2. 阅读 [SIMPLIFIED_IMPLEMENTATION_ANALYSIS.md](./SIMPLIFIED_IMPLEMENTATION_ANALYSIS.md) 了解功能问题
3. 阅读 [ARCHITECTURE_REVIEW.md](./ARCHITECTURE_REVIEW.md) 了解架构问题
4. 阅读 [SECURITY_ASSESSMENT.md](./SECURITY_ASSESSMENT.md) 了解安全要求
5. 阅读 [TESTING_STRATEGY.md](./TESTING_STRATEGY.md) 了解测试要求
6. 在 [TASK_BREAKDOWN.md](./TASK_BREAKDOWN.md) 中认领任务
7. 在 `designs/` 目录创建设计文档 (复杂功能)
8. 提交 PR 并关联本计划

## 时间线

```
Week 1-2:  WASM 执行修复
Week 2-3:  P2P 网络完善
Week 3-4:  Agent 联邦
Week 4-5:  调度器完善
Week 5-6:  CLI 优化 & 测试
```

## 联系

如有问题，请在项目仓库创建 Issue 讨论。

---

*计划创建日期: 2026-02-10*  
*计划版本: v1.1.0*  
*状态: ✅ 核心功能开发完成*
