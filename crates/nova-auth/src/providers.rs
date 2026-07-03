use crate::error::{AuthError, Result};
use crate::types::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

/// Abstract authentication provider trait.
/// Each provider handles one credential type.
#[async_trait]
pub trait AuthProvider: Send + Sync {
    fn name(&self) -> &str;
    fn credential_type(&self) -> CredentialType;

    /// Authenticate with the given credentials.
    async fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<AuthResult>;

    /// Validate the credential strength (e.g., password policy).
    fn validate_credential(&self, credential: &str) -> Result<()>;

    /// Create a credential record.
    async fn create_credential(&self, credential: Credential) -> Result<()>;
}

/// Password-based authentication provider.
pub struct PasswordProvider {
    name: String,
    config: AuthConfig,
}

impl PasswordProvider {
    pub fn new(name: &str, config: AuthConfig) -> Self {
        PasswordProvider {
            name: name.to_string(),
            config,
        }
    }

    /// Hash a password using a simple SHA-256 + salt approach.
    /// In production, this would use argon2 or bcrypt.
    fn hash_password(password: &str, salt: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(salt.as_bytes());
        hasher.update(password.as_bytes());
        let result = hasher.finalize();
        format!("sha256:{}:{}", salt, hex::encode(result))
    }

    fn verify_password(password: &str, stored_hash: &str) -> bool {
        if let Some(hash_body) = stored_hash.strip_prefix("sha256:") {
            if let Some(salt_end) = hash_body.find(':') {
                let salt = &hash_body[..salt_end];
                let expected = &hash_body[salt_end + 1..];
                let computed = Self::hash_password(password, salt);
                let computed_hash = computed.strip_prefix("sha256:").unwrap_or(&computed);
                // Extract just the hex part
                if let Some(computed_hex_end) = computed_hash.find(':') {
                    let computed_hex = &computed_hash[computed_hex_end + 1..];
                    return computed_hex == expected;
                }
            }
        }
        false
    }
}

#[async_trait]
impl AuthProvider for PasswordProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn credential_type(&self) -> CredentialType {
        CredentialType::Password
    }

    async fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<AuthResult> {
        let username = credentials.get("username")
            .ok_or_else(|| AuthError::InvalidCredentials("missing username".into()))?;
        let password = credentials.get("password")
            .ok_or_else(|| AuthError::InvalidCredentials("missing password".into()))?;

        // Validate password format
        let _ = validate_password(password, &self.config)
            .map_err(|errors| AuthError::PasswordPolicyViolation(errors.join("; ")))?;

        // In a real implementation, we'd look up the stored credential from the backend.
        // For now, return a simple auth result.
        Ok(AuthResult {
            success: true,
            session: None,
            user_id: None,
            username: Some(username.clone()),
            roles: vec!["user".to_string()],
            permissions: vec!["read".to_string()],
            mfa_required: false,
            error_message: None,
            retry_after_ms: None,
        })
    }

    fn validate_credential(&self, credential: &str) -> Result<()> {
        let _ = validate_password(credential, &self.config)
            .map_err(|errors| AuthError::PasswordPolicyViolation(errors.join("; ")))?;
        Ok(())
    }

    async fn create_credential(&self, credential: Credential) -> Result<()> {
        // Stub: in production, store in StorageEngine via a CredentialBackend
        tracing::info!("Creating credential for user {} via provider {}", credential.user_id, self.name);
        Ok(())
    }
}

/// API key authentication provider.
pub struct ApiKeyProvider {
    name: String,
}

