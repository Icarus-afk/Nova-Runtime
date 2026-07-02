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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use crate::document::*;
    use crate::types::Value;

    // --- Document creation ---

    #[test]
    fn test_document_new() {
        let doc = Document::new("users");
        assert_eq!(doc.meta.collection, "users");
        assert_eq!(doc.meta.status, DocumentStatus::Active);
        assert_eq!(doc.meta.version, 1);
        assert_eq!(doc.meta.schema_version, 1);
        assert!(doc.data.is_empty());
    }

    #[test]
    fn test_document_new_generates_id() {
        let doc1 = Document::new("c");
        let doc2 = Document::new("c");
        assert_ne!(doc1.meta.id, doc2.meta.id);
    }

    #[test]
    fn test_document_new_sets_timestamps() {
        let doc = Document::new("c");
        assert!(doc.meta.created_at > 0);
        assert_eq!(doc.meta.created_at, doc.meta.updated_at);
    }

    // --- DocumentStatus ---

    #[test]
    fn test_document_status_variants() {
        assert_eq!(format!("{:?}", DocumentStatus::Active), "Active");
        assert_eq!(format!("{:?}", DocumentStatus::Archived), "Archived");
        assert_eq!(format!("{:?}", DocumentStatus::Deleted), "Deleted");
        assert_eq!(format!("{:?}", DocumentStatus::Draft), "Draft");
    }

    // --- get_field ---

    #[test]
    fn test_get_field_top_level() {
        let mut doc = Document::new("t");
        doc.data.insert("name".into(), Value::String("alice".into()));
        assert_eq!(doc.get_field("name"), Some(&Value::String("alice".into())));
    }

    #[test]
    fn test_get_field_missing() {
        let doc = Document::new("t");
        assert_eq!(doc.get_field("nonexistent"), None);
    }

    #[test]
    fn test_get_field_nested() {
        let mut doc = Document::new("t");
        let mut inner = HashMap::new();
        inner.insert("x".into(), Value::Int64(42));
        doc.data.insert("outer".into(), Value::Object(inner));
        assert_eq!(doc.get_field("outer.x"), Some(&Value::Int64(42)));
    }

    #[test]
    fn test_get_field_deeply_nested() {
        let mut doc = Document::new("t");
        let mut level2 = HashMap::new();
        level2.insert("z".into(), Value::String("deep".into()));
        let mut level1 = HashMap::new();
        level1.insert("y".into(), Value::Object(level2));
        doc.data.insert("x".into(), Value::Object(level1));
        assert_eq!(doc.get_field("x.y.z"), Some(&Value::String("deep".into())));
    }

    #[test]
    fn test_get_field_empty_path() {
        let doc = Document::new("t");
        assert_eq!(doc.get_field(""), None);
    }

    #[test]
    fn test_get_field_non_object_intermediate() {
        let mut doc = Document::new("t");
        doc.data.insert("x".into(), Value::Int64(1));
        assert_eq!(doc.get_field("x.y"), None);
    }

    // --- set_field ---

    #[test]
    fn test_set_field_top_level() {
        let mut doc = Document::new("t");
        doc.set_field("name", Value::String("bob".into())).unwrap();
        assert_eq!(doc.data.get("name"), Some(&Value::String("bob".into())));
    }

    #[test]
    fn test_set_field_overwrites() {
        let mut doc = Document::new("t");
        doc.set_field("x", Value::Int64(1)).unwrap();
        doc.set_field("x", Value::Int64(2)).unwrap();
        assert_eq!(doc.data.get("x"), Some(&Value::Int64(2)));
    }

    #[test]
    fn test_set_field_nested() {
        let mut doc = Document::new("t");
        let mut inner = HashMap::new();
        inner.insert("y".into(), Value::Int64(0));
        doc.data.insert("x".into(), Value::Object(inner));

        doc.set_field("x.y", Value::Int64(42)).unwrap();
        let outer = doc.data.get("x").unwrap();
        if let Value::Object(map) = outer {
            assert_eq!(map.get("y"), Some(&Value::Int64(42)));
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn test_set_field_empty_path_inserts_empty_key() {
        let mut doc = Document::new("t");
        doc.set_field("", Value::String("val".into())).unwrap();
        assert_eq!(doc.data.get(""), Some(&Value::String("val".into())));
    }

    #[test]
    fn test_set_field_non_object_intermediate() {
        let mut doc = Document::new("t");
        doc.data.insert("x".into(), Value::Int64(1));
        let result = doc.set_field("x.y", Value::Int64(2));
        assert!(result.is_err());
    }

    #[test]
    fn test_set_field_missing_intermediate() {
        let mut doc = Document::new("t");
        let result = doc.set_field("a.b.c", Value::Int64(1));
        assert!(result.is_err());
    }

    #[test]
    fn test_set_field_updates_updated_at() {
        let mut doc = Document::new("t");
        let before = doc.meta.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        doc.set_field("x", Value::Int64(1)).unwrap();
        assert!(doc.meta.updated_at > before);
    }

    // --- remove_field ---

    #[test]
    fn test_remove_field_top_level() {
        let mut doc = Document::new("t");
        doc.data.insert("x".into(), Value::Int64(1));
        assert!(doc.remove_field("x"));
        assert!(doc.data.is_empty());
    }

    #[test]
    fn test_remove_field_missing() {
        let mut doc = Document::new("t");
        assert!(!doc.remove_field("nonexistent"));
    }

    #[test]
    fn test_remove_field_empty_path() {
        let mut doc = Document::new("t");
        assert!(!doc.remove_field(""));
    }

    #[test]
    fn test_remove_field_nested() {
        let mut doc = Document::new("t");
        let mut inner = HashMap::new();
        inner.insert("y".into(), Value::Int64(42));
        doc.data.insert("x".into(), Value::Object(inner));

        assert!(doc.remove_field("x.y"));
        let outer = doc.data.get("x").unwrap();
        if let Value::Object(map) = outer {
            assert!(map.is_empty());
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn test_remove_field_nested_missing_intermediate() {
        let mut doc = Document::new("t");
        assert!(!doc.remove_field("a.b"));
    }

    // --- compute_size ---

    #[test]
    fn test_compute_size_empty() {
        let doc = Document::new("t");
        assert!(doc.compute_size() > 0);
    }

    #[test]
    fn test_compute_size_with_data() {
        let mut doc = Document::new("t");
        doc.data.insert("data".into(), Value::String("hello".repeat(100)));
        let size = doc.compute_size();
        assert!(size > 0);
    }

    // --- compute_checksum ---

    #[test]
    fn test_compute_checksum_consistent() {
        let mut doc = Document::new("t");
        doc.data.insert("x".into(), Value::Int64(1));
        let c1 = doc.compute_checksum();
        let c2 = doc.compute_checksum();
        assert_eq!(c1, c2);
    }

    #[test]
    fn test_compute_checksum_changes_with_data() {
        let mut doc = Document::new("t");
        doc.data.insert("x".into(), Value::Int64(1));
        let c1 = doc.compute_checksum();
        doc.data.insert("y".into(), Value::Int64(2));
        let c2 = doc.compute_checksum();
        assert_ne!(c1, c2);
    }

    // --- DocumentMeta ---

    #[test]
    fn test_document_meta_defaults() {
        let doc = Document::new("test_coll");
        assert_eq!(doc.meta.collection, "test_coll");
        assert_eq!(doc.meta.version, 1);
        assert_eq!(doc.meta.status, DocumentStatus::Active);
        assert_eq!(doc.meta.tags, Vec::<String>::new());
    }

    // --- Clone ---

    #[test]
    fn test_document_clone() {
        let mut doc = Document::new("t");
        doc.data.insert("x".into(), Value::String("val".into()));
        let cloned = doc.clone();
        assert_eq!(doc.data, cloned.data);
        assert_eq!(doc.meta.id, cloned.meta.id);
    }

    // --- Serialization round-trip ---

    #[test]
    fn test_document_serialization_roundtrip() {
        let mut doc = Document::new("roundtrip");
        doc.meta.document_type = "profile".into();
        doc.data.insert("name".into(), Value::String("alice".into()));
        doc.data.insert("age".into(), Value::Int32(30));

        let bytes = rmp_serde::to_vec(&doc).unwrap();
        let back: Document = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(doc.meta.collection, back.meta.collection);
        assert_eq!(doc.meta.document_type, back.meta.document_type);
        assert_eq!(doc.data, back.data);
    }
}
