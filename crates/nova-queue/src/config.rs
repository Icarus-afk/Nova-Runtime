use serde::{Deserialize, Serialize};

fn default_max_queues() -> usize { 1000 }
fn default_max_messages_per_queue() -> usize { 10000 }
fn default_max_message_size() -> usize { 262144 }
fn default_default_visibility_timeout_secs() -> u32 { 30 }
fn default_message_ttl_secs() -> u32 { 86400 }
fn default_max_receive_count() -> u32 { 3 }
fn default_scanner_interval_ms() -> u64 { 1000 }
fn default_backpressure_threshold() -> f64 { 0.9 }
fn default_dlq_max_entries() -> usize { 100000 }
fn default_dlq_max_retries() -> u32 { 3 }
fn default_enable_dlq() -> bool { true }
fn default_enable_scanners() -> bool { true }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueueConfig {
    #[serde(default = "default_max_queues")]
    pub max_queues: usize,
    #[serde(default = "default_max_messages_per_queue")]
    pub max_messages_per_queue: usize,
    #[serde(default = "default_max_message_size")]
    pub max_message_size: usize,
    #[serde(default = "default_default_visibility_timeout_secs")]
    pub default_visibility_timeout_secs: u32,
    #[serde(default = "default_message_ttl_secs")]
    pub message_ttl_secs: u32,
    #[serde(default = "default_max_receive_count")]
    pub max_receive_count: u32,
    #[serde(default = "default_scanner_interval_ms")]
    pub scanner_interval_ms: u64,
    #[serde(default = "default_backpressure_threshold")]
    pub backpressure_threshold: f64,
    #[serde(default = "default_dlq_max_entries")]
    pub dlq_max_entries: usize,
    #[serde(default = "default_dlq_max_retries")]
    pub dlq_max_retries: u32,
    #[serde(default = "default_enable_dlq")]
    pub enable_dlq: bool,
    #[serde(default = "default_enable_scanners")]
    pub enable_scanners: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queues: default_max_queues(),
            max_messages_per_queue: default_max_messages_per_queue(),
            max_message_size: default_max_message_size(),
            default_visibility_timeout_secs: default_default_visibility_timeout_secs(),
            message_ttl_secs: default_message_ttl_secs(),
            max_receive_count: default_max_receive_count(),
            scanner_interval_ms: default_scanner_interval_ms(),
            backpressure_threshold: default_backpressure_threshold(),
            dlq_max_entries: default_dlq_max_entries(),
            dlq_max_retries: default_dlq_max_retries(),
            enable_dlq: default_enable_dlq(),
            enable_scanners: default_enable_scanners(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_config_defaults() {
        let c = QueueConfig::default();
        assert_eq!(c.max_queues, 1000);
        assert_eq!(c.max_messages_per_queue, 10000);
        assert_eq!(c.max_message_size, 262144);
        assert_eq!(c.default_visibility_timeout_secs, 30);
        assert_eq!(c.message_ttl_secs, 86400);
        assert_eq!(c.max_receive_count, 3);
        assert_eq!(c.scanner_interval_ms, 1000);
        assert_eq!(c.backpressure_threshold, 0.9);
        assert_eq!(c.dlq_max_entries, 100000);
        assert_eq!(c.dlq_max_retries, 3);
        assert!(c.enable_dlq);
        assert!(c.enable_scanners);
    }

    #[test]
    fn test_queue_config_serde_roundtrip() {
        let c = QueueConfig::default();
        let json = serde_json::to_string(&c).unwrap();
        let deserialized: QueueConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(c, deserialized);
    }
}
