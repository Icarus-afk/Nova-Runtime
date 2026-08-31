use crate::error::{AuthError, Result};
use crate::types::UserRecord;
use crate::types::*;
use async_trait::async_trait;
use dashmap::DashMap;
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
    users: Arc<DashMap<String, UserRecord>>,
}

impl PasswordProvider {
    pub fn new(name: &str, config: AuthConfig, users: Arc<DashMap<String, UserRecord>>) -> Self {
        PasswordProvider {
            name: name.to_string(),
            config,
            users,
        }
    }

    /// Hash a password with bcrypt.
    pub fn hash_password(password: &str, cost: u32) -> String {
        use bcrypt::{DEFAULT_COST, hash};
        let actual_cost = if cost == 0 { DEFAULT_COST } else { cost };
        hash(password, actual_cost).unwrap_or_else(|_| panic!("bcrypt hash failed"))
    }

    pub fn verify_password(password: &str, stored_hash: &str) -> bool {
        bcrypt::verify(password, stored_hash).unwrap_or(false)
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
        let username = credentials
            .get("username")
            .ok_or_else(|| AuthError::InvalidCredentials("missing username".into()))?;
        let password = credentials
            .get("password")
            .ok_or_else(|| AuthError::InvalidCredentials("missing password".into()))?;

        let user = self
            .users
            .get(username)
            .ok_or_else(|| AuthError::InvalidCredentials("invalid username or password".into()))?;

        if !Self::verify_password(password, &user.password_hash) {
            return Err(AuthError::InvalidCredentials(
                "invalid username or password".into(),
            ));
        }

        Ok(AuthResult {
            success: true,
            session: None,
            user_id: Some(user.id),
            username: Some(username.clone()),
            roles: user.roles.clone(),
            permissions: vec!["*".to_string()],
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
        let user = UserRecord {
            id: credential.user_id,
            username: credential.identifier.clone(),
            password_hash: credential.secret_hash,
            roles: vec!["user".to_string()],
            created_at: credential.created_at,
            updated_at: credential.created_at,
        };
        self.users.insert(credential.identifier.clone(), user);
        tracing::info!(
            "Created credential for user {} via provider {}",
            credential.user_id,
            self.name
        );
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
        let api_key = credentials
            .get("api_key")
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
        tracing::info!(
            "Creating API key credential for user {}",
            credential.user_id
        );
        Ok(())
    }
}

/// JWT authentication provider (HS256).
///
/// Validates the HMAC-SHA256 signature and `exp` claim against a configured
/// secret. A fake/unverifiable token is rejected — the provider no longer
/// accepts arbitrary non-empty strings.
pub struct JwtProvider {
    name: String,
    secret: Vec<u8>,
}

impl JwtProvider {
    pub fn new(name: &str) -> Self {
        JwtProvider {
            name: name.to_string(),
            secret: Vec::new(),
        }
    }

    pub fn with_secret(name: &str, secret: impl Into<Vec<u8>>) -> Self {
        JwtProvider {
            name: name.to_string(),
            secret: secret.into(),
        }
    }

    pub fn set_secret(&mut self, secret: impl Into<Vec<u8>>) {
        self.secret = secret.into();
    }

    fn secret(&self) -> Result<&[u8]> {
        if self.secret.is_empty() {
            return Err(AuthError::TokenInvalid(
                "JWT provider has no signing secret configured".into(),
            ));
        }
        Ok(&self.secret)
    }

    fn hmac_sha256(secret: &[u8], data: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        const BLOCK_SIZE: usize = 64;
        let mut key = [0u8; BLOCK_SIZE];
        if secret.len() > BLOCK_SIZE {
            let mut hasher = Sha256::new();
            hasher.update(secret);
            let digest = hasher.finalize();
            key[..32].copy_from_slice(&digest);
        } else {
            key[..secret.len()].copy_from_slice(secret);
        }
        let mut ipad = [0x36u8; BLOCK_SIZE];
        let mut opad = [0x5cu8; BLOCK_SIZE];
        for i in 0..BLOCK_SIZE {
            ipad[i] ^= key[i];
            opad[i] ^= key[i];
        }
        let mut inner = Sha256::new();
        inner.update(&ipad);
        inner.update(data);
        let inner_hash = inner.finalize();
        let mut outer = Sha256::new();
        outer.update(&opad);
        outer.update(inner_hash);
        outer.finalize().into()
    }

    /// Sign an HS256 JWT for a given header + payload. Public so callers
    /// (e.g. tests/issuers) can mint tokens for the configured secret.
    pub fn sign(secret: &[u8], header: &str, payload: &str) -> String {
        let input = format!("{}.{}", header, payload);
        let sig = Self::hmac_sha256(secret, input.as_bytes());
        use base64::Engine;
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sig)
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
        let token = credentials
            .get("token")
            .ok_or_else(|| AuthError::InvalidCredentials("missing token".into()))?;
        let secret = self.secret()?;

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(AuthError::TokenInvalid("malformed JWT".into()));
        }

        let expected = Self::sign(secret, parts[0], parts[1]);
        let provided = parts[2].trim_end_matches('=');
        if provided != expected {
            return Err(AuthError::TokenInvalid("invalid JWT signature".into()));
        }

        use base64::Engine;
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(|_| AuthError::TokenInvalid("invalid JWT payload encoding".into()))?;
        let json: serde_json::Value = serde_json::from_slice(&payload)
            .map_err(|_| AuthError::TokenInvalid("invalid JWT payload".into()))?;

        if let Some(exp) = json.get("exp").and_then(|v| v.as_i64()) {
            let now = chrono::Utc::now().timestamp();
            if now >= exp {
                return Err(AuthError::TokenExpired("JWT expired".into()));
            }
        }

        let username = json
            .get("sub")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let user_id = json
            .get("uid")
            .and_then(|v| v.as_str())
            .and_then(|s| uuid::Uuid::parse_str(s).ok());
        let roles: Vec<String> = json
            .get("roles")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|r| r.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(AuthResult {
            success: true,
            session: None,
            user_id,
            username,
            roles,
            permissions: vec!["read".to_string(), "write".to_string()],
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
            return Err(AuthError::ProviderNotFound(format!(
                "Provider '{}' already registered",
                name
            )));
        }
        self.provider_by_name.insert(name, self.providers.len());
        self.providers.push(provider);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn AuthProvider>> {
        self.provider_by_name
            .get(name)
            .map(|idx| &self.providers[*idx])
    }

    pub fn get_by_type(&self, credential_type: CredentialType) -> Option<&Arc<dyn AuthProvider>> {
        self.providers
            .iter()
            .find(|p| p.credential_type() == credential_type)
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
        let users = Arc::new(DashMap::new());
        let provider = PasswordProvider::new("local", config, users);
        assert_eq!(provider.name(), "local");
        assert_eq!(provider.credential_type(), CredentialType::Password);
    }

    #[test]
    fn test_password_hashing_and_verification() {
        let password = "MySecurePassword123!";
        let hash = PasswordProvider::hash_password(password, 4);
        assert!(hash.starts_with("$2"));
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

    #[tokio::test]
    async fn test_jwt_authenticate_rejects_fake_token() {
        let provider = JwtProvider::with_secret("jwt", b"unit-test-secret");
        let ok = provider
            .authenticate(&HashMap::from([("token".to_string(), "not-a-real-jwt".to_string())]))
            .await;
        assert!(ok.is_err());

        // Signed token with valid signature but expired claim
        use base64::Engine;
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
            r#"{"sub":"alice","uid":"00000000-0000-0000-0000-000000000001","roles":["admin"],"exp":0}"#,
        );
        let expired = format!(
            "{}.{}.{}",
            header,
            payload,
            JwtProvider::sign(b"unit-test-secret", &header, &payload)
        );
        let result = provider
            .authenticate(&HashMap::from([("token".to_string(), expired.clone())]))
            .await;
        assert!(result.is_err());

        // Valid token (future exp) authenticates
        let exp = chrono::Utc::now().timestamp() + 3600;
        let valid_payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(format!(r#"{{"sub":"alice","uid":"00000000-0000-0000-0000-000000000001","roles":["admin"],"exp":{}}}"#, exp));
        let valid = format!(
            "{}.{}.{}",
            header,
            valid_payload,
            JwtProvider::sign(b"unit-test-secret", &header, &valid_payload)
        );
        let result = provider
            .authenticate(&HashMap::from([("token".to_string(), valid)]))
            .await
            .unwrap();
        assert!(result.success);
        assert_eq!(result.username.as_deref(), Some("alice"));
        assert_eq!(result.roles, vec!["admin".to_string()]);
    }

    #[test]
    fn test_jwt_no_secret_fails() {
        let provider = JwtProvider::new("jwt");
        assert!(provider.secret().is_err());
    }

    #[test]
    fn test_provider_registry() {
        let mut registry = ProviderRegistry::new();
        let users = Arc::new(DashMap::new());
        let password_provider =
            Arc::new(PasswordProvider::new("local", AuthConfig::default(), users));
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
        let users = Arc::new(DashMap::new());
        let provider = Arc::new(PasswordProvider::new("local", AuthConfig::default(), users));
        assert!(registry.register(provider.clone()).is_ok());
        assert!(registry.register(provider).is_err());
    }

    #[test]
    fn test_provider_registry_get_by_type() {
        let mut registry = ProviderRegistry::new();
        let users = Arc::new(DashMap::new());
        registry
            .register(Arc::new(PasswordProvider::new(
                "local",
                AuthConfig::default(),
                users,
            )))
            .unwrap();
        registry
            .register(Arc::new(ApiKeyProvider::new("api")))
            .unwrap();

        assert!(registry.get_by_type(CredentialType::Password).is_some());
        assert!(registry.get_by_type(CredentialType::ApiKey).is_some());
        assert!(registry.get_by_type(CredentialType::Jwt).is_none());
    }

    #[test]
    fn test_password_provider_validate_credential() {
        let config = AuthConfig::default();
        let users = Arc::new(DashMap::new());
        let provider = PasswordProvider::new("local", config, users);
        assert!(provider.validate_credential("ValidPass1!").is_ok());
        assert!(provider.validate_credential("short").is_err());
    }
}
