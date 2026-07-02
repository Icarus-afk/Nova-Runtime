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
