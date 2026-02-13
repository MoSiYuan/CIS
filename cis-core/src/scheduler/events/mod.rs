//! # 事件系统
//!
//! 管理调度器事件的监听、触发和传播。

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::types::TaskStatus;
use super::execution::ExecutionResult;

/// 调度器事件类型
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SchedulerEventType {
    /// 任务完成
    TaskCompleted,
    /// 任务失败
    TaskFailed,
    /// DAG 构建完成
    DagBuilt,
    /// DAG 执行开始
    DagStarted,
    /// DAG 执行完成
    DagCompleted,
    /// DAG 执行失败
    DagFailed,
}

/// 调度器事件
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    /// 任务完成事件
    TaskCompleted {
        task_id: String,
        result: ExecutionResult,
    },
    /// 任务失败事件
    TaskFailed {
        task_id: String,
        error: String,
    },
    /// DAG 构建完成事件
    DagBuilt {
        dag_id: String,
        node_count: usize,
    },
    /// DAG 执行开始事件
    DagStarted {
        dag_id: String,
    },
    /// DAG 执行完成事件
    DagCompleted {
        dag_id: String,
        duration_secs: f64,
    },
    /// DAG 执行失败事件
    DagFailed {
        dag_id: String,
        error: String,
    },
}

impl SchedulerEvent {
    /// 获取事件类型
    pub fn event_type(&self) -> SchedulerEventType {
        match self {
            Self::TaskCompleted { .. } => SchedulerEventType::TaskCompleted,
            Self::TaskFailed { .. } => SchedulerEventType::TaskFailed,
            Self::DagBuilt { .. } => SchedulerEventType::DagBuilt,
            Self::DagStarted { .. } => SchedulerEventType::DagStarted,
            Self::DagCompleted { .. } => SchedulerEventType::DagCompleted,
            Self::DagFailed { .. } => SchedulerEventType::DagFailed,
        }
    }

    /// 获取事件 ID（用于日志和追踪）
    pub fn event_id(&self) -> String {
        match self {
            Self::TaskCompleted { task_id, .. } => format!("task-completed:{}", task_id),
            Self::TaskFailed { task_id, .. } => format!("task-failed:{}", task_id),
            Self::DagBuilt { dag_id, .. } => format!("dag-built:{}", dag_id),
            Self::DagStarted { dag_id } => format!("dag-started:{}", dag_id),
            Self::DagCompleted { dag_id, .. } => format!("dag-completed:{}", dag_id),
            Self::DagFailed { dag_id, .. } => format!("dag-failed:{}", dag_id),
        }
    }
}

/// 事件监听器 Trait
///
/// 定义事件处理接口。
#[async_trait]
pub trait EventListener: Send + Sync {
    /// 处理事件
    async fn on_event(&self, event: &SchedulerEvent) -> Result<()>;

    /// 获取监听器名称
    fn name(&self) -> &str {
        "listener"
    }
}

/// 事件注册表
///
/// 管理事件监听器的注册和触发。
pub struct EventRegistry {
    /// 监听器映射：事件类型 -> 监听器列表
    listeners: Arc<RwLock<HashMap<SchedulerEventType, Vec<Arc<dyn EventListener>>>>>,
}

