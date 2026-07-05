use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Type of credential a provider handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CredentialType {
    Password,
    Token,
    ApiKey,
    Jwt,
    OAuth2,
    MfaTotp,
    Session,
}

impl Default for CredentialType {
    fn default() -> Self {
        CredentialType::Password
    }
}

/// A user session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub token: String,
    pub token_type: String,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub created_at: i64,
    pub expires_at: i64,
    pub last_activity_at: i64,
    pub revoked: bool,
    pub mfa_verified: bool,
    pub source_ip: Option<String>,
    pub user_agent: Option<String>,
}

impl Session {
    pub fn new(user_id: Uuid, username: &str, ttl_secs: u32) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        let token = generate_session_token();
        Session {
            id: Uuid::new_v4(),
            user_id,
            username: username.to_string(),
            token,
            token_type: "bearer".to_string(),
            roles: Vec::new(),
            permissions: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            expires_at: now + (ttl_secs as i64) * 1000,
            last_activity_at: now,
            revoked: false,
            mfa_verified: false,
            source_ip: None,
            user_agent: None,
        }
    }

    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() >= self.expires_at
    }

    pub fn is_valid(&self) -> bool {
        !self.revoked && !self.is_expired()
    }

    pub fn touch(&mut self) {
        self.last_activity_at = chrono::Utc::now().timestamp_millis();
    }
}

/// A credential record stored for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_type: CredentialType,
    pub identifier: String,      // username, email, etc.
    pub secret_hash: String,     // hashed password, API key hash, etc.
    pub salt: Option<String>,
    pub algorithm: String,       // "bcrypt", "argon2", "sha256" etc.
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub locked_until: Option<i64>,
    pub failed_attempts: u32,
    pub metadata: HashMap<String, String>,
}

impl Credential {
    pub fn new(user_id: Uuid, identifier: &str, secret_hash: &str, algo: &str) -> Self {
        Credential {
            id: Uuid::new_v4(),
            user_id,
            credential_type: CredentialType::Password,
            identifier: identifier.to_string(),
            secret_hash: secret_hash.to_string(),
            salt: None,
            algorithm: algo.to_string(),
            created_at: chrono::Utc::now().timestamp_millis(),
            expires_at: None,
            locked_until: None,
            failed_attempts: 0,
            metadata: HashMap::new(),
        }
    }
}

/// A stored user record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// A stored API key record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    pub id: Uuid,
    pub name: String,
    pub key_hash: String,
    pub prefix: String,
    pub permissions: Vec<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
    pub enabled: bool,
}

/// Result of an authentication attempt.
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub success: bool,
    pub session: Option<Session>,
    pub user_id: Option<Uuid>,
    pub username: Option<String>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
    pub mfa_required: bool,
    pub error_message: Option<String>,
    pub retry_after_ms: Option<u64>,
}

/// Permission evaluation request.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    pub user_id: Uuid,
    pub action: String,
    pub resource: String,
    pub context: HashMap<String, String>,
}

/// A role definition with assigned permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Role {
    pub name: String,
    pub description: String,
    pub permissions: Vec<String>,
    pub created_at: i64,
}

/// A permission definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Permission {
    pub name: String,
    pub description: String,
    pub resource_pattern: String,
    pub action: String,
}

/// Configuration for the auth subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub session_ttl_secs: u32,
    pub max_active_sessions: u32,
    pub token_length_bytes: usize,
    pub mfa_issuer: String,
    pub mfa_window: u8,
    pub bcrypt_cost: u32,
    pub max_failed_attempts: u32,
    pub lockout_duration_secs: u64,
    pub enable_brute_force_detection: bool,
    pub session_cache_size: usize,
    pub password_min_length: u8,
    pub password_max_length: u8,
    pub password_min_lowercase: u8,
    pub password_min_uppercase: u8,
    pub password_min_digits: u8,
    pub password_min_special: u8,
}

impl Default for AuthConfig {
    fn default() -> Self {
        AuthConfig {
            session_ttl_secs: 86400,
            max_active_sessions: 100,
            token_length_bytes: 32,
            mfa_issuer: "Nova Runtime".to_string(),
            mfa_window: 1,
            bcrypt_cost: 12,
            max_failed_attempts: 5,
            lockout_duration_secs: 900,
            enable_brute_force_detection: true,
            session_cache_size: 100000,
            password_min_length: 8,
            password_max_length: 128,
            password_min_lowercase: 1,
            password_min_uppercase: 1,
            password_min_digits: 1,
            password_min_special: 0,
        }
    }
}

/// Generate an opaque session token: nova_sess_<base64url(32 random bytes)>.
pub fn generate_session_token() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::thread_rng().r#gen();
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes);
    format!("nova_sess_{}", encoded)
}

