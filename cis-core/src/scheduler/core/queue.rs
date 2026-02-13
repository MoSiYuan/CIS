//! # 任务队列
//!
//! 基于优先级的任务队列管理。
//!
//! ## 核心职责
//! - 优先级队列（二叉堆）
//! - 任务去重
//! - 状态过滤

use std::collections::HashMap;
use std::cmp::Ordering;

use crate::types::{Task, TaskPriority};

/// 任务队列项
#[derive(Debug, Clone)]
pub struct TaskQueueItem {
    /// 任务
    pub task: Task,
    /// 优先级
    pub priority: TaskPriority,
    /// 加入队列时间戳
    pub enqueued_at: chrono::DateTime<chrono::Utc>,
}

impl TaskQueueItem {
    /// 创建新的队列项
    pub fn new(task: Task, priority: TaskPriority) -> Self {
        Self {
            task,
            priority,
            enqueued_at: chrono::Utc::now(),
        }
    }
}

/// 实现优先级比较
impl PartialEq for TaskQueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.task.id == other.task.id
    }
}

impl Eq for TaskQueueItem {}

impl PartialOrd for TaskQueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaskQueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // 优先级高的先出队（Critical > High > Medium > Low）
        match (self.priority, other.priority) {
            (TaskPriority::Critical, TaskPriority::Critical) => other.enqueued_at.cmp(&self.enqueued_at),
            (TaskPriority::Critical, _) => Ordering::Less,
            (_, TaskPriority::Critical) => Ordering::Greater,

            (TaskPriority::High, TaskPriority::High) => other.enqueued_at.cmp(&self.enqueued_at),
            (TaskPriority::High, TaskPriority::Medium | TaskPriority::Low) => Ordering::Less,
            (TaskPriority::Medium | TaskPriority::Low, TaskPriority::High) => Ordering::Greater,

            (TaskPriority::Medium, TaskPriority::Medium) => other.enqueued_at.cmp(&self.enqueued_at),
            (TaskPriority::Medium, TaskPriority::Low) => Ordering::Less,
            (TaskPriority::Low, TaskPriority::Medium) => Ordering::Greater,

            (TaskPriority::Low, TaskPriority::Low) => other.enqueued_at.cmp(&self.enqueued_at),
        }
    }
}

/// 任务队列
///
/// 使用二叉堆实现优先级队列。
#[derive(Debug)]
pub struct TaskQueue {
    /// 内部堆
    inner: std::collections::BinaryHeap<TaskQueueItem>,
    /// 任务 ID 到索引的映射（用于去重）
    task_ids: HashMap<String, bool>,
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskQueue {
    /// 创建新的任务队列
    pub fn new() -> Self {
        Self {
            inner: std::collections::BinaryHeap::new(),
            task_ids: HashMap::new(),
        }
    }

    /// 添加任务到队列
    ///
    /// 如果任务已存在，则忽略。
    pub fn push(&mut self, task: Task, priority: TaskPriority) -> Result<(), TaskQueueError> {
        // 检查是否已存在
        if self.task_ids.contains_key(&task.id) {
            return Err(TaskQueueError::DuplicateTask(task.id));
        }

        let item = TaskQueueItem::new(task, priority);
        self.inner.push(item);
        self.task_ids.insert(item.task.id.clone(), true);

        Ok(())
    }

    /// 从队列中弹出最高优先级任务
    ///
    /// 如果队列为空，返回 None。
    pub fn pop(&mut self) -> Option<Task> {
        match self.inner.pop() {
            Some(item) => {
                self.task_ids.remove(&item.task.id);
                Some(item.task)
            }
            None => None,
        }
    }

    /// 查看队列首元素（不移除）
    pub fn peek(&self) -> Option<&Task> {
        self.inner.peek().map(|item| &item.task)
    }

    /// 获取队列长度
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// 检查队列是否为空
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// 清空队列
    pub fn clear(&mut self) {
        self.inner.clear();
        self.task_ids.clear();
    }

    /// 移除指定任务
    ///
    /// 注意：由于 BinaryHeap 不支持高效删除，此操作为 O(n)。
    pub fn remove(&mut self, task_id: &str) -> bool {
        if !self.task_ids.contains_key(task_id) {
            return false;
        }

        // 重建堆，排除指定任务
        let mut new_heap = std::collections::BinaryHeap::new();
        let mut found = false;

        while let Some(item) = self.inner.pop() {
            if item.task.id == task_id {
                found = true;
            } else {
                new_heap.push(item);
            }
        }

        self.inner = new_heap;
        self.task_ids.remove(task_id);

        found
    }

    /// 按优先级获取任务列表（不修改队列）
    pub fn get_sorted_tasks(&self) -> Vec<Task> {
        let mut items: Vec<_> = self.inner.clone().into_iter().collect();
        items.sort_by(|a, b| a.cmp(b));
        items.into_iter().map(|item| item.task).collect()
    }

    /// 获取队列统计信息
    pub fn get_stats(&self) -> TaskQueueStats {
        let mut stats = TaskQueueStats::default();

        for item in &self.inner {
            match item.priority {
                TaskPriority::Critical => stats.critical += 1,
                TaskPriority::High => stats.high += 1,
                TaskPriority::Medium => stats.medium += 1,
                TaskPriority::Low => stats.low += 1,
            }
        }

        stats.total = self.inner.len();

        stats
    }

    /// 批量添加任务
    pub fn push_batch(&mut self, tasks: Vec<(Task, TaskPriority)>) -> Result<(), TaskQueueError> {
        for (task, priority) in tasks {
            self.push(task, priority)?;
        }
        Ok(())
    }
}

/// 任务队列错误
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskQueueError {
    /// 重复任务
    DuplicateTask(String),
    /// 队列已满
    QueueFull,
}

impl std::fmt::Display for TaskQueueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateTask(id) => write!(f, "Task already in queue: {}", id),
            Self::QueueFull => write!(f, "Task queue is full"),
        }
    }
}

