use std::collections::HashMap;
use std::sync::Arc;

use tracing::debug;

use crate::backend::filesystem::FilesystemBackend;
use crate::backend::BlobStore;
use crate::chunk::ChunkManager;
use crate::config::BlobConfig;
use crate::dedup::DeduplicationEngine;
use crate::download::DownloadHandler;
use crate::error::Result;
use crate::gc::GarbageCollector;
use crate::merkle::MerkleTree;
use crate::metadata::{BlobMetadata, UploadState};
use crate::namespace::NamespaceManager;
use crate::stats::BlobStats;
use crate::stats::StatsCollector;
use crate::upload::UploadManager;
use crate::upload::UploadSession;

pub struct BlobManager {
    store: Arc<dyn BlobStore>,
    chunk_manager: ChunkManager,
    dedup: Arc<DeduplicationEngine>,
    upload_mgr: UploadManager,
    download_handler: DownloadHandler,
    gc: GarbageCollector,
    ns_manager: NamespaceManager,
    stats: Arc<StatsCollector>,
    config: BlobConfig,
}

impl BlobManager {
    pub async fn new(config: BlobConfig) -> Result<Self> {
        let backend = FilesystemBackend::new(&config);
        backend.init().await?;
        let store: Arc<dyn BlobStore> = Arc::new(backend);
        Ok(Self::new_with_backend(store, config))
    }

    pub fn new_with_backend(store: Arc<dyn BlobStore>, config: BlobConfig) -> Self {
        let chunk_manager = ChunkManager::new(config.chunk_size);
        let dedup = Arc::new(DeduplicationEngine::new());
        let upload_mgr = UploadManager::new(&config);
        let download_handler = DownloadHandler::new(store.clone(), config.chunk_size);
        let gc = GarbageCollector::new(store.clone(), dedup.clone(), &config);
        let ns_manager = NamespaceManager::new(store.clone());
        let stats = Arc::new(StatsCollector::with_dedup(dedup.clone()));

        Self {
            store,
            chunk_manager,
            dedup,
            upload_mgr,
            download_handler,
            gc,
            ns_manager,
            stats,
            config,
        }
    }

