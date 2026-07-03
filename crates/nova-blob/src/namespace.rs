use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use tracing::debug;

use crate::backend::BlobStore;
use crate::error::{BlobError, Result};

pub struct NamespaceManager {
    store: Arc<dyn BlobStore>,
    quotas: Arc<RwLock<HashMap<String, NamespaceQuota>>>,
}

#[derive(Debug, Clone)]
pub struct NamespaceQuota {
    pub max_blobs: u64,
    pub max_total_bytes: u64,
    pub current_blobs: u64,
    pub current_bytes: u64,
}

pub fn validate_namespace(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(BlobError::InvalidInput("namespace name cannot be empty".into()));
    }
    if name.len() > 255 {
        return Err(BlobError::InvalidInput("namespace name too long".into()));
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") || name.contains('\0') {
        return Err(BlobError::InvalidInput("invalid namespace name".into()));
    }
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.') {
        return Err(BlobError::InvalidInput("namespace must be alphanumeric".into()));
    }
    Ok(())
}

impl NamespaceManager {
    pub fn new(store: Arc<dyn BlobStore>) -> Self {
        Self {
            store,
            quotas: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn ensure_namespace(&self, namespace: &str) -> Result<()> {
        validate_namespace(namespace)?;
        if !self.store.namespace_exists(namespace).await? {
            self.store.create_namespace(namespace).await?;
            debug!("created namespace {}", namespace);
        }
        Ok(())
    }

    pub async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        validate_namespace(namespace)?;
        if !self.store.namespace_exists(namespace).await? {
            return Err(BlobError::NamespaceNotFound(namespace.to_string()));
        }
        let blobs = self.store.list_blobs(namespace).await?;
        if !blobs.is_empty() {
            return Err(BlobError::Internal(format!(
                "namespace {} is not empty ({} blobs)",
                namespace,
                blobs.len()
            )));
        }
        self.store.delete_namespace(namespace).await?;
        self.quotas.write().remove(namespace);
        debug!("deleted namespace {}", namespace);
        Ok(())
    }

    pub async fn list_namespaces(&self) -> Result<Vec<String>> {
        let path = std::path::Path::new("/tmp/nova/blobs/namespaces");
        // Fallback: scan via known directories
        let mut namespaces = Vec::new();
        let ns_dir = std::path::Path::new("/tmp/nova/blobs/namespaces");
        if ns_dir.exists() {
            if let Ok(mut entries) = tokio::fs::read_dir(ns_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                        if let Some(name) = entry.file_name().to_str() {
                            namespaces.push(name.to_string());
                        }
                    }
                }
            }
        }
        // Remove path that was accidentally used
        let _ = path;
        Ok(namespaces)
    }

    pub fn set_quota(&self, namespace: &str, max_blobs: u64, max_total_bytes: u64) {
        let mut quotas = self.quotas.write();
        let entry = quotas
            .entry(namespace.to_string())
            .or_insert_with(|| NamespaceQuota {
                max_blobs,
                max_total_bytes,
                current_blobs: 0,
                current_bytes: 0,
            });
        entry.max_blobs = max_blobs;
        entry.max_total_bytes = max_total_bytes;
    }

    pub fn get_quota(&self, namespace: &str) -> Option<NamespaceQuota> {
        self.quotas.read().get(namespace).cloned()
    }

    pub fn check_quota(&self, namespace: &str, additional_bytes: u64) -> Result<()> {
        if let Some(quota) = self.quotas.read().get(namespace) {
            if quota.current_blobs >= quota.max_blobs {
                return Err(BlobError::QuotaExceeded(format!(
                    "namespace {} blob count quota exceeded ({} >= {})",
                    namespace, quota.current_blobs, quota.max_blobs
                )));
            }
            if quota.current_bytes + additional_bytes > quota.max_total_bytes {
                return Err(BlobError::QuotaExceeded(format!(
                    "namespace {} byte quota exceeded ({} + {} > {})",
                    namespace, quota.current_bytes, additional_bytes, quota.max_total_bytes
                )));
            }
        }
        Ok(())
    }

    pub fn increment_usage(&self, namespace: &str, size: u64) {
        let mut quotas = self.quotas.write();
        if let Some(quota) = quotas.get_mut(namespace) {
            quota.current_blobs += 1;
            quota.current_bytes += size;
        }
    }

    pub fn decrement_usage(&self, namespace: &str, size: u64) {
        let mut quotas = self.quotas.write();
        if let Some(quota) = quotas.get_mut(namespace) {
            quota.current_blobs = quota.current_blobs.saturating_sub(1);
            quota.current_bytes = quota.current_bytes.saturating_sub(size);
        }
    }
}
