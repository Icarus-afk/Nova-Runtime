use async_graphql::*;
use chrono::Utc;
use uuid::Uuid;

use crate::error::ContextExt;
use crate::input::*;
use crate::types::*;

#[derive(Default)]
pub struct QueryRoot;

#[Object]
impl QueryRoot {
    // ============================================================
    // Runtime
    // ============================================================

    async fn health(&self, ctx: &Context<'_>) -> Result<HealthStatus> {
        let app = ctx.app()?;
        let uptime = app.started_at.elapsed().as_secs() as i64;
        let now = Utc::now().to_rfc3339();
        Ok(HealthStatus {
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
            ],
            last_startup: now,
        })
    }

    async fn configuration(&self, ctx: &Context<'_>) -> Result<ServerConfiguration> {
        let app = ctx.app()?;
        let cfg = app.config.read();
        Ok(ServerConfiguration {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_mode: if cfg!(debug_assertions) { "DEBUG".into() } else { "RELEASE".into() },
            log_level: cfg.logging.level.clone(),
            max_connections: cfg.general.max_connections as i32,
            query_timeout_ms: cfg.execution.default_operation_timeout_ms as i32,
            subsystems: SubsystemConfigs {
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
        })
    }

    async fn metrics(&self, ctx: &Context<'_>, _input: Option<MetricsInput>) -> Result<MetricsSnapshot> {
        let app = ctx.app()?;
        let snap = app.pipeline.metrics().snapshot();
        let now = Utc::now().to_rfc3339();
        Ok(MetricsSnapshot {
            collected_at: now.clone(),
            time_range: MetricsTimeRange { start: now.clone(), end: now },
            system: SystemMetrics {
                cpu_usage_percent: 0.0,
                memory_usage_bytes: 0,
                memory_total_bytes: 0,
                disk_usage_bytes: 0,
                disk_total_bytes: 0,
                network_bytes_in: 0,
                network_bytes_out: 0,
                open_file_descriptors: 0,
                goroutines: 0,
            },
            subsystems: SubsystemMetrics {
                database: Some(DatabaseMetrics {
                    queries_total: snap.operations_total as i64,
                    queries_per_second: 0.0,
                    avg_latency_ms: snap.avg_latency_ns as f64 / 1_000_000.0,
                    p50_latency_ms: snap.p50_latency_ns as f64 / 1_000_000.0,
                    p95_latency_ms: 0.0,
                    p99_latency_ms: snap.p99_latency_ns as f64 / 1_000_000.0,
                    active_connections: snap.active_operations as i32,
                    cache_hit_rate: 0.0,
                    transactions_committed: 0,
                    transactions_rolled_back: 0,
                }),
                cache: None,
                queue: None,
                scheduler: None,
                search: None,
                blob: None,
            },
        })
    }

    async fn version(&self) -> Result<VersionInfo> {
        Ok(VersionInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            build_commit: env!("CARGO_PKG_VERSION").to_string(),
            build_date: Utc::now().to_rfc3339(),
            rust_version: "1.85".into(),
        })
    }

    // ============================================================
    // Database
    // ============================================================

    async fn sql_query(
        &self,
        ctx: &Context<'_>,
        query: String,
        _params: Option<Vec<Option<serde_json::Value>>>,
    ) -> Result<SqlQueryResult> {
        let app = ctx.app()?;
        let result = app.sql.execute(&query)
            .map_err(|e| FieldError::new(format!("SQL error: {}", e)))?;
        match result {
            nova_sql::engine::SQLResult::Query { batches, stats } => {
                let columns = if batches.is_empty() {
                    Vec::new()
                } else {
                    batch_columns(&batches[0])
                };
                let col_infos: Vec<ColumnInfo> = columns.iter().map(|(name, _sql_type)| ColumnInfo {
                    name: name.clone(),
                    data_type: "TEXT".into(),
                    nullable: true,
                    primary_key: false,
                    default_value: None,
                    comment: None,
                }).collect();
                let rows: Vec<serde_json::Value> = batches.iter().flat_map(|batch| {
                    (0..batch.num_rows).map(|row_idx| {
                        let mut map = serde_json::Map::new();
                        for (col_idx, (name, _)) in columns.iter().enumerate() {
                            if col_idx < batch.columns.len() {
                                use nova_sql::result::Column;
                                let val = match &batch.columns[col_idx] {
                                    Column::Integer(v) => v.get(row_idx).copied().flatten()
                                        .map(|x| serde_json::Value::Number(x.into())),
                                    Column::Float(v) => v.get(row_idx).copied().flatten()
                                        .map(|x| serde_json::json!(x)),
                                    Column::Boolean(v) => v.get(row_idx).copied().flatten()
                                        .map(serde_json::Value::Bool),
                                    Column::String(v) => v.get(row_idx).cloned().flatten()
                                        .map(serde_json::Value::String),
                                    Column::Null(_) => None,
                                };
                                map.insert(name.clone(), val.unwrap_or(serde_json::Value::Null));
                            }
                        }
                        serde_json::Value::Object(map)
                    }).collect::<Vec<_>>()
                }).collect();
                Ok(SqlQueryResult {
                    columns: col_infos,
                    row_count: rows.len() as i32,
                    execution_time_ms: stats.execution_time_ms as f64,
                    rows,
                    warnings: Vec::new(),
                })
            }
            nova_sql::engine::SQLResult::Exec { rows_affected, stats } => {
                Ok(SqlQueryResult {
                    columns: Vec::new(),
                    rows: Vec::new(),
                    row_count: rows_affected as i32,
                    execution_time_ms: stats.execution_time_ms as f64,
                    warnings: Vec::new(),
                })
            }
        }
    }

    async fn tables(&self, _ctx: &Context<'_>) -> Result<Vec<TableInfo>> {
        Ok(Vec::new())
    }

    async fn table(&self, _ctx: &Context<'_>, name: String) -> Result<TableInfo> {
        Err(FieldError::new(format!("Table '{}' not found", name)))
    }

    async fn schema_info(&self, _ctx: &Context<'_>) -> Result<SchemaInfo> {
        Ok(SchemaInfo {
            version: 1,
            tables: 0,
            size_bytes: 0,
            last_migration: Utc::now().to_rfc3339(),
        })
    }

    async fn database_stats(&self, _ctx: &Context<'_>) -> Result<DatabaseStats> {
        Ok(DatabaseStats {
            query_count: 0,
            avg_query_time_ms: 0.0,
            p95_query_time_ms: 0.0,
            cache_hit_rate: 0.0,
            active_connections: 0,
            deadlocks_detected: 0,
            transactions: TransactionStats {
                committed: 0,
                rolled_back: 0,
                active: 0,
                avg_duration_ms: 0.0,
            },
        })
    }

    // ============================================================
    // Cache
    // ============================================================

    async fn cache_get(&self, ctx: &Context<'_>, key: String) -> Result<Option<CacheEntry>> {
        let app = ctx.app()?;
        match app.cache.get(&key).await {
            Ok(Some(val)) => {
                let now = Utc::now().to_rfc3339();
                Ok(Some(CacheEntry {
                    key,
                    value: serde_json::json!({}),
                    data_type: "STRING".into(),
                    ttl_ms: None,
                    expires_at: None,
                    size_bytes: val.len() as i32,
                    created_at: now.clone(),
                    last_accessed_at: now,
                    access_count: 0,
                }))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(FieldError::new(format!("Cache error: {}", e))),
        }
    }

    async fn cache_exists(&self, ctx: &Context<'_>, key: String) -> Result<bool> {
        let app = ctx.app()?;
        app.cache.exists(&key).await
            .map_err(|e| FieldError::new(format!("Cache error: {}", e)))
    }

    async fn cache_stats(&self, ctx: &Context<'_>) -> Result<CacheStats> {
        let app = ctx.app()?;
        let metrics = app.cache.metrics();
        let len = app.cache.len().await.unwrap_or(0);
        Ok(CacheStats {
            hit_count: metrics.hits() as i64,
            miss_count: metrics.misses() as i64,
            hit_rate: metrics.hit_rate(),
            entry_count: len as i32,
            memory_used_bytes: 0,
            max_memory_bytes: app.cache.config().max_size as i64,
            eviction_count: metrics.evictions() as i64,
            avg_ttl_ms: (app.cache.config().default_ttl_secs as f64) * 1000.0,
            keyspace_hits: metrics.hits() as i64,
            keyspace_misses: metrics.misses() as i64,
        })
    }

    async fn cache_keys(
        &self,
        _ctx: &Context<'_>,
        _pattern: Option<String>,
        _input: Option<PaginationInput>,
    ) -> Result<Vec<CacheEntry>> {
        Ok(Vec::new())
    }

    // ============================================================
    // Queue
    // ============================================================

    async fn queues(&self, ctx: &Context<'_>) -> Result<Vec<Queue>> {
        let app = ctx.app()?;
        let summaries = app.queue.list_queues().await
            .map_err(|e| FieldError::new(format!("Queue error: {}", e)))?;
        let mut queues = Vec::new();
        for s in summaries {
            queues.push(Queue {
                name: s.name.clone(),
                description: None,
                created_at: Utc::now().to_rfc3339(),
                updated_at: Utc::now().to_rfc3339(),
                message_count: s.total as i64,
                messages_sent: s.total as i64,
                messages_received: 0,
                messages_deleted: 0,
                messages_dead_lettered: 0,
                oldest_message_age_ms: 0,
                config: QueueConfigStats {
                    visibility_timeout_ms: 30000,
                    max_message_size_bytes: 262144,
                    message_retention_ms: 86400000,
                    dead_letter_max_receives: 5,
                    dead_letter_queue: false,
                    delivery_delay_ms: 0,
                },
            })
        }
        Ok(queues)
    }

    async fn queue(&self, _ctx: &Context<'_>, name: String) -> Result<Queue> {
        Err(FieldError::new(format!("Queue '{}' not found", name)))
    }

    async fn queue_stats(&self, ctx: &Context<'_>) -> Result<QueueOverallStats> {
        let app = ctx.app()?;
        let summaries = app.queue.list_queues().await
            .map_err(|e| FieldError::new(format!("Queue error: {}", e)))?;
        let total_messages: i64 = summaries.iter().map(|s| s.total as i64).sum();
        Ok(QueueOverallStats {
            total_queues: summaries.len() as i32,
            total_messages,
            total_messages_sent: total_messages,
            total_messages_received: 0,
            total_messages_dead_lettered: 0,
            avg_queue_depth: if summaries.is_empty() { 0.0 } else { total_messages as f64 / summaries.len() as f64 },
            avg_processing_time_ms: 0.0,
        })
    }

    async fn dead_letter_stats(&self, _ctx: &Context<'_>, _queue: String) -> Result<DeadLetterStats> {
        Ok(DeadLetterStats {
            total_dead_lettered: 0,
            total_dead_letter_queues: 0,
            top_reasons: Vec::new(),
        })
    }

    // ============================================================
    // Scheduler
    // ============================================================

    async fn jobs(&self, ctx: &Context<'_>) -> Result<Vec<Job>> {
        let app = ctx.app()?;
        let summaries = app.scheduler.list_jobs(None).await
            .map_err(|e| FieldError::new(format!("Scheduler error: {}", e)))?;
        let mut jobs = Vec::new();
        for s in summaries {
            jobs.push(Job {
                id: s.id,
                name: s.name,
                description: None,
                job_type: match s.schedule_type {
                    nova_scheduler::ScheduleType::OneTime => JobType::ScheduledOnce,
                    nova_scheduler::ScheduleType::Interval => JobType::Cron,
                    nova_scheduler::ScheduleType::Cron => JobType::Cron,
                },
                state: match s.state {
                    nova_scheduler::JobState::Pending => JobStateEnum::Active,
                    nova_scheduler::JobState::Running => JobStateEnum::Active,
                    nova_scheduler::JobState::Completed => JobStateEnum::Completed,
                    nova_scheduler::JobState::Failed => JobStateEnum::Failed,
                    nova_scheduler::JobState::Cancelled => JobStateEnum::Cancelled,
                    nova_scheduler::JobState::Skipped => JobStateEnum::Completed,
                },
                schedule: None,
                max_retries: 3,
                retry_count: s.retry_count as i32,
                timeout_ms: 300000,
                created_at: String::new(),
                updated_at: String::new(),
                last_executed_at: s.last_run_at
                    .and_then(|t| chrono::DateTime::from_timestamp_millis(t).map(|d| d.to_rfc3339())),
                last_error: None,
                next_execution_at: Some(chrono::DateTime::from_timestamp_millis(s.next_run_at)
                    .map(|d| d.to_rfc3339()).unwrap_or_default()),
                tags: Vec::new(),
                input: None,
                metadata: JobMetadata {
                    total_executions: 0,
                    successful_executions: 0,
                    failed_executions: 0,
                    avg_duration_ms: 0.0,
                    total_duration_ms: 0,
                    last_execution_id: None,
                },
            })
        }
        Ok(jobs)
    }

    async fn job(&self, ctx: &Context<'_>, id: Uuid) -> Result<Job> {
        let app = ctx.app()?;
        let job = app.scheduler.get_job(&id).await
            .map_err(|e| FieldError::new(format!("Job not found: {}", e)))?;
        Ok(Job {
            id: job.id,
            name: job.name,
            description: None,
            job_type: match job.schedule_type {
                nova_scheduler::ScheduleType::OneTime => JobType::ScheduledOnce,
                nova_scheduler::ScheduleType::Interval => JobType::Cron,
                nova_scheduler::ScheduleType::Cron => JobType::Cron,
            },
            state: match job.state {
                nova_scheduler::JobState::Pending | nova_scheduler::JobState::Running => JobStateEnum::Active,
                nova_scheduler::JobState::Completed => JobStateEnum::Completed,
                nova_scheduler::JobState::Failed => JobStateEnum::Failed,
                nova_scheduler::JobState::Cancelled => JobStateEnum::Cancelled,
                nova_scheduler::JobState::Skipped => JobStateEnum::Completed,
            },
            schedule: None,
            max_retries: job.max_retries as i32,
            retry_count: job.retry_count as i32,
            timeout_ms: job.timeout_secs as i32 * 1000,
            created_at: chrono::DateTime::from_timestamp_millis(job.created_at)
                .map(|d| d.to_rfc3339()).unwrap_or_default(),
            updated_at: chrono::DateTime::from_timestamp_millis(job.updated_at)
                .map(|d| d.to_rfc3339()).unwrap_or_default(),
            last_executed_at: job.last_run_at
                .and_then(|t| chrono::DateTime::from_timestamp_millis(t).map(|d| d.to_rfc3339())),
            last_error: None,
            next_execution_at: Some(chrono::DateTime::from_timestamp_millis(job.next_run_at)
                .map(|d| d.to_rfc3339()).unwrap_or_default()),
            tags: job.tags.into_values().collect(),
            input: serde_json::from_slice(&job.payload).ok(),
            metadata: JobMetadata {
                total_executions: 0,
                successful_executions: 0,
                failed_executions: 0,
                avg_duration_ms: 0.0,
                total_duration_ms: 0,
                last_execution_id: None,
            },
        })
    }

    async fn scheduler_stats(&self, ctx: &Context<'_>) -> Result<SchedulerStats> {
        let app = ctx.app()?;
        let jobs = app.scheduler.list_jobs(None).await.unwrap_or_default();
        let running = jobs.iter().filter(|j| j.state == nova_scheduler::JobState::Running).count() as i32;
        let pending = jobs.iter().filter(|j| j.state == nova_scheduler::JobState::Pending).count() as i32;
        let failed = jobs.iter().filter(|j| j.state == nova_scheduler::JobState::Failed).count() as i32;
        let completed = jobs.iter().filter(|j| j.state == nova_scheduler::JobState::Completed).count() as i32;
        Ok(SchedulerStats {
            total_jobs: jobs.len() as i32,
            active_jobs: running,
            paused_jobs: pending,
            failed_jobs: failed,
            completed_jobs: completed,
            executions_total: 0,
            executions_today: 0,
            avg_execution_time_ms: 0.0,
            p95_execution_time_ms: 0.0,
            p99_execution_time_ms: 0.0,
            success_rate: if jobs.is_empty() { 1.0 } else { completed as f64 / jobs.len() as f64 },
            triggers_fired_total: 0,
        })
    }

    // ============================================================
    // Search
    // ============================================================

    async fn search(
        &self,
        ctx: &Context<'_>,
        index: String,
        query: String,
        _options: Option<SearchOptions>,
    ) -> Result<SearchResultConnection> {
        let app = ctx.app()?;
        let results = app.search.search(&query, 25).unwrap_or_default();
        let edges: Vec<SearchResultEdge> = results.iter().enumerate().map(|(i, r)| {
            SearchResultEdge {
                node: SearchResult {
                    id: Uuid::new_v4(),
                    index: index.clone(),
                    document: serde_json::json!({}),
                    score: r.score,
                },
                cursor: base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    format!("cursor:{}", i),
                ),
                score: r.score,
            }
        }).collect();
        let start_cursor = edges.first().map(|e| e.cursor.clone());
        let end_cursor = edges.last().map(|e| e.cursor.clone());
        Ok(SearchResultConnection {
            total_count: edges.len() as i32,
            edges,
            max_score: results.first().map(|r| r.score).unwrap_or(0.0),
            took_ms: 0.0,
            page_info: PageInfo {
                has_next_page: false,
                has_previous_page: false,
                start_cursor,
                end_cursor,
            },
        })
    }

    async fn search_indexes(&self, _ctx: &Context<'_>) -> Result<Vec<SearchIndex>> {
        Ok(Vec::new())
    }

    async fn search_stats(&self, _ctx: &Context<'_>) -> Result<SearchStats> {
        Ok(SearchStats {
            total_indexes: 0,
            total_documents: 0,
            total_size_bytes: 0,
            avg_index_time_ms: 0.0,
            avg_query_time_ms: 0.0,
            p95_query_time_ms: 0.0,
            queries_total: 0,
            indexing_total: 0,
        })
    }

    // ============================================================
    // Blob
    // ============================================================

    async fn blob(&self, _ctx: &Context<'_>, _key: String) -> Result<Option<Blob>> {
        Ok(None)
    }

    async fn blob_exists(&self, _ctx: &Context<'_>, _key: String) -> Result<bool> {
        Ok(false)
    }

    async fn blob_stats(&self, _ctx: &Context<'_>) -> Result<BlobStats> {
        Ok(BlobStats {
            total_blobs: 0,
            total_size_bytes: 0,
            total_hot_bytes: 0,
            total_warm_bytes: 0,
            total_cold_bytes: 0,
            avg_blob_size_bytes: 0.0,
            largest_blob_bytes: 0,
            uploads_total: 0,
            downloads_total: 0,
            deletes_total: 0,
        })
    }

    // ============================================================
    // Auth
    // ============================================================

    async fn me(&self, _ctx: &Context<'_>) -> Result<User> {
        Err(FieldError::new("Not authenticated"))
    }

    async fn users(&self, _ctx: &Context<'_>) -> Result<Vec<User>> {
        Ok(Vec::new())
    }

    async fn roles(&self, _ctx: &Context<'_>) -> Result<Vec<Role>> {
        Ok(Vec::new())
    }

    async fn permissions(&self, _ctx: &Context<'_>) -> Result<Vec<Permission>> {
        Ok(Vec::new())
    }

    async fn api_keys(&self, _ctx: &Context<'_>) -> Result<Vec<ApiKey>> {
        Ok(Vec::new())
    }
}

fn batch_columns(batch: &nova_sql::result::RecordBatch) -> Vec<(String, String)> {
    batch.columns.iter().enumerate().map(|(i, _col)| {
        (format!("col_{}", i), "TEXT".into())
    }).collect()
}
