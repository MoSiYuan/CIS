# TASK 2.3: 提取 cis-scheduler

> **Phase**: 2 - 模块提取
> **状态**: ✅ 已完成
> **完成日期**: 2026-02-21
> **提交**: 1aa447a
> **负责人**: TBD
> **周期**: Week 4

---

## 任务概述

将任务调度器从 cis-core 提取为独立的 `cis-scheduler` crate。

## 工作内容

### 1. 分析现有调度器实现

审查 `cis-core/src/scheduler/`：
- `Scheduler` trait 定义
- `PriorityScheduler` 实现
- 任务队列管理
- 执行器管理

### 2. 创建 crate 结构

```
crates/cis-scheduler/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── scheduler.rs    # Scheduler trait
│   ├── priority.rs     # 优先级调度器
│   ├── queue.rs        # 任务队列
│   ├── executor.rs     # 执行器管理
│   └── dag.rs          # DAG 编排（可选）
└── tests/
    └── scheduler_tests.rs
```

### 3. 实现优先级调度器

```rust
// priority.rs
pub struct PriorityScheduler<E: Executor> {
    queues: [TaskQueue; 3],  // High, Normal, Low
    executors: Vec<E>,
    running: AtomicUsize,
}

impl<E: Executor> PriorityScheduler<E> {
    pub fn new(num_executors: usize) -> Self {
        // 创建执行器池
    }
    
    pub async fn submit(&self, task: Task, priority: Priority) -> Result<TaskHandle, SchedulerError> {
        // 根据优先级放入对应队列
    }
}

#[async_trait]
impl<E: Executor> Scheduler for PriorityScheduler<E> {
    async fn schedule(&self, task: Task) -> Result<TaskHandle, SchedulerError> {
        self.submit(task, task.priority).await
    }
    // ...
}
```

### 4. 实现 DAG 编排（基础版）

```rust
// dag.rs
pub struct TaskGraph {
    nodes: HashMap<TaskId, TaskNode>,
    edges: Vec<(TaskId, TaskId)>,  // from -> to
}

impl TaskGraph {
    pub fn add_task(&mut self, task: Task) -> Result<(), DagError> {
        // 添加任务节点
    }
    
    pub fn add_dependency(&mut self, from: TaskId, to: TaskId) -> Result<(), DagError> {
        // 添加依赖边，检查循环
    }
    
    pub fn topological_sort(&self) -> Result<Vec<TaskId>, DagError> {
        // 拓扑排序
    }
}
```

## 验收标准

- [ ] 优先级调度器工作正常
- [ ] 任务队列管理正确
- [ ] 多执行器并发执行
- [ ] DAG 基础编排可用
- [ ] 单元测试覆盖主要场景

## 依赖

- Task 2.1 (cis-storage, 用于任务持久化)

## 阻塞

- Task 3.2 (重构 cis-core)

---
