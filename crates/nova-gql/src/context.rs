use std::sync::Arc;
use std::time::Instant;

use nova_auth::AuthManager;
use nova_blob::BlobManager;
use nova_cache::CacheManager;
use nova_config::Config;
use nova_executor::PipelineExecutor;
use nova_queue::QueueManager;
use nova_scheduler::SchedulerManager;
use nova_search::SearchManager;
use nova_sql::SQLEngine;

pub struct AppContext {
    pub started_at: Instant,
    pub pipeline: Arc<PipelineExecutor>,
    pub config: Arc<parking_lot::RwLock<Config>>,
    pub memory_mgr: Option<Arc<nova_memory::MemoryManager>>,
    pub cache: Arc<CacheManager>,
    pub queue: Arc<QueueManager>,
    pub scheduler: Arc<SchedulerManager>,
    pub search: Arc<SearchManager>,
    pub blob: Arc<BlobManager>,
    pub auth: Arc<AuthManager>,
    pub sql: Arc<SQLEngine>,
}
