use std::fmt;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Out of memory: {0}")]
    OutOfMemory(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Checksum mismatch: expected {expected:#x}, got {actual:#x}")]
    ChecksumMismatch { expected: u32, actual: u32 },

    #[error("Corrupt data: {0}")]
    CorruptData(String),

    #[error("Busy: {0}")]
    Busy(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Transaction conflict: {0}")]
    TransactionConflict(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Internal: {0}")]
    Internal(String),

    #[error("Shutting down")]
    ShuttingDown,

    #[error("Transaction error: {0}")]
    TransactionError(String),

    #[error("Deadlock detected: {0}")]
    DeadlockDetected(String),

    #[error("Capacity exceeded: {0}")]
    Capacity(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;

pub trait IntoError {
    fn into_error(self) -> RuntimeError;
}

impl IntoError for RuntimeError {
    fn into_error(self) -> RuntimeError {
        self
    }
}

impl From<std::io::Error> for RuntimeError {
    fn from(e: std::io::Error) -> Self {
        RuntimeError::Io(e.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Debug => write!(f, "DEBUG"),
            Severity::Info => write!(f, "INFO"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Error => write!(f, "ERROR"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}
