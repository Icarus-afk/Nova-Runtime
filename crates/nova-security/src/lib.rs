pub mod encryption;
pub mod secrets;
pub mod audit;
pub mod rate_limiter;
pub mod rng;
pub mod validator;

pub use encryption::*;
pub use secrets::*;
pub use audit::*;
pub use rate_limiter::*;
pub use rng::*;
pub use validator::*;

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Encryption error: {0}")]
    Encryption(String),
    #[error("Decryption error: {0}")]
    Decryption(String),
    #[error("Key not found: {0}")]
    KeyNotFound(String),
    #[error("Key expired: {0}")]
    KeyExpired(String),
    #[error("Secret not found: {0}")]
    SecretNotFound(String),
    #[error("Rate limited: retry after {0}ms")]
    RateLimited(u64),
    #[error("Validation failed: {0}")]
    Validation(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, SecurityError>;
