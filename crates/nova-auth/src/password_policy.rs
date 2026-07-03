use crate::error::AuthError;
use crate::types::AuthConfig;

/// Policy engine for password strength validation.
pub struct PasswordPolicyEngine {
    config: AuthConfig,
}

impl PasswordPolicyEngine {
    pub fn new(config: &AuthConfig) -> Self {
        PasswordPolicyEngine {
            config: config.clone(),
        }
    }

    /// Validate a password against all configured policies.
    /// Returns Ok(()) if the password meets all requirements.
    pub fn validate(&self, password: &str) -> Result<(), AuthError> {
        let errors = self.validate_with_details(password);
        if errors.is_empty() {
            Ok(())
        } else {
            Err(AuthError::PasswordPolicyViolation(errors.join("; ")))
        }
    }

    /// Validate a password and return detailed error messages.
    pub fn validate_with_details(&self, password: &str) -> Vec<String> {
        let mut errors = Vec::new();
        let len = password.len();

        if len < self.config.password_min_length as usize {
            errors.push(format!(
                "Password must be at least {} characters long",
                self.config.password_min_length
            ));
        }

        if len > self.config.password_max_length as usize {
            errors.push(format!(
                "Password must be at most {} characters long",
                self.config.password_max_length
            ));
        }

        let lowercase_count = password.chars().filter(|c| c.is_lowercase()).count();
        if lowercase_count < self.config.password_min_lowercase as usize {
            errors.push(format!(
                "Password must contain at least {} lowercase character(s)",
                self.config.password_min_lowercase
            ));
        }

        let uppercase_count = password.chars().filter(|c| c.is_uppercase()).count();
        if uppercase_count < self.config.password_min_uppercase as usize {
            errors.push(format!(
                "Password must contain at least {} uppercase character(s)",
                self.config.password_min_uppercase
            ));
        }

        let digit_count = password.chars().filter(|c| c.is_ascii_digit()).count();
        if digit_count < self.config.password_min_digits as usize {
            errors.push(format!(
                "Password must contain at least {} digit(s)",
                self.config.password_min_digits
            ));
        }

        let special_count = password.chars().filter(|c| !c.is_alphanumeric()).count();
        if special_count < self.config.password_min_special as usize {
            errors.push(format!(
                "Password must contain at least {} special character(s)",
                self.config.password_min_special
            ));
        }

        // Check for common patterns
        if password.to_lowercase().contains("password") {
            errors.push("Password must not contain the word 'password'".to_string());
        }

        if password.chars().all(|c| c.is_ascii_digit()) {
            errors.push("Password must not be entirely numeric".to_string());
        }

        errors
    }

    /// Estimate password strength on a scale of 0-100.
    pub fn estimate_strength(&self, password: &str) -> u8 {
        let mut score: u8 = 0;
        let len = password.len();

        // Length score (up to 40 points)
        if len >= 8 {
            score += 10;
        }
        if len >= 12 {
            score += 10;
        }
        if len >= 16 {
            score += 10;
        }
        if len >= 24 {
            score += 10;
        }

        // Character diversity (up to 40 points)
        if password.chars().any(|c| c.is_lowercase()) {
            score += 10;
        }
        if password.chars().any(|c| c.is_uppercase()) {
            score += 10;
        }
        if password.chars().any(|c| c.is_ascii_digit()) {
            score += 10;
        }
        if password.chars().any(|c| !c.is_alphanumeric()) {
            score += 10;
        }

        // Bonus for mixed types (up to 20 points)
        if password.chars().any(|c| c.is_lowercase())
            && password.chars().any(|c| c.is_uppercase())
            && password.chars().any(|c| c.is_ascii_digit())
            && password.chars().any(|c| !c.is_alphanumeric())
        {
            score += 20;
        }

        score.min(100)
    }

    /// Check if a password has been compromised (simulated — would use HIBP-like API in prod).
    pub fn is_compromised(_password: &str) -> bool {
        // In production, check against HaveIBeenPwned API or local compromised password list
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> PasswordPolicyEngine {
        let config = AuthConfig {
            password_min_length: 8,
            password_max_length: 128,
            password_min_lowercase: 1,
            password_min_uppercase: 1,
            password_min_digits: 1,
            password_min_special: 0,
            ..AuthConfig::default()
        };
        PasswordPolicyEngine::new(&config)
    }

    #[test]
    fn test_valid_password() {
        let engine = make_engine();
        assert!(engine.validate("ValidPass1").is_ok());
    }

    #[test]
    fn test_password_too_short() {
        let engine = make_engine();
        let result = engine.validate("Ab1");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least 8"));
    }

    #[test]
    fn test_password_no_uppercase() {
        let engine = make_engine();
        let result = engine.validate("lowercase1");
        assert!(result.is_err());
    }

    #[test]
    fn test_password_no_digit() {
        let engine = make_engine();
        let result = engine.validate("NoDigits!");
        assert!(result.is_err());
    }

    #[test]
    fn test_password_all_numeric() {
        let engine = make_engine();
        let result = engine.validate("12345678");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("entirely numeric"));
    }

    #[test]
    fn test_password_contains_password_word() {
        let engine = make_engine();
        let result = engine.validate("Password1");
        // Has uppercase, lowercase, digit, but contains "password" — should warn
        assert!(result.is_err());
    }

    #[test]
    fn test_strength_estimation() {
        let engine = make_engine();
        let weak = engine.estimate_strength("abc");
        let strong = engine.estimate_strength("Tr0ub4dor&3LongEn0ugh!");
        assert!(weak < strong);
    }

    #[test]
    fn test_strength_max_score() {
        let engine = make_engine();
        let score = engine.estimate_strength("C0mpl3x!L0ngEn0ughP@ssw0rd#2024");
        assert!(score >= 90);
    }

    #[test]
    fn test_validate_with_details_returns_multiple_errors() {
        let engine = make_engine();
        let errors = engine.validate_with_details("weak");
        assert!(!errors.is_empty());
        assert!(errors.len() >= 2); // too short + no uppercase + no digit
    }

    #[test]
    fn test_is_compromised() {
        // In the stub, this always returns false
        assert!(!PasswordPolicyEngine::is_compromised("password123"));
    }
}
