use crate::types::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use parking_lot::RwLock;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,
    Open(Instant),
    HalfOpen,
}

pub struct CircuitBreaker {
    state: RwLock<HashMap<SubsystemId, CircuitState>>,
    failure_count: RwLock<HashMap<SubsystemId, u64>>,
    success_count: RwLock<HashMap<SubsystemId, u64>>,
    failure_threshold: u64,
    success_threshold: u64,
    half_open_timeout: Duration,
    window: Duration,
    last_state_change: RwLock<HashMap<SubsystemId, Instant>>,
    opens: AtomicU64,
    half_opens: AtomicU64,
    closes: AtomicU64,
    rejected: AtomicU64,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u64, success_threshold: u64, half_open_timeout_ms: u64, window_ms: u64) -> Self {
        Self {
            state: RwLock::new(HashMap::new()),
            failure_count: RwLock::new(HashMap::new()),
            success_count: RwLock::new(HashMap::new()),
            failure_threshold,
            success_threshold,
            half_open_timeout: Duration::from_millis(half_open_timeout_ms),
            window: Duration::from_millis(window_ms),
            last_state_change: RwLock::new(HashMap::new()),
            opens: AtomicU64::new(0),
            half_opens: AtomicU64::new(0),
            closes: AtomicU64::new(0),
            rejected: AtomicU64::new(0),
        }
    }

    pub fn execute<F, T>(&self, subsystem: &SubsystemId, f: F) -> Result<T, CircuitError>
    where
        F: Fn() -> Result<T, nova_core::RuntimeError>,
    {
        let state = self.get_state(subsystem);
        match state {
            CircuitState::Open(until) if Instant::now() < until => {
                self.rejected.fetch_add(1, Ordering::Relaxed);
                return Err(CircuitError::Open);
            }
            CircuitState::Open(_) => {
                self.transition(subsystem, CircuitState::HalfOpen);
            }
            _ => {}
        }

        match f() {
            Ok(result) => {
                let should_close = {
                    let mut s_count = self.success_count.write();
                    let count = s_count.entry(subsystem.clone()).or_insert(0);
                    *count += 1;
                    *count >= self.success_threshold
                        && matches!(self.state.read().get(subsystem), Some(CircuitState::HalfOpen))
                };
                if should_close {
                    self.close(subsystem);
                }
                Ok(result)
            }
            Err(err) if is_tracked_failure(&err) => {
                let should_open = {
                    let mut f_count = self.failure_count.write();
                    let count = f_count.entry(subsystem.clone()).or_insert(0);
                    *count += 1;
                    *count >= self.failure_threshold
                };
                if should_open {
                    self.open(subsystem);
                }
                Err(CircuitError::Failure(err.to_string()))
            }
            Err(err) => Err(CircuitError::Failure(err.to_string())),
        }
    }

    fn get_state(&self, subsystem: &SubsystemId) -> CircuitState {
        self.state.read().get(subsystem).copied().unwrap_or(CircuitState::Closed)
    }

    fn transition(&self, subsystem: &SubsystemId, new_state: CircuitState) {
        self.state.write().insert(subsystem.clone(), new_state);
        self.last_state_change.write().insert(subsystem.clone(), Instant::now());
        match new_state {
            CircuitState::Open(_) => { self.opens.fetch_add(1, Ordering::Relaxed); }
            CircuitState::HalfOpen => { self.half_opens.fetch_add(1, Ordering::Relaxed); }
            CircuitState::Closed => { self.closes.fetch_add(1, Ordering::Relaxed); }
        }
    }

    fn open(&self, subsystem: &SubsystemId) {
        self.transition(subsystem, CircuitState::Open(Instant::now() + self.half_open_timeout));
        self.failure_count.write().remove(subsystem);
    }

    fn close(&self, subsystem: &SubsystemId) {
        self.transition(subsystem, CircuitState::Closed);
        self.failure_count.write().remove(subsystem);
        self.success_count.write().remove(subsystem);
    }

    pub fn force_open(&self, subsystem: &SubsystemId) {
        self.transition(subsystem, CircuitState::Open(Instant::now() + Duration::from_secs(3600)));
    }

    pub fn force_close(&self, subsystem: &SubsystemId) {
        self.close(subsystem);
    }

    pub fn reset(&self, subsystem: &SubsystemId) {
        self.state.write().remove(subsystem);
        self.failure_count.write().remove(subsystem);
        self.success_count.write().remove(subsystem);
        self.last_state_change.write().remove(subsystem);
    }

    pub fn state(&self, subsystem: &SubsystemId) -> CircuitState {
        self.get_state(subsystem)
    }

    pub fn opens(&self) -> u64 {
        self.opens.load(Ordering::Relaxed)
    }

    pub fn half_opens(&self) -> u64 {
        self.half_opens.load(Ordering::Relaxed)
    }

    pub fn closes(&self) -> u64 {
        self.closes.load(Ordering::Relaxed)
    }

    pub fn rejected(&self) -> u64 {
        self.rejected.load(Ordering::Relaxed)
    }
}

