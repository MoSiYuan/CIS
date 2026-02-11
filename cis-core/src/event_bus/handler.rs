//! # 事件处理器
//!
//! 定义事件处理器的 trait 和工具类型。
//!
//! ## 设计原则
//!
//! - **类型安全**: 支持强类型的处理器
//! - **可组合**: 处理器可以组合使用
//! - **异步优先**: 所有处理器都是异步的
//! - **错误隔离**: 单个处理器失败不影响其他处理器

use async_trait::async_trait;
use crate::error::Result;
use crate::events::EventWrapper;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// 异步事件处理结果
pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

/// 通用事件处理器 Trait
///
/// 所有事件处理器都需要实现此 trait。
#[async_trait]
pub trait EventHandler: Send + Sync {
    /// 处理事件
    ///
    /// # Arguments
    /// * `event` - 事件包装器
    ///
    /// # Returns
    /// * `Ok(())` - 处理成功
    /// * `Err(CisError)` - 处理失败
    async fn handle(&self, event: EventWrapper) -> Result<()>;

    /// 处理器名称（用于日志和调试）
    fn name(&self) -> &str {
        "anonymous"
    }
}

/// 类型化事件处理器 Trait
///
/// 针对特定事件类型的处理器 trait。
///
/// ## 示例
///
/// ```rust
/// use async_trait::async_trait;
/// use cis_core::event_bus::TypedEventHandler;
/// use cis_core::events::{RoomMessageEvent, EventWrapper};
/// use cis_core::error::Result;
///
/// struct MyHandler;
///
/// #[async_trait]
/// impl TypedEventHandler<RoomMessageEvent> for MyHandler {
///     async fn handle_typed(&self, event: RoomMessageEvent) -> Result<()> {
///         println!("Received message: {:?}", event);
///         Ok(())
///     }
/// }
/// ```
#[async_trait]
pub trait TypedEventHandler<E>: Send + Sync {
    /// 处理特定类型的事件
    async fn handle_typed(&self, event: E) -> Result<()>;

    /// 处理器名称
    fn name(&self) -> &str {
        "typed_handler"
    }
}

/// 将类型化处理器包装为通用处理器
pub struct TypedHandlerWrapper<E, H> {
    handler: H,
    _phantom: std::marker::PhantomData<E>,
}

impl<E, H> TypedHandlerWrapper<E, H> {
    /// 创建包装器
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<E, H, Err> EventHandler for TypedHandlerWrapper<E, H>
where
    E: TryFrom<EventWrapper, Error = Err> + Send + Sync,
    Err: Send,
    H: TypedEventHandler<E> + Send + Sync,
{
    async fn handle(&self, event: EventWrapper) -> Result<()> {
        match E::try_from(event) {
            Ok(typed_event) => self.handler.handle_typed(typed_event).await,
            Err(_) => Ok(()), // 类型不匹配，忽略
        }
    }

    fn name(&self) -> &str {
        self.handler.name()
    }
}

/// 函数式事件处理器
pub struct FnEventHandler<F> {
    f: F,
    name: String,
}

impl<F> FnEventHandler<F> {
    /// 创建函数式处理器
    pub fn new(name: impl Into<String>, f: F) -> Self {
        Self {
            f,
            name: name.into(),
        }
    }
}

#[async_trait]
impl<F, Fut> EventHandler for FnEventHandler<F>
where
    F: Fn(EventWrapper) -> Fut + Send + Sync,
    Fut: Future<Output = Result<()>> + Send,
{
    async fn handle(&self, event: EventWrapper) -> Result<()> {
        (self.f)(event).await
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 组合处理器
///
/// 按顺序调用多个处理器
pub struct CompositeHandler {
    handlers: Vec<Box<dyn EventHandler>>,
    name: String,
}

impl CompositeHandler {
    /// 创建新的组合处理器
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            handlers: Vec::new(),
            name: name.into(),
        }
    }

    /// 添加处理器
    pub fn add_handler<H>(mut self, handler: H) -> Self
    where
        H: EventHandler + 'static,
    {
        self.handlers.push(Box::new(handler));
        self
    }

    /// 获取处理器数量
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

#[async_trait]
impl EventHandler for CompositeHandler {
    async fn handle(&self, event: EventWrapper) -> Result<()> {
        for handler in &self.handlers {
            if let Err(e) = handler.handle(event.clone()).await {
                // 记录错误但继续处理其他处理器
                tracing::warn!(
                    handler = handler.name(),
                    error = %e,
                    "Handler failed"
                );
            }
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 条件处理器
///
/// 只在满足条件时调用处理器
pub struct ConditionalHandler<F, H> {
    condition: F,
    handler: H,
    name: String,
}

impl<F, H> ConditionalHandler<F, H> {
    /// 创建条件处理器
    pub fn new(name: impl Into<String>, condition: F, handler: H) -> Self {
        Self {
            condition,
            handler,
            name: name.into(),
        }
    }
}

#[async_trait]
impl<F, H> EventHandler for ConditionalHandler<F, H>
where
    F: Fn(&EventWrapper) -> bool + Send + Sync,
    H: EventHandler + Send + Sync,
{
    async fn handle(&self, event: EventWrapper) -> Result<()> {
        if (self.condition)(&event) {
            self.handler.handle(event).await
        } else {
            Ok(())
        }
    }

    fn name(&self) -> &str {
        &self.name
    }
}

/// 处理器构建器
pub struct HandlerBuilder {
    name: String,
}

impl HandlerBuilder {
    /// 创建新的构建器
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
        }
    }

    /// 使用函数创建处理器
    pub fn from_fn<F, Fut>(self, f: F) -> FnEventHandler<F>
    where
        F: Fn(EventWrapper) -> Fut + Send + Sync,
        Fut: Future<Output = Result<()>> + Send,
    {
        FnEventHandler::new(self.name, f)
    }

    /// 创建组合处理器
    pub fn composite(self) -> CompositeHandler {
        CompositeHandler::new(self.name)
    }
}

/// 处理器注册表
///
/// 管理所有事件处理器的注册和查找
pub struct HandlerRegistry {
    handlers: std::collections::HashMap<String, Vec<Arc<dyn EventHandler>>>,
}

impl HandlerRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            handlers: std::collections::HashMap::new(),
        }
    }

