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

#[cfg(test)]
mod tests {
    use super::*;

    // --- RuntimeError: string variants ---

    #[test]
    fn test_error_not_found() {
        let e = RuntimeError::NotFound("table".into());
        assert_eq!(e.to_string(), "Not found: table");
        assert!(matches!(e, RuntimeError::NotFound(_)));
    }

    #[test]
    fn test_error_already_exists() {
        let e = RuntimeError::AlreadyExists("key".into());
        assert_eq!(e.to_string(), "Already exists: key");
    }

    #[test]
    fn test_error_invalid_argument() {
        let e = RuntimeError::InvalidArgument("bad param".into());
        assert_eq!(e.to_string(), "Invalid argument: bad param");
    }

    #[test]
    fn test_error_permission_denied() {
        let e = RuntimeError::PermissionDenied("read only".into());
        assert_eq!(e.to_string(), "Permission denied: read only");
    }

    #[test]
    fn test_error_out_of_memory() {
        let e = RuntimeError::OutOfMemory("cannot allocate".into());
        assert_eq!(e.to_string(), "Out of memory: cannot allocate");
    }

    #[test]
    fn test_error_storage() {
        let e = RuntimeError::Storage("disk failure".into());
        assert_eq!(e.to_string(), "Storage error: disk failure");
    }

    #[test]
    fn test_error_io() {
        let e = RuntimeError::Io("permission denied".into());
        assert_eq!(e.to_string(), "IO error: permission denied");
    }

    #[test]
    fn test_error_checksum_mismatch() {
        let e = RuntimeError::ChecksumMismatch {
            expected: 0xDEAD,
            actual: 0xBEEF,
        };
        let s = e.to_string();
        assert!(s.contains("Checksum mismatch"));
        assert!(s.contains("0xdead"));
        assert!(s.contains("0xbeef"));
    }

    #[test]
    fn test_error_corrupt_data() {
        let e = RuntimeError::CorruptData("bad header".into());
        assert_eq!(e.to_string(), "Corrupt data: bad header");
    }

    #[test]
    fn test_error_busy() {
        let e = RuntimeError::Busy("resource locked".into());
        assert_eq!(e.to_string(), "Busy: resource locked");
    }

    #[test]
    fn test_error_timeout() {
        let e = RuntimeError::Timeout("operation timed out".into());
        assert_eq!(e.to_string(), "Timeout: operation timed out");
    }

    #[test]
    fn test_error_transaction_conflict() {
        let e = RuntimeError::TransactionConflict("write conflict".into());
        assert_eq!(e.to_string(), "Transaction conflict: write conflict");
    }

    #[test]
    fn test_error_protocol() {
        let e = RuntimeError::Protocol("unexpected message".into());
        assert_eq!(e.to_string(), "Protocol error: unexpected message");
    }

    #[test]
    fn test_error_serialization() {
        let e = RuntimeError::Serialization("encode failed".into());
        assert_eq!(e.to_string(), "Serialization error: encode failed");
    }

    #[test]
    fn test_error_deserialization() {
        let e = RuntimeError::Deserialization("decode failed".into());
        assert_eq!(
            e.to_string(),
            "Deserialization error: decode failed"
        );
    }

    #[test]
    fn test_error_internal() {
        let e = RuntimeError::Internal("unreachable".into());
        assert_eq!(e.to_string(), "Internal: unreachable");
    }

    #[test]
    fn test_error_shutting_down() {
        let e = RuntimeError::ShuttingDown;
        assert_eq!(e.to_string(), "Shutting down");
    }

    #[test]
    fn test_error_transaction_error() {
        let e = RuntimeError::TransactionError("aborted".into());
        assert_eq!(e.to_string(), "Transaction error: aborted");
    }

    #[test]
    fn test_error_deadlock_detected() {
        let e = RuntimeError::DeadlockDetected("cycle".into());
        assert_eq!(e.to_string(), "Deadlock detected: cycle");
    }

    #[test]
    fn test_error_capacity() {
        let e = RuntimeError::Capacity("too many connections".into());
        assert_eq!(e.to_string(), "Capacity exceeded: too many connections");
    }

    // --- RuntimeError: Clone, Debug, PartialEq ---

    #[test]
    fn test_error_clone() {
        let e = RuntimeError::NotFound("original".into());
        let cloned = e.clone();
        assert_eq!(e, cloned);
    }

    #[test]
    fn test_error_equality_same() {
        let a = RuntimeError::InvalidArgument("x".into());
        let b = RuntimeError::InvalidArgument("x".into());
        assert_eq!(a, b);
    }

    #[test]
    fn test_error_equality_different() {
        let a = RuntimeError::NotFound("x".into());
        let b = RuntimeError::AlreadyExists("x".into());
        assert_ne!(a, b);
    }

    #[test]
    fn test_error_debug_contains_variant() {
        let e = RuntimeError::Internal("msg".into());
        let d = format!("{e:?}");
        assert!(d.contains("Internal"));
        assert!(d.contains("msg"));
    }

    // --- From impls ---

    #[test]
    fn test_from_io_error() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: RuntimeError = io.into();
        assert!(matches!(err, RuntimeError::Io(_)));
        assert_eq!(err.to_string(), "IO error: file missing");
    }

    // --- Result alias ---

    #[test]
    fn test_result_ok() {
        let r: Result<i32> = Ok(42);
        assert!(r.is_ok());
        assert_eq!(r.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let r: Result<i32> = Err(RuntimeError::NotFound("x".into()));
        assert!(r.is_err());
        assert_eq!(
            r.unwrap_err().to_string(),
            "Not found: x"
        );
    }

    // --- IntoError ---

    #[test]
    fn test_into_error_for_runtime_error() {
        let e = RuntimeError::Internal("test".into());
        let converted: RuntimeError = e.into_error();
        assert_eq!(converted.to_string(), "Internal: test");
    }

    // --- Severity ---

    #[test]
    fn test_severity_display() {
        assert_eq!(format!("{}", Severity::Debug), "DEBUG");
        assert_eq!(format!("{}", Severity::Info), "INFO");
        assert_eq!(format!("{}", Severity::Warning), "WARNING");
        assert_eq!(format!("{}", Severity::Error), "ERROR");
        assert_eq!(format!("{}", Severity::Critical), "CRITICAL");
    }

    #[test]
    fn test_severity_debug() {
        assert_eq!(format!("{:?}", Severity::Debug), "Debug");
        assert_eq!(format!("{:?}", Severity::Info), "Info");
        assert_eq!(format!("{:?}", Severity::Warning), "Warning");
        assert_eq!(format!("{:?}", Severity::Error), "Error");
        assert_eq!(format!("{:?}", Severity::Critical), "Critical");
    }

    #[test]
    fn test_severity_equality() {
        assert_eq!(Severity::Error, Severity::Error);
        assert_ne!(Severity::Debug, Severity::Critical);
    }

    #[test]
    fn test_severity_clone() {
        let s = Severity::Warning;
        assert_eq!(s.clone(), s);
    }

    #[test]
    fn test_severity_copy() {
        let a = Severity::Info;
        let b = a;
        assert_eq!(a, b);
    }
}
