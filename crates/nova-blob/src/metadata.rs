use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMetadata {
    pub id: String,
    pub namespace: String,
    pub size: u64,
    pub content_type: String,
    pub sha256: String,
    pub merkle_root: String,
    pub chunk_size: usize,
    pub chunk_count: u32,
    pub chunk_hashes: Vec<String>,
    pub upload_id: Option<String>,
    pub upload_state: UploadState,
    pub metadata: HashMap<String, String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UploadState {
    Pending,
    Completed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub hash: String,
    pub size: u32,
    pub ref_count: u64,
    pub created_at: i64,
}
