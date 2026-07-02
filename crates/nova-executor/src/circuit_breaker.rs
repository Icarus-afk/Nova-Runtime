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
                let mut s_count = self.success_count.write();
                let count = s_count.entry(subsystem.clone()).or_insert(0);
                *count += 1;
                if *count >= self.success_threshold {
                    if let Some(CircuitState::HalfOpen) = self.state.read().get(subsystem) {
                        self.close(subsystem);
                    }
                }
                Ok(result)
            }
            Err(err) if is_tracked_failure(&err) => {
                let mut f_count = self.failure_count.write();
                let count = f_count.entry(subsystem.clone()).or_insert(0);
                *count += 1;
                if *count >= self.failure_threshold {
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
