pub mod event;
pub mod subscription;
pub mod trie;
pub mod bus;
pub mod dead_letter;
pub mod middleware;
pub mod store;

pub use event::*;
pub use subscription::*;
pub use trie::*;
pub use bus::*;
pub use dead_letter::*;
pub use middleware::*;
pub use store::{StoredEvent, ReplayCursor, EventStore};

use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum EventError {
    #[error("Payload too large: {size} > {max}")]
    PayloadTooLarge { size: u64, max: u64 },
    #[error("Invalid event type: {0}")]
    InvalidEventType(String),
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),
    #[error("Validation error on field '{field}': {reason}")]
    ValidationError { field: String, reason: String },
    #[error("Bus full: {0}")]
    BusFull(String),
    #[error("Subscriber not found")]
    SubscriberNotFound,
    #[error("System is shutting down")]
    ShuttingDown,
    #[error("Internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, EventError>;
