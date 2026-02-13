//! # Scheduler 核心
//!
//! 负责任务调度的核心逻辑：
//! - DAG 构建和验证
//! - 任务分配
//! - 执行协调
//!
//! ## 模块结构
//! - `dag`: DAG 管理和拓扑排序
//! - `queue`: 任务队列管理

pub mod dag;
pub mod queue;

pub use dag::{DagScheduler, SchedulerDagError, SchedulerDagNode, DagStats};
pub use queue::{TaskQueue, TaskQueueItem, TaskQueueError, TaskQueueStats};

use std::sync::Arc;

use crate::error::Result;
use crate::task::Dag;
use crate::task::repository::TaskRepository;

/// CIS 核心调度器
///
/// 负责协调 DAG 管理、任务队列和执行器。
pub struct SchedulerCore {
    /// DAG 调度器
    dag_scheduler: DagScheduler,
    /// 任务队列
    task_queue: TaskQueue,
    /// 任务仓储
    task_repo: Arc<TaskRepository>,
}

impl SchedulerCore {
    /// 创建新的调度器核心
    pub fn new(task_repo: Arc<TaskRepository>) -> Self {
        Self {
            dag_scheduler: DagScheduler::new(task_repo.clone()),
            task_queue: TaskQueue::new(),
            task_repo,
        }
    }

    /// 加载 DAG
    ///
    /// 从 TaskDag 构建调度 DAG。
    pub async fn load_dag(&mut self, task_dag: Dag) -> Result<()> {
        self.dag_scheduler.from_task_dag(task_dag).await?;
        Ok(())
    }

    /// 获取可执行任务
    ///
    /// 返回当前可以执行的任务列表（依赖已满足）。
    pub fn get_ready_tasks(&self) -> Vec<String> {
        self.dag_scheduler.get_ready_nodes()
    }

    /// 更新任务状态
    ///
    /// 更新 DAG 节点状态，影响后续任务的可执行性。
    pub fn update_task_status(&mut self, task_id: &str, status: crate::scheduler::DagNodeStatus) -> Result<()> {
        self.dag_scheduler.update_node_status(task_id, status)?;
        Ok(())
    }

    /// 获取 DAG 统计信息
    pub fn get_dag_stats(&self) -> DagStats {
        self.dag_scheduler.get_stats()
    }

    /// 获取队列统计信息
    pub fn get_queue_stats(&self) -> TaskQueueStats {
        self.task_queue.get_stats()
    }

    /// 获取 DAG 调度器引用
    pub fn dag_scheduler(&self) -> &DagScheduler {
        &self.dag_scheduler
    }

    /// 获取 DAG 调度器可变引用
    pub fn dag_scheduler_mut(&mut self) -> &mut DagScheduler {
        &mut self.dag_scheduler
    }

    /// 获取任务队列引用
    pub fn task_queue(&self) -> &TaskQueue {
        &self.task_queue
    }

    /// 获取任务队列可变引用
    pub fn task_queue_mut(&mut self) -> &mut TaskQueue {
        &mut self.task_queue
    }

    /// 检查 DAG 是否完成
    pub fn is_dag_completed(&self) -> bool {
        self.dag_scheduler.is_completed()
    }

    /// 获取拓扑排序层级
    ///
    /// 返回按层级组织的任务 ID，同层任务可并行执行。
    pub fn topological_levels(&self) -> Result<Vec<Vec<String>>> {
        self.dag_scheduler.topological_levels()
    }

    /// 重置调度器
    ///
    /// 清空队列，重置 DAG 状态。
    pub fn reset(&mut self) {
        self.task_queue.clear();
        self.dag_scheduler.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::repository::TaskRepository;

    #[tokio::test]
    async fn test_scheduler_core_creation() {
        let task_repo = Arc::new(TaskRepository::new_in_memory());
        let core = SchedulerCore::new(task_repo);

        assert_eq!(core.task_queue().len(), 0);
        assert!(core.is_dag_completed());
    }
}
