//! # EventBus Trait
//!
//! 事件总线的抽象接口，定义事件发布订阅的基本操作。
//!
//! ## 注意
//!
//! 此模块已迁移到 `crate::event_bus`，此处保留是为了向后兼容。
//! 新代码应该直接使用 `crate::event_bus` 模块。

pub use crate::event_bus::{
    EventBus, 
    EventBusRef, 
    EventBusExt,
    Subscription,
    EventHandlerFn,
};
pub use crate::events::EventWrapper as DomainEvent;

/// 事件处理器（向后兼容）
pub use crate::event_bus::EventHandler;

/// BoxedEventHandler 类型别名（向后兼容）
pub type BoxedEventHandler = Box<dyn EventHandler>;

use crate::error::Result;
use async_trait::async_trait;
use crate::events::EventWrapper;

/// 兼容层：将旧的 DomainEvent 转换
pub fn convert_to_wrapper(event: DomainEvent) -> EventWrapper {
    event
}

/// 向后兼容的 EventBus 扩展 trait
#[async_trait]
pub trait EventBusExtCompat: EventBus {
    /// 发布包装事件（向后兼容）
    async fn publish_wrapper(&self, event: EventWrapper) -> Result<()> {
        self.publish(event).await
    }
}

impl<T: EventBus + ?Sized> EventBusExtCompat for T {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::MemoryEventBus;
    use crate::events::{RoomMessageEvent, MessageContent};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_backward_compatibility() {
        // 确保旧的 API 仍然工作
        let bus: EventBusRef = Arc::new(MemoryEventBus::new());
        
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        // 使用 EventBusExt trait 的 subscribe_fn 方法
        bus.subscribe_fn("room.message", move |_event| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }).await.unwrap();

        let event = RoomMessageEvent::new(
            "room-1",
            "user-1",
            MessageContent::Text { body: "test".to_string() },
        );

        bus.publish(EventWrapper::RoomMessage(event)).await.unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }
}
