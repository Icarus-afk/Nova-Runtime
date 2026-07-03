use crate::{Result, SecurityError};

use regex::Regex;

#[derive(Debug, Clone)]
pub struct InputValidator {
    pub max_body_size: u64,
    pub max_nesting_depth: u8,
    pub allowed_content_types: Vec<String>,
}

impl InputValidator {
    pub fn new(max_body_size: u64) -> Self {
        InputValidator {
            max_body_size,
            max_nesting_depth: 64,
            allowed_content_types: vec!["application/json".to_string()],
        }
    }

    pub fn validate_body_size(&self, size: u64) -> Result<()> {
        if size > self.max_body_size {
            return Err(SecurityError::Validation(format!(
                "Body size {} exceeds maximum {}",
                size, self.max_body_size
            )));
        }
        Ok(())
    }

    pub fn validate_content_type(&self, content_type: &str) -> Result<()> {
        if self.allowed_content_types.is_empty() {
            return Ok(());
        }
        let ct = content_type.split(';').next().unwrap_or(content_type).trim();
        if self.allowed_content_types.iter().any(|allowed| allowed == ct) {
            Ok(())
        } else {
            Err(SecurityError::Validation(format!(
                "Content type '{}' is not allowed",
                content_type
            )))
        }
    }

    pub fn validate_utf8(&self, data: &[u8]) -> Result<()> {
        match std::str::from_utf8(data) {
            Ok(_) => Ok(()),
            Err(e) => Err(SecurityError::Validation(format!(
                "Invalid UTF-8: {}",
                e
            ))),
        }
    }

