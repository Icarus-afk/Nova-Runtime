use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::dedup::DeduplicationEngine;

#[derive(Debug, Clone, Default)]
pub struct BlobStats {
    pub total_blobs: u64,
    pub total_bytes: u64,
    pub total_chunks: u64,
    pub unique_chunks: u64,
    pub chunk_dedup_savings: u64,
    pub active_uploads: u64,
    pub namespaces: u64,
}

pub struct StatsCollector {
    total_blobs: AtomicU64,
    total_bytes: AtomicU64,
    total_chunks: AtomicU64,
    active_uploads: AtomicU64,
    namespaces: AtomicU64,
    dedup: Option<Arc<DeduplicationEngine>>,
}

impl StatsCollector {
    pub fn new() -> Self {
        Self {
            total_blobs: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            total_chunks: AtomicU64::new(0),
            active_uploads: AtomicU64::new(0),
            namespaces: AtomicU64::new(0),
            dedup: None,
        }
    }

    pub fn with_dedup(dedup: Arc<DeduplicationEngine>) -> Self {
        Self {
            total_blobs: AtomicU64::new(0),
            total_bytes: AtomicU64::new(0),
            total_chunks: AtomicU64::new(0),
            active_uploads: AtomicU64::new(0),
            namespaces: AtomicU64::new(0),
            dedup: Some(dedup),
        }
    }

    pub fn increment_blobs(&self, count: u64) {
        self.total_blobs.fetch_add(count, Ordering::Relaxed);
    }

    pub fn decrement_blobs(&self, count: u64) {
        self.total_blobs.fetch_sub(count, Ordering::Relaxed);
    }

    pub fn add_bytes(&self, bytes: u64) {
        self.total_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn remove_bytes(&self, bytes: u64) {
        self.total_bytes.fetch_sub(bytes, Ordering::Relaxed);
    }

    pub fn increment_chunks(&self, count: u64) {
        self.total_chunks.fetch_add(count, Ordering::Relaxed);
    }

    pub fn increment_uploads(&self) {
        self.active_uploads.fetch_add(1, Ordering::Relaxed);
    }

    pub fn decrement_uploads(&self) {
        self.active_uploads.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn set_namespaces(&self, count: u64) {
        self.namespaces.store(count, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> BlobStats {
        let unique_chunks = self
            .dedup
            .as_ref()
            .map(|d| d.tracked_chunks().len() as u64)
            .unwrap_or(0);

        let total_tracked: u64 = self
            .dedup
            .as_ref()
            .map(|d| {
                d.tracked_chunks()
                    .iter()
                    .map(|r| r.ref_count)
                    .sum::<u64>()
            })
            .unwrap_or(0);

        let chunk_dedup_savings = if unique_chunks > 0 {
            total_tracked.saturating_sub(unique_chunks)
        } else {
            0
        };

        BlobStats {
            total_blobs: self.total_blobs.load(Ordering::Relaxed),
            total_bytes: self.total_bytes.load(Ordering::Relaxed),
            total_chunks: self.total_chunks.load(Ordering::Relaxed),
            unique_chunks,
            chunk_dedup_savings,
            active_uploads: self.active_uploads.load(Ordering::Relaxed),
            namespaces: self.namespaces.load(Ordering::Relaxed),
        }
    }
}
