use crate::types::*;
use crate::middleware::{MiddlewareChain, MiddlewareRegistration, StageFn};
use crate::rate_limiter::{RateLimiter, RateLimitConfig};
use crate::circuit_breaker::{CircuitBreaker, CircuitState};
use crate::operation_queue::OperationQueue;
use crate::metrics::PipelineMetrics;

use std::sync::Arc;
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicU64, Ordering};
use std::collections::HashMap;
use parking_lot::RwLock;
use tokio::sync::Semaphore;

#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub max_concurrent_ops: u32,
    pub pipeline_queue_depth: u32,
    pub worker_threads: u32,
    pub default_operation_timeout_ms: u64,
    pub max_operation_timeout_ms: u64,
    pub rate_limit_global_per_sec: f64,
    pub rate_limit_global_burst: f64,
    pub rate_limit_user_per_sec: f64,
    pub rate_limit_user_burst: f64,
    pub rate_limit_ip_per_sec: f64,
    pub rate_limit_ip_burst: f64,
    pub circuit_breaker_threshold: u64,
    pub circuit_breaker_window_ms: u64,
    pub circuit_breaker_half_open_timeout_ms: u64,
    pub circuit_breaker_success_threshold: u64,
    pub audit_enabled: bool,
    pub audit_include_payloads: bool,
    pub audit_max_entry_size: u32,
    pub idempotency_key_ttl_secs: u64,
    pub max_idempotency_keys: u32,
    pub max_retries: u8,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        PipelineConfig {
            max_concurrent_ops: 256,
            pipeline_queue_depth: 1024,
            worker_threads: 4,
            default_operation_timeout_ms: 5000,
            max_operation_timeout_ms: 60000,
            rate_limit_global_per_sec: 10000.0,
            rate_limit_global_burst: 20000.0,
            rate_limit_user_per_sec: 100.0,
            rate_limit_user_burst: 200.0,
            rate_limit_ip_per_sec: 1000.0,
            rate_limit_ip_burst: 2000.0,
            circuit_breaker_threshold: 50,
            circuit_breaker_window_ms: 10000,
            circuit_breaker_half_open_timeout_ms: 10000,
            circuit_breaker_success_threshold: 10,
            audit_enabled: true,
            audit_include_payloads: false,
            audit_max_entry_size: 4096,
            idempotency_key_ttl_secs: 86400,
            max_idempotency_keys: 100000,
            max_retries: 3,
            retry_base_delay_ms: 10,
            retry_max_delay_ms: 1000,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PipelineStatus {
    pub is_running: bool,
    pub is_draining: bool,
    pub active_operations: u64,
    pub total_operations: u64,
    pub started_at: Instant,
    pub uptime_secs: u64,
}

type IdempotencyMap = HashMap<u128, (OperationResponse, Instant)>;

pub struct PipelineExecutor {
    config: PipelineConfig,
    middleware: RwLock<MiddlewareChain>,
    rate_limiter: Arc<RwLock<RateLimiter>>,
    circuit_breaker: Arc<CircuitBreaker>,
    operation_queue: Arc<OperationQueue>,
    metrics: Arc<PipelineMetrics>,
    active_operations: AtomicU64,
    total_operations: AtomicU64,
    max_concurrent: Semaphore,
    is_running: RwLock<bool>,
    is_draining: RwLock<bool>,
    started_at: Instant,
    idempotency_cache: RwLock<IdempotencyMap>,
}

impl PipelineExecutor {
    pub fn new(config: PipelineConfig) -> Self {
        let max_concurrent_ops = config.max_concurrent_ops.max(1);
        let rate_cfg = RateLimitConfig {
            global_per_sec: config.rate_limit_global_per_sec,
            global_burst: config.rate_limit_global_burst,
            user_per_sec: config.rate_limit_user_per_sec,
            user_burst: config.rate_limit_user_burst,
            ip_per_sec: config.rate_limit_ip_per_sec,
            ip_burst: config.rate_limit_ip_burst,
            ..RateLimitConfig::default()
        };
        PipelineExecutor {
            rate_limiter: Arc::new(RwLock::new(RateLimiter::new(rate_cfg))),
            circuit_breaker: Arc::new(CircuitBreaker::new(
                config.circuit_breaker_threshold,
                config.circuit_breaker_success_threshold,
                config.circuit_breaker_half_open_timeout_ms,
                config.circuit_breaker_window_ms,
            )),
            operation_queue: Arc::new(OperationQueue::new(
                config.pipeline_queue_depth as usize,
            )),
            metrics: Arc::new(PipelineMetrics::new()),
            middleware: RwLock::new(MiddlewareChain::new()),
            max_concurrent: Semaphore::new(max_concurrent_ops as usize),
            active_operations: AtomicU64::new(0),
            total_operations: AtomicU64::new(0),
            is_running: RwLock::new(true),
            is_draining: RwLock::new(false),
            started_at: Instant::now(),
            config,
            idempotency_cache: RwLock::new(HashMap::new()),
        }
    }

    pub async fn execute(&self, req: OperationRequest, ctx: OperationContext) -> OperationResponse {
        if !*self.is_running.read() {
            return OperationResponse::error(ErrorCode::ServiceUnavailable, "pipeline is not running");
        }
        if *self.is_draining.read() {
            return OperationResponse::error(ErrorCode::ServiceUnavailable, "pipeline is draining");
        }

        let _permit = self.max_concurrent.acquire().await;
        self.active_operations.fetch_add(1, Ordering::AcqRel);

        {
            let rl = self.rate_limiter.read();
            if rl.check(&ctx, &req).is_err() {
                self.active_operations.fetch_sub(1, Ordering::AcqRel);
                return OperationResponse::error(ErrorCode::RateLimited, "rate limit exceeded");
            }
        }

        let started_at = Instant::now();
        let mut ctx = ctx;
        let mut req = req;

        let timeout_ms = req.options.timeout
            .map(|d| d.as_millis() as u64)
            .unwrap_or(self.config.default_operation_timeout_ms);
        let clamped = timeout_ms.min(self.config.max_operation_timeout_ms);
        ctx.deadline = Instant::now() + Duration::from_millis(clamped);

        let response = self.run_pipeline(started_at, &mut ctx, &mut req);

        self.metrics.record_operation(req.operation_type, response.status);
        self.total_operations.fetch_add(1, Ordering::Relaxed);
        self.active_operations.fetch_sub(1, Ordering::AcqRel);

        response
    }

    fn run_pipeline(&self, started_at: Instant, ctx: &mut OperationContext, req: &mut OperationRequest) -> OperationResponse {
        // Stages 1-3: Parse, Validate, Authorize (pre-execution)
        let mut stage_result: Option<PipelineResult> = None;

        for &stage in &[PipelineStage::Parse, PipelineStage::Validate, PipelineStage::Authorize] {
            ctx.stage = stage;
            let stage_start = Instant::now();
            let stage_fn: StageFn = match stage {
                PipelineStage::Parse => Arc::new(Self::parse_stage),
                PipelineStage::Validate => Arc::new(Self::validate_stage),
                PipelineStage::Authorize => Arc::new(Self::authorize_stage),
                _ => unreachable!(),
            };
            let result = self.middleware.read().run_chain(stage, ctx, req, stage_fn);
            let elapsed = stage_start.elapsed();
            ctx.stage_elapsed = elapsed;
            match result {
                PipelineResult::Continue => continue,
                PipelineResult::ShortCircuit(resp) => {
                    stage_result = Some(PipelineResult::ShortCircuit(resp));
                    break;
                }
                PipelineResult::Error(e) => {
                    stage_result = Some(PipelineResult::Error(e));
                    break;
                }
            }
        }

        let exec_result = match stage_result {
            Some(PipelineResult::ShortCircuit(resp)) => return resp,
            Some(PipelineResult::Error(e)) => return Self::build_error_response(ctx, started_at, e),
            None => self.execute_with_retry(ctx, req),
            _ => unreachable!(),
        };

        // Log and Notify run even on failure
        ctx.stage = PipelineStage::Log;
        let _ = self.middleware.read().run_chain(
            PipelineStage::Log, ctx, req,
            Arc::new(|ctx, req| { Self::log_stage(ctx, req); PipelineResult::Continue }),
        );

        ctx.stage = PipelineStage::Notify;
        let _ = self.middleware.read().run_chain(
            PipelineStage::Notify, ctx, req,
            Arc::new(|ctx, req| { Self::notify_stage(ctx, req); PipelineResult::Continue }),
        );

        let duration_ns = started_at.elapsed().as_nanos() as u64;
        match exec_result {
            Ok(StageStatus::Success) | Ok(StageStatus::ShortCircuit) | Ok(StageStatus::Skipped) => OperationResponse {
                status: StatusCode::Ok,
                success: true,
                data: None,
                data_size: 0,
                trace_id: ctx.trace_id,
                duration_ns,
                error: None,
                warnings: Vec::new(),
                stage_timings: Vec::new(),
            },
            Err(e) => Self::build_error_response(ctx, started_at, e),
            Ok(StageStatus::Error) => Self::build_error_response(ctx, started_at, PipelineError::new(ErrorCode::InternalError, "stage returned error status").with_stage(PipelineStage::Execute)),
        }
    }

    fn execute_with_retry(&self, ctx: &mut OperationContext, req: &mut OperationRequest) -> Result<StageStatus, PipelineError> {
        let max_retries = req.options.max_retries.min(self.config.max_retries);

        let mut attempt: u8 = 0;
        loop {
            if attempt > 0 {
                let delay = Self::calculate_backoff(attempt, self.config.retry_base_delay_ms, self.config.retry_max_delay_ms);
                if ctx.remaining_deadline() <= delay {
                    return Err(PipelineError::new(ErrorCode::DeadlineExceeded, "deadline exceeded before retry").with_stage(PipelineStage::Execute));
                }
                let jitter_frac = (Instant::now().elapsed().as_nanos() % 26) as f64 / 100.0;
                let jitter_ms = (delay.as_millis() as f64 * jitter_frac) as u64;
                let actual = delay + Duration::from_millis(jitter_ms);
                if ctx.remaining_deadline() <= actual {
                    return Err(PipelineError::new(ErrorCode::DeadlineExceeded, "deadline exceeded before retry").with_stage(PipelineStage::Execute));
                }
                std::thread::sleep(actual);
            }

            if ctx.is_expired() {
                return Err(PipelineError::new(ErrorCode::DeadlineExceeded, "deadline exceeded").with_stage(PipelineStage::Execute));
            }

            ctx.retry_count = attempt;
            let subsystem = match &req.target {
                OperationTarget::Object { .. } | OperationTarget::Collection { .. } => SubsystemId::Storage,
                OperationTarget::Queue { .. } => SubsystemId::Queue,
                OperationTarget::Schedule { .. } => SubsystemId::Scheduler,
                OperationTarget::Blob { .. } => SubsystemId::Blob,
                OperationTarget::Auth { .. } => SubsystemId::Auth,
                OperationTarget::Admin { .. } => SubsystemId::Admin,
                OperationTarget::System => SubsystemId::Pipeline,
            };

            // Check circuit breaker state
            match self.circuit_breaker.state(&subsystem) {
                CircuitState::Open(until) if Instant::now() < until => {
                    return Err(PipelineError::new(ErrorCode::CircuitBreakerOpen, "circuit breaker is open").with_stage(PipelineStage::Execute));
                }
                _ => {}
            }

            let stage_start = Instant::now();
            let execute_mw = self.middleware.read().run_chain(
                PipelineStage::Execute, ctx, req,
                Arc::new(Self::execute_stage),
            );
            ctx.stage_elapsed = stage_start.elapsed();

            match execute_mw {
                PipelineResult::Continue => {
                    let _ = self.circuit_breaker.execute(&subsystem, || Ok::<_, nova_core::RuntimeError>(()));
                    return Ok(StageStatus::Success);
                }
                PipelineResult::ShortCircuit(_) => {
                    return Ok(StageStatus::ShortCircuit);
                }
                PipelineResult::Error(e) => {
                    if !e.retryable || attempt >= max_retries {
                        return Err(e);
                    }
                }
            }

            attempt += 1;
        }
    }

    fn build_error_response(ctx: &OperationContext, started_at: Instant, err: PipelineError) -> OperationResponse {
        let status = match err.code {
            ErrorCode::ParseError => StatusCode::BadRequest,
            ErrorCode::ValidationError => StatusCode::UnprocessableEntity,
            ErrorCode::AuthorizationError => StatusCode::Forbidden,
            ErrorCode::AuthenticationError => StatusCode::Unauthorized,
            ErrorCode::NotFound => StatusCode::NotFound,
            ErrorCode::Conflict => StatusCode::Conflict,
            ErrorCode::RateLimited => StatusCode::TooManyRequests,
            ErrorCode::CircuitBreakerOpen => StatusCode::ServiceUnavailable,
            ErrorCode::DeadlineExceeded => StatusCode::DeadlineExceeded,
            ErrorCode::Cancelled => StatusCode::Cancelled,
            ErrorCode::PayloadTooLarge => StatusCode::RequestTooLarge,
            ErrorCode::Unprocessable => StatusCode::UnprocessableEntity,
            ErrorCode::InternalError => StatusCode::InternalError,
            ErrorCode::ServiceUnavailable => StatusCode::ServiceUnavailable,
            ErrorCode::NotImplemented => StatusCode::NotImplemented,
            ErrorCode::InsufficientStorage => StatusCode::InsufficientStorage,
        };
        let retryable = matches!(err.code, ErrorCode::RateLimited
            | ErrorCode::CircuitBreakerOpen | ErrorCode::DeadlineExceeded
            | ErrorCode::ServiceUnavailable | ErrorCode::InternalError);
        OperationResponse {
            status,
            success: false,
            data: None,
            data_size: 0,
            trace_id: ctx.trace_id,
            duration_ns: started_at.elapsed().as_nanos() as u64,
            error: Some(ErrorInfo {
                code: err.code,
                message: err.message,
                details: err.details,
                retryable,
                retry_after_ms: None,
            }),
            warnings: Vec::new(),
            stage_timings: Vec::new(),
        }
    }

    fn parse_stage(ctx: &mut OperationContext, req: &mut OperationRequest) -> PipelineResult {
        ctx.stage = PipelineStage::Parse;
        if req.payload.is_some() && req.payload_size == 0 {
            req.payload_size = req.payload.as_ref().map(|p| p.len() as u64).unwrap_or(0);
        }
        if req.payload_size > 10 * 1024 * 1024 {
            return PipelineResult::Error(
                PipelineError::new(ErrorCode::PayloadTooLarge, format!("payload size {} exceeds limit", req.payload_size))
                    .with_stage(PipelineStage::Parse)
            );
        }
        PipelineResult::Continue
    }

    fn validate_stage(ctx: &mut OperationContext, req: &mut OperationRequest) -> PipelineResult {
        ctx.stage = PipelineStage::Validate;
        if req.payload_size > 0 && req.payload.is_none() {
            return PipelineResult::Error(
                PipelineError::new(ErrorCode::ValidationError, "payload_size > 0 but no payload provided")
                    .with_stage(PipelineStage::Validate)
            );
        }
        let target_valid = match &req.target {
            OperationTarget::Object { type_name, .. } => !type_name.is_empty(),
            OperationTarget::Collection { type_name } => !type_name.is_empty(),
            OperationTarget::Queue { name } => !name.is_empty(),
            OperationTarget::Schedule { .. } => true,
            OperationTarget::Blob { .. } => true,
            OperationTarget::Auth { realm } => !realm.is_empty(),
            OperationTarget::Admin { endpoint } => !endpoint.is_empty(),
            OperationTarget::System => true,
        };
        if !target_valid {
            return PipelineResult::Error(
                PipelineError::new(ErrorCode::ValidationError, "invalid operation target")
                    .with_stage(PipelineStage::Validate)
            );
        }
        if let Some(timeout) = req.options.timeout {
            if timeout.as_millis() as u64 > 3_600_000 {
                return PipelineResult::Error(
                    PipelineError::new(ErrorCode::ValidationError, "operation timeout exceeds maximum")
                        .with_stage(PipelineStage::Validate)
                );
            }
        }
        PipelineResult::Continue
    }

    fn authorize_stage(ctx: &mut OperationContext, req: &mut OperationRequest) -> PipelineResult {
        ctx.stage = PipelineStage::Authorize;
        if req.operation_type.is_mutation() {
            let authorized = ctx.user_session.as_ref()
                .map(|s| s.permissions.iter().any(|p| p.contains("write") || p.contains("admin")))
                .unwrap_or(false);
            if !authorized {
                return PipelineResult::Error(
                    PipelineError::new(ErrorCode::AuthorizationError, "insufficient permissions for mutation operation")
                        .with_stage(PipelineStage::Authorize)
                );
            }
        }
        PipelineResult::Continue
    }

    fn execute_stage(ctx: &mut OperationContext, req: &mut OperationRequest) -> PipelineResult {
        ctx.stage = PipelineStage::Execute;
        if ctx.is_expired() {
            return PipelineResult::Error(
                PipelineError::new(ErrorCode::DeadlineExceeded, "deadline exceeded before execution")
                    .with_stage(PipelineStage::Execute)
            );
        }
        ctx.subsystem = match &req.target {
            OperationTarget::Object { .. } | OperationTarget::Collection { .. } => SubsystemId::Storage,
            OperationTarget::Queue { .. } => SubsystemId::Queue,
            OperationTarget::Schedule { .. } => SubsystemId::Scheduler,
            OperationTarget::Blob { .. } => SubsystemId::Blob,
            OperationTarget::Auth { .. } => SubsystemId::Auth,
            OperationTarget::Admin { .. } => SubsystemId::Admin,
            OperationTarget::System => SubsystemId::Pipeline,
        };
        PipelineResult::Continue
    }

    fn log_stage(_ctx: &mut OperationContext, _req: &mut OperationRequest) {
    }

    fn notify_stage(_ctx: &mut OperationContext, _req: &mut OperationRequest) {
    }

    fn calculate_backoff(attempt: u8, base_ms: u64, max_ms: u64) -> Duration {
        let exponent = attempt.saturating_sub(1) as u32;
        let delay = base_ms.saturating_mul(5u64.saturating_pow(exponent));
        let capped = delay.min(max_ms);
        Duration::from_millis(capped)
    }

    pub fn register_middleware(&self, registration: MiddlewareRegistration) -> Result<(), String> {
        self.middleware.write().register(registration)
    }

    pub fn unregister_middleware(&self, name: &str) -> Result<(), String> {
        self.middleware.write().unregister(name)
    }

    pub fn metrics(&self) -> Arc<PipelineMetrics> {
        self.metrics.clone()
    }

    pub fn status(&self) -> PipelineStatus {
        PipelineStatus {
            is_running: *self.is_running.read(),
            is_draining: *self.is_draining.read(),
            active_operations: self.active_operations.load(Ordering::Acquire),
            total_operations: self.total_operations.load(Ordering::Relaxed),
            started_at: self.started_at,
            uptime_secs: self.started_at.elapsed().as_secs(),
        }
    }

    pub async fn drain(&self, timeout: Duration) -> Result<(), ()> {
        *self.is_draining.write() = true;
        let deadline = Instant::now() + timeout;
        while self.active_operations.load(Ordering::Acquire) > 0 {
            if Instant::now() >= deadline {
                *self.is_draining.write() = false;
                return Err(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        *self.is_running.write() = false;
        Ok(())
    }

    pub fn update_rate_limits(&self, config: RateLimitConfig) {
        let mut rl = self.rate_limiter.write();
        rl.update_config(config);
    }

    fn check_idempotency(&self, key: u128, op_type: OperationType) -> Option<OperationResponse> {
        if !op_type.is_mutation() {
            return None;
        }
        let cache = self.idempotency_cache.read();
        cache.get(&key).and_then(|(resp, expires)| {
            if Instant::now() < *expires { Some(resp.clone()) } else { None }
        })
    }

    fn store_idempotency(&self, key: u128, response: &OperationResponse) {
        if response.success {
            let mut cache = self.idempotency_cache.write();
            if cache.len() >= self.config.max_idempotency_keys as usize {
                cache.clear();
            }
            let ttl = Duration::from_secs(self.config.idempotency_key_ttl_secs);
            cache.insert(key, (response.clone(), Instant::now() + ttl));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OperationRequest;
    use crate::OperationTarget;
    use crate::OperationType;
    use crate::UserSession;
    use crate::ErrorCode;
    use crate::PipelineStage;
    use std::collections::HashMap;
    use crate::context::OperationContextBuilder;
    use std::net::SocketAddr;

    fn test_addr() -> SocketAddr {
        "127.0.0.1:8080".parse().unwrap()
    }

    #[test]
    fn test_pipeline_config_default_values() {
        let cfg = PipelineConfig::default();
        assert_eq!(cfg.max_concurrent_ops, 256);
        assert_eq!(cfg.pipeline_queue_depth, 1024);
        assert_eq!(cfg.worker_threads, 4);
        assert_eq!(cfg.default_operation_timeout_ms, 5000);
        assert_eq!(cfg.max_operation_timeout_ms, 60000);
        assert_eq!(cfg.rate_limit_global_per_sec, 10000.0);
        assert_eq!(cfg.rate_limit_global_burst, 20000.0);
        assert_eq!(cfg.circuit_breaker_threshold, 50);
        assert_eq!(cfg.circuit_breaker_half_open_timeout_ms, 10000);
        assert_eq!(cfg.circuit_breaker_success_threshold, 10);
        assert_eq!(cfg.max_retries, 3);
        assert_eq!(cfg.retry_base_delay_ms, 10);
        assert_eq!(cfg.retry_max_delay_ms, 1000);
        assert!(cfg.audit_enabled);
    }

    #[test]
    fn test_parse_stage_valid_payload() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        req.payload = Some(vec![1, 2, 3]);
        req.payload_size = 3;

        let result = PipelineExecutor::parse_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_parse_stage_payload_size_zero_auto_detects() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        req.payload = Some(vec![1, 2, 3, 4, 5]);
        req.payload_size = 0;

        let result = PipelineExecutor::parse_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
        assert_eq!(req.payload_size, 5);
    }

    #[test]
    fn test_parse_stage_payload_too_large() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        req.payload = Some(vec![0u8; 11 * 1024 * 1024]);
        req.payload_size = 11 * 1024 * 1024;

        let result = PipelineExecutor::parse_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => {
                assert_eq!(e.code, ErrorCode::PayloadTooLarge);
                assert_eq!(e.stage, PipelineStage::Parse);
            }
            _ => panic!("expected PayloadTooLarge error"),
        }
    }

    #[test]
    fn test_validate_stage_valid_target() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::Object { type_name: "user".into(), id: Some(1) },
        );
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_validate_stage_invalid_target_empty_type_name() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::Object { type_name: "".into(), id: None },
        );
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => {
                assert_eq!(e.code, ErrorCode::ValidationError);
                assert_eq!(e.stage, PipelineStage::Validate);
            }
            _ => panic!("expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_stage_invalid_target_empty_collection() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::Collection { type_name: "".into() },
        );
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => assert_eq!(e.code, ErrorCode::ValidationError),
            _ => panic!("expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_stage_invalid_target_empty_queue_name() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::Queue { name: "".into() },
        );
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => assert_eq!(e.code, ErrorCode::ValidationError),
            _ => panic!("expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_stage_invalid_target_empty_auth_realm() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::Auth { realm: "".into() },
        );
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => assert_eq!(e.code, ErrorCode::ValidationError),
            _ => panic!("expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_stage_timeout_exceeds_maximum() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::System,
        );
        req.options.timeout = Some(Duration::from_millis(3_600_001));
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => {
                assert_eq!(e.code, ErrorCode::ValidationError);
                assert!(e.message.contains("timeout exceeds maximum"));
            }
            _ => panic!("expected ValidationError"),
        }
    }

    #[test]
    fn test_validate_stage_valid_timeout() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::System,
        );
        req.options.timeout = Some(Duration::from_millis(100));
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_validate_stage_payload_size_without_payload() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::System,
        );
        req.payload_size = 100;
        req.payload = None;
        let result = PipelineExecutor::validate_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => assert_eq!(e.code, ErrorCode::ValidationError),
            _ => panic!("expected ValidationError"),
        }
    }

    #[test]
    fn test_authorize_stage_read_bypasses_auth() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        let result = PipelineExecutor::authorize_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_authorize_stage_mutation_without_permissions() {
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Create, OperationTarget::System);
        let result = PipelineExecutor::authorize_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => assert_eq!(e.code, ErrorCode::AuthorizationError),
            _ => panic!("expected AuthorizationError"),
        }
    }

    #[test]
    fn test_authorize_stage_mutation_with_write_permission() {
        let session = UserSession {
            user_id: 1,
            username: "test".into(),
            roles: vec![],
            permissions: vec!["write".into()],
            session_id: 1,
            metadata: HashMap::new(),
        };
        let mut ctx = OperationContextBuilder::new(test_addr())
            .user_session(session)
            .build();
        let mut req = OperationRequest::new(OperationType::Create, OperationTarget::System);
        let result = PipelineExecutor::authorize_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_authorize_stage_mutation_with_admin_permission() {
        let session = UserSession {
            user_id: 1,
            username: "admin".into(),
            roles: vec![],
            permissions: vec!["admin".into()],
            session_id: 1,
            metadata: HashMap::new(),
        };
        let mut ctx = OperationContextBuilder::new(test_addr())
            .user_session(session)
            .build();
        let mut req = OperationRequest::new(OperationType::Delete, OperationTarget::System);
        let result = PipelineExecutor::authorize_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_calculate_backoff_retry_count_zero() {
        let delay = PipelineExecutor::calculate_backoff(0, 10, 1000);
        // attempt=0, exponent = saturating_sub(1) = 0, 5^0 = 1, delay = 10 * 1 = 10
        assert_eq!(delay, Duration::from_millis(10));
    }

    #[test]
    fn test_calculate_backoff_retry_count_one() {
        let delay = PipelineExecutor::calculate_backoff(1, 10, 1000);
        // attempt=1, exponent = 0, 5^0 = 1, delay = 10 * 1 = 10
        assert_eq!(delay, Duration::from_millis(10));
    }

    #[test]
    fn test_calculate_backoff_retry_count_two() {
        let delay = PipelineExecutor::calculate_backoff(2, 10, 1000);
        // attempt=2, exponent = 1, 5^1 = 5, delay = 10 * 5 = 50
        assert_eq!(delay, Duration::from_millis(50));
    }

    #[test]
    fn test_calculate_backoff_retry_count_three() {
        let delay = PipelineExecutor::calculate_backoff(3, 10, 1000);
        // attempt=3, exponent = 2, 5^2 = 25, delay = 10 * 25 = 250
        assert_eq!(delay, Duration::from_millis(250));
    }

    #[test]
    fn test_calculate_backoff_retry_count_four() {
        let delay = PipelineExecutor::calculate_backoff(4, 10, 1000);
        // attempt=4, exponent = 3, 5^3 = 125, delay = 10 * 125 = 1250, capped to 1000
        assert_eq!(delay, Duration::from_millis(1000));
    }

    #[test]
    fn test_calculate_backoff_caps_at_max() {
        let delay = PipelineExecutor::calculate_backoff(100, 100, 500);
        // Should be capped at 500ms
        assert_eq!(delay, Duration::from_millis(500));
    }

    #[test]
    fn test_build_error_response_maps_error_codes() {
        let ctx = OperationContextBuilder::new(test_addr()).build();
        let started_at = Instant::now();
        let test_cases = vec![
            (ErrorCode::ParseError, StatusCode::BadRequest),
            (ErrorCode::ValidationError, StatusCode::UnprocessableEntity),
            (ErrorCode::AuthorizationError, StatusCode::Forbidden),
            (ErrorCode::AuthenticationError, StatusCode::Unauthorized),
            (ErrorCode::NotFound, StatusCode::NotFound),
            (ErrorCode::Conflict, StatusCode::Conflict),
            (ErrorCode::RateLimited, StatusCode::TooManyRequests),
            (ErrorCode::CircuitBreakerOpen, StatusCode::ServiceUnavailable),
            (ErrorCode::DeadlineExceeded, StatusCode::DeadlineExceeded),
            (ErrorCode::Cancelled, StatusCode::Cancelled),
            (ErrorCode::PayloadTooLarge, StatusCode::RequestTooLarge),
            (ErrorCode::Unprocessable, StatusCode::UnprocessableEntity),
            (ErrorCode::InternalError, StatusCode::InternalError),
            (ErrorCode::ServiceUnavailable, StatusCode::ServiceUnavailable),
            (ErrorCode::NotImplemented, StatusCode::NotImplemented),
            (ErrorCode::InsufficientStorage, StatusCode::InsufficientStorage),
        ];

        for (code, expected_status) in test_cases {
            let err = PipelineError::new(code, "test message");
            let resp = PipelineExecutor::build_error_response(&ctx, started_at, err);
            assert_eq!(resp.status, expected_status, "mismatch for error code {:?}", code);
            assert!(!resp.success);
            assert!(resp.error.is_some());
            assert_eq!(resp.error.as_ref().unwrap().code, code);
        }
    }

    #[test]
    fn test_build_error_response_retryable_codes() {
        let ctx = OperationContextBuilder::new(test_addr()).build();
        let started_at = Instant::now();

        let retryable_codes = vec![
            ErrorCode::RateLimited,
            ErrorCode::CircuitBreakerOpen,
            ErrorCode::DeadlineExceeded,
            ErrorCode::ServiceUnavailable,
            ErrorCode::InternalError,
        ];
        let non_retryable_codes = vec![
            ErrorCode::ParseError,
            ErrorCode::ValidationError,
            ErrorCode::AuthorizationError,
            ErrorCode::NotFound,
            ErrorCode::Conflict,
            ErrorCode::Cancelled,
            ErrorCode::NotImplemented,
        ];

        for code in retryable_codes {
            let err = PipelineError::new(code, "msg");
            let resp = PipelineExecutor::build_error_response(&ctx, started_at, err);
            assert!(resp.error.as_ref().unwrap().retryable, "{:?} should be retryable", code);
        }

        for code in non_retryable_codes {
            let err = PipelineError::new(code, "msg");
            let resp = PipelineExecutor::build_error_response(&ctx, started_at, err);
            assert!(!resp.error.as_ref().unwrap().retryable, "{:?} should NOT be retryable", code);
        }
    }

    #[test]
    fn test_executor_status_after_creation() {
        let config = PipelineConfig::default();
        let executor = PipelineExecutor::new(config);
        let status = executor.status();
        assert!(status.is_running);
        assert!(!status.is_draining);
        assert_eq!(status.active_operations, 0);
        assert_eq!(status.total_operations, 0);
    }

    #[test]
    fn test_executor_metrics_after_creation() {
        let config = PipelineConfig::default();
        let executor = PipelineExecutor::new(config);
        let metrics = executor.metrics();
        let snapshot = metrics.snapshot();
        assert_eq!(snapshot.operations_total, 0);
        assert_eq!(snapshot.active_operations, 0);
        assert_eq!(snapshot.queue_depth, 0);
        assert_eq!(snapshot.circuit_opens, 0);
        assert_eq!(snapshot.circuit_rejected, 0);
    }

    #[test]
    fn test_register_and_unregister_middleware() {
        use crate::middleware::{MiddlewareRegistration, Middleware};
        use std::sync::Arc;

        struct DummyMiddleware;
        impl Middleware for DummyMiddleware {
            fn name(&self) -> &'static str { "dummy" }
            fn stage(&self) -> PipelineStage { PipelineStage::Parse }
            fn handle(&self, _ctx: &mut OperationContext, _req: &mut OperationRequest, next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult) -> PipelineResult {
                next(_ctx, _req)
            }
        }

        let config = PipelineConfig::default();
        let executor = PipelineExecutor::new(config);

        let reg = MiddlewareRegistration {
            name: "dummy".into(),
            stage: PipelineStage::Parse,
            order: 0,
            middleware: Arc::new(DummyMiddleware),
            enabled: true,
            config: HashMap::new(),
        };

        assert!(executor.register_middleware(reg).is_ok());
        assert!(executor.register_middleware(MiddlewareRegistration {
            name: "dummy".into(),
            stage: PipelineStage::Parse,
            order: 0,
            middleware: Arc::new(DummyMiddleware),
            enabled: true,
            config: HashMap::new(),
        }).is_err());

        assert!(executor.unregister_middleware("dummy").is_ok());
        assert!(executor.unregister_middleware("nonexistent").is_err());
    }

    #[test]
    fn test_execute_stage_expired_deadline() {
        let mut ctx = OperationContextBuilder::new(test_addr())
            .deadline(Instant::now() - Duration::from_secs(1))
            .build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        let result = PipelineExecutor::execute_stage(&mut ctx, &mut req);
        match result {
            PipelineResult::Error(e) => {
                assert_eq!(e.code, ErrorCode::DeadlineExceeded);
                assert_eq!(e.stage, PipelineStage::Execute);
            }
            _ => panic!("expected DeadlineExceeded"),
        }
    }

    #[test]
    fn test_execute_stage_sets_subsystem() {
        let mut ctx = OperationContextBuilder::new(test_addr())
            .deadline(Instant::now() + Duration::from_secs(10))
            .build();
        let mut req = OperationRequest::new(
            OperationType::Get,
            OperationTarget::Object { type_name: "user".into(), id: None },
        );
        let result = PipelineExecutor::execute_stage(&mut ctx, &mut req);
        assert_eq!(result, PipelineResult::Continue);
        assert_eq!(ctx.subsystem, SubsystemId::Storage);
    }
}
