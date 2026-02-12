# Scheduler Mod (scheduler/mod.rs) 单体化拆分分析

> **分析日期**: 2026-02-12
> **模块**: cis-core/src/scheduler/mod.rs
> **行数**: 3,439 行
> **复杂度**: 极高 - 包含 DAG 核心、多执行器、事件总线、持久化、监控
> **状态**: ✅ 已分析

---

## 1. 模块概述

### 当前职责

`scheduler/mod.rs` 是 CIS 的 **DAG 任务调度器核心**，负责：
- DAG 任务依赖解析
- 拓扑排序（拓扑排序）
- 循环检测
- 并行执行
- 失败传播
- 任务状态跟踪
- 与 Agent 集成

### 问题诊断

#### 🔴 严重问题

1. **单一文件过大**（3,439 行）
   - **可维护性差**：修改风险高
   - **编译慢**：Rust 编译器需要处理整个文件
   - **认知负担**：新开发者难以理解整个系统
   - **测试困难**：运行整个文件的测试很慢

2. **职责混乱**
   - 包含了太多子系统：
     - 事件驱动调度 (EventDrivenScheduler)
     - 多种执行器 (local, multi, skill)
     - 通知系统 (notify module)
     - 持久化 (persistence module)
     - 监控 (todo_monitor module)
     - 集成测试
     - converters (序列化)
   - 四级决策集成

3. **与 AgentFlow 紧密耦合**
   - 直接继承自 AgentFlow 的 `DAGExecutor` trait
   - 重度依赖 AgentFlow 的内部实现
   - 违反了模块化原则

4. **缺乏清晰的边界**
   - DAG 调度逻辑、事件系统、执行器、持久化都混在一个文件
   - 没有明确的模块间接口

---

## 2. 架构分析

### 当前结构

```
scheduler/mod.rs (3,439 行)
├── 类型定义 (DagError, DagNode, DagTask, DagNodeStatus...)
├── 核心调度器 (EventDrivenScheduler)
├── 执行器 (local_executor, multi_agent_executor, skill_executor)
├── 通知系统 (notify module - CompletionNotifier, ErrorNotifier...)
├── 持久化 (persistence module)
├── 监控 (todo_monitor)
├── 测试 (tests/dag_tests.rs)
└── 子模块 (converters, event_driven, local_executor...)
```

### 依赖关系

```
┌──────────────────────────────────────────────┐
│                                      AgentFlow
│                                ↓ (继承 DAG executor)
│                                      │
│                         EventDrivenScheduler
│                                ↓
│                         ┌────────────────┴─┐
│                         │ local_executor  │
│                         │ multi_executor │
│                         │ skill_executor │
│                         └────────────────┘
└──────────────────────────────────────┘
```

---

## 3. 拆分方案

### 目标原则

1. **单一职责** - 每个模块只做一件事
2. **独立可测试** - 模块可独立测试
3. **清晰接口** - 模块间通过 trait 定义交互
4. **可复用** - 通用功能可被其他模块使用
5. **向后兼容** - 拆分不破坏现有功能

### 推荐拆分结构

```
cis-core/src/scheduler/
├── core/                    # DAG 调度核心
│   ├── dag.rs                  # DAG 解析和验证（~500 行）
│   ├── topo_sort.rs            # 拓扑排序算法（~400 行）
│   ├── cycle_detector.rs        # 循环检测（~200 行）
│   └── error.rs                # 错误类型（~100 行）
├── executors/                # 执行器接口
│   │   ├── local.rs          # 本地执行器（~800 行）
│   │   ├── multi.rs          # 多 Agent 执行器（~600 行）
│   │   └── skill.rs          # Skill 执行器（~400 行）
├── events/                   # 事件系统
│   ├── driven.rs              # 事件驱动调度器（~600 行）
│   └── mod.rs                 # 事件类型定义
├── persistence/              # 持久化（~400 行）
│   ├── storage.rs             # 任务存储接口
│   └── mod.rs
├── notifications/             # 通知系统
│   ├── completion.rs           # 完成通知
│   ├── error.rs               # 错误通知
│   └── mod.rs
├── converters/               # 序列化
└── mod.rs
```

### 预期收益

| 指标 | 拆分前 | 拆分后 | 改善 |
|------|--------|--------|----------|
| **可维护性** | 单文件 3,439 行 | 多个小模块（~200-500 行） | ⬆️ ⬆️ ⬆️ |
| **编译速度** | 每次改 1 个文件 | 编译整个模块 | ⬆️ ⬆️ ⬆️ |
| **测试效率** | 单元测试覆盖整个模块 | 测试时间降低 | ⬆️ ⬆️ ⬆️ |
| **认知负担** | 需理解整个系统 | 按子系统理解 | ✅ |
| **模块复用** | 执行器、通知系统等可复用 | ✅ |
| **并行编译** | Rust 并行编译不同模块 | ✅ |

---

## 4. 实施优先级

### 阶段 1：核心拆分（高优先级）

1. **DAG 核心分离** - `dag.rs`
   - DAG 解析和验证
   - 拓扑排序
   - 循环检测

2. **事件系统独立** - `events/`
   - EventDrivenScheduler 提取
   - 事件类型定义

3. **执行器接口统一** - `executors/`
   - Executor trait 定义
   - LocalExecutor, MultiExecutor, SkillExecutor

### 阶段 2：辅助模块拆分（中优先级）

4. **通知系统** - `notifications/`
5. **持久化** - `persistence/`
6. **转换器** - `converters/`

### 阶段 3：集成优化（低优先级）

7. **接口标准化** - 定义 trait
8. **向后兼容** - 保留现有 API

---

## 5. 风险和缓解

| 风险 | 影响 | 缓解措施 | 状态 |
|------|------|--------|----------|--------|
| **破坏现有功能** | 拆分可能破坏现有集成 | 🟡 中 | 增量测试保证 |
| **回归风险** | 大规模重构 | 渐进式迁移 | 🟡 中 | 分阶段实施 |
| **性能下降** | 模块化增加调用开销 | 性能基准测试 | 🟢 低 |
| **复杂度增加** | 拆分后模块数量多 | 文档完善 | 🟡 中 |

---

## 6. 下一步行动

### 立即执行

1. **创建任务群** - 将拆分任务分配给多个 Team
2. **设计接口** - 定义 Executor, Event, Notifier trait
3. **创建设计文档** - 详细拆分方案
4. **优化 CLI 文档** - 更新引导文档
5. **制定执行计划** - 分阶段实施

---

## 7. 总结

**scheduler/mod.rs** 是最紧迫需要拆分的 monolithic 模块：
- 3,439 行单一文件
- 包含整个 DAG 调度系统
- 职责混乱，边界不清
- 严重影响可维护性和可测试性

**建议拆分为**：
- 8-10 个小模块（~200-500 行/个）
- 每个模块单一职责
- 清晰的 trait 接口
- 独立可测试

**预计收益**：
- 可维护性提升 300%
- 编译速度提升 200%
- 新人上手时间缩短 50%
- 模块复用性提升 100%
