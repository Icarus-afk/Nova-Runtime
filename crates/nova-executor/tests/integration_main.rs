#![allow(unused_imports)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nova_executor::*;
use nova_storage::{Key, StorageConfig, Store, Value};

fn test_addr() -> SocketAddr {
    "127.0.0.1:8080".parse().unwrap()
}

fn user_session_with_write() -> UserSession {
    UserSession {
        user_id: 1,
        username: "test_user".into(),
        roles: vec!["user".into()],
        permissions: vec!["write".into()],
        session_id: 42,
        metadata: HashMap::new(),
    }
}

// Manual temp dir that cleans up on drop — no external dependency needed
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("nova_exec_int_{}", ts));
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ==================== a) Storage write → read roundtrip ====================

#[tokio::test]
async fn test_storage_write_read_roundtrip() {
    let _dir = TempDir::new();
    let config = StorageConfig {
        data_dir: _dir.path().join("data"),
        wal_dir: _dir.path().join("wal"),
        ..Default::default()
    };
    let store = Store::open(&config).unwrap();

    // Write directly to storage
    let key = Key::from("integration_test_key");
    let value = Value::new(b"integration_test_value".to_vec());
    store.set(key.clone(), value.clone()).unwrap();

    // Read back from storage — verify the value
    let result = store.get(&key).unwrap();
    assert_eq!(result, Some(value));

    // Create pipeline and process a mutation operation
    let pipeline = PipelineExecutor::new(PipelineConfig::default());

    let write_ctx = OperationContextBuilder::new(test_addr())
        .user_session(user_session_with_write())
        .operation_type(OperationType::Create)
        .build();
    let write_req = OperationRequest::new(
        OperationType::Create,
        OperationTarget::Object {
            type_name: "test".into(),
            id: Some(1),
        },
    );
    let write_resp = pipeline.execute(write_req, write_ctx).await;
    assert!(
        write_resp.success,
        "write (Create) operation should succeed: {:?}",
        write_resp.error
    );

    // Execute a read operation through pipeline
    let read_ctx = OperationContextBuilder::new(test_addr())
        .operation_type(OperationType::Get)
        .build();
    let read_req = OperationRequest::new(
        OperationType::Get,
        OperationTarget::Object {
            type_name: "test".into(),
            id: Some(1),
        },
    );
    let read_resp = pipeline.execute(read_req, read_ctx).await;
    assert!(
        read_resp.success,
        "read (Get) operation should succeed: {:?}",
        read_resp.error
    );

    // Verify store data is still intact after pipeline operations
    let stored = store.get(&Key::from("integration_test_key")).unwrap();
    assert!(stored.is_some());
    assert_eq!(stored.unwrap().as_bytes(), b"integration_test_value");

    store.close().unwrap();
}

// ==================== b) Pipeline with cancellation/timeout ====================

#[tokio::test]
async fn test_pipeline_with_timeout() {
    let config = PipelineConfig {
        default_operation_timeout_ms: 1,
        max_operation_timeout_ms: 5,
        ..Default::default()
    };
    let executor = PipelineExecutor::new(config);

    let ctx = OperationContextBuilder::new(test_addr()).build();

    // Set timeout to 0ms so the deadline is effectively "now"
    let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
    req.options.timeout = Some(Duration::from_millis(0));

    let resp = executor.execute(req, ctx).await;
    assert!(!resp.success, "operation past deadline should fail");
    assert!(resp.error.is_some());

    let code = resp.error.as_ref().unwrap().code;
    assert_eq!(
        code,
        ErrorCode::DeadlineExceeded,
        "expected DeadlineExceeded, got {:?}",
        code
    );
}

// ==================== c) Middleware chain execution ====================

struct RecorderMiddleware {
    name: &'static str,
    stage: PipelineStage,
    order: u32,
    log: Arc<Mutex<Vec<String>>>,
}

impl Middleware for RecorderMiddleware {
    fn name(&self) -> &'static str {
        self.name
    }
    fn stage(&self) -> PipelineStage {
        self.stage
    }
    fn handle(
        &self,
        ctx: &mut OperationContext,
        req: &mut OperationRequest,
        next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
    ) -> PipelineResult {
        self.log.lock().unwrap().push(format!("enter_{}", self.name));
        let result = next(ctx, req);
        self.log.lock().unwrap().push(format!("exit_{}", self.name));
        result
    }
}

