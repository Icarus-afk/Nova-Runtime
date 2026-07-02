#![allow(unused_imports)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use nova_core::RuntimeError;
use nova_executor::*;

fn test_addr() -> SocketAddr {
    "127.0.0.1:8080".parse().unwrap()
}

// ==================== a) Rate limiter with pipeline (user-level) ====================

#[tokio::test]
async fn test_user_level_rate_limiting() {
    // Configure pipeline with user-level rate limiting (burst=2, low refill)
    let config = PipelineConfig {
        rate_limit_global_per_sec: 10000.0,
        rate_limit_global_burst: 10000.0,
        rate_limit_user_per_sec: 1.0,
        rate_limit_user_burst: 2.0,
        rate_limit_ip_per_sec: 10000.0,
        rate_limit_ip_burst: 10000.0,
        ..Default::default()
    };
    let executor = PipelineExecutor::new(config);

    let session = UserSession {
        user_id: 42,
        username: "limited_user".into(),
        roles: vec![],
        permissions: vec!["write".into()],
        session_id: 1,
        metadata: HashMap::new(),
    };
    let ctx = OperationContextBuilder::new(test_addr())
        .user_session(session)
        .build();

    // First two requests should succeed (within burst capacity)
    for i in 1..=2 {
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        let resp = executor.execute(req, ctx.clone()).await;
        assert!(
            resp.success,
            "request {}/2 (within user burst) should succeed: {:?}",
            i,
            resp.error
        );
    }

    // Third request should be rate limited (user bucket exhausted)
    let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
    let resp = executor.execute(req, ctx).await;
    assert!(!resp.success, "third request should be rate limited");
    assert_eq!(
        resp.error.as_ref().unwrap().code,
        ErrorCode::RateLimited,
        "expected RateLimited error"
    );
}

// ==================== b) Circuit breaker state transitions ====================

#[test]
fn test_circuit_breaker_state_transitions() {
    let cb = CircuitBreaker::new(
        2,   // failure_threshold: open after 2 failures
        1,   // success_threshold: close after 1 success in half-open
        100, // half_open_timeout_ms
        10000, // window_ms
    );
    let subsystem = SubsystemId::Storage;

    // === Initial state ===
    assert_eq!(
        cb.state(&subsystem),
        CircuitState::Closed,
        "circuit should start Closed"
    );

    // === First failure: still closed (count = 1) ===
    let result: Result<i32, CircuitError> =
        cb.execute(&subsystem, || Err(RuntimeError::Timeout("failure".into())));
    assert!(matches!(result, Err(CircuitError::Failure(_))));
    assert_eq!(
        cb.state(&subsystem),
        CircuitState::Closed,
        "first failure should not open circuit yet"
    );

    // === Second failure: opens the circuit ===
    let result: Result<i32, CircuitError> =
        cb.execute(&subsystem, || Err(RuntimeError::Timeout("failure".into())));
    assert!(matches!(result, Err(CircuitError::Failure(_))));
    assert!(
        matches!(cb.state(&subsystem), CircuitState::Open(_)),
        "second failure should open the circuit"
    );
    assert_eq!(cb.opens(), 1, "opens counter should be 1");

    // === Requests rejected while open ===
    let result: Result<i32, CircuitError> =
        cb.execute(&subsystem, || Ok::<_, RuntimeError>(42));
    assert!(
        matches!(result, Err(CircuitError::Open)),
        "requests should be rejected in Open state"
    );
    assert_eq!(cb.rejected(), 1, "rejected counter should be 1");

    // === Wait for half-open timeout (100ms), then succeed → close ===
    std::thread::sleep(Duration::from_millis(150));

    let result: Result<i32, CircuitError> =
        cb.execute(&subsystem, || Ok::<_, RuntimeError>(99));
    assert!(result.is_ok(), "request should succeed after timeout");
    assert_eq!(result.unwrap(), 99);

    assert_eq!(
        cb.state(&subsystem),
        CircuitState::Closed,
        "circuit should close after success in half-open"
    );
    assert_eq!(cb.closes(), 1, "closes counter should be 1");
}
