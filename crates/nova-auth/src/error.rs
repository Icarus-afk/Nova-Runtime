use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Authorization denied: {0}")]
    AuthorizationDenied(String),

    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),

    #[error("Session expired: {0}")]
    SessionExpired(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Token invalid: {0}")]
    TokenInvalid(String),

    #[error("Token expired: {0}")]
    TokenExpired(String),

    #[error("MFA required")]
    MfaRequired,

    #[error("MFA failed: {0}")]
    MfaFailed(String),

    #[error("Account locked: {0}")]
    AccountLocked(String),

    #[error("Rate limited: retry after {0}ms")]
    RateLimited(u64),

    #[error("Password policy violated: {0}")]
    PasswordPolicyViolation(String),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Security error: {0}")]
    Security(String),
}

pub type Result<T> = std::result::Result<T, AuthError>;

impl From<nova_core::RuntimeError> for AuthError {
    fn from(e: nova_core::RuntimeError) -> Self {
        AuthError::Storage(e.to_string())
    }
}

impl From<nova_security::SecurityError> for AuthError {
    fn from(e: nova_security::SecurityError) -> Self {
        AuthError::Security(e.to_string())
    }
}