#[tokio::test]
async fn test_middleware_chain_execution_order() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let executor = PipelineExecutor::new(PipelineConfig::default());

    // Register three middleware at the Parse stage with increasing order values
    for (name, order) in [("first", 1u32), ("second", 2), ("third", 3)] {
        executor
            .register_middleware(MiddlewareRegistration {
                name: name.into(),
                stage: PipelineStage::Parse,
                order,
                middleware: Arc::new(RecorderMiddleware {
                    name,
                    stage: PipelineStage::Parse,
                    order,
                    log: log.clone(),
                }),
                enabled: true,
                config: HashMap::new(),
            })
            .unwrap();
    }

    let ctx = OperationContextBuilder::new(test_addr()).build();
    let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
    let resp = executor.execute(req, ctx).await;
    assert!(
        resp.success,
        "middleware chain execution should succeed: {:?}",
        resp.error
    );

    let order = log.lock().unwrap().clone();
    // The middleware chain wraps inside-out: first wraps second wraps third wraps stage_fn
    // So execution order is: enter_first → enter_second → enter_third → exit_third → exit_second → exit_first
    assert_eq!(
        order,
        vec![
            "enter_first",
            "enter_second",
            "enter_third",
            "exit_third",
            "exit_second",
            "exit_first",
        ],
        "middleware execution order is incorrect"
    );
}

// ==================== d) Rate limiting ====================

#[tokio::test]
async fn test_pipeline_rate_limiting() {
    let config = PipelineConfig {
        rate_limit_global_per_sec: 1.0,
        rate_limit_global_burst: 1.0,
        rate_limit_user_per_sec: 1.0,
        rate_limit_user_burst: 1.0,
        rate_limit_ip_per_sec: 1.0,
        rate_limit_ip_burst: 1.0,
        ..Default::default()
    };
    let executor = PipelineExecutor::new(config);

    let session = UserSession {
        user_id: 100,
        username: "ratelimit_user".into(),
        roles: vec![],
        permissions: vec!["write".into()],
        session_id: 1,
        metadata: HashMap::new(),
    };
    let ctx = OperationContextBuilder::new(test_addr())
        .user_session(session)
        .build();

    // First request consumes the only available token
    let req1 = OperationRequest::new(OperationType::Get, OperationTarget::System);
    let resp1 = executor.execute(req1, ctx.clone()).await;
    assert!(resp1.success, "first request should succeed");

    // Second request should be rate limited (no tokens left)
    let req2 = OperationRequest::new(OperationType::Get, OperationTarget::System);
    let resp2 = executor.execute(req2, ctx).await;
    assert!(!resp2.success, "second request should be rate limited");
    assert_eq!(
        resp2.error.as_ref().unwrap().code,
        ErrorCode::RateLimited
    );
}

// ==================== e) Circuit breaker integration ====================

#[tokio::test]
async fn test_circuit_breaker_starts_closed() {
    let executor = PipelineExecutor::new(PipelineConfig::default());
    let metrics = executor.metrics();
    let snap = metrics.snapshot();
    assert_eq!(
        snap.circuit_opens, 0,
        "circuit breaker should start closed with 0 opens"
    );
    assert_eq!(
        snap.circuit_rejected, 0,
        "circuit breaker should start with 0 rejections"
    );

    // Execute a few successful operations
    let ctx = OperationContextBuilder::new(test_addr()).build();
    for _ in 0..3 {
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        let resp = executor.execute(req, ctx.clone()).await;
        assert!(resp.success);
    }

    // Circuit breaker should remain closed after successful ops
    let snap = executor.metrics().snapshot();
    assert_eq!(
        snap.circuit_opens, 0,
        "circuit should still be closed after successful ops"
    );
}

// ==================== f) Concurrent pipeline execution ====================

#[tokio::test]
async fn test_concurrent_pipeline_execution() {
    let config = PipelineConfig {
        max_concurrent_ops: 10,
        ..Default::default()
    };
    let executor = Arc::new(PipelineExecutor::new(config));

    let mut handles = Vec::new();
    for i in 0..10 {
        let ex = executor.clone();
        let ctx = OperationContextBuilder::new(test_addr())
            .trace_id(i as u128)
            .build();
        handles.push(tokio::spawn(async move {
            let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
            ex.execute(req, ctx).await
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let resp = handle.await.unwrap();
        assert!(
            resp.success,
            "concurrent operation {} failed: {:?}",
            i,
            resp.error
        );
    }
}
