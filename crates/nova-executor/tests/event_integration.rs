#![allow(unused_imports)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crossbeam::channel;
use nova_event::*;
use nova_executor::*;
use uuid::Uuid;

fn test_addr() -> SocketAddr {
    "127.0.0.1:8080".parse().unwrap()
}

// Middleware that publishes an event to the bus when the pipeline runs
struct EventPublishingMiddleware {
    bus: Arc<EventBus>,
    event_type: &'static str,
    stage: PipelineStage,
}

impl Middleware for EventPublishingMiddleware {
    fn name(&self) -> &'static str {
        "event_publisher"
    }
    fn stage(&self) -> PipelineStage {
        self.stage
    }
    fn handle(
        &self,
        ctx: &mut OperationContext,
        req: &mut OperationRequest,
        next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
    ) -> PipelineResult {
        // Build and publish an event about this operation
        let payload = serde_json::json!({
            "operation_type": format!("{:?}", req.operation_type),
            "trace_id": format!("{:x}", ctx.trace_id),
        });
        let event = EventBuilder::new(self.event_type)
            .unwrap()
            .source(Subsystem::Execution, "pipeline", "node-1", "inst-a")
            .build(serde_json::to_vec(&payload).unwrap());
        let _ = self.bus.publish(event);

        next(ctx, req)
    }
}

// Helper to create a subscription with a channel receiver
fn make_subscription(
    topic: &str,
    capacity: usize,
) -> (Subscription, channel::Receiver<Event>) {
    let (tx, rx) = crossbeam::channel::bounded(capacity);
    let sub = Subscription {
        id: Uuid::new_v4(),
        subscriber: SubscriberId {
            id: "test-sub".into(),
            subsystem: Subsystem::Execution,
            name: "integration-test".into(),
        },
        topic: TopicPattern::new(topic).unwrap(),
        content_filter: None,
        delivery_guarantee: DeliveryGuarantee::AtMostOnce,
        max_retries: 0,
        retry_backoff_ms: 0,
        max_backoff_ms: 0,
        queue_capacity: capacity,
        created_at: 0,
        active: true,
        consumer_group: None,
        sender: tx,
    };
    (sub, rx)
}

// ==================== a) Event publish → subscribe cycle through executor ====================

#[tokio::test]
async fn test_event_publish_subscribe_cycle() {
    let bus = Arc::new(EventBus::new(4, OverflowPolicy::DropNewest, 1024 * 1024, 1000));

    // Subscribe to our event topic
    let (sub, rx) = make_subscription("nova.executor.operation.completed", 16);
    bus.subscribe(sub).unwrap();

    // Create pipeline executor with event-publishing middleware at the Notify stage
    let pipeline = PipelineExecutor::new(PipelineConfig::default());
    let publisher = EventPublishingMiddleware {
        bus: bus.clone(),
        event_type: "nova.executor.operation.completed",
        stage: PipelineStage::Notify,
    };
    pipeline
        .register_middleware(MiddlewareRegistration {
            name: "event_publisher".into(),
            stage: PipelineStage::Notify,
            order: 1,
            middleware: Arc::new(publisher),
            enabled: true,
            config: HashMap::new(),
        })
        .unwrap();

    // Execute an operation through the pipeline
    let ctx = OperationContextBuilder::new(test_addr()).build();
    let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
    let resp = pipeline.execute(req, ctx).await;
    assert!(resp.success, "pipeline operation should succeed");

    // Verify the subscriber received the event
    let event = rx.try_recv().expect("subscriber should receive an event");
    assert_eq!(
        event.metadata.event_type.canonical,
        "nova.executor.operation.completed"
    );
    assert_eq!(event.metadata.source.subsystem, Subsystem::Execution);
    assert_eq!(event.metadata.source.component, "pipeline");
}

// ==================== b) Multiple subscribers ====================

#[tokio::test]
async fn test_multiple_subscribers_different_patterns() {
    let bus = Arc::new(EventBus::new(4, OverflowPolicy::DropNewest, 1024 * 1024, 1000));

    // Subscribe two subscribers with different patterns
    let (sub_exact, rx_exact) = make_subscription("nova.executor.operation.completed", 16);
    let (sub_wild, rx_wild) = make_subscription("nova.executor.*", 16);
    let (sub_unrelated, rx_unrelated) = make_subscription("nova.storage.*", 16);

    bus.subscribe(sub_exact).unwrap();
    bus.subscribe(sub_wild).unwrap();
    bus.subscribe(sub_unrelated).unwrap();

    // Create pipeline executor with event-publishing middleware
    let pipeline = PipelineExecutor::new(PipelineConfig::default());
    let publisher = EventPublishingMiddleware {
        bus: bus.clone(),
        event_type: "nova.executor.operation.completed",
        stage: PipelineStage::Notify,
    };
    pipeline
        .register_middleware(MiddlewareRegistration {
            name: "event_publisher".into(),
            stage: PipelineStage::Notify,
            order: 1,
            middleware: Arc::new(publisher),
            enabled: true,
            config: HashMap::new(),
        })
        .unwrap();

    // Execute an operation
    let ctx = OperationContextBuilder::new(test_addr()).build();
    let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
    let resp = pipeline.execute(req, ctx).await;
    assert!(resp.success);

    // Exact-match subscriber should get the event
    let event_exact = rx_exact.try_recv().expect("exact-match sub should receive event");
    assert_eq!(
        event_exact.metadata.event_type.canonical,
        "nova.executor.operation.completed"
    );

    // Wildcard subscriber should also get the event
    let event_wild = rx_wild.try_recv().expect("wildcard sub should receive event");
    assert_eq!(
        event_wild.metadata.event_type.canonical,
        "nova.executor.operation.completed"
    );

    // Unrelated subscriber should NOT receive the event (different topic tree)
    let result = rx_unrelated.try_recv();
    assert!(
        result.is_err(),
        "unrelated subscriber should not receive the event"
    );
}
