//! # TODO List Monitor
//!
//! 监控 TODO list 变动，与进程内版本比对，触发动态重调度。
//!
//! ## 使用场景
//! - 用户通过 CLI/Web 修改任务优先级
//! - 外部系统插入紧急任务
//! - Agent 自动根据执行状态调整计划
//!
//! ## 工作流程
//! 1. 定期或事件驱动加载外部 TODO list
//! 2. 与内存中的 snapshot 进行 diff
//! 3. 如有变化，调用变更处理器
//! 4. 更新内存中的 snapshot

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::error::Result;
use crate::scheduler::{
    DagTodoList, TodoListDiff, DynamicTaskScheduler,
    DagRun,
};

/// TODO list 变更事件
#[derive(Debug, Clone)]
pub enum TodoChangeEvent {
    /// 新增任务
    TasksAdded(Vec<String>),
    /// 移除任务
    TasksRemoved(Vec<String>),
    /// 优先级变更 (task_id, old_priority, new_priority)
    PriorityChanged(Vec<(String, i32, i32)>),
    /// 状态变更 (task_id, old_status, new_status)
    StatusChanged(Vec<(String, String, String)>),
    /// 执行顺序重排
    ExecutionReordered,
}

impl TodoChangeEvent {
    /// 从 diff 创建事件
    pub fn from_diff(diff: &TodoListDiff) -> Vec<Self> {
        let mut events = Vec::new();

        // 新增任务
        if !diff.added.is_empty() {
            events.push(Self::TasksAdded(
                diff.added.iter().map(|i| i.id.clone()).collect()
            ));
        }

        // 移除任务
        if !diff.removed.is_empty() {
            events.push(Self::TasksRemoved(
                diff.removed.iter().map(|i| i.id.clone()).collect()
            ));
        }

        // 优先级变更
        let priority_changes: Vec<_> = diff.modified
            .iter()
            .filter(|m| m.old_priority != m.new_priority)
            .map(|m| (m.id.clone(), m.old_priority, m.new_priority))
            .collect();
        if !priority_changes.is_empty() {
            events.push(Self::PriorityChanged(priority_changes));
        }

        // 状态变更
        let status_changes: Vec<_> = diff.modified
            .iter()
            .filter(|m| m.old_status != m.new_status)
            .map(|m| {
                (
                    m.id.clone(),
                    format!("{:?}", m.old_status),
                    format!("{:?}", m.new_status),
                )
            })
            .collect();
        if !status_changes.is_empty() {
            events.push(Self::StatusChanged(status_changes));
        }

        events
    }
}

/// TODO list 加载器 trait
#[async_trait::async_trait]
pub trait TodoListLoader: Send + Sync {
    /// 加载最新的 TODO list
    async fn load(&self, run_id: &str) -> Result<DagTodoList>;
}

/// 文件系统加载器
pub struct FileSystemLoader {
    base_path: std::path::PathBuf,
}

impl FileSystemLoader {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }
}

#[async_trait::async_trait]
impl TodoListLoader for FileSystemLoader {
    async fn load(&self, run_id: &str) -> Result<DagTodoList> {
        let path = self.base_path.join(format!("{}_todo.json", run_id));
        
        if !path.exists() {
            return Ok(DagTodoList::new());
        }

        let content = tokio::fs::read_to_string(&path).await
            .map_err(|e| crate::error::CisError::storage(
                format!("Failed to read TODO list: {}", e)
            ))?;

        let todo_list: DagTodoList = serde_json::from_str(&content)
            .map_err(|e| crate::error::CisError::storage(
                format!("Failed to parse TODO list: {}", e)
            ))?;

        Ok(todo_list)
    }
}

/// 变更处理器
pub type ChangeHandler = Box<dyn Fn(TodoChangeEvent, &mut DagRun) + Send + Sync>;

