use crate::error::{AuthError, Result};
use crate::session::SessionManager;
use crate::types::*;
use nova_executor::middleware::Middleware;
use nova_executor::types::*;
use std::collections::HashMap;
use std::sync::Arc;

/// Authentication middleware for the pipeline.
/// Validates sessions and sets user context on requests.
pub struct AuthMiddleware {
    session_manager: Arc<SessionManager>,
    config: AuthConfig,
}

impl AuthMiddleware {
    pub fn new(session_manager: Arc<SessionManager>, config: AuthConfig) -> Self {
        AuthMiddleware {
            session_manager,
            config,
        }
    }

    /// Extract a bearer token from the request params.
    fn extract_token(req: &OperationRequest) -> Option<String> {
        // Check Authorization header-style param
        if let Some(auth) = req.params.get("authorization") {
            if let Some(val) = auth.as_str() {
                if let Some(token) = val.strip_prefix("Bearer ") {
                    return Some(token.to_string());
                }
                return Some(val.to_string());
            }
        }

        // Check token param directly
        if let Some(token) = req.params.get("token") {
            if let Some(val) = token.as_str() {
                return Some(val.to_string());
            }
        }

        // Check session_id param
        if let Some(sid) = req.params.get("session_id") {
            if let Some(val) = sid.as_str() {
                return Some(val.to_string());
            }
        }

        None
    }
}

impl Middleware for AuthMiddleware {
    fn name(&self) -> &'static str {
        "nova-auth"
    }

    fn stage(&self) -> PipelineStage {
        PipelineStage::Authorize
    }

    fn handle(
        &self,
        ctx: &mut OperationContext,
        req: &mut OperationRequest,
        next: &dyn Fn(&mut OperationContext, &mut OperationRequest) -> PipelineResult,
    ) -> PipelineResult {
        // Skip auth for non-protected targets
        match &req.target {
            OperationTarget::Auth { .. } | OperationTarget::Admin { .. } => {}
            OperationTarget::System => {
                // System operations bypass auth
                return next(ctx, req);
            }
            _ => {
                // All other targets require authentication
                let token = match Self::extract_token(req) {
                    Some(t) => t,
                    None => {
                        return PipelineResult::ShortCircuit(
                            OperationResponse::error(ErrorCode::AuthenticationError, "missing authentication token")
                        );
                    }
                };

                // Validate session
                match self.session_manager.get_session(&token) {
                    Ok(session) => {
                        // Set user context
                        ctx.user_session = Some(UserSession {
                            user_id: session.user_id.as_u128(),
                            username: session.username,
                            roles: session.roles.clone(),
                            permissions: session.permissions.clone(),
                            session_id: session.id.as_u128(),
                            metadata: session.metadata.clone(),
                        });
                        ctx.metadata.insert("auth_method".into(), "session".into());
                    }
                    Err(_) => {
                        return PipelineResult::ShortCircuit(
                            OperationResponse::error(ErrorCode::AuthenticationError, "invalid or expired session")
                        );
                    }
                }
            }
        }

        next(ctx, req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nova_executor::context::OperationContextBuilder;
    use std::net::SocketAddr;

    fn test_addr() -> SocketAddr {
        "127.0.0.1:8080".parse().unwrap()
    }

    #[test]
    fn test_auth_middleware_name() {
        let session_mgr = Arc::new(SessionManager::new(AuthConfig::default()));
        let middleware = AuthMiddleware::new(session_mgr, AuthConfig::default());
        assert_eq!(middleware.name(), "nova-auth");
        assert_eq!(middleware.stage(), PipelineStage::Authorize);
    }

    #[test]
    fn test_extract_token_from_authorization_header() {
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        req.params.insert("authorization".into(), serde_json::Value::String("Bearer test_token".into()));
        let token = AuthMiddleware::extract_token(&req);
        assert_eq!(token, Some("test_token".to_string()));
    }

    #[test]
    fn test_extract_token_from_token_param() {
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        req.params.insert("token".into(), serde_json::Value::String("direct_token".into()));
        let token = AuthMiddleware::extract_token(&req);
        assert_eq!(token, Some("direct_token".to_string()));
    }

    #[test]
    fn test_extract_token_missing() {
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);
        assert!(AuthMiddleware::extract_token(&req).is_none());
    }

    #[test]
    fn test_system_target_bypasses_auth() {
        let session_mgr = Arc::new(SessionManager::new(AuthConfig::default()));
        let middleware = AuthMiddleware::new(session_mgr, AuthConfig::default());
        let mut ctx = OperationContextBuilder::new(test_addr()).build();
        let mut req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        let called = std::sync::atomic::AtomicBool::new(false);
        let result = middleware.handle(&mut ctx, &mut req, &|_, _| {
            called.store(true, std::sync::atomic::Ordering::SeqCst);
            PipelineResult::Continue
        });

        assert_eq!(result, PipelineResult::Continue);
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
