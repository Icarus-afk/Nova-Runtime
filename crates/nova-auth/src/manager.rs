use crate::brute_force::BruteForceDetector;
use crate::error::{AuthError, Result};
use crate::middleware::AuthMiddleware;
use crate::mfa::{MfaProvider, MfaStore};
use crate::password_policy::PasswordPolicyEngine;
use crate::providers::{AuthProvider, ProviderRegistry};
use crate::rbac::RbacEngine;
use crate::session::SessionManager;
use crate::types::*;
use nova_executor::middleware::{Middleware, MiddlewareRegistration};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Central manager for the authentication subsystem.
pub struct AuthManager {
    config: AuthConfig,
    session_manager: Arc<SessionManager>,
    provider_registry: Arc<parking_lot::RwLock<ProviderRegistry>>,
    rbac_engine: Arc<parking_lot::RwLock<RbacEngine>>,
    brute_force_detector: Arc<BruteForceDetector>,
    mfa_store: Arc<parking_lot::RwLock<MfaStore>>,
    password_policy: PasswordPolicyEngine,
    mfa_provider: MfaProvider,
}

impl AuthManager {
    pub fn new(config: AuthConfig) -> Self {
        let session_manager = Arc::new(SessionManager::new(config.clone()));
        let brute_force = Arc::new(BruteForceDetector::new(
            config.max_failed_attempts,
            300, // 5 minute window
            config.lockout_duration_secs,
        ));

        AuthManager {
            config: config.clone(),
            session_manager,
            provider_registry: Arc::new(parking_lot::RwLock::new(ProviderRegistry::new())),
            rbac_engine: Arc::new(parking_lot::RwLock::new(RbacEngine::new())),
            brute_force_detector: brute_force,
            mfa_store: Arc::new(parking_lot::RwLock::new(MfaStore::new())),
            password_policy: PasswordPolicyEngine::new(&config),
            mfa_provider: MfaProvider::new(&config.mfa_issuer, config.mfa_window),
        }
    }

    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    pub fn provider_registry(&self) -> &Arc<parking_lot::RwLock<ProviderRegistry>> {
        &self.provider_registry
    }

    pub fn rbac_engine(&self) -> &Arc<parking_lot::RwLock<RbacEngine>> {
        &self.rbac_engine
    }

    pub fn brute_force_detector(&self) -> &Arc<BruteForceDetector> {
        &self.brute_force_detector
    }

    pub fn mfa_store(&self) -> &Arc<parking_lot::RwLock<MfaStore>> {
        &self.mfa_store
    }

    pub fn password_policy(&self) -> &PasswordPolicyEngine {
        &self.password_policy
    }

    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    /// Register an authentication provider.
    pub fn register_provider(&self, provider: Arc<dyn AuthProvider>) -> Result<()> {
        self.provider_registry.write().register(provider)
    }

    /// Authenticate a user with the given credentials using the specified provider.
    pub async fn authenticate(
        &self,
        provider_name: &str,
        credentials: HashMap<String, String>,
    ) -> std::result::Result<AuthResult, AuthError> {
        let provider = {
            let registry = self.provider_registry.read();
            registry.get(provider_name)
                .ok_or_else(|| AuthError::ProviderNotFound(provider_name.to_string()))?
                .clone()
        };

        // Check brute-force lockout
        let identifier = credentials.get("username")
            .or_else(|| credentials.get("api_key"))
            .map(|s| s.as_str())
            .unwrap_or("unknown");

        if self.config.enable_brute_force_detection && self.brute_force_detector.is_locked(identifier) {
            let remaining = self.brute_force_detector.remaining_lockout_ms(identifier);
            return Err(AuthError::RateLimited(remaining));
        }

        // Perform authentication
        let result = provider.authenticate(&credentials).await?;

        if result.success {
            self.brute_force_detector.record_success(identifier);

            // Create session
            let user_id = result.user_id.unwrap_or_else(Uuid::new_v4);
            let username = result.username.as_deref().unwrap_or("unknown");
            let mut session = self.session_manager.create_session(user_id, username);

            session.roles = result.roles.clone();
            session.permissions = result.permissions.clone();

            if result.mfa_required {
                session.mfa_verified = false;
            }

            Ok(AuthResult {
                success: true,
                session: Some(session),
                user_id: Some(user_id),
                username: Some(username.to_string()),
                roles: result.roles,
                permissions: result.permissions,
                mfa_required: result.mfa_required,
                error_message: None,
                retry_after_ms: None,
            })
        } else {
            self.brute_force_detector.record_failure(identifier);
            Err(AuthError::AuthenticationFailed(
                result.error_message.unwrap_or_else(|| "authentication failed".to_string()),
            ))
        }
    }

    /// Validate a session token and return the session.
    pub fn validate_session(&self, token: &str) -> Result<Session> {
        let session = self.session_manager.get_session(token)?;
        self.session_manager.touch_session(token)?;
        Ok(session)
    }

    /// Revoke a session.
    pub fn revoke_session(&self, token: &str) -> Result<()> {
        self.session_manager.revoke_session(token)
    }

    /// Revoke all sessions for a user.
    pub fn revoke_user_sessions(&self, user_id: Uuid) -> u32 {
        self.session_manager.revoke_user_sessions(user_id)
    }

    /// Check if a user has a specific permission.
    pub fn check_permission(&self, user_id: &Uuid, permission: &str) -> bool {
        self.rbac_engine.read().has_permission(user_id, permission)
    }