/// Validate password against policy.
pub fn validate_password(password: &str, config: &AuthConfig) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let len = password.len() as u8;

    if len < config.password_min_length {
        errors.push(format!("Password must be at least {} characters", config.password_min_length));
    }
    if len > config.password_max_length {
        errors.push(format!("Password must be at most {} characters", config.password_max_length));
    }

    let lowercase = password.chars().filter(|c| c.is_lowercase()).count() as u8;
    let uppercase = password.chars().filter(|c| c.is_uppercase()).count() as u8;
    let digits = password.chars().filter(|c| c.is_ascii_digit()).count() as u8;
    let special = password.chars().filter(|c| !c.is_alphanumeric()).count() as u8;

    if lowercase < config.password_min_lowercase {
        errors.push(format!("Password must have at least {} lowercase character(s)", config.password_min_lowercase));
    }
    if uppercase < config.password_min_uppercase {
        errors.push(format!("Password must have at least {} uppercase character(s)", config.password_min_uppercase));
    }
    if digits < config.password_min_digits {
        errors.push(format!("Password must have at least {} digit(s)", config.password_min_digits));
    }
    if special < config.password_min_special {
        errors.push(format!("Password must have at least {} special character(s)", config.password_min_special));
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_config_defaults() {
        let c = AuthConfig::default();
        assert_eq!(c.session_ttl_secs, 86400);
        assert_eq!(c.max_active_sessions, 100);
        assert_eq!(c.token_length_bytes, 32);
        assert_eq!(c.mfa_issuer, "Nova Runtime");
        assert_eq!(c.max_failed_attempts, 5);
        assert_eq!(c.lockout_duration_secs, 900);
        assert_eq!(c.session_cache_size, 100000);
    }

    #[test]
    fn test_session_new() {
        let user_id = Uuid::new_v4();
        let session = Session::new(user_id, "testuser", 3600);
        assert_eq!(session.user_id, user_id);
        assert_eq!(session.username, "testuser");
        assert!(session.token.starts_with("nova_sess_"));
        assert!(!session.revoked);
        assert!(!session.mfa_verified);
        assert!(session.is_valid());
    }

    #[test]
    fn test_session_expiry() {
        let user_id = Uuid::new_v4();
        let mut session = Session::new(user_id, "test", 3600);
        assert!(!session.is_expired());

        // Set expiry in the past
        session.expires_at = 0;
        assert!(session.is_expired());
        assert!(!session.is_valid());
    }

    #[test]
    fn test_session_revocation() {
        let user_id = Uuid::new_v4();
        let mut session = Session::new(user_id, "test", 3600);
        session.revoked = true;
        assert!(!session.is_valid());
    }

    #[test]
    fn test_session_touch() {
        let user_id = Uuid::new_v4();
        let mut session = Session::new(user_id, "test", 3600);
        let before = session.last_activity_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        session.touch();
        assert!(session.last_activity_at > before);
    }

    #[test]
    fn test_generate_session_token_format() {
        let token = generate_session_token();
        assert!(token.starts_with("nova_sess_"));
        assert!(token.len() > 20);
    }

    #[test]
    fn test_credential_new() {
        let user_id = Uuid::new_v4();
        let cred = Credential::new(user_id, "testuser", "$2b$12$hash", "bcrypt");
        assert_eq!(cred.user_id, user_id);
        assert_eq!(cred.identifier, "testuser");
        assert_eq!(cred.algorithm, "bcrypt");
        assert_eq!(cred.failed_attempts, 0);
        assert!(cred.locked_until.is_none());
    }

    #[test]
    fn test_validate_password_min_length() {
        let config = AuthConfig::default();
        assert!(validate_password("Abcdef1!", &config).is_ok());
        // Too short
        assert!(validate_password("Ab1!", &config).is_err());
    }

    #[test]
    fn test_validate_password_requirements() {
        let mut config = AuthConfig::default();
        config.password_min_lowercase = 1;
        config.password_min_uppercase = 1;
        config.password_min_digits = 1;
        config.password_min_special = 1;

        assert!(validate_password("Abcdef1!", &config).is_ok());
        assert!(validate_password("abcdef1!", &config).is_err()); // No uppercase
        assert!(validate_password("ABCDEF1!", &config).is_err()); // No lowercase
        assert!(validate_password("Abcdefg!", &config).is_err()); // No digit
        assert!(validate_password("Abcdef12", &config).is_err()); // No special
    }

    #[test]
    fn test_credential_type_default() {
        assert_eq!(CredentialType::default(), CredentialType::Password);
    }

    #[test]
    fn test_permission_request_construction() {
        let req = PermissionRequest {
            user_id: Uuid::new_v4(),
            action: "read".into(),
            resource: "document:123".into(),
            context: HashMap::new(),
        };
        assert_eq!(req.action, "read");
    }

    #[test]
    fn test_role_construction() {
        let role = Role {
            name: "admin".into(),
            description: "Administrator".into(),
            permissions: vec!["read".into(), "write".into()],
            created_at: chrono::Utc::now().timestamp_millis(),
        };
        assert_eq!(role.name, "admin");
        assert_eq!(role.permissions.len(), 2);
    }
}
