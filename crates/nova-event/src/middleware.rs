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

#[cfg(test)]
mod tests {
    use super::*;

    struct TestMiddleware {
        before_count: std::sync::atomic::AtomicU64,
        after_count: std::sync::atomic::AtomicU64,
        fail_count: std::sync::atomic::AtomicU64,
        reject: bool,
    }

    impl TestMiddleware {
        fn new() -> Self {
            TestMiddleware {
                before_count: std::sync::atomic::AtomicU64::new(0),
                after_count: std::sync::atomic::AtomicU64::new(0),
                fail_count: std::sync::atomic::AtomicU64::new(0),
                reject: false,
            }
        }

        fn with_reject() -> Self {
            TestMiddleware {
                before_count: std::sync::atomic::AtomicU64::new(0),
                after_count: std::sync::atomic::AtomicU64::new(0),
                fail_count: std::sync::atomic::AtomicU64::new(0),
                reject: true,
            }
        }
    }

    impl EventMiddleware for TestMiddleware {
        fn before_publish(&self, _event: &mut Event) -> Result<MiddlewareAction, EventError> {
            self.before_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if self.reject {
                Ok(MiddlewareAction::Reject(EventError::Internal("rejected".into())))
            } else {
                Ok(MiddlewareAction::Continue)
            }
        }

        fn after_publish(&self, _event: &Event, _result: &Result<EventId, EventError>) {
            self.after_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        fn on_delivery_failure(&self, _event: &Event, _subscriber: &SubscriberId, _error: &EventError) {
            self.fail_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }

    #[test]
    fn test_middleware_action_continue() {
        match MiddlewareAction::Continue {
            MiddlewareAction::Continue => {}
            _ => panic!("expected Continue"),
        }
    }

    #[test]
    fn test_middleware_action_reject() {
        let action = MiddlewareAction::Reject(EventError::Internal("test".into()));
        match action {
            MiddlewareAction::Reject(e) => {
                assert!(format!("{}", e).contains("test"));
            }
            _ => panic!("expected Reject"),
        }
    }

    #[test]
    fn test_middleware_action_transform() {
        match MiddlewareAction::Transform {
            MiddlewareAction::Transform => {}
            _ => panic!("expected Transform"),
        }
    }

    #[test]
    fn test_middleware_before_publish_continue() {
        let mw = TestMiddleware::new();
        let mut event = crate::EventBuilder::new("test.event").unwrap().build(vec![]);
        let action = mw.before_publish(&mut event).unwrap();
        assert!(matches!(action, MiddlewareAction::Continue));
        assert_eq!(mw.before_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_middleware_before_publish_reject() {
        let mw = TestMiddleware::with_reject();
        let mut event = crate::EventBuilder::new("test.event").unwrap().build(vec![]);
        let action = mw.before_publish(&mut event).unwrap();
        assert!(matches!(action, MiddlewareAction::Reject(_)));
    }

    #[test]
    fn test_middleware_after_publish() {
        let mw = TestMiddleware::new();
        let event = crate::EventBuilder::new("test.event").unwrap().build(vec![]);
        let id = EventId::new();
        mw.after_publish(&event, &Ok(id));
        assert_eq!(mw.after_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_middleware_after_publish_error() {
        let mw = TestMiddleware::new();
        let event = crate::EventBuilder::new("test.event").unwrap().build(vec![]);
        mw.after_publish(&event, &Err(EventError::Internal("fail".into())));
        assert_eq!(mw.after_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_middleware_on_delivery_failure() {
        let mw = TestMiddleware::new();
        let event = crate::EventBuilder::new("test.event").unwrap().build(vec![]);
        let sub = SubscriberId {
            id: "sub-1".into(),
            subsystem: crate::Subsystem::Execution,
            name: "worker".into(),
        };
        mw.on_delivery_failure(&event, &sub, &EventError::Internal("fail".into()));
        assert_eq!(mw.fail_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_middleware_modifies_event() {
        struct ModifyPayload;
        impl EventMiddleware for ModifyPayload {
        fn before_publish(&self, event: &mut Event) -> Result<MiddlewareAction, EventError> {
                event.payload.push(99);
                Ok(MiddlewareAction::Transform)
            }
            fn after_publish(&self, _event: &Event, _result: &Result<EventId, EventError>) {}
            fn on_delivery_failure(&self, _event: &Event, _subscriber: &SubscriberId, _error: &EventError) {}
        }
        let mw = ModifyPayload;
        let mut event = crate::EventBuilder::new("test.event").unwrap().build(vec![1, 2, 3]);
        mw.before_publish(&mut event).unwrap();
        assert_eq!(event.payload, vec![1, 2, 3, 99]);
    }

    #[test]
    fn test_middleware_fn_type() {
        let mw = TestMiddleware::new();
        let arc_mw: EventMiddlewareFn = Arc::new(mw);
        let mut event = crate::EventBuilder::new("test.event").unwrap().build(vec![]);
        let action = arc_mw.before_publish(&mut event).unwrap();
        assert!(matches!(action, MiddlewareAction::Continue));
    }
}
