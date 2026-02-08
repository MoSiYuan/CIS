# CIS Plan 文档

**当前版本**: v1.0.0 (Foundation)  
**下一版本**: v1.1.0 (Production Ready)  

---

## 📂 文档结构

```
plan/
├── README.md                          # 本文档
├── v1.1.0_ROADMAP.md                 ⭐ 当前重点 (综合重构版 v2.0.0)
├── CIS_PRODUCTION_READINESS_PLAN.md  # 外界详细评估计划 (参考)
├── 
├── 📁 archives/                       # 归档文档 (已过期)
│   ├── DAG_IMPLEMENTATION_STATUS.md      # 实现状态 (已实现)
│   ├── DAG_IMPLEMENTATION_GAP_ANALYSIS.md # 差距分析 (已过时)
│   ├── TASKPLAN_DAG_PRIORITY.md          # 任务计划 (已完成)
│   ├── REMAINING_WORK.md                 # 剩余工作 (已过时)
│   └── CIS_ENGINEERING_REVIEW_2026_02_02.md # 评审报告
│
└── 📄 当前设计文档 (按日期排序)
    ├── CLI_AI_NATIVE_DESIGN.md (Feb 7)
    ├── CLI_AI_NATIVE_REFACTOR.md (Feb 7)
    ├── GUI_ELEMENT_STYLE_DESIGN.md (Feb 7)
    ├── cis_dual_mode_arch.md (Feb 6)
    ├── mcp_integration_design.md (Feb 6)
    ├── mcp_skill_proxy.md (Feb 6)
    ├── mcp_value_analysis.md (Feb 6)
    ├── dag_agent_cluster_design.md (Feb 6)
    ├── unified_dag_architecture.md (Feb 6)
    ├── unified_dag_visual.md (Feb 6)
    ├── matrix_room_broadcast_research.md (Feb 6)
    ├── room_store_design.md (Feb 6)
    ├── IMPLEMENTATION_PLAN.md (Feb 6)
    ├── DAG_SKILL_ARCHITECTURE.md (Feb 4)
    ├── ARCHITECTURE_DESIGN.md (Feb 4)
    ├── NETWORK_ACCESS_DESIGN.md (Feb 4)
    └── user.md (Feb 7)
```

---

## 🎯 当前重点

### ⭐ v1.1.0 路线图 (重构版)
[v1.1.0_ROADMAP.md](v1.1.0_ROADMAP.md) - **综合外界评估与内部规划的重构版本**

**文档版本**: 2.0.0 (2026-02-08)  
**参考来源**: [kimi_agent评估](archives/kimi_agent.md) + [CIS_PRODUCTION_READINESS_PLAN](CIS_PRODUCTION_READINESS_PLAN.md)

**6个阶段** (12周):
1. **Phase 1**: 稳定性加固 (Week 1-2) - 修复SIGBUS等阻塞问题
2. **Phase 2**: 核心功能完善 (Week 3-6) - WASM、GUI、P2P
3. **Phase 3**: 性能优化 (Week 7-8) - 内存、异步、存储
4. **Phase 4**: 生态集成 (Week 9-10) - Element、VS Code、Homebrew
5. **Phase 5**: 安全审计 (Week 11) - 代码审计、渗透测试
6. **Phase 6**: 发布准备 (Week 12) - v1.1.0正式发布

### 🚀 可执行任务 (AI Agent并行)
[tasks/](tasks/) - **拆分后的可执行任务包**

| 文档 | 用途 |
|------|------|
| [tasks/QUICKSTART.md](tasks/QUICKSTART.md) | 5分钟快速开始 |
| [tasks/CONTEXT.md](tasks/CONTEXT.md) | 压缩版上下文 |
| [tasks/TASK_INDEX.md](tasks/TASK_INDEX.md) | 完整任务索引 |
| [tasks/phase1/](tasks/phase1/) | Week 1-2 任务包 |
| [tasks/phase2/](tasks/phase2/) | Week 3-6 任务包 |

**当前可并行任务**: 6个 (Week 1)

---

## 📊 文档分类

### 架构设计
| 文档 | 日期 | 状态 |
|------|------|------|
| [ARCHITECTURE_DESIGN.md](ARCHITECTURE_DESIGN.md) | Feb 4 | ✅ 有效 |
| [DAG_SKILL_ARCHITECTURE.md](DAG_SKILL_ARCHITECTURE.md) | Feb 4 | ✅ 有效 |
| [unified_dag_architecture.md](unified_dag_architecture.md) | Feb 6 | ✅ 有效 |
| [cis_dual_mode_arch.md](cis_dual_mode_arch.md) | Feb 6 | ✅ 有效 |

### GUI 设计
| 文档 | 日期 | 状态 |
|------|------|------|
| [GUI_ELEMENT_STYLE_DESIGN.md](GUI_ELEMENT_STYLE_DESIGN.md) | Feb 7 | ✅ 有效 |
| [unified_dag_visual.md](unified_dag_visual.md) | Feb 6 | ✅ 有效 |

