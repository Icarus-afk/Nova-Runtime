use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::types::Value;
use nova_core::error::{Result, RuntimeError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub meta: DocumentMeta,
    pub data: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub id: [u8; 16],
    pub collection: String,
    pub document_type: String,
    pub status: DocumentStatus,
    pub schema_version: u32,
    pub created_at: u64,
    pub updated_at: u64,
    pub version: u64,
    pub size: u32,
    pub checksum: u32,
    pub tags: Vec<String>,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentStatus {
    Active,
    Archived,
    Deleted,
    Draft,
}

impl Document {
    pub fn new(collection: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let id = uuid::Uuid::new_v4().into_bytes();
        debug!("created new document in collection '{}'", collection);
        Document {
            meta: DocumentMeta {
                id,
                collection: collection.to_string(),
                document_type: String::new(),
                status: DocumentStatus::Active,
                schema_version: 1,
                created_at: now,
                updated_at: now,
                version: 1,
                size: 0,
                checksum: 0,
                tags: Vec::new(),
                expires_at: 0,
            },
            data: HashMap::new(),
        }
    }

    pub fn get_field(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return None;
        }
        let mut current: Option<&Value> = self.data.get(parts[0]);
        for &part in &parts[1..] {
            match current {
                Some(Value::Object(map)) => {
                    current = map.get(part);
                }
                _ => return None,
            }
        }
        current
    }

    pub fn set_field(&mut self, path: &str, value: Value) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return Err(RuntimeError::InvalidArgument("empty path".to_string()));
        }
        if parts.len() == 1 {
            self.data.insert(parts[0].to_string(), value);
            self.meta.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            return Ok(());
        }
        let mut current = &mut self.data;
        for i in 0..parts.len() - 1 {
            let key = parts[i];
            let entry = current.get_mut(key);
            match entry {
                Some(Value::Object(map)) => {
                    current = map;
                }
                Some(_) => {
                    return Err(RuntimeError::InvalidArgument(
                        format!("cannot descend into non-object field '{}'", key),
                    ));
                }
                None => {
                    let mut new_map = HashMap::new();
                    new_map.insert(parts[i + 1].to_string(), value);
                    return Err(RuntimeError::InvalidArgument(
                        format!("intermediate field '{}' does not exist", key),
                    ));
                }
            }
        }
        current.insert(parts[parts.len() - 1].to_string(), value);
        self.meta.updated_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Ok(())
    }

    pub fn remove_field(&mut self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() {
            return false;
        }
        if parts.len() == 1 {
            let removed = self.data.remove(parts[0]);
            if removed.is_some() {
                self.meta.updated_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                return true;
            }
            return false;
        }
        let mut current = &mut self.data;
        for i in 0..parts.len() - 1 {
            let key = parts[i];
            match current.get_mut(key) {
                Some(Value::Object(map)) => {
                    current = map;
                }
                _ => return false,
            }
        }
        let removed = current.remove(parts[parts.len() - 1]);
        if removed.is_some() {
            self.meta.updated_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            true
        } else {
            false
        }
    }

    pub fn compute_size(&self) -> u32 {
        match rmp_serde::to_vec(self) {
            Ok(bytes) => bytes.len() as u32,
            Err(_) => 0,
        }
    }

    pub fn compute_checksum(&self) -> u32 {
        match rmp_serde::to_vec(self) {
            Ok(bytes) => crc32c::crc32c(&bytes),
            Err(_) => 0,
        }
    }
}
