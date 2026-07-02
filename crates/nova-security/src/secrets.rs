use crate::{Result, SecurityError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone)]
pub struct SecretValue {
    data: Vec<u8>,
}

impl SecretValue {
    pub fn new(data: Vec<u8>) -> Self {
        SecretValue { data }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn as_str(&self) -> std::result::Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.data)
    }

    pub fn into_inner(mut self) -> Vec<u8> {
        let result = std::mem::take(&mut self.data);
        result
    }
}

impl Drop for SecretValue {
    fn drop(&mut self) {
        for byte in &mut self.data {
            unsafe {
                std::ptr::write_volatile(byte, 0);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecretsProvider {
    Environment { prefix: String },
    File { directory: String },
}

pub struct SecretsManager {
    provider: SecretsProvider,
}

impl SecretsManager {
    pub fn new(provider: SecretsProvider) -> Self {
        SecretsManager { provider }
    }

    pub fn get_secret(&self, name: &str) -> Result<SecretValue> {
        match &self.provider {
            SecretsProvider::Environment { prefix } => {
                let var_name = format!("{}{}", prefix, name.to_uppercase());
                let value = std::env::var(&var_name)
                    .map_err(|_| SecurityError::SecretNotFound(var_name))?;
                Ok(SecretValue::new(value.into_bytes()))
            }
            SecretsProvider::File { directory } => {
                let path = PathBuf::from(directory).join(format!("{}.secret", name));
                let data = fs::read(&path)
                    .map_err(|_| SecurityError::SecretNotFound(path.to_string_lossy().to_string()))?;
                Ok(SecretValue::new(data))
            }
        }
    }

    pub fn set_secret(&self, name: &str, value: &[u8]) -> Result<()> {
        match &self.provider {
            SecretsProvider::Environment { .. } => {
                tracing::warn!(
                    "set_secret called for Environment provider (no-op): {}",
                    name
                );
                Ok(())
            }
            SecretsProvider::File { directory } => {
                let dir = PathBuf::from(directory);
                fs::create_dir_all(&dir)
                    .map_err(|e| SecurityError::Internal(e.to_string()))?;
                let path = dir.join(format!("{}.secret", name));
                fs::write(&path, value)
                    .map_err(|e| SecurityError::Internal(e.to_string()))?;
                Ok(())
            }
        }
    }

    pub fn list_secrets(&self) -> Result<Vec<String>> {
        match &self.provider {
            SecretsProvider::Environment { prefix } => {
                let mut secrets = Vec::new();
                for (key, _) in std::env::vars() {
                    if key.starts_with(prefix) {
                        secrets.push(key[prefix.len()..].to_string());
                    }
                }
                Ok(secrets)
            }
            SecretsProvider::File { directory } => {
                let dir = PathBuf::from(directory);
                let mut secrets = Vec::new();
                if let Ok(entries) = fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(name) = path.file_stem() {
                            if path.extension().map_or(false, |e| e == "secret") {
                                secrets.push(name.to_string_lossy().to_string());
                            }
                        }
                    }
                }
                Ok(secrets)
            }
        }
    }
}
