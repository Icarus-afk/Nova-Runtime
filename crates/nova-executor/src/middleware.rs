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