### CLI 设计
| 文档 | 日期 | 状态 |
|------|------|------|
| [CLI_AI_NATIVE_DESIGN.md](CLI_AI_NATIVE_DESIGN.md) | Feb 7 | ✅ 有效 |
| [CLI_AI_NATIVE_REFACTOR.md](CLI_AI_NATIVE_REFACTOR.md) | Feb 7 | ✅ 有效 |

### 网络与 Matrix
| 文档 | 日期 | 状态 |
|------|------|------|
| [NETWORK_ACCESS_DESIGN.md](NETWORK_ACCESS_DESIGN.md) | Feb 4 | ✅ 有效 |
| [matrix_room_broadcast_research.md](matrix_room_broadcast_research.md) | Feb 6 | ✅ 有效 |
| [room_store_design.md](room_store_design.md) | Feb 6 | ✅ 有效 |
| [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) | Feb 6 | ✅ 有效 (DAG Agent) |

### MCP 集成
| 文档 | 日期 | 状态 |
|------|------|------|
| [mcp_integration_design.md](mcp_integration_design.md) | Feb 6 | ✅ 有效 |
| [mcp_skill_proxy.md](mcp_skill_proxy.md) | Feb 6 | ✅ 有效 |
| [mcp_value_analysis.md](mcp_value_analysis.md) | Feb 6 | ✅ 有效 |

### Agent Cluster
| 文档 | 日期 | 状态 |
|------|------|------|
| [dag_agent_cluster_design.md](dag_agent_cluster_design.md) | Feb 6 | ✅ 有效 |

---

## 🗑️ 已归档文档

以下文档已过期，移动到 `archives/` 目录:

| 文档 | 原因 | 归档日期 |
|------|------|----------|
| DAG_IMPLEMENTATION_STATUS.md | Phase 1-3 已完成 | 2026-02-08 |
| DAG_IMPLEMENTATION_GAP_ANALYSIS.md | 差距已弥补 | 2026-02-08 |
| TASKPLAN_DAG_PRIORITY.md | 任务已完成 | 2026-02-08 |
| REMAINING_WORK.md | 工作清单已过时 | 2026-02-08 |
| CIS_ENGINEERING_REVIEW_2026_02_02.md | 评审报告过期 | 2026-02-08 |

---

## 📖 阅读指南

### 如果你是新开发者
1. 阅读 [v1.1.0_ROADMAP.md](v1.1.0_ROADMAP.md) 了解当前计划 (重构版)
2. 参考 [CIS_PRODUCTION_READINESS_PLAN.md](CIS_PRODUCTION_READINESS_PLAN.md) 了解详细任务分解
3. 阅读 [ARCHITECTURE_DESIGN.md](ARCHITECTURE_DESIGN.md) 了解架构
4. 查看 [DAG_SKILL_ARCHITECTURE.md](DAG_SKILL_ARCHITECTURE.md) 了解 DAG

### 如果你是项目经理
1. 阅读 [v1.1.0_ROADMAP.md](v1.1.0_ROADMAP.md) 了解阶段规划
2. 参考 [CIS_PRODUCTION_READINESS_PLAN.md](CIS_PRODUCTION_READINESS_PLAN.md) 了解详细任务包
3. 查看 [archives/kimi_agent.md](archives/kimi_agent.md) 了解当前状态评估

### 如果你要做 GUI 开发
1. 阅读 [v1.1.0_ROADMAP.md](v1.1.0_ROADMAP.md) Phase 2 (Week 5-6)
2. 阅读 [GUI_ELEMENT_STYLE_DESIGN.md](GUI_ELEMENT_STYLE_DESIGN.md)
3. 查看 [unified_dag_visual.md](unified_dag_visual.md)

### 如果你要做网络开发
1. 阅读 [v1.1.0_ROADMAP.md](v1.1.0_ROADMAP.md) Phase 2/4
2. 阅读 [NETWORK_ACCESS_DESIGN.md](NETWORK_ACCESS_DESIGN.md)
3. 查看 [matrix_room_broadcast_research.md](matrix_room_broadcast_research.md)

---

## 📝 文档维护规则

### 创建新文档
```bash
# 命名规范
YYYY-MM-DD_description.md

# 示例
2026-02-10_new_feature_design.md
```

### 归档过期文档
```bash
# 当文档内容已过时或已实现
mv old_document.md archives/
```

### 更新路线图
```bash
# 每周更新 v1.1.0_ROADMAP.md 进度
# 里程碑完成后归档旧路线图
```

---

## 🔗 相关链接

- [发布文档](../releases/) - 版本发布说明
- [开发路线图](../releases/v1.0.0/archives/COMPLETION_ROADMAP.md) - 完善计划
- [执行计划](../releases/v1.0.0/archives/EXECUTION_PLAN.md) - 任务清单

---

**维护者**: CIS Core Team  
**最后更新**: 2026-02-08
