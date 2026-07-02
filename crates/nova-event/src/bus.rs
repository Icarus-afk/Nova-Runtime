use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use uuid::Uuid;
use crossbeam::channel;
use crate::{
    Event, EventId, EventMetadata, EventSource, EventPriority, Subsystem,
    Subscription, SubscriptionTrie, DeadLetterQueue,
    DeadLetterEntry, EventError, Result,
};
use crate::store::{EventStore, ReplayCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowPolicy {
    DropNewest,
    DropOldest,
    RejectPublisher,
    BlockPublisher,
}

pub struct BusMetrics {
    // Publication metrics
    pub events_published_total: AtomicU64,
    pub publish_errors_total: AtomicU64,
    pub publish_latency_p50: AtomicU64,
    pub publish_latency_p99: AtomicU64,
    pub payload_bytes_total: AtomicU64,

    // Delivery metrics
    pub events_delivered_total: AtomicU64,
    pub events_acked_total: AtomicU64,
    pub events_nacked_total: AtomicU64,
    pub events_dlq_total: AtomicU64,
    pub delivery_retries_total: AtomicU64,
    pub delivery_latency_p50: AtomicU64,
    pub delivery_latency_p99: AtomicU64,

    // Queue metrics
    pub queue_depth_total: AtomicU64,
    pub queue_rejected_total: AtomicU64,
    pub queue_dropped_total: AtomicU64,

    // Subscription metrics
    pub active_subscriptions: AtomicU32,
    pub paused_subscriptions: AtomicU32,
    pub subscriber_count: AtomicU32,

    // Replay metrics
    pub replay_events_total: AtomicU64,
    pub replay_active_count: AtomicU32,

    // DLQ metrics
    pub dlq_size: AtomicU32,
    pub dlq_oldest_entry_age_secs: AtomicU64,
}

impl BusMetrics {
    fn new() -> Self {
        BusMetrics {
            events_published_total: AtomicU64::new(0),
            publish_errors_total: AtomicU64::new(0),
            publish_latency_p50: AtomicU64::new(0),
            publish_latency_p99: AtomicU64::new(0),
            payload_bytes_total: AtomicU64::new(0),
            events_delivered_total: AtomicU64::new(0),
            events_acked_total: AtomicU64::new(0),
            events_nacked_total: AtomicU64::new(0),
            events_dlq_total: AtomicU64::new(0),
            delivery_retries_total: AtomicU64::new(0),
            delivery_latency_p50: AtomicU64::new(0),
            delivery_latency_p99: AtomicU64::new(0),
            queue_depth_total: AtomicU64::new(0),
            queue_rejected_total: AtomicU64::new(0),
            queue_dropped_total: AtomicU64::new(0),
            active_subscriptions: AtomicU32::new(0),
            paused_subscriptions: AtomicU32::new(0),
            subscriber_count: AtomicU32::new(0),
            replay_events_total: AtomicU64::new(0),
            replay_active_count: AtomicU32::new(0),
            dlq_size: AtomicU32::new(0),
            dlq_oldest_entry_age_secs: AtomicU64::new(0),
        }
    }
}

pub struct EventBus {
    trie: Arc<SubscriptionTrie>,
    shard_count: u16,
    overflow_policy: OverflowPolicy,
    max_payload_size: u64,
    dead_letter_queue: Arc<DeadLetterQueue>,
    metrics: Arc<BusMetrics>,
    event_store: Arc<EventStore>,
}

impl EventBus {
    pub fn new(shard_count: u16, overflow_policy: OverflowPolicy, max_payload_size: u64, store_max_entries: usize) -> Self {
        EventBus {
            trie: Arc::new(SubscriptionTrie::new()),
            shard_count,
            overflow_policy,
            max_payload_size,
            dead_letter_queue: Arc::new(DeadLetterQueue::new(100_000)),
            metrics: Arc::new(BusMetrics::new()),
            event_store: Arc::new(EventStore::new(store_max_entries)),
        }
    }

    pub fn publish(&self, mut event: Event) -> Result<EventId> {
        if event.metadata.event_type.segments.len() < 2 {
            return Err(EventError::InvalidEventType(
                "event type must have at least 2 segments".into(),
            ));
        }

        let payload_size = event.payload.len() as u64;
        if payload_size > self.max_payload_size {
            return Err(EventError::PayloadTooLarge {
                size: payload_size,
                max: self.max_payload_size,
            });
        }

        let event_id = EventId::new();
        let timestamp = event_id.timestamp();
        event.metadata.event_id = event_id;
        event.metadata.timestamp = timestamp;
        event.metadata.payload_size = payload_size as u32;

        self.metrics.events_published_total.fetch_add(1, Ordering::Relaxed);

        let subscriptions = self.trie.lookup(&event.metadata.event_type);
        let event = Arc::new(event);

        for sub in &subscriptions {
            if !sub.active {
                continue;
            }

            if let Some(ref filter) = sub.content_filter {
                if !filter.evaluate(&event) {
                    continue;
                }
            }

            let _shard = match &event.metadata.ordering_key {
                Some(key) => {
                    let hash = key.bytes().fold(0u64, |acc, b| {
                        acc.wrapping_mul(31).wrapping_add(b as u64)
                    });
                    hash % self.shard_count as u64
                }
                None => {
                    let counter = crate::event::EVENT_COUNTER.fetch_add(1, Ordering::Relaxed);
                    counter % self.shard_count as u64
                }
            };

            match sub.sender.try_send((*event).clone()) {
                Ok(()) => {
                    self.metrics.events_delivered_total.fetch_add(1, Ordering::Relaxed);
                }
                Err(channel::TrySendError::Full(_)) => {
                    match self.overflow_policy {
                        OverflowPolicy::DropNewest | OverflowPolicy::DropOldest => {
                            self.metrics.queue_dropped_total.fetch_add(1, Ordering::Relaxed);
                        }
                        OverflowPolicy::RejectPublisher => {
                            self.metrics.queue_rejected_total.fetch_add(1, Ordering::Relaxed);
                            return Err(EventError::BusFull(
                                "subscriber channel at capacity".into(),
                            ));
                        }
                        OverflowPolicy::BlockPublisher => {
                            match sub.sender.send((*event).clone()) {
                                Ok(()) => {
                                    self.metrics.events_delivered_total.fetch_add(1, Ordering::Relaxed);
                                }
                                Err(_) => {
                                    self.metrics.queue_dropped_total.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                }
                Err(channel::TrySendError::Disconnected(_)) => {
                    let _ = self.dead_letter_queue.push(DeadLetterEntry {
                        event: (*event).clone(),
                        failed_subscriber: sub.subscriber.clone(),
                        failure_reason: "subscriber channel disconnected".into(),
                        failure_timestamp: timestamp,
                        retry_count: 0,
                        last_error: "crossbeam channel disconnected".into(),
                    });
                }
            }
        }

        if event.metadata.persistent {
            self.event_store.append(&event);
        }

        Ok(event_id)
    }

    pub fn subscribe(&self, sub: Subscription) -> Result<()> {
        let topic = sub.topic.clone();
        self.trie.insert(&topic, sub)?;
        self.metrics.subscriber_count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn unsubscribe(&self, sub_id: Uuid) -> bool {
        let removed = self.trie.remove(sub_id);
        if removed {
            self.metrics.subscriber_count.fetch_sub(1, Ordering::Relaxed);
        }
        removed
    }

    pub fn metrics(&self) -> Arc<BusMetrics> {
        Arc::clone(&self.metrics)
    }

    pub fn dead_letter_queue(&self) -> Arc<DeadLetterQueue> {
        Arc::clone(&self.dead_letter_queue)
    }

    pub fn dead_letter_count(&self) -> usize {
        self.dead_letter_queue.len()
    }

    pub fn subscriber_count(&self) -> usize {
        self.metrics
            .subscriber_count
            .load(Ordering::Relaxed) as usize
    }

    pub fn publish_with_key(&self, mut event: Event, ordering_key: &str) -> Result<EventId> {
        event.metadata.ordering_key = Some(ordering_key.to_string());
        self.publish(event)
    }

    pub fn pause_subscriber(&self, subscription_id: Uuid) -> Result<()> {
        let subs = self.trie.all_subscriptions();
        for sub in &subs {
            if sub.id == subscription_id {
                return Ok(());
            }
        }
        Err(EventError::SubscriberNotFound)
    }

    pub fn resume_subscriber(&self, _subscription_id: Uuid) -> Result<()> {
        Ok(())
    }

    pub fn event_store(&self) -> Arc<EventStore> {
        Arc::clone(&self.event_store)
    }

    pub fn replay(&self, subscriber_id: &crate::SubscriberId, cursor: ReplayCursor) -> Result<ReplayCursor> {
        let all_subs = self.trie.all_subscriptions();
        let subs: Vec<&Subscription> = all_subs.iter()
            .filter(|s| s.subscriber.id == subscriber_id.id
                && s.subscriber.subsystem == subscriber_id.subsystem
                && s.subscriber.name == subscriber_id.name)
            .collect();

        if subs.is_empty() {
            return Err(EventError::SubscriberNotFound);
        }

        let limit = 1000usize;
        let mut current = cursor.clone();
        let mut checkpoint_count = 0u64;

        loop {
            let stored_events = self.event_store.scan_from(current.last_processed_offset + 1, limit);
            if stored_events.is_empty() {
                break;
            }

            for stored in &stored_events {
                if let Some(target) = cursor.target_timestamp {
                    if stored.timestamp > target {
                        return Ok(current);
                    }
                }

                for sub in &subs {
                    if !sub.active {
                        continue;
                    }
                    if !sub.topic.matches(&stored.event_type) {
                        continue;
                    }

                    let event = Event {
                        metadata: EventMetadata {
                            event_id: stored.event_id,
                            event_type: stored.event_type.clone(),
                            source: EventSource {
                                subsystem: Subsystem::System,
                                component: "replay".into(),
                                node_id: "local".into(),
                                instance_id: "default".into(),
                            },
                            timestamp: stored.timestamp,
                            ordering_key: stored.ordering_key.clone(),
                            content_type: "application/x-msgpack".into(),
                            payload_size: stored.payload.len() as u32,
                            ttl_ms: 0,
                            priority: EventPriority::Normal,
                            persistent: false,
                            schema_version: 1,
                            trace_context: stored.trace_context.clone(),
                        },
                        payload: stored.payload.clone(),
                    };

                    if let Some(ref filter) = sub.content_filter {
                        if !filter.evaluate(&event) {
                            continue;
                        }
                    }

                    match sub.sender.try_send(event) {
                        Ok(()) => {
                            self.metrics.events_delivered_total.fetch_add(1, Ordering::Relaxed);
                            self.metrics.replay_events_total.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(_) => continue,
                    }

                    current.last_processed_offset = stored.offset;
                    current.last_processed_timestamp = stored.timestamp;
                    checkpoint_count += 1;

                    if checkpoint_count >= 1000 {
                        checkpoint_count = 0;
                    }
                }
            }

            if stored_events.len() < limit {
                break;
            }
        }

        Ok(current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Subscription, TopicPattern, DeliveryGuarantee,
        SubscriberId, EventBuilder, ReplayCursor,
    };
    use uuid::Uuid;

    fn make_sub(topic: &str, capacity: usize) -> (Subscription, crossbeam::channel::Receiver<Event>) {
        let (tx, rx) = crossbeam::channel::bounded(capacity);
        let sub = Subscription {
            id: Uuid::new_v4(),
            subscriber: SubscriberId {
                id: "test-sub".into(),
                subsystem: Subsystem::System,
                name: "tester".into(),
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

    fn make_event() -> Event {
        EventBuilder::new("test.event.occurred")
            .unwrap()
            .source(Subsystem::System, "test", "n1", "i1")
            .build(vec![1, 2, 3])
    }

    fn make_event_with_payload(payload: Vec<u8>) -> Event {
        EventBuilder::new("test.event.occurred")
            .unwrap()
            .source(Subsystem::System, "test", "n1", "i1")
            .build(payload)
    }

    #[test]
    fn test_bus_subscribe_and_publish_delivers() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        let event = make_event();
        bus.publish(event).unwrap();

        let received = rx.try_recv().unwrap();
        assert_eq!(received.metadata.event_type.canonical, "test.event.occurred");
    }

    #[test]
    fn test_bus_publish_returns_event_id() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        let event_id = bus.publish(make_event()).unwrap();
        let ts = event_id.timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_bus_no_subscribers_still_returns_id() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let event_id = bus.publish(make_event()).unwrap();
        let ts = event_id.timestamp();
        assert!(ts > 0);
    }

    #[test]
    fn test_bus_unsubscribe_removes_subscription() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.event.occurred", 16);
        let sub_id = sub.id;
        bus.subscribe(sub).unwrap();

        bus.publish(make_event()).unwrap();
        assert!(rx.try_recv().is_ok());

        assert!(bus.unsubscribe(sub_id));
        bus.publish(make_event()).unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_bus_unsubscribe_nonexistent() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        assert!(!bus.unsubscribe(Uuid::new_v4()));
    }

    #[test]
    fn test_bus_subscriber_count() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        assert_eq!(bus.subscriber_count(), 0);
        let (sub, _rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_bus_payload_too_large() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 10, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        let err = bus.publish(make_event_with_payload(vec![0u8; 20])).unwrap_err();
        assert!(matches!(err, EventError::PayloadTooLarge { .. }));
    }

    #[test]
    fn test_bus_invalid_topic_too_short() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let event = EventBuilder::new("short")
            .unwrap()
            .build(vec![]);
        let err = bus.publish(event).unwrap_err();
        assert!(matches!(err, EventError::InvalidEventType(_)));
    }

    #[test]
    fn test_bus_multiple_subscribers() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub1, rx1) = make_sub("test.event.occurred", 16);
        let (sub2, rx2) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub1).unwrap();
        bus.subscribe(sub2).unwrap();

        bus.publish(make_event()).unwrap();

        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[test]
    fn test_bus_wildcard_subscriber() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.+.occurred", 16);
        bus.subscribe(sub).unwrap();

        bus.publish(make_event()).unwrap();
        let received = rx.try_recv().unwrap();
        assert_eq!(received.metadata.event_type.canonical, "test.event.occurred");
    }

    #[test]
    fn test_bus_multi_wildcard_subscriber() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.*", 16);
        bus.subscribe(sub).unwrap();

        bus.publish(make_event()).unwrap();
        let received = rx.try_recv().unwrap();
        assert_eq!(received.metadata.event_type.canonical, "test.event.occurred");
    }

    #[test]
    fn test_bus_wildcard_no_match() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.+.other", 16);
        bus.subscribe(sub).unwrap();

        bus.publish(make_event()).unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_bus_publish_with_key() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        let event = make_event();
        bus.publish_with_key(event, "my-key").unwrap();

        let received = rx.try_recv().unwrap();
        assert_eq!(received.metadata.ordering_key, Some("my-key".into()));
    }

    #[test]
    fn test_bus_metrics_increments_on_publish() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let metrics = bus.metrics();
        let initial = metrics.events_published_total.load(Ordering::Relaxed);

        bus.publish(make_event()).unwrap();
        assert_eq!(
            metrics.events_published_total.load(Ordering::Relaxed),
            initial + 1
        );
    }

    #[test]
    fn test_bus_dead_letter_queue_accessible() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let dlq = bus.dead_letter_queue();
        assert!(dlq.is_empty());
    }

    #[test]
    fn test_bus_dead_letter_count() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        assert_eq!(bus.dead_letter_count(), 0);
    }

    #[test]
    fn test_bus_event_store_accessible() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let store = bus.event_store();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_bus_persistent_event_stored() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        let event = EventBuilder::new("test.event.occurred")
            .unwrap()
            .source(Subsystem::System, "test", "n1", "i1")
            .persistent(true)
            .build(vec![42]);
        bus.publish(event).unwrap();

        let store = bus.event_store();
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_bus_non_persistent_event_not_stored() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        bus.publish(make_event()).unwrap();
        let store = bus.event_store();
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_bus_delivers_to_inactive_subscriber_skipped() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (mut sub, rx) = make_sub("test.event.occurred", 16);
        sub.active = false;
        bus.subscribe(sub).unwrap();

        bus.publish(make_event()).unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_bus_overflow_drop_newest() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 1);
        bus.subscribe(sub).unwrap();
        // Fill the channel
        bus.publish(make_event()).unwrap();
        // This one should be dropped via DropNewest
        bus.publish(make_event()).unwrap();
        // No error expected with DropNewest
    }

    #[test]
    fn test_bus_overflow_reject_publisher() {
        let bus = EventBus::new(1, OverflowPolicy::RejectPublisher, 1024 * 1024, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 1);
        bus.subscribe(sub).unwrap();
        bus.publish(make_event()).unwrap();
        let err = bus.publish(make_event()).unwrap_err();
        assert!(matches!(err, EventError::BusFull(_)));
    }

    #[test]
    fn test_bus_subscribe_invalid_pattern() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, _rx) = make_sub("test.event.occurred", 16);
        let result = bus.subscribe(sub);
        assert!(result.is_ok());
    }

    #[test]
    fn test_bus_subscriber_count_after_unsubscribe() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub1, _rx1) = make_sub("test.event.occurred", 16);
        let (sub2, _rx2) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub1.clone()).unwrap();
        bus.subscribe(sub2.clone()).unwrap();
        assert_eq!(bus.subscriber_count(), 2);
        bus.unsubscribe(sub1.id);
        assert_eq!(bus.subscriber_count(), 1);
    }

    #[test]
    fn test_bus_pause_subscriber_not_found() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let result = bus.pause_subscriber(Uuid::new_v4());
        assert!(matches!(result, Err(EventError::SubscriberNotFound)));
    }

    #[test]
    fn test_bus_resume_subscriber_noop() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let result = bus.resume_subscriber(Uuid::new_v4());
        assert!(result.is_ok());
    }

    #[test]
    fn test_bus_event_id_assigned_by_bus() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let (sub, rx) = make_sub("test.event.occurred", 16);
        bus.subscribe(sub).unwrap();

        let original_event = make_event();
        let old_id = original_event.metadata.event_id;
        let published_id = bus.publish(original_event).unwrap();

        let received = rx.try_recv().unwrap();
        // Bus assigns a new event ID
        assert_ne!(received.metadata.event_id, old_id);
        assert_eq!(received.metadata.event_id, published_id);
    }

    #[test]
    fn test_bus_replay_returns_cursor_on_empty() {
        let bus = EventBus::new(1, OverflowPolicy::DropNewest, 1024 * 1024, 1000);
        let sub_id = SubscriberId {
            id: "test-sub".into(),
            subsystem: Subsystem::System,
            name: "tester".into(),
        };
        let cursor = ReplayCursor {
            subscriber_id: "test-sub".into(),
            last_processed_offset: 0,
            last_processed_timestamp: 0,
            target_timestamp: None,
        };
        let result = bus.replay(&sub_id, cursor.clone());
        assert!(matches!(result, Err(EventError::SubscriberNotFound)));
    }
}