fn is_tracked_failure(err: &nova_core::RuntimeError) -> bool {
    match err {
        nova_core::RuntimeError::Timeout(_) => true,
        nova_core::RuntimeError::TransactionConflict(_) => true,
        nova_core::RuntimeError::DeadlockDetected(_) => true,
        nova_core::RuntimeError::TransactionError(_) => true,
        nova_core::RuntimeError::Busy(_) => true,
        nova_core::RuntimeError::Io(_) => true,
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub enum CircuitError {
    Open,
    Failure(String),
}

impl std::fmt::Display for CircuitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitError::Open => write!(f, "circuit breaker is open"),
            CircuitError::Failure(msg) => write!(f, "circuit breaker failure: {}", msg),
        }
    }
}

impl std::error::Error for CircuitError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_initial_state_closed() {
        let cb = CircuitBreaker::new(5, 3, 1000, 10000);
        assert_eq!(cb.state(&SubsystemId::Storage), CircuitState::Closed);
    }

    #[test]
    fn test_force_open_transitions_to_open() {
        let cb = CircuitBreaker::new(5, 3, 1000, 10000);
        let subsystem = SubsystemId::Storage;
        cb.force_open(&subsystem);
        match cb.state(&subsystem) {
            CircuitState::Open(_) => {}
            _ => panic!("expected Open state"),
        }
        assert_eq!(cb.opens(), 1);
    }

    #[test]
    fn test_force_close_transitions_to_closed() {
        let cb = CircuitBreaker::new(5, 3, 1000, 10000);
        let subsystem = SubsystemId::Storage;
        cb.force_open(&subsystem);
        cb.force_close(&subsystem);
        assert_eq!(cb.state(&subsystem), CircuitState::Closed);
        assert_eq!(cb.closes(), 1);
    }

    #[test]
    fn test_reset_returns_to_closed() {
        let cb = CircuitBreaker::new(5, 3, 1000, 10000);
        let subsystem = SubsystemId::Storage;
        cb.force_open(&subsystem);
        cb.reset(&subsystem);
        assert_eq!(cb.state(&subsystem), CircuitState::Closed);
    }

    #[test]
    fn test_threshold_failures_opens_circuit() {
        let cb = CircuitBreaker::new(2, 1, 100, 10000);
        let subsystem = SubsystemId::Storage;

        // First failure: still closed, count = 1
        let result: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));
        assert!(matches!(result, Err(CircuitError::Failure(_))));
        assert_eq!(cb.state(&subsystem), CircuitState::Closed);

        // Second failure: triggers open
        let result: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));
        assert!(matches!(result, Err(CircuitError::Failure(_))));
        match cb.state(&subsystem) {
            CircuitState::Open(_) => {}
            _ => panic!("expected Open state after threshold failures"),
        }
        assert_eq!(cb.opens(), 1);
    }

    #[test]
    fn test_open_rejects_requests() {
        let cb = CircuitBreaker::new(1, 1, 100, 10000);
        let subsystem = SubsystemId::Storage;

        let _: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));
        match cb.state(&subsystem) {
            CircuitState::Open(_) => {}
            _ => panic!("expected Open"),
        }

        // Request while open should be rejected
        let result: Result<i32, CircuitError> = cb.execute(&subsystem, || Ok::<_, nova_core::RuntimeError>(42));
        assert!(matches!(result, Err(CircuitError::Open)));
        assert_eq!(cb.rejected(), 1);
    }

    #[test]
    fn test_half_open_transitions_to_closed_on_success() {
        let cb = CircuitBreaker::new(1, 1, 1, 10000);
        let subsystem = SubsystemId::Storage;

        // Open the circuit
        let _: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));
        match cb.state(&subsystem) {
            CircuitState::Open(_) => {}
            _ => panic!("expected Open"),
        }

        // Wait for half-open timeout
        std::thread::sleep(Duration::from_millis(5));

        // This call should see expired Open, transition to HalfOpen, succeed, and close
        let result: Result<i32, CircuitError> = cb.execute(&subsystem, || Ok::<_, nova_core::RuntimeError>(42));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        assert_eq!(cb.state(&subsystem), CircuitState::Closed);
        assert_eq!(cb.closes(), 1);
    }

    #[test]
    fn test_half_open_failure_returns_to_open() {
        let cb = CircuitBreaker::new(1, 5, 1, 10000);
        let subsystem = SubsystemId::Storage;

        // Open the circuit
        let _: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));

        // Wait for half-open timeout
        std::thread::sleep(Duration::from_millis(5));

        // Failure in HalfOpen should re-open
        let result: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));
        assert!(matches!(result, Err(CircuitError::Failure(_))));
        match cb.state(&subsystem) {
            CircuitState::Open(_) => {}
            _ => panic!("expected Open after failure in HalfOpen"),
        }
        assert_eq!(cb.opens(), 2);
    }

    #[test]
    fn test_non_tracked_failure_does_not_open() {
        let cb = CircuitBreaker::new(1, 1, 100, 10000);
        let subsystem = SubsystemId::Storage;

        // InvalidArgument is not a tracked failure
        let result: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::InvalidArgument("t".into())));
        assert!(matches!(result, Err(CircuitError::Failure(_))));
        assert_eq!(cb.state(&subsystem), CircuitState::Closed);
        assert_eq!(cb.opens(), 0);
    }

    #[test]
    fn test_rejected_counter_increments() {
        let cb = CircuitBreaker::new(1, 1, 100, 10000);
        let subsystem = SubsystemId::Storage;

        let _: Result<i32, CircuitError> = cb.execute(&subsystem, || Err(nova_core::RuntimeError::Timeout("t".into())));

        assert_eq!(cb.rejected(), 0); // first rejection hasn't happened yet
        let _: Result<i32, CircuitError> = cb.execute(&subsystem, || Ok::<_, nova_core::RuntimeError>(42));
        assert_eq!(cb.rejected(), 1);
    }

    #[test]
    fn test_counters_initialized_to_zero() {
        let cb = CircuitBreaker::new(5, 3, 1000, 10000);
        assert_eq!(cb.opens(), 0);
        assert_eq!(cb.half_opens(), 0);
        assert_eq!(cb.closes(), 0);
        assert_eq!(cb.rejected(), 0);
    }

    #[test]
    fn test_mixed_subsystems_independent() {
        let cb = CircuitBreaker::new(1, 1, 100, 10000);
        let storage = SubsystemId::Storage;
        let queue = SubsystemId::Queue;

        // Open storage
        let _: Result<i32, CircuitError> = cb.execute(&storage, || Err(nova_core::RuntimeError::Timeout("t".into())));
        match cb.state(&storage) {
            CircuitState::Open(_) => {}
            _ => panic!("expected Open for storage"),
        }

        // Queue should still be closed
        assert_eq!(cb.state(&queue), CircuitState::Closed);
    }
}
