# CIS v1.2.0 开发任务清单

> **版本**: v1.2.0  
> **目标**: 模块化架构 + ZeroClaw 兼容 + 多 Agent 支持

---

## 📋 任务概览

| 阶段 | 名称 | 周期 | 状态 | 任务数 |
|------|------|------|------|--------|
| Phase 0 | 研究与分析 | - | ✅ 完成 | 2 |
| Phase 1 | cis-common 基础 | Week 1 | ⏳ 待开始 | 4 |
| Phase 2 | 模块提取 | Week 2-5 | ⏳ 待开始 | 5 |
| Phase 3 | cis-core 重构 | Week 6-7 | ⏳ 待开始 | 5 |
| Phase 4 | ZeroClaw 兼容 | Week 8-9 | ⏳ 待开始 | 4 |
| Phase 5 | 测试与验证 | Week 9-10 | ⏳ 待开始 | 3 |
| Phase 6 | 发布准备 | Week 11-12 | ⏳ 待开始 | 3 |
| Phase 7 | 多 Agent 架构 (P3) | Week 13+ | ⏳ 待开始 | 6 |
| **总计** | | **Week 1-16+** | | **32** |

---

## 📁 目录结构

```
task/
├── README.md                    # 本文件
├── phase0_research/            # Phase 0: 研究 (已完成)
│   ├── TASK_0_1_ZEROCLAW_ANALYSIS.md
│   └── TASK_0_2_PLAN_INTEGRATION.md
├── phase1_cis_common/          # Phase 1: 基础 (Week 1)
│   ├── TASK_1_1_CREATE_CRATE.md
│   ├── TASK_1_2_EXTRACT_TYPES.md
│   ├── TASK_1_3_EXTRACT_TRAITS.md
│   └── TASK_1_4_UPDATE_WORKSPACE.md      # 新增
├── phase2_extract_modules/     # Phase 2: 模块提取 (Week 2-5)
│   ├── TASK_2_1_EXTRACT_STORAGE.md
│   ├── TASK_2_2_EXTRACT_MEMORY.md
│   ├── TASK_2_3_EXTRACT_SCHEDULER.md
│   ├── TASK_2_4_EXTRACT_VECTOR.md
│   └── TASK_2_5_EXTRACT_P2P.md
├── phase3_refactor_core/       # Phase 3: 核心重构 (Week 6-7)
│   ├── TASK_3_1_DEPENDENCY_UPDATE.md
│   ├── TASK_3_2_CORE_REFACTOR.md
│   ├── TASK_3_3_REMOVE_MODULES.md        # 新增
│   ├── TASK_3_4_UPDATE_IMPORTS.md        # 新增
│   └── TASK_3_5_TEST_BUILD.md            # 新增
├── phase4_zeroclaw/            # Phase 4: ZeroClaw 兼容 (Week 8-9)
│   ├── TASK_4_1_ADAPTER_LAYER.md
│   ├── TASK_4_2_E2E_TEST.md
│   ├── TASK_4_3_INTEGRATION_TESTS.md     # 新增
│   └── TASK_4_4_DOCUMENTATION.md         # 新增
├── phase5_testing/             # Phase 5: 测试 (Week 9-10)
│   ├── TASK_5_1_TEST_FRAMEWORK.md
│   ├── TASK_5_2_CI_CONFIG.md
│   └── TASK_5_3_BENCHMARKS.md            # 新增
├── phase6_release/             # Phase 6: 发布 (Week 11-12)
│   ├── TASK_6_1_DOC_UPDATE.md
│   ├── TASK_6_2_RELEASE.md
│   └── TASK_6_3_RELEASE_CIS.md           # 新增
└── phase7_multi_agent/         # Phase 7: 多 Agent (P3, Week 13+)
    ├── TASK_7_1_AGENT_TRAIT.md
    ├── TASK_7_2_RECEPTIONIST.md
    ├── TASK_7_3_WORKER_AGENTS.md
    ├── TASK_7_4_DAG_ORCHESTRATION.md
    ├── TASK_7_5_MEMORY_ISOLATION.md      # 新增
    └── TASK_7_6_INTEGRATION_TESTS.md     # 可选新增
```

---

## 🎯 关键里程碑

