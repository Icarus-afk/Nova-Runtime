use crate::error::Result;
use crate::types::*;
use async_trait::async_trait;
use nova_core::StorageEngine;
use std::sync::Arc;

/// Abstract queue backend trait.
/// All implementations must be Send + Sync.
#[async_trait]
pub trait QueueBackend: Send + Sync {
    /// Create a new queue with the given config.
    async fn create_queue(&self, config: IndividualQueueConfig) -> Result<()>;

    /// Delete a queue and all its messages.
    async fn delete_queue(&self, name: &str) -> Result<()>;

    /// Get queue configuration.
    async fn get_queue(&self, name: &str) -> Result<IndividualQueueConfig>;

    /// Update queue configuration.
    async fn update_queue(&self, config: IndividualQueueConfig) -> Result<()>;

    /// List all queues with summaries.
    async fn list_queues(&self) -> Result<Vec<QueueSummary>>;

    /// Enqueue a single message.
    async fn enqueue(&self, message: QueueMessage) -> Result<()>;

    /// Enqueue a batch of messages.
    async fn enqueue_batch(&self, messages: Vec<QueueMessage>) -> Result<Vec<Result<()>>>;

    /// Dequeue up to `max_messages` visible messages.
    async fn dequeue(&self, queue_name: &str, max_messages: u32) -> Result<Vec<QueueMessage>>;

    /// Acknowledge a message by receipt handle (removes it).
    async fn ack(&self, queue_name: &str, receipt_handle: &str) -> Result<()>;

    /// Negative-acknowledge: make message visible again immediately.
    async fn nack(&self, queue_name: &str, receipt_handle: &str) -> Result<()>;

    /// Peek at messages without changing visibility.
    async fn peek(&self, queue_name: &str, max_messages: u32) -> Result<Vec<QueueMessage>>;

    /// Purge all messages from a queue.
    async fn purge(&self, queue_name: &str) -> Result<()>;

    /// Get queue statistics.
    async fn stats(&self, queue_name: &str) -> Result<QueueStats>;

    /// Pause a queue (prevents dequeue).
    async fn pause(&self, queue_name: &str) -> Result<()>;

    /// Resume a paused queue.
    async fn resume(&self, queue_name: &str) -> Result<()>;

    /// Move expired/in-flight messages back to available state.
    async fn release_expired_messages(&self, queue_name: &str, now_ms: i64) -> Result<u64>;

    /// Move messages that exceeded max_receives to DLQ.
    async fn move_to_dlq(&self, queue_name: &str, now_ms: i64) -> Result<u64>;

    /// Delete expired messages.
    async fn purge_expired_messages(&self, queue_name: &str, now_ms: i64) -> Result<u64>;
}

/// StorageEngine-backed queue backend implementation.
pub struct StorageQueueBackend {
    store: Arc<dyn StorageEngine>,
}

impl StorageQueueBackend {
    pub fn new(store: Arc<dyn StorageEngine>) -> Self {
        Self { store }
    }

    /// Build storage key prefixes for queue data.
    fn queue_meta_key(name: &str) -> nova_core::Key {
        nova_core::Key::from(format!("queue:meta:{}", name).into_bytes())
    }

    fn queue_msg_key(queue_name: &str, msg_id: &uuid::Uuid) -> nova_core::Key {
        nova_core::Key::from(format!("queue:msg:{}:{}", queue_name, msg_id).into_bytes())
    }

    fn queue_available_key(queue_name: &str, msg_id: &uuid::Uuid) -> nova_core::Key {
        nova_core::Key::from(format!("queue:available:{}:{}", queue_name, msg_id).into_bytes())
    }

    fn queue_inflight_key(queue_name: &str, msg_id: &uuid::Uuid) -> nova_core::Key {
        nova_core::Key::from(format!("queue:inflight:{}:{}", queue_name, msg_id).into_bytes())
    }

    fn queue_delayed_key(queue_name: &str, msg_id: &uuid::Uuid) -> nova_core::Key {
        nova_core::Key::from(format!("queue:delayed:{}:{}", queue_name, msg_id).into_bytes())
    }

    fn queue_dlq_key(queue_name: &str, msg_id: &uuid::Uuid) -> nova_core::Key {
        nova_core::Key::from(format!("queue:dlq:{}:{}", queue_name, msg_id).into_bytes())
    }

