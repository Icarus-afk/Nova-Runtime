use std::sync::Arc;
use async_graphql::*;
use parking_lot::RwLock;

use nova_config::Config;

use crate::context::AppContext;

pub fn internal_error(msg: impl Into<String>) -> FieldError {
    FieldError::new(msg.into())
}

pub fn not_found(msg: impl Into<String>) -> FieldError {
    FieldError::new(msg.into())
        .extend_with(|_, e| e.set("code", "NOT_FOUND"))
}

pub trait ContextExt {
    fn app(&self) -> Result<Arc<AppContext>, FieldError>;
    fn config(&self) -> Result<Arc<RwLock<Config>>, FieldError>;
}

impl ContextExt for Context<'_> {
    fn app(&self) -> Result<Arc<AppContext>, FieldError> {
        self.data::<Arc<AppContext>>().cloned()
            .map_err(|_| internal_error("AppContext not available"))
    }

    fn config(&self) -> Result<Arc<RwLock<Config>>, FieldError> {
        self.data::<Arc<RwLock<Config>>>().cloned()
            .map_err(|_| internal_error("Config not available"))
    }
}