    pub fn validate_headers(&self, headers: &[(String, String)]) -> std::result::Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for (i, (name, value)) in headers.iter().enumerate() {
            if name.len() > 128 {
                errors.push(format!("Header name at index {} exceeds max length of 128", i));
            }
            if name.contains('\0') {
                errors.push(format!("Header name at index {} contains null bytes", i));
            }
            if value.len() > 4096 {
                errors.push(format!("Header value at index {} exceeds max length of 4096", i));
            }
            if value.contains('\0') {
                errors.push(format!("Header value at index {} contains null bytes", i));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn validate_query_params(&self, params: &[(String, String)]) -> std::result::Result<(), Vec<String>> {
        let mut errors = Vec::new();
        for (i, (name, value)) in params.iter().enumerate() {
            if name.len() > 128 {
                errors.push(format!("Query param name at index {} exceeds max length of 128", i));
            }
            if name.contains('\0') {
                errors.push(format!("Query param name at index {} contains null bytes", i));
            }
            if value.len() > 4096 {
                errors.push(format!("Query param value at index {} exceeds max length of 4096", i));
            }
            if value.contains('\0') {
                errors.push(format!("Query param value at index {} contains null bytes", i));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn sanitize_path(&self, path: &str) -> Result<String> {
        if path.contains('\0') {
            return Err(SecurityError::Validation(
                "Path contains null bytes".into(),
            ));
        }
        let is_absolute = path.starts_with('/');
        let components: Vec<&str> = path.split('/').collect();
        let mut result = Vec::new();
        for component in components {
            match component {
                "" | "." => continue,
                ".." => {
                    if result.is_empty() {
                        return Err(SecurityError::Validation(
                            "Path traversal detected".into(),
                        ));
                    }
                    result.pop();
                }
                _ => {
                    result.push(component);
                }
            }
        }
        let mut sanitized = result.join("/");
        if is_absolute {
            sanitized.insert(0, '/');
        }
        Ok(sanitized)
    }
}

pub fn sanitize_path_component(component: &str) -> Result<String> {
    let re = Regex::new(r"^[a-zA-Z0-9_.-]{1,255}$")
        .map_err(|e| SecurityError::Internal(e.to_string()))?;
    if re.is_match(component) {
        Ok(component.to_string())
    } else {
        Err(SecurityError::Validation(format!(
            "Invalid path component: '{}'",
            component
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_validate_body_size() {
        let validator = InputValidator::new(1024);
        
        // Happy path
        assert!(validator.validate_body_size(0).is_ok());
        assert!(validator.validate_body_size(1024).is_ok());
        
        // Error path
        let err = validator.validate_body_size(1025).unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        assert!(err.to_string().contains("exceeds maximum"));
    }

    #[test]
    fn test_validate_content_type() {
        let mut validator = InputValidator::new(1024);
        validator.allowed_content_types = vec!["application/json".to_string(), "text/plain".to_string()];
        
        // Happy path
        assert!(validator.validate_content_type("application/json").is_ok());
        assert!(validator.validate_content_type("text/plain").is_ok());
        assert!(validator.validate_content_type("application/json; charset=utf-8").is_ok());
        
        // Empty allowed content types
        validator.allowed_content_types = vec![];
        assert!(validator.validate_content_type("any/type").is_ok());
        
        // Error path
        validator.allowed_content_types = vec!["application/json".to_string()];
        let err = validator.validate_content_type("text/plain").unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        assert!(err.to_string().contains("not allowed"));
    }

    #[test]
    fn test_validate_utf8() {
        let validator = InputValidator::new(1024);
        
        // Happy path
        assert!(validator.validate_utf8(b"valid utf8").is_ok());
        assert!(validator.validate_utf8("".as_bytes()).is_ok());
        
        // Error path
        let invalid_utf8 = b"\xff\xfe";
        let err = validator.validate_utf8(invalid_utf8).unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        assert!(err.to_string().contains("Invalid UTF-8"));
    }

    #[test]
    fn test_validate_headers() {
        let validator = InputValidator::new(1024);
        
        // Happy path
        let valid_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-custom".to_string(), "value".to_string()),
        ];
        assert!(validator.validate_headers(&valid_headers).is_ok());
        
        // Error path - name too long
        let long_name = "a".repeat(129);
        let invalid_headers = vec![(long_name, "value".to_string())];
        let err = validator.validate_headers(&invalid_headers).unwrap_err();
        assert!(err[0].contains("exceeds max length"));
        
        // Error path - null bytes
        let invalid_headers = vec![("name\0".to_string(), "value".to_string())];
        let err = validator.validate_headers(&invalid_headers).unwrap_err();
        assert!(err[0].contains("null bytes"));
        
        // Error path - value too long
        let long_value = "a".repeat(4097);
        let invalid_headers = vec![("name".to_string(), long_value)];
        let err = validator.validate_headers(&invalid_headers).unwrap_err();
        assert!(err[0].contains("exceeds max length"));
    }

    #[test]
    fn test_validate_query_params() {
        let validator = InputValidator::new(1024);
        
        // Happy path
        let valid_params = vec![
            ("key".to_string(), "value".to_string()),
            ("page".to_string(), "1".to_string()),
        ];
        assert!(validator.validate_query_params(&valid_params).is_ok());
        
        // Error path - name too long
        let long_name = "a".repeat(129);
        let invalid_params = vec![(long_name, "value".to_string())];
        let err = validator.validate_query_params(&invalid_params).unwrap_err();
        assert!(err[0].contains("exceeds max length"));
        
        // Error path - null bytes
        let invalid_params = vec![("name\0".to_string(), "value".to_string())];
        let err = validator.validate_query_params(&invalid_params).unwrap_err();
        assert!(err[0].contains("null bytes"));
        
        // Error path - value too long
        let long_value = "a".repeat(4097);
        let invalid_params = vec![("name".to_string(), long_value)];
        let err = validator.validate_query_params(&invalid_params).unwrap_err();
        assert!(err[0].contains("exceeds max length"));
    }

    #[test]
    fn test_sanitize_path() {
        let validator = InputValidator::new(1024);
        
        // Happy path
        assert_eq!(validator.sanitize_path("/valid/path").unwrap(), "/valid/path");
        assert_eq!(validator.sanitize_path("valid/path").unwrap(), "valid/path");
        assert_eq!(validator.sanitize_path("/").unwrap(), "/");
        assert_eq!(validator.sanitize_path("").unwrap(), "");
        
        // Error path - null bytes
        let err = validator.sanitize_path("path\0").unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        assert!(err.to_string().contains("null bytes"));
        
        // Error path - path traversal
        let err = validator.sanitize_path("/../etc/passwd").unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        assert!(err.to_string().contains("traversal detected"));
        
        // Edge cases
        assert_eq!(validator.sanitize_path("/./a/../b").unwrap(), "/b");
        assert_eq!(validator.sanitize_path("a/./b/../c").unwrap(), "a/c");
    }

    #[test]
    fn test_sanitize_path_component() {
        // Happy path
        assert_eq!(sanitize_path_component("valid_component").unwrap(), "valid_component");
        assert_eq!(sanitize_path_component("123").unwrap(), "123");
        assert_eq!(sanitize_path_component("a.b_c-d").unwrap(), "a.b_c-d");
        
        // Error path
        let err = sanitize_path_component("invalid component").unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        
        let err = sanitize_path_component("").unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        
        let err = sanitize_path_component("a".repeat(256).as_str()).unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
        
        let err = sanitize_path_component("../").unwrap_err();
        assert!(matches!(err, SecurityError::Validation(_)));
    }

    proptest! {
        #[test]
        fn prop_test_utf8_validation(data in any::<Vec<u8>>()) {
            let validator = InputValidator::new(1024);
            let _ = validator.validate_utf8(&data);
        }
        
        #[test]
        fn prop_test_path_component_validation(component in ".{0,256}") {
            let _ = sanitize_path_component(&component);
        }
    }
}
