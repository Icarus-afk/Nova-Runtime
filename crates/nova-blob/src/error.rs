use thiserror::Error;

#[derive(Error, Debug)]
pub enum BlobError {
    #[error("blob not found: {0}")]
    NotFound(String),
    #[error("namespace not found: {0}")]
    NamespaceNotFound(String),
    #[error("upload not found: {0}")]
    UploadNotFound(String),
    #[error("checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("quota exceeded: {0}")]
    QuotaExceeded(String),
    #[error("invalid range: {0}")]
    InvalidRange(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for BlobError {
    fn from(e: std::io::Error) -> Self {
        BlobError::Internal(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, BlobError>;