    pub async fn create_blob(
        &self,
        namespace: &str,
        data: &[u8],
        content_type: &str,
        metadata: HashMap<String, String>,
    ) -> Result<BlobMetadata> {
        self.ns_manager.ensure_namespace(namespace).await?;
        self.ns_manager.check_quota(namespace, data.len() as u64)?;

        let (chunks, chunk_hashes) = self.chunk_manager.split(data);
        let sha256 = ChunkManager::hash(data);
        let merkle_root = MerkleTree::build(&chunk_hashes);

        for (chunk, hash) in chunks.iter().zip(&chunk_hashes) {
            let is_dup = self.dedup.record_chunk(hash, chunk.len() as u32);
            if !is_dup {
                self.store.put_chunk(hash, chunk).await?;
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let blob_id = uuid::Uuid::new_v4().to_string();
        let meta = BlobMetadata {
            id: blob_id.clone(),
            namespace: namespace.to_string(),
            size: data.len() as u64,
            content_type: content_type.to_string(),
            sha256,
            merkle_root,
            chunk_size: self.config.chunk_size,
            chunk_count: chunks.len() as u32,
            chunk_hashes,
            upload_id: None,
            upload_state: UploadState::Completed,
            metadata,
            created_at: now,
            expires_at: None,
        };

        self.store.put_metadata(&meta).await?;
        self.ns_manager.increment_usage(namespace, meta.size);
        self.stats.increment_blobs(1);
        self.stats.add_bytes(meta.size);
        self.stats.increment_chunks(chunks.len() as u64);

        debug!(
            "created blob {} in namespace {} ({} bytes, {} chunks)",
            blob_id, namespace, meta.size, chunks.len()
        );
        Ok(meta)
    }

    pub async fn get_blob(&self, blob_id: &str) -> Result<Vec<u8>> {
        self.download_handler.download(blob_id).await
    }

    pub async fn get_blob_range(
        &self,
        blob_id: &str,
        offset: u64,
        length: u64,
    ) -> Result<Vec<u8>> {
        self.download_handler.download_range(blob_id, offset, length).await
    }

    pub async fn delete_blob(&self, blob_id: &str) -> Result<()> {
        let meta = self.store.get_metadata(blob_id).await?;
        for chunk_hash in &meta.chunk_hashes {
            let ref_count = self.dedup.release_chunk(chunk_hash);
            if ref_count == 0 {
                self.store.delete_chunk(chunk_hash).await?;
            }
        }
        self.store.delete_metadata(blob_id).await?;
        self.ns_manager.decrement_usage(&meta.namespace, meta.size);
        self.stats.decrement_blobs(1);
        self.stats.remove_bytes(meta.size);
        debug!("deleted blob {}", blob_id);
        Ok(())
    }

    pub async fn get_metadata(&self, blob_id: &str) -> Result<BlobMetadata> {
        self.store.get_metadata(blob_id).await
    }

    pub async fn list_blobs(&self, namespace: &str) -> Result<Vec<String>> {
        self.store.list_blobs(namespace).await
    }

    pub async fn create_namespace(&self, namespace: &str) -> Result<()> {
        self.ns_manager.ensure_namespace(namespace).await
    }

    pub async fn delete_namespace(&self, namespace: &str) -> Result<()> {
        self.ns_manager.delete_namespace(namespace).await
    }

    pub async fn namespace_exists(&self, namespace: &str) -> Result<bool> {
        self.store.namespace_exists(namespace).await
    }

    pub async fn initiate_upload(
        &self,
        namespace: &str,
        content_type: &str,
        metadata: HashMap<String, String>,
    ) -> Result<UploadSession> {
        self.ns_manager.ensure_namespace(namespace).await?;
        self.stats.increment_uploads();
        self.upload_mgr.initiate(namespace, content_type, metadata)
    }

    pub async fn upload_part(&self, upload_id: &str, data: Vec<u8>) -> Result<()> {
        self.upload_mgr.upload_part(upload_id, data)
    }

    pub async fn complete_upload(&self, upload_id: &str) -> Result<BlobMetadata> {
        let (meta, chunks) = self.upload_mgr.complete(upload_id)?;

        self.ns_manager.ensure_namespace(&meta.namespace).await?;
        self.ns_manager.check_quota(&meta.namespace, meta.size)?;

        for (chunk, hash) in chunks.iter().zip(&meta.chunk_hashes) {
            let is_dup = self.dedup.record_chunk(hash, chunk.len() as u32);
            if !is_dup {
                self.store.put_chunk(hash, chunk).await?;
            }
        }

        self.store.put_metadata(&meta).await?;
        self.ns_manager.increment_usage(&meta.namespace, meta.size);
        self.stats.increment_blobs(1);
        self.stats.add_bytes(meta.size);
        self.stats.increment_chunks(meta.chunk_count as u64);
        self.stats.decrement_uploads();

        debug!(
            "completed upload {} for blob {} in namespace {}",
            upload_id, meta.id, meta.namespace
        );
        Ok(meta)
    }

    pub async fn abort_upload(&self, upload_id: &str) -> Result<()> {
        self.upload_mgr.abort(upload_id)?;
        self.stats.decrement_uploads();
        Ok(())
    }

    pub fn get_upload_session(&self, upload_id: &str) -> Result<UploadSession> {
        self.upload_mgr.get_session(upload_id)
    }

    pub async fn run_gc(&self) -> Result<usize> {
        self.gc.run_once().await
    }

    pub fn set_namespace_quota(
        &self,
        namespace: &str,
        max_blobs: u64,
        max_total_bytes: u64,
    ) {
        self.ns_manager.set_quota(namespace, max_blobs, max_total_bytes);
    }

    pub fn stats(&self) -> BlobStats {
        self.stats.snapshot()
    }

    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    pub fn dedup(&self) -> &Arc<DeduplicationEngine> {
        &self.dedup
    }

    pub fn store(&self) -> &Arc<dyn BlobStore> {
        &self.store
    }
}
