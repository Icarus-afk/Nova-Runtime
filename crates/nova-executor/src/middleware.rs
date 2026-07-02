use crate::types::*;
use std::sync::Arc;

pub type StageFn = Arc<dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult + Send + Sync>;

pub trait Middleware: Send + Sync {
    fn name(&self) -> &'static str;
    fn stage(&self) -> PipelineStage;
    fn handle(
        &self,
        ctx: &mut OperationContext,
        req: &mut OperationRequest,
        next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
    ) -> PipelineResult;
}

pub struct MiddlewareRegistration {
    pub name: String,
    pub stage: PipelineStage,
    pub order: u32,
    pub middleware: Arc<dyn Middleware>,
    pub enabled: bool,
    pub config: std::collections::HashMap<String, serde_json::Value>,
}

pub struct MiddlewareChain {
    middleware: Vec<MiddlewareRegistration>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self { middleware: Vec::new() }
    }

    pub fn register(&mut self, registration: MiddlewareRegistration) -> Result<(), String> {
        if self.middleware.iter().any(|m| m.name == registration.name) {
            return Err(format!("Middleware '{}' already registered", registration.name));
        }
        self.middleware.push(registration);
        Ok(())
    }

    pub fn unregister(&mut self, name: &str) -> Result<(), String> {
        let idx = self.middleware.iter().position(|m| m.name == name)
            .ok_or_else(|| format!("Middleware '{}' not found", name))?;
        self.middleware.remove(idx);
        Ok(())
    }

    pub fn enable(&mut self, name: &str) -> Result<(), String> {
        self.middleware.iter_mut()
            .find(|m| m.name == name)
            .map(|m| m.enabled = true)
            .ok_or_else(|| format!("Middleware '{}' not found", name))
    }

    pub fn disable(&mut self, name: &str) -> Result<(), String> {
        self.middleware.iter_mut()
            .find(|m| m.name == name)
            .map(|m| m.enabled = false)
            .ok_or_else(|| format!("Middleware '{}' not found", name))
    }

    pub fn for_stage(&self, stage: PipelineStage) -> Vec<&MiddlewareRegistration> {
        let mut chain: Vec<_> = self.middleware.iter()
            .filter(|m| m.stage == stage && m.enabled)
            .collect();
        chain.sort_by_key(|m| m.order);
        chain
    }

    pub fn run_chain(
        &self,
        stage: PipelineStage,
        ctx: &mut OperationContext,
        req: &mut OperationRequest,
        stage_fn: StageFn,
    ) -> PipelineResult {
        let middleware_list = self.for_stage(stage);
        if middleware_list.is_empty() {
            return (stage_fn)(ctx, req);
        }

        let mut composed: Box<dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult> =
            Box::new(move |ctx, req| (stage_fn)(ctx, req));

        for mw in middleware_list.into_iter().rev() {
            let mw = mw.middleware.clone();
            let prev = std::mem::replace(&mut composed, Box::new(|_, _| unreachable!()));
            composed = Box::new(move |ctx, req| {
                mw.handle(ctx, req, &prev)
            });
        }

        (composed)(ctx, req)
    }

    pub fn len(&self) -> usize {
        self.middleware.len()
    }

    pub fn is_empty(&self) -> bool {
        self.middleware.is_empty()
    }
}

