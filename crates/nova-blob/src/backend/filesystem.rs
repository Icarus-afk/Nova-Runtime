use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use super::BlobStore;
use crate::config::BlobConfig;
use crate::error::{BlobError, Result};
use crate::metadata::BlobMetadata;

pub struct FilesystemBackend {
    data_dir: PathBuf,
    chunk_nesting_depth: usize,
    _lock: Arc<RwLock<()>>,
}

impl FilesystemBackend {
    pub fn new(config: &BlobConfig) -> Self {
        Self {
            data_dir: PathBuf::from(&config.data_dir),
            chunk_nesting_depth: config.chunk_nesting_depth,
            _lock: Arc::new(RwLock::new(())),
        }
    }

    pub async fn init(&self) -> Result<()> {
        fs::create_dir_all(self.data_dir.join("metadata")).await?;
        fs::create_dir_all(self.data_dir.join("chunks")).await?;
        fs::create_dir_all(self.data_dir.join("namespaces")).await?;
        Ok(())
    }

    fn validate_blob_id(id: &str) -> Result<()> {
        if id.is_empty() {
            return Err(BlobError::InvalidInput("blob_id is required".into()));
        }
        if id.contains("..") || id.contains('/') || id.contains('\\') {
            return Err(BlobError::InvalidInput(format!("invalid blob_id: '{}'", id)));
        }
        if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return Err(BlobError::InvalidInput(format!("invalid blob_id: '{}'", id)));
        }
        Ok(())
    }

    fn validate_namespace(ns: &str) -> Result<()> {
        if ns.is_empty() {
            return Err(BlobError::InvalidInput("namespace is required".into()));
        }
        if ns.contains("..") || ns.contains('/') || ns.contains('\\') {
            return Err(BlobError::InvalidInput(format!("invalid namespace: '{}'", ns)));
        }
        if !ns.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
            return Err(BlobError::InvalidInput(format!("invalid namespace: '{}'", ns)));
        }
        Ok(())
    }

    fn metadata_path(&self, blob_id: &str) -> PathBuf {
        self.data_dir.join("metadata").join(blob_id).with_extension("json")
    }

    fn chunk_path(&self, hash: &str) -> PathBuf {
        let mut dir = self.data_dir.join("chunks");
        for i in 0..self.chunk_nesting_depth {
            let start = i * 2;
            if start + 2 <= hash.len() {
                dir = dir.join(&hash[start..start + 2]);
            }
        }
        dir.join(hash)
    }

    fn namespace_path(&self, namespace: &str) -> PathBuf {
        self.data_dir.join("namespaces").join(namespace)
    }
}

#[async_trait]
impl BlobStore for FilesystemBackend {
    async fn put_metadata(&self, metadata: &BlobMetadata) -> Result<()> {
        Self::validate_blob_id(&metadata.id)?;
        let path = self.metadata_path(&metadata.id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let data = serde_json::to_vec(metadata)
            .map_err(|e| BlobError::Internal(e.to_string()))?;
        let mut file = fs::File::create(&path).await?;
        file.write_all(&data).await?;
        file.sync_all().await?;
        Ok(())
    }

    async fn get_metadata(&self, blob_id: &str) -> Result<BlobMetadata> {
        Self::validate_blob_id(blob_id)?;
        let path = self.metadata_path(blob_id);
        let data = fs::read(&path).await
            .map_err(|_| BlobError::NotFound(blob_id.to_string()))?;
        serde_json::from_slice(&data)
            .map_err(|e| BlobError::Internal(e.to_string()))
    }

    async fn delete_metadata(&self, blob_id: &str) -> Result<()> {
        Self::validate_blob_id(blob_id)?;
        let path = self.metadata_path(blob_id);
        if path.exists() {
            fs::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn put_chunk(&self, hash: &str, data: &[u8]) -> Result<()> {
        let path = self.chunk_path(hash);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let mut file = fs::File::create(&path).await?;
        file.write_all(data).await?;
        file.sync_all().await?;
        Ok(())
    }

    async fn get_chunk(&self, hash: &str) -> Result<Vec<u8>> {
        let path = self.chunk_path(hash);
        fs::read(&path).await
            .map_err(|_| BlobError::NotFound(format!("chunk {}", hash)))
    }

    async fn delete_chunk(&self, hash: &str) -> Result<()> {
        let path = self.chunk_path(hash);
        if path.exists() {
            fs::remove_file(&path).await?;
            for ancestor in path.ancestors().skip(1).take(self.chunk_nesting_depth) {
                match fs::remove_dir(ancestor).await {
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                    Err(e) => {
                        tracing::warn!("failed to clean up dir {:?}: {}", ancestor, e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    async fn list_blobs(&self, namespace: &str) -> Result<Vec<String>> {
        Self::validate_namespace(namespace)?;
        let meta_dir = self.data_dir.join("metadata");
        if !meta_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&meta_dir).await
            .map_err(|e| BlobError::Internal(e.to_string()))?;
        let mut blobs = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await.map(|t| t.is_file()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    if let Some(blob_id) = name.strip_suffix(".json") {
                        let data = fs::read(entry.path()).await?;
                        if let Ok(meta) = serde_json::from_slice::<BlobMetadata>(&data) {
                            if meta.namespace == namespace {
                                blobs.push(blob_id.to_string());
                            }
                        }
                    }
                }
            }
        }
        Ok(blobs)
    }

    async fn list_blobs_paginated(&self, namespace: &str, offset: usize, limit: usize) -> Result<(Vec<String>, usize)> {
        Self::validate_namespace(namespace)?;
        let all = self.list_blobs(namespace).await?;
        let total = all.len();
        let page: Vec<String> = all.into_iter().skip(offset).take(limit).collect();
        Ok((page, total))
    }

    async fn namespace_exists(&self, namespace: &str) -> Result<bool> {
        Self::validate_namespace(namespace)?;
        let path = self.namespace_path(namespace);
        Ok(path.exists())
    }

    async fn create_namespace(&self, namespace: &str) -> Result<()> {
        Self::validate_namespace(namespace)?;
        let path = self.namespace_path(namespace);
        fs::create_dir_all(&path).await?;
        // Also ensure metadata dir exists for this namespace
        let meta_dir = self.data_dir.join("metadata").join(namespace);
        fs::create_dir_all(&meta_dir).await?;
        Ok(())
    }

    async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        Self::validate_namespace(namespace)?;
        let path = self.namespace_path(namespace);
        if path.exists() {
            fs::remove_dir_all(&path).await?;
        }
        let meta_dir = self.data_dir.join("metadata").join(namespace);
        if meta_dir.exists() {
            fs::remove_dir_all(&meta_dir).await?;
        }
        Ok(())
    }

    async fn list_namespaces(&self) -> Result<Vec<String>> {
        let ns_path = self.data_dir.join("namespaces");
        if !ns_path.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&ns_path).await
            .map_err(|e| BlobError::Internal(e.to_string()))?;
        let mut namespaces = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    namespaces.push(name.to_string());
                }
            }
        }
        Ok(namespaces)
    }
}