    /// 注册处理器
    pub fn register<H>(&mut self, topic: impl Into<String>, handler: H)
    where
        H: EventHandler + 'static,
    {
        let topic = topic.into();
        self.handlers
            .entry(topic)
            .or_insert_with(Vec::new)
            .push(Arc::new(handler));
    }

    /// 获取主题的所有处理器
    pub fn get_handlers(&self, topic: &str) -> Vec<Arc<dyn EventHandler>> {
        self.handlers
            .get(topic)
            .cloned()
            .unwrap_or_default()
    }

    /// 获取所有支持的主题
    pub fn topics(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// 清空注册表
    pub fn clear(&mut self) {
        self.handlers.clear();
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{RoomMessageEvent, MessageContent};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_fn_event_handler() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let handler = FnEventHandler::new("test_handler", move |event: EventWrapper| {
            let counter = counter_clone.clone();
            async move {
                if let EventWrapper::RoomMessage(_) = event {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
                Ok(())
            }
        });

        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );

        handler.handle(EventWrapper::RoomMessage(event)).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_composite_handler() {
        let counter = Arc::new(AtomicUsize::new(0));

        let handler1 = {
            let counter = counter.clone();
            FnEventHandler::new("h1", move |_event: EventWrapper| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
        };

        let handler2 = {
            let counter = counter.clone();
            FnEventHandler::new("h2", move |_event: EventWrapper| {
                let counter = counter.clone();
                async move {
                    counter.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            })
        };

        let composite = CompositeHandler::new("composite")
            .add_handler(handler1)
            .add_handler(handler2);

        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );

        composite.handle(EventWrapper::RoomMessage(event)).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_conditional_handler() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let inner_handler = FnEventHandler::new("inner", move |_event: EventWrapper| {
            let counter = counter_clone.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });

        // 只处理 room.message 事件
        let conditional = ConditionalHandler::new(
            "conditional",
            |event: &EventWrapper| matches!(event, EventWrapper::RoomMessage(_)),
            inner_handler,
        );

        let room_event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );

        // 应该触发
        conditional.handle(EventWrapper::RoomMessage(room_event.clone())).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        // 不应该触发
        let skill_event = crate::events::SkillExecuteEvent::new(
            "test-skill",
            "execute",
            serde_json::json!({}),
            "user-1",
            crate::events::ExecutionContext::from_room("room-1", "user-1"),
        );
        conditional.handle(EventWrapper::SkillExecute(skill_event)).await.unwrap();
        assert_eq!(counter.load(Ordering::SeqCst), 1); // 计数不变
    }

    #[tokio::test]
    async fn test_handler_registry() {
        let mut registry = HandlerRegistry::new();

        let handler = FnEventHandler::new("test", |_event: EventWrapper| async move { Ok(()) });
        registry.register("room.message", handler);

        let handlers = registry.get_handlers("room.message");
        assert_eq!(handlers.len(), 1);

        let empty = registry.get_handlers("unknown");
        assert!(empty.is_empty());

        let topics = registry.topics();
        assert!(topics.contains(&"room.message".to_string()));
    }
}
