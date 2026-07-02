use crate::{Result, SecurityError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(label: &str) -> PathBuf {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!("nova_sec_test_{}_{}", label, ts));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn test_file_provider_set_and_get() {
        let dir = temp_dir("setget");
        let sm = SecretsManager::new(SecretsProvider::File {
            directory: dir.to_string_lossy().to_string(),
        });
        sm.set_secret("my_key", b"my_value").unwrap();
        let val = sm.get_secret("my_key").unwrap();
        assert_eq!(val.as_bytes(), b"my_value");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_provider_get_nonexistent() {
        let dir = temp_dir("nonexist");
        let sm = SecretsManager::new(SecretsProvider::File {
            directory: dir.to_string_lossy().to_string(),
        });
        let err = sm.get_secret("does_not_exist").unwrap_err();
        assert!(matches!(err, SecurityError::SecretNotFound(_)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_provider_list_secrets() {
        let dir = temp_dir("list");
        let sm = SecretsManager::new(SecretsProvider::File {
            directory: dir.to_string_lossy().to_string(),
        });
        sm.set_secret("alpha", b"a").unwrap();
        sm.set_secret("beta", b"b").unwrap();
        let list = sm.list_secrets().unwrap();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"alpha".to_string()));
        assert!(list.contains(&"beta".to_string()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_provider_list_empty() {
        let dir = temp_dir("empty");
        let sm = SecretsManager::new(SecretsProvider::File {
            directory: dir.to_string_lossy().to_string(),
        });
        let list = sm.list_secrets().unwrap();
        assert!(list.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_secret_value_as_bytes() {
        let sv = SecretValue::new(b"hello".to_vec());
        assert_eq!(sv.as_bytes(), b"hello");
    }

    #[test]
    fn test_secret_value_as_str_valid_utf8() {
        let sv = SecretValue::new(b"hello".to_vec());
        assert_eq!(sv.as_str().unwrap(), "hello");
    }

    #[test]
    fn test_secret_value_as_str_invalid_utf8() {
        let sv = SecretValue::new(vec![0xFF, 0xFE]);
        assert!(sv.as_str().is_err());
    }

    #[test]
    fn test_secret_value_into_inner() {
        let sv = SecretValue::new(b"data".to_vec());
        let inner = sv.into_inner();
        assert_eq!(inner, b"data");
    }

    #[test]
    fn test_environment_provider_set_noop() {
        let sm = SecretsManager::new(SecretsProvider::Environment {
            prefix: "TEST_".to_string(),
        });
        assert!(sm.set_secret("SOME_KEY", b"value").is_ok());
    }

    #[test]
    fn test_environment_provider_get_nonexistent() {
        let sm = SecretsManager::new(SecretsProvider::Environment {
            prefix: "UNLIKELY_PREFIX_X7K9_".to_string(),
        });
        let err = sm.get_secret("NONEXISTENT").unwrap_err();
        assert!(matches!(err, SecurityError::SecretNotFound(_)));
    }
}
