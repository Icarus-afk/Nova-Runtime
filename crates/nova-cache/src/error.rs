use thiserror::Error;

#[derive(Error, Debug)]
pub enum CacheError {
    #[error("cache key not found: {0}")]
    NotFound(String),

    #[error("cache capacity exceeded")]
    CapacityExceeded,

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, CacheError>;
