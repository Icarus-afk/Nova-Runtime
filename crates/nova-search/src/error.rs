use thiserror::Error;

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("index not found: {0}")]
    IndexNotFound(String),
    #[error("field not found: {0}")]
    FieldNotFound(String),
    #[error("invalid query: {0}")]
    InvalidQuery(String),
    #[error("internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, SearchError>;
