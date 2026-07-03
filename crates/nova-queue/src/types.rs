use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Message priority levels for priority queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MessagePriority {
    Critical = 0,
    High = 1,
    Normal = 2,
    Low = 3,
}

impl Default for MessagePriority {
    fn default() -> Self {
        MessagePriority::Normal
    }
}

impl MessagePriority {
    pub fn max_wait(&self) -> Duration {
        match self {
            MessagePriority::Critical => Duration::from_millis(50),
            MessagePriority::High => Duration::from_millis(200),
            MessagePriority::Normal => Duration::from_secs(1),
            MessagePriority::Low => Duration::from_secs(5),
        }
    }
}

/// Queue type determines ordering and behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QueueType {
    /// Strict FIFO ordering.
    Fifo,
    /// Ordered by priority, then FIFO within same priority.
    Priority,
    /// Messages become visible after a delay.
    Delayed,
    /// Dead-letter queue — holds messages that exceeded max receives.
    DeadLetter,
}

impl Default for QueueType {
    fn default() -> Self {
        QueueType::Fifo
    }
}

/// A message stored in a queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueMessage {
    pub id: Uuid,
    pub queue_name: String,
    pub body: Vec<u8>,
    pub priority: MessagePriority,
    pub delay_until: Option<i64>,
    pub enqueued_at: i64,
    pub visible_at: i64,
    pub visibility_timeout_secs: u32,
    pub receipt_handle: Option<String>,
    pub attempt_count: u32,
    pub max_receives: u32,
    pub ttl_secs: Option<u32>,
    pub expires_at: Option<i64>,
    pub deduplication_id: Option<String>,
    pub group_id: Option<String>,
    pub attributes: std::collections::HashMap<String, String>,
}

impl QueueMessage {
    pub fn new(queue_name: &str, body: Vec<u8>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        QueueMessage {
            id: Uuid::new_v4(),
            queue_name: queue_name.to_string(),
            body,
            priority: MessagePriority::Normal,
            delay_until: None,
            enqueued_at: now,
            visible_at: now,
            visibility_timeout_secs: 30,
            receipt_handle: None,
            attempt_count: 0,
            max_receives: 3,
            ttl_secs: Some(86400),
            expires_at: Some(now + 86400_000),
            deduplication_id: None,
            group_id: None,
            attributes: std::collections::HashMap::new(),
        }
    }

    pub fn is_visible(&self, now_ms: i64) -> bool {
        self.visible_at <= now_ms
            && self.expires_at.map(|e| e > now_ms).unwrap_or(true)
    }

    pub fn has_expired(&self, now_ms: i64) -> bool {
        self.expires_at.map(|e| e <= now_ms).unwrap_or(false)
    }

    pub fn mark_received(&mut self, now_ms: i64) {
        self.attempt_count += 1;
        self.receipt_handle = Some(Uuid::new_v4().to_string());
        let timeout_ms = (self.visibility_timeout_secs as i64) * 1000;
        self.visible_at = now_ms + timeout_ms;
    }
}

/// Consumer group configuration for a queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsumerGroup {
    pub name: String,
    pub queue_name: String,
    pub max_concurrent: u32,
    pub created_at: i64,
}

/// Queue configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndividualQueueConfig {
    pub name: String,
    pub queue_type: QueueType,
    pub max_size: usize,
    pub max_message_size: usize,
    pub default_visibility_timeout_secs: u32,
    pub message_ttl_secs: u32,
    pub delivery_delay_secs: u32,
    pub receive_message_wait_secs: u32,
    pub max_receive_count: u32,
    pub dlq_name: Option<String>,
    pub dlq_max_entries: usize,
    pub dlq_max_retries: u32,
    pub created_at: i64,
    pub updated_at: i64,
    pub paused: bool,
    pub consumer_groups: Vec<ConsumerGroup>,
}

impl IndividualQueueConfig {
    pub fn new(name: &str) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        IndividualQueueConfig {
            name: name.to_string(),
            queue_type: QueueType::Fifo,
            max_size: 10000,
            max_message_size: 262144,
            default_visibility_timeout_secs: 30,
            message_ttl_secs: 86400,
            delivery_delay_secs: 0,
            receive_message_wait_secs: 0,
            max_receive_count: 3,
            dlq_name: None,
            dlq_max_entries: 100000,
            dlq_max_retries: 3,
            created_at: now,
            updated_at: now,
            paused: false,
            consumer_groups: Vec::new(),
        }
    }
}

/// Statistics for a queue.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueStats {
    pub available_messages: u64,
    pub in_flight_messages: u64,
    pub delayed_messages: u64,
    pub total_messages: u64,
    pub dlq_messages: u64,
    pub messages_enqueued: u64,
    pub messages_dequeued: u64,
    pub messages_acked: u64,
    pub messages_expired: u64,
    pub messages_dlq: u64,
    pub bytes_total: u64,
}

