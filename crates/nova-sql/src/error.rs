use thiserror::Error;

#[derive(Error, Debug)]
pub enum SQLError {
    #[error("syntax error at {start}:{end}: {message}")]
    Syntax {
        message: String,
        start: usize,
        end: usize,
    },

    #[error("table not found: {0}")]
    TableNotFound(String),

    #[error("column not found: {0}")]
    ColumnNotFound(String),

    #[error("type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },

    #[error("internal error: {0}")]
    Internal(String),

    #[error("constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("query too complex: {0}")]
    QueryTooComplex(String),
}

impl SQLError {
    pub fn syntax(msg: impl Into<String>) -> Self {
        SQLError::Syntax {
            message: msg.into(),
            start: 0,
            end: 0,
        }
    }

    pub fn syntax_at(msg: impl Into<String>, start: usize, end: usize) -> Self {
        SQLError::Syntax {
            message: msg.into(),
            start,
            end,
        }
    }
}

pub type Result<T> = std::result::Result<T, SQLError>;
