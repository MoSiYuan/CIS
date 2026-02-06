//! # TODO List Monitor Example
//!
//! 展示如何让 Agent 检测 TODO list 变动并进行动态重调度。
//!
//! ## 运行方式
//! ```bash
//! cargo run --example todo_monitor_example
//! ```

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use cis_core::scheduler::{
    DagRun, TaskDag, DagTodoItem, DagTodoList, TodoListMonitor,
    TodoChangeEvent, TodoListLoader, FileSystemLoader,
    DynamicTaskScheduler, ScheduleChanges,
};
use cis_core::error::Result;

/// 模拟 Agent 的执行逻辑
struct AgentExecutor {
    dag_run: Arc<RwLock<DagRun>>,
    monitor: Arc<TodoListMonitor>,
    running: bool,
}

impl AgentExecutor {
    async fn new(run_id: String, data_dir: &str) -> Result<Self> {
        // 创建初始 DAG
        let mut dag = TaskDag::new();
        dag.add_node("task-1".to_string(), vec![]).map_err(|e| 
            cis_core::error::CisError::execution(format!("{:?}", e))
        )?;
        dag.add_node("task-2".to_string(), vec!["task-1".to_string()]).map_err(|e| 
            cis_core::error::CisError::execution(format!("{:?}", e))
        )?;
        dag.add_node("task-3".to_string(), vec!["task-1".to_string()]).map_err(|e| 
            cis_core::error::CisError::execution(format!("{:?}", e))
        )?;
        dag.initialize();

        let dag_run = Arc::new(RwLock::new(DagRun::new(dag)));
        
        // 初始化 TODO list
        {
            let mut run = dag_run.write().await;
            run.init_todo_from_tasks();
            // 添加一些额外的 TODO 项
            run.todo_list.add("review-1", "Review task 1 results");
            run.todo_list.add("cleanup", "Cleanup temporary files");
        }

        // 创建监控器
        let loader = Arc::new(FileSystemLoader::new(data_dir));
        let monitor = Arc::new(TodoListMonitor::new(
            run_id.clone(),
            dag_run.clone(),
            loader,
            Duration::from_secs(2), // 每 2 秒检查一次
        ));

        Ok(Self {
            dag_run,
            monitor,
            running: false,
        })
    }

    /// 启动 Agent 执行
    async fn start(&mut self) -> Result<()> {
        self.running = true;
        
        // 启动监控
        self.monitor.start().await?;
        
        println!("Agent started, monitoring TODO list changes...");
        println!("Try modifying the TODO file to see dynamic re-scheduling in action!");
        
        // 模拟执行循环
        while self.running {
            // 获取当前执行计划
            let plan = self.monitor.current_plan().await;
            
            if !plan.is_empty() {
                println!("\nCurrent execution plan: {:?}", plan);
                
                // 模拟执行任务
                if let Some(task_id) = plan.first() {
                    println!("Executing task: {}", task_id);
                    
                    // 更新 TODO list
                    {
                        let mut run = self.dag_run.write().await;
                        if let Some(item) = run.todo_list.get_mut(task_id) {
                            item.mark_in_progress();
                        }
                    }
                    
                    // 模拟执行时间
                    sleep(Duration::from_secs(1)).await;
                    
                    // 标记完成
                    {
                        let mut run = self.dag_run.write().await;
                        if let Some(item) = run.todo_list.get_mut(task_id) {
                            item.mark_completed();
                        }
                        run.checkpoint(format!("Task {} completed", task_id));
                    }
                    
                    println!("Task {} completed", task_id);
                }
            } else {
                println!("No pending tasks, waiting...");
                sleep(Duration::from_secs(2)).await;
            }
        }
        
        Ok(())
    }

    async fn stop(&mut self) {
        self.running = false;
        self.monitor.stop().await;
    }
}

/// 处理 TODO list 变更
fn handle_todo_change(event: TodoChangeEvent, run: &mut DagRun) {
    match &event {
        TodoChangeEvent::TasksAdded(task_ids) => {
            println!("\n>>> New tasks added: {:?}", task_ids);
            println!(">>> Will include these in next scheduling cycle");
        }
        TodoChangeEvent::TasksRemoved(task_ids) => {
            println!("\n>>> Tasks removed: {:?}", task_ids);
            println!(">>> Will cancel these if running");
        }
        TodoChangeEvent::PriorityChanged(changes) => {
            println!("\n>>> Priority changes detected:");
            for (id, old, new) in changes {
                println!(">>>   {}: {} -> {}", id, old, new);
            }
            println!(">>> Will re-order execution plan");
        }
        TodoChangeEvent::StatusChanged(changes) => {
            println!("\n>>> Status changes detected:");
            for (id, old, new) in changes {
                println!(">>>   {}: {} -> {}", id, old, new);
            }
        }
        TodoChangeEvent::ExecutionReordered => {
            println!("\n>>> Execution plan reordered based on new priorities");
        }
    }
}

/// 手动触发变更检测的示例
async fn manual_check_example() -> Result<()> {
    println!("=== Manual Check Example ===\n");
    
    let mut dag = TaskDag::new();
    dag.add_node("task-a".to_string(), vec![]).map_err(|e| 
        cis_core::error::CisError::execution(format!("{:?}", e))
    )?;
    dag.add_node("task-b".to_string(), vec![]).map_err(|e| 
        cis_core::error::CisError::execution(format!("{:?}", e))
    )?;
    
    let dag_run = Arc::new(RwLock::new(DagRun::new(dag)));
    
    // 初始化 TODO list
    {
        let mut run = dag_run.write().await;
        run.init_todo_from_tasks();
    }
    
    // 创建外部 TODO list（模拟用户修改）
    let mut external = DagTodoList::new();
    external.add("task-a", "Task A");
    external.add("task-b", "Task B");
    external.add("task-c", "New urgent task"); // 新增任务
    
    // 修改 task-a 的优先级
    if let Some(item) = external.get_mut("task-a") {
        item.priority = 100; // 提高优先级
    }
    
    // 同步并检测变更
    let diff = {
        let mut run = dag_run.write().await;
        run.sync_todo_list(&external)
    };
    
    if diff.has_changes() {
        println!("Changes detected:");
        println!("  Added: {:?}", diff.added.iter().map(|i| &i.id).collect::<Vec<_>>());
        println!("  Modified: {:?}", diff.modified.iter().map(|m| &m.id).collect::<Vec<_>>());
        
        // 生成事件
        let events = TodoChangeEvent::from_diff(&diff);
        for event in events {
            println!("  Event: {:?}", event);
        }
        
        // 应用调度变更
        let mut scheduler = DynamicTaskScheduler::new(true);
        let changes = scheduler.apply_diff(&diff);
        
        if changes.has_changes() {
            println!("\nSchedule changes:");
            println!("  To start: {:?}", changes.to_start);
            println!("  To cancel: {:?}", changes.to_cancel);
            println!("  Reordered: {}", changes.reordered);
        }
    } else {
        println!("No changes detected");
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("TODO List Monitor Example\n");
    println!("=========================\n");
    
    // 运行手动检查示例
    manual_check_example().await?;
    
    println!("\n=== Full Agent Example ===\n");
    println!("This would run a continuous agent that monitors TODO list.");
    println!("For this demo, we'll just show the setup.\n");
    
    // 创建临时目录
    let temp_dir = std::env::temp_dir().join("cis_todo_example");
    tokio::fs::create_dir_all(&temp_dir).await?;
    
    println!("Data directory: {:?}", temp_dir);
    println!("You can modify the TODO file there to trigger changes.");
    
    // 清理
    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    
    Ok(())
}