impl ApiKeyProvider {
    pub fn new(name: &str) -> Self {
        ApiKeyProvider {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl AuthProvider for ApiKeyProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn credential_type(&self) -> CredentialType {
        CredentialType::ApiKey
    }

    async fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<AuthResult> {
        let api_key = credentials.get("api_key")
            .ok_or_else(|| AuthError::InvalidCredentials("missing api_key".into()))?;

        // Stub: validate API key against stored keys
        if api_key.len() < 16 {
            return Err(AuthError::InvalidCredentials("invalid API key".into()));
        }

        Ok(AuthResult {
            success: true,
            session: None,
            user_id: None,
            username: Some("api-user".to_string()),
            roles: vec!["api".to_string()],
            permissions: vec!["read".to_string(), "write".to_string()],
            mfa_required: false,
            error_message: None,
            retry_after_ms: None,
        })
    }

    fn validate_credential(&self, credential: &str) -> Result<()> {
        if credential.len() < 16 {
            return Err(AuthError::InvalidCredentials("API key too short".into()));
        }
        Ok(())
    }

    async fn create_credential(&self, credential: Credential) -> Result<()> {
        tracing::info!("Creating API key credential for user {}", credential.user_id);
        Ok(())
    }
}

/// JWT authentication provider.
pub struct JwtProvider {
    name: String,
}

impl JwtProvider {
    pub fn new(name: &str) -> Self {
        JwtProvider {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl AuthProvider for JwtProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn credential_type(&self) -> CredentialType {
        CredentialType::Jwt
    }

    async fn authenticate(&self, credentials: &HashMap<String, String>) -> Result<AuthResult> {
        let token = credentials.get("token")
            .ok_or_else(|| AuthError::InvalidCredentials("missing token".into()))?;

        // Stub: JWT validation would verify signature, expiry, claims
        if token.is_empty() {
            return Err(AuthError::TokenInvalid("empty token".into()));
        }

        Ok(AuthResult {
            success: true,
            session: None,
            user_id: None,
            username: Some("jwt-user".to_string()),
            roles: vec!["user".to_string()],
            permissions: vec!["read".to_string()],
            mfa_required: false,
            error_message: None,
            retry_after_ms: None,
        })
    }

    fn validate_credential(&self, credential: &str) -> Result<()> {
        if credential.is_empty() {
            return Err(AuthError::TokenInvalid("empty token".into()));
        }
        Ok(())
    }

    async fn create_credential(&self, credential: Credential) -> Result<()> {
        tracing::info!("Creating JWT credential for user {}", credential.user_id);
        Ok(())
    }
}

/// Provider registry — maps credential types to providers.
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn AuthProvider>>,
    provider_by_name: HashMap<String, usize>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        ProviderRegistry {
            providers: Vec::new(),
            provider_by_name: HashMap::new(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn AuthProvider>) -> Result<()> {
        let name = provider.name().to_string();
        if self.provider_by_name.contains_key(&name) {
            return Err(AuthError::ProviderNotFound(format!("Provider '{}' already registered", name)));
        }
        self.provider_by_name.insert(name, self.providers.len());
        self.providers.push(provider);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn AuthProvider>> {
        self.provider_by_name.get(name).map(|idx| &self.providers[*idx])
    }

    pub fn get_by_type(&self, credential_type: CredentialType) -> Option<&Arc<dyn AuthProvider>> {
        self.providers.iter().find(|p| p.credential_type() == credential_type)
    }

    pub fn providers(&self) -> &[Arc<dyn AuthProvider>] {
        &self.providers
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_provider_new() {
        let config = AuthConfig::default();
        let provider = PasswordProvider::new("local", config);
        assert_eq!(provider.name(), "local");
        assert_eq!(provider.credential_type(), CredentialType::Password);
    }

    #[test]
    fn test_password_hashing_and_verification() {
        let password = "MySecurePassword123!";
        let salt = "random_salt_value";
        let hash = PasswordProvider::hash_password(password, salt);
        assert!(hash.starts_with("sha256:"));
        assert!(PasswordProvider::verify_password(password, &hash));
        assert!(!PasswordProvider::verify_password("wrong_password", &hash));
    }

    #[test]
    fn test_api_key_provider() {
        let provider = ApiKeyProvider::new("api-keys");
        assert_eq!(provider.name(), "api-keys");
        assert_eq!(provider.credential_type(), CredentialType::ApiKey);
    }

    #[test]
    fn test_jwt_provider() {
        let provider = JwtProvider::new("jwt");
        assert_eq!(provider.name(), "jwt");
        assert_eq!(provider.credential_type(), CredentialType::Jwt);
    }

    #[test]
    fn test_provider_registry() {
        let mut registry = ProviderRegistry::new();
        let password_provider = Arc::new(PasswordProvider::new("local", AuthConfig::default()));
        let api_key_provider = Arc::new(ApiKeyProvider::new("api-keys"));

        assert!(registry.register(password_provider).is_ok());
        assert!(registry.register(api_key_provider).is_ok());

        assert!(registry.get("local").is_some());
        assert!(registry.get("api-keys").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_provider_registry_duplicate() {
        let mut registry = ProviderRegistry::new();
        let provider = Arc::new(PasswordProvider::new("local", AuthConfig::default()));
        assert!(registry.register(provider.clone()).is_ok());
        assert!(registry.register(provider).is_err());
    }

    #[test]
    fn test_provider_registry_get_by_type() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(PasswordProvider::new("local", AuthConfig::default()))).unwrap();
        registry.register(Arc::new(ApiKeyProvider::new("api"))).unwrap();

        assert!(registry.get_by_type(CredentialType::Password).is_some());
        assert!(registry.get_by_type(CredentialType::ApiKey).is_some());
        assert!(registry.get_by_type(CredentialType::Jwt).is_none());
    }

    #[test]
    fn test_password_provider_validate_credential() {
        let config = AuthConfig::default();
        let provider = PasswordProvider::new("local", config);
        assert!(provider.validate_credential("ValidPass1!").is_ok());
        assert!(provider.validate_credential("short").is_err());
    }
}