### Milestone 1: Core Ready (Week 7)
- ✅ 所有 7 个 crates 提取完成
- ✅ cis-core 重构为轻量协调层
- ✅ Builder 模式可用
- ✅ 向后兼容层工作

### Milestone 2: ZeroClaw Compatible (Week 9)
- ✅ ZeroClaw 可作为 CIS backend 运行
- ✅ 适配层测试通过
- ✅ 性能损失 < 20%
- ✅ 文档完整

### Milestone 3: Production Ready (Week 12)
- ✅ 所有测试通过
- ✅ 性能基准测试完成
- ✅ 文档完整
- ✅ v1.2.0 发布到 crates.io

### Milestone 4: Multi-Agent (Week 16+, P3)
- ✅ Receptionist + Worker Agents
- ✅ DAG 编排
- ✅ P2P 跨设备调用
- ✅ 三级记忆隔离 + 四层幻觉过滤

---

## 🔗 依赖关系图

```
Phase 0: 研究
    │
    ▼
Phase 1: cis-common ─────────────────┐
    │                                 │
    ├── Task 1.1 (cis-common)         │
    ├── Task 1.2 (cis-types)          │
    ├── Task 1.3 (cis-traits) ◄───────┘
    └── Task 1.4 (workspace) ◄────────┘
    │
    ▼
Phase 2: 模块提取
    │
    ├── Task 2.1 (cis-storage) ◄────┬──依赖──► Task 1.3
    ├── Task 2.2 (cis-memory) ◄─────┼──依赖──► Task 2.1
    ├── Task 2.3 (cis-scheduler) ◄──┤
    ├── Task 2.4 (cis-vector) ◄─────┤
    └── Task 2.5 (cis-p2p) ◄────────┘
    │
    ▼
Phase 3: cis-core 重构 ◄──────────────依赖──► Phase 2 全部
    │
    ├── Task 3.1 (依赖更新)
    ├── Task 3.2 (核心重构)
    ├── Task 3.3 (移除模块)
    ├── Task 3.4 (更新导入) ◄───────依赖──► Task 3.3
    └── Task 3.5 (测试编译) ◄───────依赖──► Task 3.4
    │
    ▼
Phase 4: ZeroClaw 兼容 ◄──────────────依赖──► Task 3.5
    │
    ├── Task 4.1 (适配层)
    ├── Task 4.2 (E2E 验证)
    ├── Task 4.3 (集成测试)
    └── Task 4.4 (文档)
    │
    ▼
Phase 5: 测试 ◄───────────────────────依赖──► Task 4.4
    │
    ├── Task 5.1 (测试框架)
    ├── Task 5.2 (CI 配置)
    └── Task 5.3 (性能测试)
    │
    ▼
Phase 6: 发布 ◄───────────────────────依赖──► Task 5.3
    │
    ├── Task 6.1 (文档更新)
    ├── Task 6.2 (发布准备)
    └── Task 6.3 (正式发布)
    │
    ▼ (P3 可选)
Phase 7: 多 Agent ◄───────────────────依赖──► Task 6.3 + Task 2.5
    │
    ├── Task 7.1 (Agent trait)
    ├── Task 7.2 (Receptionist)
    ├── Task 7.3 (Worker Agents)
    ├── Task 7.4 (DAG 编排)
    ├── Task 7.5 (记忆隔离)
    └── Task 7.6 (集成测试)
```

---

## 📊 详细任务清单

### Phase 1: cis-common 基础 (Week 1)
| 任务 | 描述 | 负责人 | 状态 |
|------|------|--------|------|
| 1.1 | 创建 cis-common 目录结构 | TBD | ⏳ |
| 1.2 | 提取 cis-types crate | TBD | ⏳ |
| 1.3 | 定义 cis-traits crate | TBD | ⏳ |
| 1.4 | 更新根 workspace Cargo.toml | TBD | ⏳ |

### Phase 2: 模块提取 (Week 2-5)
| 任务 | 描述 | 依赖 | 状态 |
|------|------|------|------|
| 2.1 | 提取 cis-storage | 1.3 | ⏳ |
| 2.2 | 提取 cis-memory | 2.1 | ⏳ |
| 2.3 | 提取 cis-scheduler | 1.3 | ⏳ |
| 2.4 | 提取 cis-vector | 1.3 | ⏳ |
| 2.5 | 提取 cis-p2p | 1.3 | ⏳ |

