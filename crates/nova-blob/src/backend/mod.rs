use async_trait::async_trait;

use crate::error::Result;
use crate::metadata::BlobMetadata;

#[async_trait]
pub trait BlobStore: Send + Sync {
    async fn put_metadata(&self, metadata: &BlobMetadata) -> Result<()>;
    async fn get_metadata(&self, blob_id: &str) -> Result<BlobMetadata>;
    async fn delete_metadata(&self, blob_id: &str) -> Result<()>;
    async fn put_chunk(&self, hash: &str, data: &[u8]) -> Result<()>;
    async fn get_chunk(&self, hash: &str) -> Result<Vec<u8>>;
    async fn delete_chunk(&self, hash: &str) -> Result<()>;
    async fn list_blobs(&self, namespace: &str) -> Result<Vec<String>>;
    async fn namespace_exists(&self, namespace: &str) -> Result<bool>;
    async fn create_namespace(&self, namespace: &str) -> Result<()>;
    async fn delete_namespace(&self, namespace: &str) -> Result<()>;
}

pub mod filesystem;