    fn queue_stats_key(name: &str) -> nova_core::Key {
        nova_core::Key::from(format!("queue:stats:{}", name).into_bytes())
    }

    #[allow(dead_code)]
    fn queues_index_key() -> nova_core::Key {
        nova_core::Key::from("queue:index")
    }
}

#[async_trait]
impl QueueBackend for StorageQueueBackend {
    async fn create_queue(&self, config: IndividualQueueConfig) -> Result<()> {
        let meta_key = Self::queue_meta_key(&config.name);
        let data = serde_json::to_vec(&config)
            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;

        // Check if already exists
        if self.store.get(&meta_key)?.is_some() {
            return Err(crate::error::QueueError::AlreadyExists(config.name));
        }

        self.store.set(&meta_key, nova_core::Value::new(data))?;
        Ok(())
    }

    async fn delete_queue(&self, name: &str) -> Result<()> {
        let meta_key = Self::queue_meta_key(name);
        if self.store.get(&meta_key)?.is_none() {
            return Err(crate::error::QueueError::NotFound(name.to_string()));
        }
        self.store.delete(&meta_key)?;

        // Purge all messages by scanning
        let msg_prefix = nova_core::Key::from(format!("queue:msg:{}:", name).into_bytes());
        let end = {
            let mut b = msg_prefix.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(msg_prefix..end)?;
        let ops: Vec<_> = entries
            .into_iter()
            .map(|(k, _)| nova_core::WriteOperation::Delete { key: k })
            .collect();
        if !ops.is_empty() {
            self.store.batch(ops)?;
        }

        // Clean up indexes
        for prefix in &["queue:available:", "queue:inflight:", "queue:delayed:", "queue:dlq:"] {
            let start = nova_core::Key::from(format!("{}{}:", prefix, name).into_bytes());
            let end = {
                let mut b = start.as_bytes().to_vec();
                b.push(0xFFu8);
                nova_core::Key::from(b)
            };
            let entries = self.store.scan(start..end)?;
            let ops: Vec<_> = entries
                .into_iter()
                .map(|(k, _)| nova_core::WriteOperation::Delete { key: k })
                .collect();
            if !ops.is_empty() {
                self.store.batch(ops)?;
            }
        }

        self.store.delete(&Self::queue_stats_key(name))?;
        Ok(())
    }

    async fn get_queue(&self, name: &str) -> Result<IndividualQueueConfig> {
        let meta_key = Self::queue_meta_key(name);
        let data = self.store.get(&meta_key)?
            .ok_or_else(|| crate::error::QueueError::NotFound(name.to_string()))?;
        serde_json::from_slice(data.as_bytes())
            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))
    }

    async fn update_queue(&self, config: IndividualQueueConfig) -> Result<()> {
        let meta_key = Self::queue_meta_key(&config.name);
        if self.store.get(&meta_key)?.is_none() {
            return Err(crate::error::QueueError::NotFound(config.name));
        }
        let data = serde_json::to_vec(&config)
            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;
        self.store.set(&meta_key, nova_core::Value::new(data))?;
        Ok(())
    }

    async fn list_queues(&self) -> Result<Vec<QueueSummary>> {
        let start = nova_core::Key::from("queue:meta:");
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        let mut summaries = Vec::with_capacity(entries.len());
        for (_, value) in entries {
            if let Ok(cfg) = serde_json::from_slice::<IndividualQueueConfig>(value.as_bytes()) {
                let stats = self.stats(&cfg.name).await.unwrap_or_default();
                summaries.push(QueueSummary {
                    name: cfg.name,
                    queue_type: cfg.queue_type,
                    available: stats.available_messages,
                    in_flight: stats.in_flight_messages,
                    delayed: stats.delayed_messages,
                    total: stats.total_messages,
                    paused: cfg.paused,
                });
            }
        }
        Ok(summaries)
    }

    async fn enqueue(&self, message: QueueMessage) -> Result<()> {
        let config = self.get_queue(&message.queue_name).await?;
        if config.paused {
            return Err(crate::error::QueueError::QueuePaused);
        }

        let msg_key = Self::queue_msg_key(&message.queue_name, &message.id);
        let msg_data = serde_json::to_vec(&message)
            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;

        // Write the message data
        self.store.set(&msg_key, nova_core::Value::new(msg_data))?;

        // Index by visibility
        let now_ms = chrono::Utc::now().timestamp_millis();
        if let Some(delay_until) = message.delay_until {
            if delay_until > now_ms {
                let delayed_key = Self::queue_delayed_key(&message.queue_name, &message.id);
                self.store.set(&delayed_key, nova_core::Value::new(vec![]))?;
                return Ok(());
            }
        }

        let avail_key = Self::queue_available_key(&message.queue_name, &message.id);
        self.store.set(&avail_key, nova_core::Value::new(vec![]))?;
        Ok(())
    }

    async fn enqueue_batch(&self, messages: Vec<QueueMessage>) -> Result<Vec<Result<()>>> {
        let mut results = Vec::with_capacity(messages.len());
        for msg in messages {
            results.push(self.enqueue(msg).await);
        }
        Ok(results)
    }

    async fn dequeue(&self, queue_name: &str, max_messages: u32) -> Result<Vec<QueueMessage>> {
        let config = self.get_queue(queue_name).await?;
        if config.paused {
            return Err(crate::error::QueueError::QueuePaused);
        }

        let now_ms = chrono::Utc::now().timestamp_millis();
        let start = nova_core::Key::from(format!("queue:available:{}:", queue_name).into_bytes());
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };

        let entries = self.store.scan(start..end)?;
        let mut messages = Vec::new();

        for (key, _) in entries.iter().take(max_messages as usize) {
            // Extract message ID from the key
            let key_str = String::from_utf8_lossy(key.as_bytes());
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let msg_id_str = parts[3..].join(":");
            let msg_id = match uuid::Uuid::parse_str(&msg_id_str) {
                Ok(id) => id,
                Err(_) => continue,
            };

            let msg_key = Self::queue_msg_key(queue_name, &msg_id);
            if let Some(msg_data) = self.store.get(&msg_key)? {
                if let Ok(mut msg) = serde_json::from_slice::<QueueMessage>(msg_data.as_bytes()) {
                    if msg.is_visible(now_ms) && !msg.has_expired(now_ms) {
                        msg.mark_received(now_ms);
                        // Update message data
                        let updated = serde_json::to_vec(&msg)
                            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;
                        self.store.set(&msg_key, nova_core::Value::new(updated))?;

                        // Move from available to in-flight
                        self.store.delete(&key.clone())?;
                        let inflight_key = Self::queue_inflight_key(queue_name, &msg_id);
                        self.store.set(&inflight_key, nova_core::Value::new(vec![]))?;

                        messages.push(msg);
                    }
                }
            }
        }

        Ok(messages)
    }

    async fn ack(&self, queue_name: &str, receipt_handle: &str) -> Result<()> {
        // Parse the receipt handle to find the message ID
        // Receipt handles are UUIDs stored on the message
        let msg_id = uuid::Uuid::parse_str(receipt_handle)
            .map_err(|_| crate::error::QueueError::InvalidReceiptHandle(receipt_handle.to_string()))?;

        let msg_key = Self::queue_msg_key(queue_name, &msg_id);
        if self.store.get(&msg_key)?.is_none() {
            return Err(crate::error::QueueError::MessageNotFound(msg_id.to_string()));
        }

        // Delete message data and inflight index
        self.store.delete(&msg_key)?;
        let inflight_key = Self::queue_inflight_key(queue_name, &msg_id);
        self.store.delete(&inflight_key)?;
        Ok(())
    }

    async fn nack(&self, queue_name: &str, receipt_handle: &str) -> Result<()> {
        let msg_id = uuid::Uuid::parse_str(receipt_handle)
            .map_err(|_| crate::error::QueueError::InvalidReceiptHandle(receipt_handle.to_string()))?;

        let msg_key = Self::queue_msg_key(queue_name, &msg_id);
        let msg_data = self.store.get(&msg_key)?
            .ok_or_else(|| crate::error::QueueError::MessageNotFound(msg_id.to_string()))?;

        let mut msg: QueueMessage = serde_json::from_slice(msg_data.as_bytes())
            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;

        // Reset visibility to now
        let now_ms = chrono::Utc::now().timestamp_millis();
        msg.visible_at = now_ms;
        msg.receipt_handle = None;

        let updated = serde_json::to_vec(&msg)
            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;
        self.store.set(&msg_key, nova_core::Value::new(updated))?;

        // Move from inflight to available
        let inflight_key = Self::queue_inflight_key(queue_name, &msg_id);
        self.store.delete(&inflight_key)?;
        let avail_key = Self::queue_available_key(queue_name, &msg_id);
        self.store.set(&avail_key, nova_core::Value::new(vec![]))?;

        Ok(())
    }

    async fn peek(&self, queue_name: &str, max_messages: u32) -> Result<Vec<QueueMessage>> {
        let start = nova_core::Key::from(format!("queue:msg:{}:", queue_name).into_bytes());
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        let mut messages = Vec::new();
        for (_, value) in entries.iter().take(max_messages as usize) {
            if let Ok(msg) = serde_json::from_slice::<QueueMessage>(value.as_bytes()) {
                messages.push(msg);
            }
        }
        Ok(messages)
    }

    async fn purge(&self, queue_name: &str) -> Result<()> {
        let msg_prefix = nova_core::Key::from(format!("queue:msg:{}:", queue_name).into_bytes());
        let end = {
            let mut b = msg_prefix.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(msg_prefix..end)?;
        let ops: Vec<_> = entries
            .into_iter()
            .map(|(k, _)| nova_core::WriteOperation::Delete { key: k })
            .collect();
        if !ops.is_empty() {
            self.store.batch(ops)?;
        }

        // Also clean all indexes
        for prefix in &["queue:available:", "queue:inflight:", "queue:delayed:", "queue:dlq:"] {
            let start = nova_core::Key::from(format!("{}{}:", prefix, queue_name).into_bytes());
            let end = {
                let mut b = start.as_bytes().to_vec();
                b.push(0xFFu8);
                nova_core::Key::from(b)
            };
            let entries = self.store.scan(start..end)?;
            let ops: Vec<_> = entries
                .into_iter()
                .map(|(k, _)| nova_core::WriteOperation::Delete { key: k })
                .collect();
            if !ops.is_empty() {
                self.store.batch(ops)?;
            }
        }

        Ok(())
    }

    async fn stats(&self, queue_name: &str) -> Result<QueueStats> {
        let now_ms = chrono::Utc::now().timestamp_millis();

        // Count available messages
        let avail_start = nova_core::Key::from(format!("queue:available:{}:", queue_name).into_bytes());
        let avail_end = {
            let mut b = avail_start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let available = self.store.scan(avail_start..avail_end)?.len() as u64;

        // Count in-flight messages
        let inflight_start = nova_core::Key::from(format!("queue:inflight:{}:", queue_name).into_bytes());
        let inflight_end = {
            let mut b = inflight_start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let in_flight = self.store.scan(inflight_start..inflight_end)?.len() as u64;

        // Count delayed messages
        let delayed_start = nova_core::Key::from(format!("queue:delayed:{}:", queue_name).into_bytes());
        let delayed_end = {
            let mut b = delayed_start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let delayed = self.store.scan(delayed_start..delayed_end)?.len() as u64;

        // Count DLQ messages
        let dlq_start = nova_core::Key::from(format!("queue:dlq:{}:", queue_name).into_bytes());
        let dlq_end = {
            let mut b = dlq_start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let dlq_messages = self.store.scan(dlq_start..dlq_end)?.len() as u64;

        // Total messages
        let msg_start = nova_core::Key::from(format!("queue:msg:{}:", queue_name).into_bytes());
        let msg_end = {
            let mut b = msg_start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let total = self.store.scan(msg_start..msg_end)?.len() as u64;

        Ok(QueueStats {
            available_messages: available,
            in_flight_messages: in_flight,
            delayed_messages: delayed,
            total_messages: total,
            dlq_messages,
            ..QueueStats::default()
        })
    }

    async fn pause(&self, queue_name: &str) -> Result<()> {
        let mut config = self.get_queue(queue_name).await?;
        config.paused = true;
        config.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_queue(config).await
    }

    async fn resume(&self, queue_name: &str) -> Result<()> {
        let mut config = self.get_queue(queue_name).await?;
        config.paused = false;
        config.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_queue(config).await
    }

    async fn release_expired_messages(&self, queue_name: &str, now_ms: i64) -> Result<u64> {
        let start = nova_core::Key::from(format!("queue:inflight:{}:", queue_name).into_bytes());
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        let mut released = 0u64;

        for (key, _) in &entries {
            let key_str = String::from_utf8_lossy(key.as_bytes());
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let msg_id_str = parts[3..].join(":");
            let msg_id = match uuid::Uuid::parse_str(&msg_id_str) {
                Ok(id) => id,
                Err(_) => continue,
            };

            let msg_key = Self::queue_msg_key(queue_name, &msg_id);
            if let Some(msg_data) = self.store.get(&msg_key)? {
                if let Ok(msg) = serde_json::from_slice::<QueueMessage>(msg_data.as_bytes()) {
                    if msg.visible_at <= now_ms && !msg.has_expired(now_ms) {
                        // Move back to available
                        self.store.delete(&key.clone())?;
                        let avail_key = Self::queue_available_key(queue_name, &msg_id);
                        self.store.set(&avail_key, nova_core::Value::new(vec![]))?;
                        released += 1;
                    }
                }
            }
        }

        Ok(released)
    }

    async fn move_to_dlq(&self, queue_name: &str, now_ms: i64) -> Result<u64> {
        let config = self.get_queue(queue_name).await?;
        let dlq_name = match &config.dlq_name {
            Some(name) => name.clone(),
            None => return Ok(0),
        };

        let start = nova_core::Key::from(format!("queue:inflight:{}:", queue_name).into_bytes());
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        let mut moved = 0u64;

        for (key, _) in &entries {
            let key_str = String::from_utf8_lossy(key.as_bytes());
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() < 4 {
                continue;
            }
            let msg_id_str = parts[3..].join(":");
            let msg_id = match uuid::Uuid::parse_str(&msg_id_str) {
                Ok(id) => id,
                Err(_) => continue,
            };

            let msg_key = Self::queue_msg_key(queue_name, &msg_id);
            if let Some(msg_data) = self.store.get(&msg_key)? {
                if let Ok(mut msg) = serde_json::from_slice::<QueueMessage>(msg_data.as_bytes()) {
                    if msg.attempt_count >= msg.max_receives && msg.visible_at <= now_ms {
                        // Move to DLQ
                        msg.queue_name = dlq_name.clone();
                        msg.receipt_handle = None;
                        msg.attempt_count = 0;
                        msg.enqueued_at = now_ms;
                        msg.visible_at = now_ms;

                        let dlq_key = Self::queue_dlq_key(&dlq_name, &msg_id);
                        let dlq_msg_key = Self::queue_msg_key(&dlq_name, &msg_id);
                        let dlq_data = serde_json::to_vec(&msg)
                            .map_err(|e| crate::error::QueueError::Internal(e.to_string()))?;

                        self.store.set(&dlq_msg_key, nova_core::Value::new(dlq_data))?;
                        self.store.set(&dlq_key, nova_core::Value::new(vec![]))?;

                        // Remove from source
                        self.store.delete(&msg_key)?;
                        self.store.delete(&key.clone())?;
                        moved += 1;
                    }
                }
            }
        }

        Ok(moved)
    }

    async fn purge_expired_messages(&self, queue_name: &str, now_ms: i64) -> Result<u64> {
        let start = nova_core::Key::from(format!("queue:msg:{}:", queue_name).into_bytes());
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        let mut purged = 0u64;
        let mut ops = Vec::new();

        for (key, value) in &entries {
            if let Ok(msg) = serde_json::from_slice::<QueueMessage>(value.as_bytes()) {
                if msg.has_expired(now_ms) {
                    ops.push(nova_core::WriteOperation::Delete { key: key.clone() });

                    // Also remove from any index
                    let id = msg.id;
                    let prefixes = [
                        format!("queue:available:{}:", queue_name),
                        format!("queue:inflight:{}:", queue_name),
                        format!("queue:delayed:{}:", queue_name),
                        format!("queue:dlq:{}:", queue_name),
                    ];
                    for prefix in &prefixes {
                        let idx_key = nova_core::Key::from(format!("{}{}", prefix, id).into_bytes());
                        ops.push(nova_core::WriteOperation::Delete { key: idx_key });
                    }
                    purged += 1;
                }
            }
        }

        if !ops.is_empty() {
            self.store.batch(ops)?;
        }

        Ok(purged)
    }
}
