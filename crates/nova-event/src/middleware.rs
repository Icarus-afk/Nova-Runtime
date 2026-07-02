use crate::{Event, EventId, EventError, SubscriberId};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum MiddlewareAction {
    Continue,
    Reject(EventError),
    Transform,
}

pub trait EventMiddleware: Send + Sync {
    fn before_publish(&self, event: &mut Event) -> Result<MiddlewareAction, EventError>;
    fn after_publish(&self, event: &Event, result: &Result<EventId, EventError>);
    fn on_delivery_failure(&self, event: &Event, subscriber: &SubscriberId, error: &EventError);
}

pub type EventMiddlewareFn = Arc<dyn EventMiddleware>;
