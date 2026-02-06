# TODO List 监控与动态重调度指南

## 概述

TODO List Monitor 允许 Agent 检测 TODO list 的变动，与进程内的版本比对，发现差异后按新的计划处理。

## 核心组件

### 1. DagTodoList - TODO 列表

```rust
let mut todo_list = DagTodoList::new();
todo_list.add("task-1", "执行数据清洗");
todo_list.add_item(
    DagTodoItem::new("task-2".into(), "训练模型".into())
        .with_priority(100)
        .with_task("model_training")
);
```

### 2. TodoListDiff - 差异检测

```rust
// 对比两个 TODO list
let diff = old_todo_list.diff(&new_todo_list);

if diff.has_changes() {
    println!("新增: {:?}", diff.added);
    println!("删除: {:?}", diff.removed);
    println!("修改: {:?}", diff.modified);
}
```

### 3. TodoListMonitor - 监控器

```rust
// 创建监控器
let monitor = TodoListMonitor::new(
    run_id,
    dag_run.clone(),
    loader,
    Duration::from_secs(5), // 每 5 秒检查一次
);

// 启动后台监控
monitor.start().await?;

// 手动触发检查
let diff = monitor.check_now().await?;
```

### 4. DynamicTaskScheduler - 动态调度器

```rust
let mut scheduler = DynamicTaskScheduler::new(true); // 允许重排序

// 应用变更
let changes = scheduler.apply_diff(&diff);

// 获取当前执行计划
let plan = scheduler.current_plan();
```

## 使用场景

### 场景 1：用户调整任务优先级

```rust
// 用户通过 CLI 提高某个任务的优先级
// 外部 TODO 文件更新

// Agent 检测到变更
let diff = dag_run.sync_todo_list(&external_todo).await;

if diff.has_priority_changes() {
    // 重新排序执行计划
    let changes = scheduler.apply_diff(&diff);
    dag_run.apply_schedule_changes(&changes);
}
```

### 场景 2：插入紧急任务

```rust
// 外部系统插入紧急任务
external_todo.add("urgent-task", "紧急修复");

// Agent 检测并处理
let diff = monitor.check_now().await?;
if !diff.added.is_empty() {
    // 将新任务加入执行计划
    for task in &diff.added {
        dag_run.todo_list.add_item(task.clone());
    }
}
```

### 场景 3：取消正在进行的任务

```rust
// 用户取消某个任务
if let Some(item) = external_todo.get_mut("task-1") {
    item.mark_skipped("用户取消");
}

// Agent 检测并取消
let events = TodoChangeEvent::from_diff(&diff);
for event in events {
    match event {
        TodoChangeEvent::StatusChanged(changes) => {
            for (id, old, new) in changes {
                if new == "Skipped" {
                    executor.cancel_task(&id).await?;
                }
            }
        }
        _ => {}
    }
}
```

## 集成到 DagRun

```rust
impl DagRun {
    /// 同步 TODO list 并返回差异
    pub fn sync_todo_list(&mut self, external: &DagTodoList) -> TodoListDiff {
        let diff = self.todo_list.diff(external);
        
        if diff.has_changes() {
            // 应用外部变更
            self.todo_list = external.clone();
            self.updated_at = Utc::now();
        }
        
        diff
    }
    
    /// 应用调度变更
    pub fn apply_schedule_changes(&mut self, changes: &ScheduleChanges) {
        // 取消被移除或跳过的任务
        for task_id in &changes.to_cancel {
            if let Some(node) = self.dag.nodes_mut().get_mut(task_id) {
                // 处理取消逻辑
            }
        }
        
        // 重置需要重新执行的任务
        for task_id in &changes.to_start {
            let _ = self.dag.reset_node(task_id);
        }
    }
}
```

## 完整示例

```rust
use cis_core::scheduler::{
    DagRun, TaskDag, TodoListMonitor, FileSystemLoader,
    TodoChangeEvent, DynamicTaskScheduler,
};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. 创建 DAG 运行实例
    let mut dag = TaskDag::new();
    dag.add_node("step1".into(), vec![])?;
    dag.add_node("step2".into(), vec!["step1".into()])?;
    
    let dag_run = Arc::new(RwLock::new(DagRun::new(dag)));
    
    // 2. 初始化 TODO list
    {
        let mut run = dag_run.write().await;
        run.init_todo_from_tasks();
    }
    
    // 3. 创建监控器
    let loader = Arc::new(FileSystemLoader::new("./data"));
    let monitor = Arc::new(TodoListMonitor::new(
        "run-001".into(),
        dag_run.clone(),
        loader,
        Duration::from_secs(5),
    ));
    
    // 4. 启动监控
    monitor.start().await?;
    
    // 5. 执行循环
    loop {
        // 检查变更
        let diff = monitor.check_now().await?;
        
        if diff.has_changes() {
            // 应用变更
            let mut scheduler = DynamicTaskScheduler::new(true);
            let changes = scheduler.apply_diff(&diff);
            
            {
                let mut run = dag_run.write().await;
                run.apply_schedule_changes(&changes);
            }
            
            // 处理事件
            for event in TodoChangeEvent::from_diff(&diff) {
                println!("处理事件: {:?}", event);
            }
        }
        
        // 执行当前任务...
        
        sleep(Duration::from_secs(1)).await;
    }
}
```

## 事件类型

| 事件 | 说明 | 处理方式 |
|------|------|----------|
| TasksAdded | 新增任务 | 加入执行计划 |
| TasksRemoved | 移除任务 | 取消正在执行的任务 |
| PriorityChanged | 优先级变更 | 重新排序执行队列 |
| StatusChanged | 状态变更 | 根据新状态处理 |
| ExecutionReordered | 执行重排 | 调整执行顺序 |

## 配置建议

1. **检查间隔**：生产环境建议 5-30 秒，开发环境可以更快
2. **优先级范围**：建议使用 0-100，负数表示降低优先级
3. **持久化**：TODO list 应该定期保存到磁盘，便于恢复

## 注意事项

1. **并发安全**：所有对 DagRun 的修改都需要通过 RwLock
2. **错误处理**：加载失败时不应中断执行，使用本地版本继续
3. **性能考虑**：diff 操作是 O(n)，适合中小型 TODO list