/// TODO list 监控器
pub struct TodoListMonitor {
    /// 运行 ID
    run_id: String,
    /// 内存中的 DagRun（共享）
    dag_run: Arc<RwLock<DagRun>>,
    /// 加载器
    loader: Arc<dyn TodoListLoader>,
    /// 变更处理器
    handlers: Vec<ChangeHandler>,
    /// 上次检查时间
    last_check: Arc<Mutex<chrono::DateTime<chrono::Utc>>>,
    /// 检查间隔
    check_interval: std::time::Duration,
    /// 是否正在运行
    running: Arc<Mutex<bool>>,
    /// 监控任务句柄
    monitor_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// 动态调度器
    scheduler: Arc<Mutex<DynamicTaskScheduler>>,
}

impl TodoListMonitor {
    /// 创建新的监控器
    pub fn new(
        run_id: String,
        dag_run: Arc<RwLock<DagRun>>,
        loader: Arc<dyn TodoListLoader>,
        check_interval: std::time::Duration,
    ) -> Self {
        let scheduler = Arc::new(Mutex::new(
            DynamicTaskScheduler::new(true) // 允许动态重排序
        ));

        Self {
            run_id,
            dag_run,
            loader,
            handlers: Vec::new(),
            last_check: Arc::new(Mutex::new(chrono::Utc::now())),
            check_interval,
            running: Arc::new(Mutex::new(false)),
            monitor_handle: Arc::new(Mutex::new(None)),
            scheduler,
        }
    }

    /// 添加变更处理器
    pub fn on_change<F>(&mut self, handler: F)
    where
        F: Fn(TodoChangeEvent, &mut DagRun) + Send + Sync + 'static,
    {
        self.handlers.push(Box::new(handler));
    }

    /// 启动后台监控
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.lock().await;
        if *running {
            return Ok(()); // 已经在运行
        }
        *running = true;
        drop(running);

        // 初始化调度器
        {
            let dag_run = self.dag_run.read().await;
            let mut scheduler = self.scheduler.lock().await;
            scheduler.init_from_todo(&dag_run.todo_list);
        }

        let run_id = self.run_id.clone();
        let dag_run = self.dag_run.clone();
        let loader = self.loader.clone();
        let check_interval = self.check_interval;
        let running = self.running.clone();
        let last_check = self.last_check.clone();
        let scheduler = self.scheduler.clone();

        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            
            loop {
                interval.tick().await;

                // 检查是否应该停止
                if !*running.lock().await {
                    break;
                }

                // 执行检查
                if let Err(e) = Self::check_once(
                    &run_id,
                    &dag_run,
                    &loader,
                    &scheduler,
                    &last_check,
                ).await {
                    warn!("TODO list check failed for {}: {}", run_id, e);
                }
            }

            info!("TODO list monitor stopped for {}", run_id);
        });

        *self.monitor_handle.lock().await = Some(handle);
        info!("TODO list monitor started for {}", self.run_id);

        Ok(())
    }

    /// 停止监控
    pub async fn stop(&self) {
        *self.running.lock().await = false;
        
        if let Some(handle) = self.monitor_handle.lock().await.take() {
            handle.abort();
        }
    }

    /// 执行一次检查
    async fn check_once(
        run_id: &str,
        dag_run: &Arc<RwLock<DagRun>>,
        loader: &Arc<dyn TodoListLoader>,
        scheduler: &Arc<Mutex<DynamicTaskScheduler>>,
        last_check: &Arc<Mutex<chrono::DateTime<chrono::Utc>>>,
    ) -> Result<()> {
        // 加载外部 TODO list
        let external = loader.load(run_id).await?;

        // 获取差异
        let diff = {
            let mut dag_run = dag_run.write().await;
            dag_run.sync_todo_list(&external)
        };

        if !diff.has_changes() {
            return Ok(());
        }

        info!(
            "TODO list changes detected for {}: {} added, {} removed, {} modified",
            run_id,
            diff.added.len(),
            diff.removed.len(),
            diff.modified.len()
        );

        // 应用变更到调度器
        let changes = {
            let mut sched = scheduler.lock().await;
            sched.apply_diff(&diff)
        };

        // 应用到 DagRun
        {
            let mut dag_run = dag_run.write().await;
            dag_run.apply_schedule_changes(&changes);
        }

        // 触发事件处理器
        let events = TodoChangeEvent::from_diff(&diff);
        for event in events {
            // 注意：这里需要可写锁，所以不能在持有锁的情况下调用 handler
            // 实际项目中可能需要更复杂的协调机制
            debug!("TODO change event for {}: {:?}", run_id, event);
        }

        // 更新检查时间
        *last_check.lock().await = chrono::Utc::now();

        Ok(())
    }

    /// 手动触发检查（用于测试或事件驱动场景）
    pub async fn check_now(&self) -> Result<TodoListDiff> {
        let external = self.loader.load(&self.run_id).await?;
        
        let diff = {
            let mut dag_run = self.dag_run.write().await;
            dag_run.sync_todo_list(&external)
        };

        if diff.has_changes() {
            let changes = {
                let mut sched = self.scheduler.lock().await;
                sched.apply_diff(&diff)
            };

            {
                let mut dag_run = self.dag_run.write().await;
                dag_run.apply_schedule_changes(&changes);
            }

            *self.last_check.lock().await = chrono::Utc::now();
        }

        Ok(diff)
    }

    /// 获取当前执行计划
    pub async fn current_plan(&self) -> Vec<String> {
        self.scheduler.lock().await.current_plan().to_vec()
    }

    /// 获取上次检查时间
    pub async fn last_check_time(&self) -> chrono::DateTime<chrono::Utc> {
        *self.last_check.lock().await
    }
}

