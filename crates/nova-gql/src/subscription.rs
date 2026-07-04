use async_graphql::*;
use chrono::Utc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::ContextExt;
use crate::types::*;

#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn health_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl futures_util::Stream<Item = HealthStatus>> {
        let app = ctx.app()?;
        let uptime = app.started_at.elapsed().as_secs() as i64;
        let now = Utc::now().to_rfc3339();
        let status = HealthStatus {
            status: HealthState::Healthy,
            uptime_seconds: uptime,
            version: env!("CARGO_PKG_VERSION").to_string(),
            subsystems: vec![
                SubsystemHealth {
                    name: "storage".into(),
                    status: HealthState::Healthy,
                    latency_ms: 0.0,
                    last_error: None,
                    last_checked: now.clone(),
                },
                SubsystemHealth {
                    name: "memory".into(),
                    status: HealthState::Healthy,
                    latency_ms: 0.0,
                    last_error: None,
                    last_checked: now.clone(),
                },
            ],
            last_startup: now,
        };

        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(status).await;

        Ok(ReceiverStream::new(rx))
    }

    async fn configuration_changed(
        &self,
        ctx: &Context<'_>,
    ) -> Result<impl futures_util::Stream<Item = ServerConfiguration>> {
        let app = ctx.app()?;
        let (log_level, max_connections, query_timeout_ms, subsystems) = {
            let cfg = app.config.read();
            (
                cfg.logging.level.clone(),
                cfg.general.max_connections as i32,
                cfg.execution.default_operation_timeout_ms as i32,
                SubsystemConfigs {
                    database: DatabaseConfig {
                        max_connections: 10,
                        statement_cache_size: 100,
                        default_fetch_size: 100,
                        transaction_timeout_ms: 30000,
                    },
                    cache: CacheConfig {
                        max_memory_mb: (cfg.memory.max_memory / 1024 / 1024) as i32,
                        default_ttl_ms: (cfg.cache.default_ttl_secs as i64) * 1000,
                        eviction_policy: cfg.cache.eviction_policy.clone(),
                        max_item_size_bytes: 1048576,
                    },
                    queue: QueueConfig {
                        max_queues: cfg.queue.max_queues as i32,
                        default_visibility_timeout_ms: cfg.queue.default_visibility_timeout_secs as i32 * 1000,
                        max_message_size_bytes: cfg.queue.max_message_size as i32,
                        message_retention_ms: (cfg.queue.message_ttl_secs as i64) * 1000,
                        dead_letter_max_receives: cfg.queue.max_receive_count as i32,
                    },
                    scheduler: SchedulerConfig {
                        max_jobs: cfg.scheduler.max_jobs_per_queue as i32,
                        scheduler_interval_ms: cfg.scheduler.priority_queue_tick_ms as i32,
                        max_retries: cfg.scheduler.default_max_retries as i32,
                        default_timeout_ms: cfg.scheduler.default_job_timeout_secs as i32 * 1000,
                    },
                    search: SearchConfig {
                        max_indexes: 10,
                        default_analyzer: "standard".into(),
                        max_result_window: 10000,
                    },
                    blob: BlobConfig {
                        max_blob_size_mb: (cfg.blob.max_blob_size / 1024 / 1024) as i32,
                        storage_path: cfg.blob.data_dir.clone(),
                        default_tier: "HOT".into(),
                    },
                    auth: AuthConfig {
                        token_expiry_ms: (cfg.auth.session.ttl_seconds as i64) * 1000,
                        refresh_token_expiry_ms: (cfg.auth.session.ttl_seconds as i64) * 1000 * 30,
                        max_api_keys_per_user: 10,
                        session_timeout_ms: (cfg.auth.session.ttl_seconds as i64) * 1000,
                        password_min_length: cfg.auth.internal.password_policy.min_length as i32,
                        bcrypt_cost: cfg.auth.internal.bcrypt_cost as i32,
                    },
                },
            )
        };
        let config = ServerConfiguration {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_mode: if cfg!(debug_assertions) { "DEBUG".into() } else { "RELEASE".into() },
            log_level,
            max_connections,
            query_timeout_ms,
            subsystems,
        };

        let (tx, rx) = mpsc::channel(1);
        let _ = tx.send(config).await;

        Ok(ReceiverStream::new(rx))
    }
}
