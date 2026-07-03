use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum QueueError {
    #[error("Queue not found: {0}")]
    NotFound(String),

    #[error("Queue already exists: {0}")]
    AlreadyExists(String),

    #[error("Queue is full: {0}")]
    QueueFull(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Invalid receipt handle: {0}")]
    InvalidReceiptHandle(String),

    #[error("Message has expired: {0}")]
    MessageExpired(String),

    #[error("Message is currently in flight")]
    MessageInFlight,

    #[error("Queue is paused")]
    QueuePaused,

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Capacity exceeded: {0}")]
    CapacityExceeded(String),

    #[error("Shutting down")]
    ShuttingDown,
}

pub type Result<T> = std::result::Result<T, QueueError>;

impl From<nova_core::RuntimeError> for QueueError {
    fn from(e: nova_core::RuntimeError) -> Self {
        QueueError::Storage(e.to_string())
    }
}