    /// Create the auth middleware for pipeline registration.
    pub fn create_middleware(&self) -> AuthMiddleware {
        AuthMiddleware::new(
            self.session_manager.clone(),
            self.config.clone(),
        )
    }

    /// Create a middleware registration suitable for the PipelineExecutor.
    pub fn create_middleware_registration(&self, order: u32) -> MiddlewareRegistration {
        let middleware = self.create_middleware();
        MiddlewareRegistration {
            name: middleware.name().to_string(),
            stage: middleware.stage(),
            order,
            middleware: Arc::new(middleware),
            enabled: true,
            config: HashMap::new(),
        }
    }

    /// Enable MFA for a user.
    pub fn enable_mfa(&self, user_id: Uuid) -> String {
        let secret = MfaProvider::generate_secret();
        let uri = self.mfa_provider.generate_otpauth_uri("user", &secret);
        self.mfa_store.write().enable_mfa(user_id, secret);
        uri
    }

    /// Verify a MFA code for a user.
    pub fn verify_mfa(&self, user_id: &Uuid, code: &str) -> bool {
        self.mfa_store.write().verify_code(user_id, code, self.config.mfa_window)
    }

    /// Check if a user has MFA enabled.
    pub fn has_mfa(&self, user_id: &Uuid) -> bool {
        self.mfa_store.read().has_mfa(user_id)
    }

    /// Define a role in the RBAC engine.
    pub fn define_role(&self, role: Role) {
        self.rbac_engine.write().define_role(role);
    }

    /// Assign a role to a user.
    pub fn assign_role(&self, user_id: Uuid, role_name: &str) -> Result<()> {
        self.rbac_engine.write().assign_role(user_id, role_name)
            .map_err(|e| AuthError::InvalidArgument(e))
    }

    /// Clean up expired sessions.
    pub fn cleanup_sessions(&self) -> u64 {
        self.session_manager.cleanup_expired()
    }

    /// Clean up stale brute-force entries.
    pub fn cleanup_brute_force(&self) {
        self.brute_force_detector.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::PasswordProvider;

    #[test]
    fn test_auth_manager_new() {
        let manager = AuthManager::new(AuthConfig::default());
        assert_eq!(manager.session_manager.active_sessions(), 0);
    }

    #[test]
    fn test_auth_manager_register_provider() {
        let manager = AuthManager::new(AuthConfig::default());
        let provider = Arc::new(PasswordProvider::new("local", AuthConfig::default()));
        assert!(manager.register_provider(provider).is_ok());
    }

    #[test]
    fn test_auth_manager_validate_session() {
        let manager = AuthManager::new(AuthConfig::default());
        let session = manager.session_manager.create_session(Uuid::new_v4(), "test");
        let validated = manager.validate_session(&session.token).unwrap();
        assert_eq!(validated.user_id, session.user_id);
    }

    #[test]
    fn test_auth_manager_revoke_session() {
        let manager = AuthManager::new(AuthConfig::default());
        let session = manager.session_manager.create_session(Uuid::new_v4(), "test");
        assert!(manager.revoke_session(&session.token).is_ok());
        assert!(manager.validate_session(&session.token).is_err());
    }

    #[test]
    fn test_auth_manager_revoke_user_sessions() {
        let manager = AuthManager::new(AuthConfig::default());
        let user_id = Uuid::new_v4();
        manager.session_manager.create_session(user_id, "test");
        manager.session_manager.create_session(user_id, "test");
        assert_eq!(manager.revoke_user_sessions(user_id), 2);
    }

    #[test]
    fn test_auth_manager_mfa() {
        let manager = AuthManager::new(AuthConfig::default());
        let user_id = Uuid::new_v4();
        assert!(!manager.has_mfa(&user_id));
        let _uri = manager.enable_mfa(user_id);
        assert!(manager.has_mfa(&user_id));
    }

    #[test]
    fn test_auth_manager_rbac() {
        let manager = AuthManager::new(AuthConfig::default());
        let user_id = Uuid::new_v4();
        manager.define_role(Role {
            name: "admin".into(),
            description: "Admin".into(),
            permissions: vec!["*:*".into()],
            created_at: 0,
        });
        assert!(manager.assign_role(user_id, "admin").is_ok());
        assert!(manager.check_permission(&user_id, "read:anything"));
    }

    #[test]
    fn test_auth_manager_cleanup_sessions() {
        let manager = AuthManager::new(AuthConfig::default());
        let session = manager.session_manager.create_session(Uuid::new_v4(), "test");
        // Directly expire the session
        if let Some(mut s) = manager.session_manager.sessions.get_mut(&session.token) {
            s.expires_at = 0;
        }
        let cleaned = manager.cleanup_sessions();
        assert_eq!(cleaned, 1);
    }

    #[test]
    fn test_auth_manager_middleware_creation() {
        let manager = AuthManager::new(AuthConfig::default());
        let reg = manager.create_middleware_registration(0);
        assert_eq!(reg.name, "nova-auth");
        assert!(reg.enabled);
    }

    #[test]
    fn test_auth_manager_password_policy() {
        let manager = AuthManager::new(AuthConfig::default());
        assert!(manager.password_policy().validate("ValidPass1").is_ok());
        assert!(manager.password_policy().validate("short").is_err());
    }
}