impl std::error::Error for TaskQueueError {}

/// 任务队列统计信息
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct TaskQueueStats {
    /// 总任务数
    pub total: usize,
    /// 关键任务数
    pub critical: usize,
    /// 高优先级任务数
    pub high: usize,
    /// 中优先级任务数
    pub medium: usize,
    /// 低优先级任务数
    pub low: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Task, TaskLevel};

    fn create_test_task(id: &str) -> Task {
        Task {
            id: id.to_string(),
            title: format!("Task {}", id),
            description: None,
            status: crate::types::TaskStatus::Pending,
            priority: TaskPriority::Medium,
            level: TaskLevel::mechanical_default(),
            group: "test".to_string(),
            skill: None,
            input: serde_json::Value::Null,
            output: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            dependencies: vec![],
        }
    }

    #[test]
    fn test_queue_push_pop() {
        let mut queue = TaskQueue::new();

        let task1 = create_test_task("1");
        let task2 = create_test_task("2");

        queue.push(task1, TaskPriority::High).unwrap();
        queue.push(task2, TaskPriority::Low).unwrap();

        assert_eq!(queue.len(), 2);
        assert!(!queue.is_empty());

        // High priority should come first
        let popped = queue.pop().unwrap();
        assert_eq!(popped.id, "1");

        let popped = queue.pop().unwrap();
        assert_eq!(popped.id, "2");

        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_duplicate_rejection() {
        let mut queue = TaskQueue::new();
        let task = create_test_task("1");

        queue.push(task.clone(), TaskPriority::Medium).unwrap();
        let result = queue.push(task, TaskPriority::High);

        assert!(result.is_err());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_priority_ordering() {
        let mut queue = TaskQueue::new();

        queue.push(create_test_task("low"), TaskPriority::Low).unwrap();
        queue.push(create_test_task("critical"), TaskPriority::Critical).unwrap();
        queue.push(create_test_task("high"), TaskPriority::High).unwrap();
        queue.push(create_test_task("medium"), TaskPriority::Medium).unwrap();

        // 应该按 Critical > High > Medium > Low 顺序弹出
        let order = vec!["critical", "high", "medium", "low"];
        for expected_id in order {
            let task = queue.pop().unwrap();
            assert_eq!(task.id, expected_id);
        }
    }

    #[test]
    fn test_remove() {
        let mut queue = TaskQueue::new();

        queue.push(create_test_task("1"), TaskPriority::High).unwrap();
        queue.push(create_test_task("2"), TaskPriority::Medium).unwrap();
        queue.push(create_test_task("3"), TaskPriority::Low).unwrap();

        let removed = queue.remove("2");
        assert!(removed);

        assert_eq!(queue.len(), 2);
        assert!(queue.remove("2")); // Already removed, should be false
    }
}