/// 便捷的监控启动函数
pub async fn start_monitoring(
    run_id: String,
    dag_run: Arc<RwLock<DagRun>>,
    data_dir: impl Into<std::path::PathBuf>,
    check_interval_secs: u64,
) -> Result<Arc<TodoListMonitor>> {
    let loader = Arc::new(FileSystemLoader::new(data_dir));
    let monitor = Arc::new(TodoListMonitor::new(
        run_id,
        dag_run,
        loader,
        std::time::Duration::from_secs(check_interval_secs),
    ));

    monitor.start().await?;
    Ok(monitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::{DagTodoItem, TodoItemStatus};

    struct MockLoader {
        data: Arc<Mutex<DagTodoList>>,
    }

    #[async_trait::async_trait]
    impl TodoListLoader for MockLoader {
        async fn load(&self, _run_id: &str) -> Result<DagTodoList> {
            Ok(self.data.lock().await.clone())
        }
    }

    #[tokio::test]
    async fn test_todo_monitor_detects_changes() {
        let external = Arc::new(Mutex::new(DagTodoList::new()));
        let loader = Arc::new(MockLoader {
            data: external.clone(),
        });

        let dag_run = Arc::new(RwLock::new(DagRun::new(
            crate::scheduler::TaskDag::new()
        )));

        let monitor = TodoListMonitor::new(
            "test-run".to_string(),
            dag_run.clone(),
            loader,
            std::time::Duration::from_secs(1),
        );

        // 初始状态
        {
            let run = dag_run.read().await;
            assert_eq!(run.todo_list.items.len(), 0);
        }

        // 模拟外部添加任务
        {
            let mut ext = external.lock().await;
            ext.add("task-1", "Test task");
        }

        // 手动触发检查
        let diff = monitor.check_now().await.unwrap();
        assert!(diff.has_changes());
        assert_eq!(diff.added.len(), 1);

        // 验证已同步
        {
            let run = dag_run.read().await;
            assert_eq!(run.todo_list.items.len(), 1);
            assert_eq!(run.todo_list.items[0].id, "task-1");
        }
    }

    #[test]
    fn test_todo_change_event_from_diff() {
        let mut diff = TodoListDiff::default();
        
        // 添加任务
        diff.added.push(DagTodoItem::new("t1".to_string(), "Task 1".to_string()));
        
        // 修改优先级
        diff.modified.push(crate::scheduler::TodoItemChange {
            id: "t2".to_string(),
            old_status: TodoItemStatus::Pending,
            new_status: TodoItemStatus::Pending,
            old_priority: 1,
            new_priority: 10,
            old_description: "Old".to_string(),
            new_description: "New".to_string(),
        });

        let events = TodoChangeEvent::from_diff(&diff);
        assert_eq!(events.len(), 2); // TasksAdded + PriorityChanged
    }
}
