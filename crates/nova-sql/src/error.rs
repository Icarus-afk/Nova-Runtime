use thiserror::Error;

#[derive(Error, Debug)]
pub enum SQLError {
    #[error("syntax error: {0}")]
    Syntax(String),

    #[error("table not found: {0}")]
    TableNotFound(String),

    #[error("column not found: {0}")]
    ColumnNotFound(String),

    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, SQLError>;