/// Receipt handle returned on dequeue for ack/nack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReceiptHandle {
    pub message_id: Uuid,
    pub queue_name: String,
    pub receipt: String,
    pub received_at: i64,
}

/// Summary of a queue for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueSummary {
    pub name: String,
    pub queue_type: QueueType,
    pub available: u64,
    pub in_flight: u64,
    pub delayed: u64,
    pub total: u64,
    pub paused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_new() {
        let msg = QueueMessage::new("test-queue", b"hello".to_vec());
        assert_eq!(msg.queue_name, "test-queue");
        assert_eq!(msg.body, b"hello");
        assert_eq!(msg.priority, MessagePriority::Normal);
        assert_eq!(msg.attempt_count, 0);
        assert_eq!(msg.max_receives, 3);
        assert!(msg.receipt_handle.is_none());
    }

    #[test]
    fn test_message_visibility() {
        let msg = QueueMessage::new("q", vec![]);
        let now = msg.enqueued_at;
        assert!(msg.is_visible(now));
        assert!(!msg.has_expired(now));
    }

    #[test]
    fn test_message_expired() {
        let mut msg = QueueMessage::new("q", vec![]);
        msg.expires_at = Some(0);
        assert!(msg.has_expired(0));
        assert!(!msg.is_visible(0));
    }

    #[test]
    fn test_message_mark_received() {
        let mut msg = QueueMessage::new("q", vec![]);
        let now = msg.enqueued_at;
        msg.mark_received(now);
        assert_eq!(msg.attempt_count, 1);
        assert!(msg.receipt_handle.is_some());
        let expected_visible = now + (msg.visibility_timeout_secs as i64) * 1000;
        assert_eq!(msg.visible_at, expected_visible);
    }

    #[test]
    fn test_individual_queue_config_new() {
        let cfg = IndividualQueueConfig::new("my-queue");
        assert_eq!(cfg.name, "my-queue");
        assert_eq!(cfg.queue_type, QueueType::Fifo);
        assert_eq!(cfg.max_size, 10000);
        assert_eq!(cfg.default_visibility_timeout_secs, 30);
        assert!(!cfg.paused);
    }

    #[test]
    fn test_message_priority_default() {
        assert_eq!(MessagePriority::default(), MessagePriority::Normal);
    }

    #[test]
    fn test_message_priority_ordering() {
        assert!(MessagePriority::Critical < MessagePriority::High);
        assert!(MessagePriority::High < MessagePriority::Normal);
        assert!(MessagePriority::Normal < MessagePriority::Low);
    }

    #[test]
    fn test_queue_type_default() {
        assert_eq!(QueueType::default(), QueueType::Fifo);
    }

    #[test]
    fn test_queue_stats_defaults() {
        let stats = QueueStats::default();
        assert_eq!(stats.available_messages, 0);
        assert_eq!(stats.in_flight_messages, 0);
        assert_eq!(stats.total_messages, 0);
    }

    #[test]
    fn test_deduplication_id_none_by_default() {
        let msg = QueueMessage::new("q", vec![]);
        assert!(msg.deduplication_id.is_none());
    }

    #[test]
    fn test_group_id_none_by_default() {
        let msg = QueueMessage::new("q", vec![]);
        assert!(msg.group_id.is_none());
    }

    #[test]
    fn test_message_attributes_empty_by_default() {
        let msg = QueueMessage::new("q", vec![]);
        assert!(msg.attributes.is_empty());
    }

    #[test]
    fn test_consumer_groups_empty_by_default() {
        let cfg = IndividualQueueConfig::new("q");
        assert!(cfg.consumer_groups.is_empty());
    }

    #[test]
    fn test_dlq_config_defaults() {
        let cfg = IndividualQueueConfig::new("q");
        assert!(cfg.dlq_name.is_none());
        assert_eq!(cfg.dlq_max_entries, 100000);
        assert_eq!(cfg.dlq_max_retries, 3);
    }

    #[test]
    fn test_message_priority_max_wait() {
        assert_eq!(MessagePriority::Critical.max_wait(), Duration::from_millis(50));
        assert_eq!(MessagePriority::High.max_wait(), Duration::from_millis(200));
        assert_eq!(MessagePriority::Normal.max_wait(), Duration::from_secs(1));
        assert_eq!(MessagePriority::Low.max_wait(), Duration::from_secs(5));
    }

    #[test]
    fn test_receipt_handle_construction() {
        let id = Uuid::new_v4();
        let handle = ReceiptHandle {
            message_id: id,
            queue_name: "q".into(),
            receipt: "receipt_abc".into(),
            received_at: 1000,
        };
        assert_eq!(handle.message_id, id);
        assert_eq!(handle.queue_name, "q");
    }
}
