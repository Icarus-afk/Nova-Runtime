use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SchedulerError {
    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Job already exists: {0}")]
    JobAlreadyExists(String),

    #[error("Invalid cron expression: {0}")]
    InvalidCronExpression(String),

    #[error("Invalid schedule: {0}")]
    InvalidSchedule(String),

    #[error("Dependency cycle detected: {0}")]
    DependencyCycle(String),

    #[error("Dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),

    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Job cancelled: {0}")]
    JobCancelled(String),

    #[error("Overlap prevented: {0}")]
    OverlapPrevented(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Shutting down")]
    ShuttingDown,
}

pub type Result<T> = std::result::Result<T, SchedulerError>;

impl From<nova_core::RuntimeError> for SchedulerError {
    fn from(e: nova_core::RuntimeError) -> Self {
        SchedulerError::Storage(e.to_string())
    }
}
