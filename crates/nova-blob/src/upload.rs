use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use crate::chunk::ChunkManager;
use crate::config::BlobConfig;
use crate::error::{BlobError, Result};
use crate::merkle::MerkleTree;
use crate::metadata::{BlobMetadata, UploadState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartInfo {
    pub part_number: usize,
    pub size: u64,
    pub hash: String,
}

#[derive(Debug, Clone)]
pub struct UploadSession {
    pub upload_id: String,
    pub namespace: String,
    pub blob_id: String,
    pub content_type: String,
    pub total_size: u64,
    pub uploaded_parts: Vec<Vec<u8>>,
    pub completed: bool,
    pub aborted: bool,
    pub metadata: HashMap<String, String>,
    pub created_at: i64,
}

pub struct UploadManager {
    sessions: Arc<RwLock<HashMap<String, UploadSession>>>,
    config: BlobConfig,
}

impl UploadManager {
    pub fn new(config: &BlobConfig) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config: config.clone(),
        }
    }

    pub fn initiate(
        &self,
        namespace: &str,
        content_type: &str,
        user_metadata: HashMap<String, String>,
        declared_total_size: u64,
    ) -> Result<UploadSession> {
        let upload_id = Uuid::new_v4().to_string();
        let blob_id = Uuid::new_v4().to_string();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let session = UploadSession {
            upload_id: upload_id.clone(),
            namespace: namespace.to_string(),
            blob_id,
            content_type: content_type.to_string(),
            total_size: declared_total_size,
            uploaded_parts: Vec::new(),
            completed: false,
            aborted: false,
            metadata: user_metadata,
            created_at: now,
        };

        self.sessions.write().insert(upload_id.clone(), session);

        let session = self.sessions.read().get(&upload_id)
            .expect("upload session was just inserted")
            .clone();
        debug!("initiated multipart upload {}", upload_id);
        Ok(session)
    }

    pub fn upload_part(&self, upload_id: &str, data: Vec<u8>) -> Result<()> {
        let mut sessions = self.sessions.write();
        let session = sessions
            .get_mut(upload_id)
            .ok_or_else(|| BlobError::UploadNotFound(upload_id.to_string()))?;

        if session.completed {
            return Err(BlobError::Internal("upload already completed".to_string()));
        }
        if session.aborted {
            return Err(BlobError::Internal("upload aborted".to_string()));
        }

        let accumulated: u64 = session.uploaded_parts.iter().map(|p| p.len() as u64).sum::<u64>() + data.len() as u64;
        if accumulated > self.config.max_blob_size {
            return Err(BlobError::QuotaExceeded(format!(
                "blob size {} exceeds max {}",
                accumulated, self.config.max_blob_size
            )));
        }

        session.uploaded_parts.push(data);
        debug!(
            "uploaded part {} for upload {}",
            session.uploaded_parts.len(),
            upload_id
        );
        Ok(())
    }

    pub fn complete(&self, upload_id: &str) -> Result<(BlobMetadata, Vec<Vec<u8>>)> {
        let mut sessions = self.sessions.write();
        let mut session = sessions
            .remove(upload_id)
            .ok_or_else(|| BlobError::UploadNotFound(upload_id.to_string()))?;

        if session.aborted {
            return Err(BlobError::Internal("upload aborted".to_string()));
        }

        let actual_size: u64 = session.uploaded_parts.iter().map(|p| p.len() as u64).sum();
        if actual_size != session.total_size {
            return Err(BlobError::Internal(format!(
                "declared total size {} does not match actual part sizes {}",
                session.total_size, actual_size
            )));
        }

        session.completed = true;

        let all_data: Vec<u8> = session.uploaded_parts.iter().flat_map(|p| p.iter().copied()).collect();
        let chunk_manager = ChunkManager::new(self.config.chunk_size);
        let (chunks, chunk_hashes) = chunk_manager.split(&all_data);
        let sha256 = ChunkManager::hash(&all_data);
        let merkle_root = MerkleTree::build(&chunk_hashes);

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let metadata = BlobMetadata {
            id: session.blob_id,
            namespace: session.namespace,
            size: session.total_size,
            content_type: session.content_type,
            sha256,
            merkle_root,
            chunk_size: self.config.chunk_size,
            chunk_count: chunks.len() as u32,
            chunk_hashes,
            upload_id: Some(upload_id.to_string()),
            upload_state: UploadState::Completed,
            metadata: session.metadata,
            created_at: now,
            expires_at: None,
        };

        debug!("completed multipart upload {}", upload_id);
        Ok((metadata, chunks))
    }

    pub fn abort(&self, upload_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write();
        let session = sessions
            .get_mut(upload_id)
            .ok_or_else(|| BlobError::UploadNotFound(upload_id.to_string()))?;
        session.aborted = true;
        debug!("aborted upload {}", upload_id);
        Ok(())
    }

    pub fn get_session(&self, upload_id: &str) -> Result<UploadSession> {
        self.sessions
            .read()
            .get(upload_id)
            .cloned()
            .ok_or_else(|| BlobError::UploadNotFound(upload_id.to_string()))
    }

    pub fn list_parts(&self, upload_id: &str) -> Result<Vec<PartInfo>> {
        let sessions = self.sessions.read();
        let session = sessions
            .get(upload_id)
            .ok_or_else(|| BlobError::UploadNotFound(upload_id.to_string()))?;
        let parts = session
            .uploaded_parts
            .iter()
            .enumerate()
            .map(|(i, data)| PartInfo {
                part_number: i + 1,
                size: data.len() as u64,
                hash: ChunkManager::hash(data),
            })
            .collect();
        Ok(parts)
    }
}