impl EventRegistry {
    /// 创建新的事件注册表
    pub fn new() -> Self {
        Self {
            listeners: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册事件监听器
    pub async fn register(&self, event_type: SchedulerEventType, listener: Arc<dyn EventListener>) {
        let mut listeners = self.listeners.write().await;
        listeners.entry(event_type).or_insert_with(Vec::new).push(listener);
    }

    /// 触发事件
    pub async fn emit(&self, event: SchedulerEvent) -> Result<()> {
        let event_type = event.event_type();
        let event_id = event.event_id();

        tracing::debug!(
            event_id = %event_id,
            event_type = ?event_type,
            "Emitting scheduler event"
        );

        let listeners = self.listeners.read().await;

        if let Some(listener_list) = listeners.get(&event_type) {
            for listener in listener_list {
                if let Err(e) = listener.on_event(&event).await {
                    tracing::error!(
                        listener = listener.name(),
                        event_id = %event_id,
                        error = %e,
                        "Failed to handle event"
                    );
                }
            }
        }

        Ok(())
    }

    /// 移除监听器
    pub async fn unregister(&self, event_type: SchedulerEventType, listener_name: &str) {
        let mut listeners = self.listeners.write().await;

        if let Some(listener_list) = listeners.get_mut(&event_type) {
            listener_list.retain(|listener| listener.name() != listener_name);
        }
    }

    /// 获取监听器数量
    pub async fn listener_count(&self, event_type: &SchedulerEventType) -> usize {
        let listeners = self.listeners.read().await;
        listeners.get(event_type).map(|list| list.len()).unwrap_or(0)
    }

    /// 清空所有监听器
    pub async fn clear(&self) {
        let mut listeners = self.listeners.write().await;
        listeners.clear();
    }
}

impl Default for EventRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 日志事件监听器
///
/// 将所有事件记录到日志。
pub struct LoggingEventListener {
    name: String,
}

impl LoggingEventListener {
    /// 创建新的日志监听器
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl EventListener for LoggingEventListener {
    async fn on_event(&self, event: &SchedulerEvent) -> Result<()> {
        match event {
            SchedulerEvent::TaskCompleted { task_id, result } => {
                tracing::info!(
                    listener = %self.name,
                    task_id = %task_id,
                    status = %result.status,
                    duration_secs = result.duration_secs,
                    "Task completed"
                );
            }
            SchedulerEvent::TaskFailed { task_id, error } => {
                tracing::error!(
                    listener = %self.name,
                    task_id = %task_id,
                    error = %error,
                    "Task failed"
                );
            }
            SchedulerEvent::DagBuilt { dag_id, node_count } => {
                tracing::info!(
                    listener = %self.name,
                    dag_id = %dag_id,
                    node_count = node_count,
                    "DAG built"
                );
            }
            SchedulerEvent::DagStarted { dag_id } => {
                tracing::info!(
                    listener = %self.name,
                    dag_id = %dag_id,
                    "DAG started"
                );
            }
            SchedulerEvent::DagCompleted { dag_id, duration_secs } => {
                tracing::info!(
                    listener = %self.name,
                    dag_id = %dag_id,
                    duration_secs = duration_secs,
                    "DAG completed"
                );
            }
            SchedulerEvent::DagFailed { dag_id, error } => {
                tracing::error!(
                    listener = %self.name,
                    dag_id = %dag_id,
                    error = %error,
                    "DAG failed"
                );
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEventListener {
        name: String,
        events: Arc<RwLock<Vec<SchedulerEvent>>>,
    }

    impl TestEventListener {
        fn new(name: String) -> Self {
            Self {
                name,
                events: Arc::new(RwLock::new(Vec::new())),
            }
        }

        async fn get_event_count(&self) -> usize {
            self.events.read().await.len()
        }
    }

    #[async_trait]
    impl EventListener for TestEventListener {
        async fn on_event(&self, event: &SchedulerEvent) -> Result<()> {
            let mut events = self.events.write().await;
            events.push(event.clone());
            Ok(())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_event_registry() {
        let registry = EventRegistry::new();
        let listener = Arc::new(TestEventListener::new("test".to_string()));

        registry.register(SchedulerEventType::TaskCompleted, listener.clone()).await;

        let event = SchedulerEvent::TaskCompleted {
            task_id: "1".to_string(),
            result: ExecutionResult::success("1".to_string(), serde_json::json!({}), 1.0),
        };

        registry.emit(event).await.unwrap();

        assert_eq!(listener.get_event_count().await, 1);
    }

    #[tokio::test]
    async fn test_logging_listener() {
        let registry = EventRegistry::new();
        let listener = Arc::new(LoggingEventListener::new("logger".to_string()));

        registry.register(SchedulerEventType::TaskCompleted, listener).await;

        let event = SchedulerEvent::TaskCompleted {
            task_id: "1".to_string(),
            result: ExecutionResult::success("1".to_string(), serde_json::json!({}), 1.0),
        };

        registry.emit(event).await.unwrap();
    }
}