### Phase 3: cis-core 重构 (Week 6-7)
| 任务 | 描述 | 依赖 | 状态 |
|------|------|------|------|
| 3.1 | 更新 cis-core 依赖 | Phase 2 | ⏳ |
| 3.2 | cis-core 核心重构 | 3.1 | ⏳ |
| 3.3 | 移除已提取的模块 | 3.2 | ⏳ |
| 3.4 | 更新导入语句 | 3.3 | ⏳ |
| 3.5 | 测试编译 | 3.4 | ⏳ |

### Phase 4: ZeroClaw 兼容 (Week 8-9)
| 任务 | 描述 | 依赖 | 状态 |
|------|------|------|------|
| 4.1 | 适配层实现 | 3.5 | ⏳ |
| 4.2 | 端到端验证 | 4.1 | ⏳ |
| 4.3 | 集成测试 | 4.2 | ⏳ |
| 4.4 | 文档 | 4.3 | ⏳ |

### Phase 5: 测试 (Week 9-10)
| 任务 | 描述 | 依赖 | 状态 |
|------|------|------|------|
| 5.1 | 测试框架 | 3.5 | ⏳ |
| 5.2 | CI 配置 | 5.1 | ⏳ |
| 5.3 | 性能基准测试 | 5.2 | ⏳ |

### Phase 6: 发布 (Week 11-12)
| 任务 | 描述 | 依赖 | 状态 |
|------|------|------|------|
| 6.1 | 文档更新 | 5.2 | ⏳ |
| 6.2 | 发布准备 | 6.1 | ⏳ |
| 6.3 | 发布 CIS v1.2.0 | 6.2 | ⏳ |

### Phase 7: 多 Agent (P3, Week 13+)
| 任务 | 描述 | 依赖 | 状态 |
|------|------|------|------|
| 7.1 | Agent trait 实现 | 6.3 | ⏳ |
| 7.2 | Receptionist Agent | 7.1 | ⏳ |
| 7.3 | Worker Agents | 7.1 | ⏳ |
| 7.4 | DAG 编排 | 7.2, 7.3 | ⏳ |
| 7.5 | 记忆分组与幻觉降低 | 7.1 | ⏳ |
| 7.6 | 集成测试 | 7.4, 7.5 | ⏳ |

---

## 📈 状态说明

| 图标 | 含义 |
|------|------|
| ✅ | 已完成 |
| 🔄 | 进行中 |
| ⏳ | 待开始 |
| ⏸️ | 阻塞中 |
| ❌ | 已取消 |

---

## 📝 使用指南

### 创建新任务

1. 根据阶段选择目录
2. 命名格式: `TASK_{phase}_{seq}_{NAME}.md`
3. 使用模板:

```markdown
# TASK X.Y: 任务名称

> **Phase**: N - 阶段名
> **状态**: ⏳ 待开始
> **负责人**: TBD
> **周期**: Week XX

---

## 任务概述

简要描述任务目标。

## 工作内容

### 1. 子任务
- 详细说明

## 验收标准

- [ ] 标准 1
- [ ] 标准 2

## 依赖

- Task X.Y

## 阻塞

- Task A.B

---
```

### 更新任务状态

当任务状态变更时，更新文件头部的状态标记：

```markdown
> **状态**: 🔄 进行中
```

---

## 📚 相关文档

- [../plan/CIS_V1.2.0_FINAL_PLAN_INTEGRATED_kimi.md](../plan/CIS_V1.2.0_FINAL_PLAN_INTEGRATED_kimi.md) - 完整计划
- [../plan/CIS_V1.2.0_MULTI_AGENT_ARCHITECTURE_kimi.md](../plan/CIS_V1.2.0_MULTI_AGENT_ARCHITECTURE_kimi.md) - 多 Agent 架构
- [../plan/REVIEW_QUESTIONS_kimi.md](../plan/REVIEW_QUESTIONS_kimi.md) - 审阅问题
- [../plan/REVIEW_RESPONSES_glm.md](../plan/REVIEW_RESPONSES_glm.md) - GLM 回复

---

**最后更新**: 2026-02-20
