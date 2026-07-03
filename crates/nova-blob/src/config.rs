use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlobConfig {
    #[serde(default = "default_chunk_size")]
    pub chunk_size: usize,
    #[serde(default = "default_max_blob_size")]
    pub max_blob_size: u64,
    #[serde(default = "default_gc_interval_secs")]
    pub gc_interval_secs: u64,
    #[serde(default = "default_gc_grace_period_secs")]
    pub gc_grace_period_secs: u64,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_chunk_nesting_depth")]
    pub chunk_nesting_depth: usize,
}

fn default_chunk_size() -> usize { 1024 * 1024 }
fn default_max_blob_size() -> u64 { 10 * 1024 * 1024 * 1024 }
fn default_gc_interval_secs() -> u64 { 3600 }
fn default_gc_grace_period_secs() -> u64 { 86400 }
fn default_data_dir() -> String { "./novad-blobs".to_string() }
fn default_chunk_nesting_depth() -> usize { 3 }

impl Default for BlobConfig {
    fn default() -> Self {
        Self {
            chunk_size: default_chunk_size(),
            max_blob_size: default_max_blob_size(),
            gc_interval_secs: default_gc_interval_secs(),
            gc_grace_period_secs: default_gc_grace_period_secs(),
            data_dir: default_data_dir(),
            chunk_nesting_depth: default_chunk_nesting_depth(),
        }
    }
}
