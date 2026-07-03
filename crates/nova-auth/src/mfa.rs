use crate::error::{AuthError, Result};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// TOTP (Time-based One-Time Password) provider for multi-factor authentication.
pub struct MfaProvider {
    issuer: String,
    window: u8,
}

impl MfaProvider {
    pub fn new(issuer: &str, window: u8) -> Self {
        MfaProvider {
            issuer: issuer.to_string(),
            window: window.max(1),
        }
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    /// Generate a TOTP secret.
    pub fn generate_secret() -> Vec<u8> {
        use rand::Rng;
        let secret: Vec<u8> = (0..20).map(|_| rand::thread_rng().r#gen()).collect();
        secret
    }

    /// Generate a TOTP URI for QR code provisioning (otpauth:// protocol).
    pub fn generate_otpauth_uri(&self, username: &str, secret: &[u8]) -> String {
        let encoded_secret = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD_NO_PAD,
            secret,
        );
        format!(
            "otpauth://totp/{}:{}?secret={}&issuer={}",
            self.issuer, username, encoded_secret, self.issuer
        )
    }

    /// Compute a TOTP code for the given secret at a specific time.
    pub fn generate_code(secret: &[u8], time_step: i64) -> String {
        // Simplified TOTP implementation
        // In production, use a proper TOTP library (e.g., oath, totp-rs)
        use sha2::{Sha256, Digest};

        let time_bytes = time_step.to_be_bytes();
        let mut hasher = Sha256::new();
        hasher.update(secret);
        hasher.update(&time_bytes);
        let hash = hasher.finalize();

        // Truncate to 6 digits
        let offset = (hash[hash.len() - 1] & 0x0F) as usize;
        let code = ((hash[offset] as u32 & 0x7F) << 24)
            | ((hash[offset + 1] as u32) << 16)
            | ((hash[offset + 2] as u32) << 8)
            | (hash[offset + 3] as u32);

        format!("{:06}", code % 1_000_000)
    }

    /// Verify a TOTP code against a secret with window tolerance.
    pub fn verify_code(&self, secret: &[u8], code: &str) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let time_step = now / 30; // Standard 30-second window

        // Check current and adjacent windows
        for offset in -(self.window as i64)..=self.window as i64 {
            let candidate = Self::generate_code(secret, time_step + offset);
            if candidate == code {
                return true;
            }
        }

        false
    }

    /// Get the current TOTP time step.
    pub fn current_time_step() -> i64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        now / 30
    }
}

/// Stores TOTP secrets for users.
pub struct MfaStore {
    secrets: HashMap<uuid::Uuid, Vec<u8>>,
}

impl MfaStore {
    pub fn new() -> Self {
        MfaStore {
            secrets: HashMap::new(),
        }
    }

    pub fn enable_mfa(&mut self, user_id: uuid::Uuid, secret: Vec<u8>) {
        self.secrets.insert(user_id, secret);
    }

    pub fn disable_mfa(&mut self, user_id: &uuid::Uuid) {
        self.secrets.remove(user_id);
    }

    pub fn has_mfa(&self, user_id: &uuid::Uuid) -> bool {
        self.secrets.contains_key(user_id)
    }

    pub fn get_secret(&self, user_id: &uuid::Uuid) -> Option<&[u8]> {
        self.secrets.get(user_id).map(|s| s.as_slice())
    }

    pub fn verify_code(&self, user_id: &uuid::Uuid, code: &str, window: u8) -> bool {
        if let Some(secret) = self.get_secret(user_id) {
            let provider = MfaProvider::new("Nova", window);
            return provider.verify_code(secret, code);
        }
        false
    }
}

impl Default for MfaStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mfa_provider_generate_secret() {
        let secret = MfaProvider::generate_secret();
        assert_eq!(secret.len(), 20);
    }

    #[test]
    fn test_mfa_provider_generate_code() {
        let secret = MfaProvider::generate_secret();
        let step = MfaProvider::current_time_step();
        let code = MfaProvider::generate_code(&secret, step);
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_mfa_provider_verify_own_code() {
        let provider = MfaProvider::new("Nova", 1);
        let secret = MfaProvider::generate_secret();
        let step = MfaProvider::current_time_step();
        let code = MfaProvider::generate_code(&secret, step);
        assert!(provider.verify_code(&secret, &code));
    }

    #[test]
    fn test_mfa_provider_wrong_code_fails() {
        let provider = MfaProvider::new("Nova", 0); // no window
        let secret = MfaProvider::generate_secret();
        assert!(!provider.verify_code(&secret, "000000"));
    }

    #[test]
    fn test_mfa_store() {
        let mut store = MfaStore::new();
        let user_id = uuid::Uuid::new_v4();

        assert!(!store.has_mfa(&user_id));

        let secret = MfaProvider::generate_secret();
        store.enable_mfa(user_id, secret.clone());
        assert!(store.has_mfa(&user_id));
        assert_eq!(store.get_secret(&user_id), Some(secret.as_slice()));

        store.disable_mfa(&user_id);
        assert!(!store.has_mfa(&user_id));
    }

    #[test]
    fn test_mfa_otpauth_uri_format() {
        let provider = MfaProvider::new("Nova Runtime", 1);
        let secret = MfaProvider::generate_secret();
        let uri = provider.generate_otpauth_uri("testuser", &secret);
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("Nova Runtime"));
        assert!(uri.contains("testuser"));
        assert!(uri.contains("secret="));
    }

    #[test]
    fn test_mfa_provider_window() {
        let provider = MfaProvider::new("Nova", 2);
        let secret = MfaProvider::generate_secret();
        let current_step = MfaProvider::current_time_step();

        // Generate a code from one step in the past — should still verify with window=2
        let past_code = MfaProvider::generate_code(&secret, current_step - 1);
        assert!(provider.verify_code(&secret, &past_code));

        // Too far in the past should fail
        let far_past_code = MfaProvider::generate_code(&secret, current_step - 5);
        // With window=2, this is within tolerance (difference of 3 <= window=2? Actually 5 > 2, so should fail)
        // Wait, the window includes both sides: -2..=2, so steps -3..=3 are covered
        // | -5 | <= 2? No, 5 > 2. So this should fail.
        assert!(!provider.verify_code(&secret, &far_past_code));
    }
}