impl Default for MiddlewareChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::OperationContextBuilder;
    use crate::OperationRequest;
    use crate::OperationType;
    use crate::OperationTarget;
    use std::collections::HashMap;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct TestMiddleware {
        name: &'static str,
        stage: PipelineStage,
        order: u32,
    }

    impl Middleware for TestMiddleware {
        fn name(&self) -> &'static str { self.name }
        fn stage(&self) -> PipelineStage { self.stage }
        fn handle(
            &self,
            _ctx: &mut OperationContext,
            _req: &mut OperationRequest,
            next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
        ) -> PipelineResult {
            next(_ctx, _req)
        }
    }

    struct TrackingMiddleware {
        name: &'static str,
        stage: PipelineStage,
        order: u32,
        call_count: Arc<AtomicU32>,
    }

    impl Middleware for TrackingMiddleware {
        fn name(&self) -> &'static str { self.name }
        fn stage(&self) -> PipelineStage { self.stage }
        fn handle(
            &self,
            ctx: &mut OperationContext,
            req: &mut OperationRequest,
            next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
        ) -> PipelineResult {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            next(ctx, req)
        }
    }

    struct ShortCircuitMiddleware;

    impl Middleware for ShortCircuitMiddleware {
        fn name(&self) -> &'static str { "short_circuit" }
        fn stage(&self) -> PipelineStage { PipelineStage::Parse }
        fn handle(
            &self,
            _ctx: &mut OperationContext,
            _req: &mut OperationRequest,
            _next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
        ) -> PipelineResult {
            PipelineResult::ShortCircuit(OperationResponse::ok(serde_json::Value::Null))
        }
    }

    fn test_addr() -> SocketAddr { "127.0.0.1:8080".parse().unwrap() }

    fn make_registration(name: &'static str, stage: PipelineStage, order: u32, enabled: bool) -> MiddlewareRegistration {
        MiddlewareRegistration {
            name: name.into(),
            stage,
            order,
            middleware: Arc::new(TestMiddleware { name, stage, order }),
            enabled,
            config: HashMap::new(),
        }
    }

    #[test]
    fn test_middleware_chain_new_is_empty() {
        let chain = MiddlewareChain::new();
        assert_eq!(chain.len(), 0);
        assert!(chain.is_empty());
    }

    #[test]
    fn test_register_adds_middleware() {
        let mut chain = MiddlewareChain::new();
        let reg = make_registration("test", PipelineStage::Parse, 0, true);
        assert!(chain.register(reg).is_ok());
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
    }

    #[test]
    fn test_register_duplicate_name_returns_error() {
        let mut chain = MiddlewareChain::new();
        let reg1 = make_registration("dup", PipelineStage::Parse, 0, true);
        let reg2 = make_registration("dup", PipelineStage::Validate, 1, true);
        assert!(chain.register(reg1).is_ok());
        let err = chain.register(reg2).unwrap_err();
        assert!(err.contains("dup"));
        assert_eq!(chain.len(), 1);
    }

    #[test]
    fn test_unregister_removes_middleware() {
        let mut chain = MiddlewareChain::new();
        let reg = make_registration("remove_me", PipelineStage::Parse, 0, true);
        chain.register(reg).unwrap();
        assert!(chain.unregister("remove_me").is_ok());
        assert_eq!(chain.len(), 0);
    }

    #[test]
    fn test_unregister_missing_returns_error() {
        let mut chain = MiddlewareChain::new();
        let err = chain.unregister("nonexistent").unwrap_err();
        assert!(err.contains("nonexistent"));
    }

    #[test]
    fn test_enable_toggles_middleware() {
        let mut chain = MiddlewareChain::new();
        let reg = make_registration("toggle", PipelineStage::Parse, 0, false);
        chain.register(reg).unwrap();

        assert!(chain.enable("toggle").is_ok());
        let list = chain.for_stage(PipelineStage::Parse);
        assert_eq!(list.len(), 1);

        assert!(chain.disable("toggle").is_ok());
        let list = chain.for_stage(PipelineStage::Parse);
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_enable_missing_returns_error() {
        let mut chain = MiddlewareChain::new();
        assert!(chain.enable("nope").is_err());
        assert!(chain.disable("nope").is_err());
    }

    #[test]
    fn test_run_chain_executes_middleware_in_order() {
        let mut chain = MiddlewareChain::new();
        let count = Arc::new(AtomicU32::new(0));

        chain.register(MiddlewareRegistration {
            name: "first".into(),
            stage: PipelineStage::Parse,
            order: 1,
            middleware: Arc::new(TrackingMiddleware {
                name: "first",
                stage: PipelineStage::Parse,
                order: 1,
                call_count: count.clone(),
            }),
            enabled: true,
            config: HashMap::new(),
        }).unwrap();

        chain.register(MiddlewareRegistration {
            name: "second".into(),
            stage: PipelineStage::Parse,
            order: 2,
            middleware: Arc::new(TrackingMiddleware {
                name: "second",
                stage: PipelineStage::Parse,
                order: 2,
                call_count: count.clone(),
            }),
            enabled: true,
            config: HashMap::new(),
        }).unwrap();

        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        let stage_fn: StageFn = Arc::new(|_ctx, _req| PipelineResult::Continue);
        let result = chain.run_chain(PipelineStage::Parse, &mut ctx, &mut req, stage_fn);

        assert_eq!(result, PipelineResult::Continue);
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_run_chain_short_circuit_stops_execution() {
        let mut chain = MiddlewareChain::new();
        let count = Arc::new(AtomicU32::new(0));

        chain.register(MiddlewareRegistration {
            name: "short".into(),
            stage: PipelineStage::Parse,
            order: 1,
            middleware: Arc::new(ShortCircuitMiddleware),
            enabled: true,
            config: HashMap::new(),
        }).unwrap();

        chain.register(MiddlewareRegistration {
            name: "after_short".into(),
            stage: PipelineStage::Parse,
            order: 2,
            middleware: Arc::new(TrackingMiddleware {
                name: "after_short",
                stage: PipelineStage::Parse,
                order: 2,
                call_count: count.clone(),
            }),
            enabled: true,
            config: HashMap::new(),
        }).unwrap();

        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        let stage_fn: StageFn = Arc::new(|_ctx, _req| PipelineResult::Continue);
        let result = chain.run_chain(PipelineStage::Parse, &mut ctx, &mut req, stage_fn);

        match result {
            PipelineResult::ShortCircuit(_) => {}
            _ => panic!("expected ShortCircuit"),
        }
        // The second middleware should NOT be called because of short circuit
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_run_chain_with_no_middleware_calls_stage_fn() {
        let chain = MiddlewareChain::new();
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        let stage_fn: StageFn = Arc::new(|_ctx, _req| PipelineResult::Continue);
        let result = chain.run_chain(PipelineStage::Parse, &mut ctx, &mut req, stage_fn);
        assert_eq!(result, PipelineResult::Continue);
    }

    #[test]
    fn test_for_stage_returns_only_enabled_middleware_for_stage() {
        let mut chain = MiddlewareChain::new();
        chain.register(make_registration("parse_mw", PipelineStage::Parse, 0, true)).unwrap();
        chain.register(make_registration("validate_mw", PipelineStage::Validate, 0, true)).unwrap();
        chain.register(make_registration("disabled_parse", PipelineStage::Parse, 1, false)).unwrap();

        let parse_mw = chain.for_stage(PipelineStage::Parse);
        assert_eq!(parse_mw.len(), 1);
        assert_eq!(parse_mw[0].name, "parse_mw");

        let validate_mw = chain.for_stage(PipelineStage::Validate);
        assert_eq!(validate_mw.len(), 1);
    }

    #[test]
    fn test_for_stage_returns_ordered_by_order_field() {
        let mut chain = MiddlewareChain::new();
        chain.register(make_registration("z_last", PipelineStage::Parse, 10, true)).unwrap();
        chain.register(make_registration("a_first", PipelineStage::Parse, 1, true)).unwrap();
        chain.register(make_registration("m_middle", PipelineStage::Parse, 5, true)).unwrap();

        let list = chain.for_stage(PipelineStage::Parse);
        assert_eq!(list.len(), 3);
        assert_eq!(list[0].name, "a_first");
        assert_eq!(list[1].name, "m_middle");
        assert_eq!(list[2].name, "z_last");
    }

    #[test]
    fn test_run_chain_disabled_middleware_not_executed() {
        let mut chain = MiddlewareChain::new();
        let count = Arc::new(AtomicU32::new(0));

        chain.register(MiddlewareRegistration {
            name: "disabled".into(),
            stage: PipelineStage::Parse,
            order: 1,
            middleware: Arc::new(TrackingMiddleware {
                name: "disabled",
                stage: PipelineStage::Parse,
                order: 1,
                call_count: count.clone(),
            }),
            enabled: false,
            config: HashMap::new(),
        }).unwrap();

        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        let stage_fn: StageFn = Arc::new(|_ctx, _req| PipelineResult::Continue);
        let _ = chain.run_chain(PipelineStage::Parse, &mut ctx, &mut req, stage_fn);

        assert_eq!(count.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_cannot_register_same_name_twice() {
        let mut chain = MiddlewareChain::new();
        let reg1 = make_registration("unique", PipelineStage::Parse, 0, true);
        let reg2 = make_registration("unique", PipelineStage::Validate, 1, true);
        assert!(chain.register(reg1).is_ok());
        assert!(chain.register(reg2).is_err());
    }
}
