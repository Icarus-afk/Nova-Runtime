use async_graphql::*;
use chrono::Utc;
use uuid::Uuid;

use crate::error::ContextExt;
use crate::input::*;
use crate::types::*;

#[derive(Default)]
pub struct MutationRoot;

#[Object]
impl MutationRoot {
    // ============================================================
    // Runtime
    // ============================================================

    async fn update_configuration(
        &self,
        ctx: &Context<'_>,
        input: ConfigurationInput,
    ) -> Result<ServerConfiguration> {
        let app = ctx.app()?;
        {
            let mut cfg = app.config.write();
            if let Some(level) = input.log_level {
                cfg.logging.level = level;
            }
            if let Some(timeout) = input.query_timeout_ms {
                cfg.execution.default_operation_timeout_ms = timeout as u64;
            }
            if let Some(conn) = input.max_connections {
                cfg.general.max_connections = conn as u32;
            }
        }
        let ctx = ctx.app()?;
        let cfg = ctx.config.read();
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

    async fn set_log_level(&self, ctx: &Context<'_>, level: String) -> Result<ServerConfiguration> {
        let app = ctx.app()?;
        {
            let mut cfg = app.config.write();
            cfg.logging.level = level;
        }
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

    // ============================================================
    // Database
    // ============================================================

    async fn sql_execute(
        &self,
        ctx: &Context<'_>,
        query: String,
        _params: Option<Vec<Option<serde_json::Value>>>,
    ) -> Result<SqlQueryResult> {
        let app = ctx.app()?;
        let result = app.sql.execute(&query)
            .map_err(|e| FieldError::new(format!("SQL error: {}", e)))?;

        match result {
            nova_sql::engine::SQLResult::Exec { rows_affected, stats } => {
                Ok(SqlQueryResult {
                    columns: Vec::new(),
                    rows: Vec::new(),
                    row_count: rows_affected as i32,
                    execution_time_ms: stats.execution_time_ms as f64,
                    warnings: Vec::new(),
                })
            }
            _ => Err(FieldError::new("Query did not return an execution result")),
        }
    }

    async fn create_table(
        &self,
        ctx: &Context<'_>,
        _name: String,
        _definition: String,
    ) -> Result<TableInfo> {
        let _app = ctx.app()?;
        Err(FieldError::new("Table creation not supported via GraphQL"))
    }

    async fn drop_table(
        &self,
        ctx: &Context<'_>,
        name: String,
        _if_exists: Option<bool>,
    ) -> Result<bool> {
        let _app = ctx.app()?;
        Err(FieldError::new(format!("Cannot drop table '{}'", name)))
    }

    // ============================================================
    // Cache
    // ============================================================

    async fn cache_set(
        &self,
        ctx: &Context<'_>,
        input: CacheSetInput,
    ) -> Result<CacheEntry> {
        let app = ctx.app()?;
        let value = serde_json::to_vec(&input.value)
            .map_err(|e| FieldError::new(format!("Serialization error: {}", e)))?;
        let ttl = input.ttl_ms.map(|ms| std::time::Duration::from_millis(ms as u64));
        app.cache.set(input.key.clone(), value, ttl).await
            .map_err(|e| FieldError::new(format!("Cache error: {}", e)))?;
        let now = Utc::now().to_rfc3339();
        Ok(CacheEntry {
            key: input.key,
            value: input.value,
            data_type: "JSON".into(),
            ttl_ms: input.ttl_ms,
            expires_at: None,
            size_bytes: 0,
            created_at: now.clone(),
            last_accessed_at: now,
            access_count: 1,
        })
    }

    async fn cache_delete(
        &self,
        ctx: &Context<'_>,
        key: String,
    ) -> Result<bool> {
        let app = ctx.app()?;
        app.cache.delete(&key).await
            .map_err(|e| FieldError::new(format!("Cache error: {}", e)))
    }

    async fn cache_flush(&self, ctx: &Context<'_>) -> Result<bool> {
        let app = ctx.app()?;
        app.cache.flush().await
            .map_err(|e| FieldError::new(format!("Cache error: {}", e)))?;
        Ok(true)
    }

    // ============================================================
    // Queue
    // ============================================================

    async fn create_queue(
        &self,
        ctx: &Context<'_>,
        input: CreateQueueInput,
    ) -> Result<Queue> {
        let app = ctx.app()?;
        app.queue.create_queue(&input.name).await
            .map_err(|e| FieldError::new(format!("Queue error: {}", e)))?;
        let now = Utc::now().to_rfc3339();
        Ok(Queue {
            name: input.name.clone(),
            description: input.description,
            created_at: now.clone(),
            updated_at: now,
            message_count: 0,
            messages_sent: 0,
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

    async fn delete_queue(
        &self,
        ctx: &Context<'_>,
        name: String,
    ) -> Result<bool> {
        let app = ctx.app()?;
        app.queue.delete_queue(&name).await
            .map_err(|e| FieldError::new(format!("Queue error: {}", e)))?;
        Ok(true)
    }

    async fn queue_send(
        &self,
        ctx: &Context<'_>,
        queue: String,
        input: QueueSendInput,
    ) -> Result<QueueMessage> {
        let app = ctx.app()?;
        let body = serde_json::to_vec(&input.body)
            .map_err(|e| FieldError::new(format!("Serialization error: {}", e)))?;
        app.queue.enqueue(&queue, body).await
            .map_err(|e| FieldError::new(format!("Queue error: {}", e)))?;
        let now = Utc::now().to_rfc3339();
        Ok(QueueMessage {
            id: Uuid::new_v4(),
            body: input.body,
            content_type: input.content_type.unwrap_or_else(|| "application/json".into()),
            sent_at: now.clone(),
            first_received_at: None,
            receive_count: 0,
            visibility_timeout_expires_at: None,
            delay_until: None,
            attributes: MessageAttributes {
                priority: input.priority.map(|p| match p {
                    MessagePriorityInput::Low => MessagePriority::Low,
                    MessagePriorityInput::Normal => MessagePriority::Normal,
                    MessagePriorityInput::High => MessagePriority::High,
                    MessagePriorityInput::Critical => MessagePriority::Critical,
                }).unwrap_or(MessagePriority::Normal),
                deduplication_id: input.deduplication_id,
                group_id: input.group_id,
                sender: None,
                custom: None,
            },
        })
    }

    async fn queue_receive(
        &self,
        ctx: &Context<'_>,
        queue: String,
        max_messages: Option<i32>,
    ) -> Result<Vec<QueueMessage>> {
        let app = ctx.app()?;
        let count = max_messages.unwrap_or(1).max(1) as u32;
        let messages = app.queue.dequeue(&queue, count).await
            .map_err(|e| FieldError::new(format!("Queue error: {}", e)))?;
        let mut result = Vec::new();
        for msg in messages {
            let body: serde_json::Value = serde_json::from_slice(&msg.body)
                .unwrap_or(serde_json::Value::Null);
            result.push(QueueMessage {
                id: msg.id,
                body,
                content_type: "application/json".into(),
                sent_at: chrono::DateTime::from_timestamp_millis(msg.enqueued_at)
                    .map(|d| d.to_rfc3339()).unwrap_or_default(),
                first_received_at: None,
                receive_count: msg.attempt_count as i32,
                visibility_timeout_expires_at: None,
                delay_until: msg.delay_until
                    .and_then(|t| chrono::DateTime::from_timestamp_millis(t).map(|d| d.to_rfc3339())),
                attributes: MessageAttributes {
                    priority: match msg.priority {
                        nova_queue::MessagePriority::Low => MessagePriority::Low,
                        nova_queue::MessagePriority::Normal => MessagePriority::Normal,
                        nova_queue::MessagePriority::High => MessagePriority::High,
                        nova_queue::MessagePriority::Critical => MessagePriority::Critical,
                    },
                    deduplication_id: msg.deduplication_id,
                    group_id: msg.group_id,
                    sender: None,
                    custom: None,
                },
            })
        }
        Ok(result)
    }

    async fn queue_delete_message(
        &self,
        ctx: &Context<'_>,
        queue: String,
        message_id: Uuid,
    ) -> Result<bool> {
        let _app = ctx.app()?;
        Err(FieldError::new(format!("Message {} not found in queue '{}'", message_id, queue)))
    }

    // ============================================================
    // Scheduler
    // ============================================================

    async fn create_job(
        &self,
        ctx: &Context<'_>,
        input: CreateJobInput,
    ) -> Result<Job> {
        let app = ctx.app()?;
        let scheduled_at = chrono::Utc::now().timestamp_millis();
        let payload = serde_json::to_vec(&input.input.unwrap_or(serde_json::Value::Null))
            .unwrap_or_default();
        let mut job = nova_scheduler::Job::new(&input.name, scheduled_at, payload);
        job.max_retries = input.max_retries.unwrap_or(3).max(0) as u32;
        job.timeout_secs = (input.timeout_ms.unwrap_or(30000) / 1000).max(1) as u32;
        job.schedule_type = match input.job_type {
            JobTypeInput::Cron => nova_scheduler::ScheduleType::Cron,
            JobTypeInput::ScheduledOnce => nova_scheduler::ScheduleType::OneTime,
            JobTypeInput::EventDriven => nova_scheduler::ScheduleType::OneTime,
        };
        if let Some(schedule) = input.schedule {
            job.cron_expression = Some(schedule);
        }
        app.scheduler.schedule_job(job).await
            .map_err(|e| FieldError::new(format!("Scheduler error: {}", e)))?;
        Err(FieldError::new("Job creation stub"))
    }

    async fn delete_job(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<bool> {
        let app = ctx.app()?;
        app.scheduler.cancel_job(&id).await
            .map_err(|e| FieldError::new(format!("Scheduler error: {}", e)))?;
        Ok(true)
    }

    async fn pause_job(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<Job> {
        let _app = ctx.app()?;
        Err(FieldError::new(format!("Job {} not found", id)))
    }

    async fn resume_job(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> Result<Job> {
        let _app = ctx.app()?;
        Err(FieldError::new(format!("Job {} not found", id)))
    }

    async fn trigger_job(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        _input: Option<serde_json::Value>,
    ) -> Result<JobExecution> {
        let _app = ctx.app()?;
        Err(FieldError::new(format!("Job {} not found", id)))
    }

    // ============================================================
    // Search
    // ============================================================

    async fn create_search_index(
        &self,
        ctx: &Context<'_>,
        _input: CreateSearchIndexInput,
    ) -> Result<SearchIndex> {
        let _app = ctx.app()?;
        Err(FieldError::new("Search index creation not supported"))
    }

    async fn index_document(
        &self,
        ctx: &Context<'_>,
        _index: String,
        _document: serde_json::Value,
        _id: Option<Uuid>,
    ) -> Result<SearchDocument> {
        let _app = ctx.app()?;
        Err(FieldError::new("Document indexing not supported"))
    }

    async fn delete_document(
        &self,
        ctx: &Context<'_>,
        _index: String,
        _id: Uuid,
    ) -> Result<bool> {
        let _app = ctx.app()?;
        Err(FieldError::new("Document deletion not supported"))
    }

    // ============================================================
    // Blob
    // ============================================================

    async fn blob_upload(
        &self,
        ctx: &Context<'_>,
        input: BlobUploadInput,
    ) -> Result<Blob> {
        let _app = ctx.app()?;
        let now = Utc::now().to_rfc3339();
        Ok(Blob {
            key: input.key,
            size_bytes: input.content.len() as i64,
            content_type: input.content_type.unwrap_or_else(|| "application/octet-stream".into()),
            content_encoding: input.content_encoding,
            etag: String::new(),
            md5: String::new(),
            sha256: String::new(),
            storage_tier: input.storage_tier.map(|t| match t {
                StorageTierInput::Hot => StorageTier::Hot,
                StorageTierInput::Warm => StorageTier::Warm,
                StorageTierInput::Cold => StorageTier::Cold,
            }).unwrap_or(StorageTier::Hot),
            created_at: now.clone(),
            updated_at: now,
            expires_at: input.expires_at,
            metadata: BlobMetadata {
                filename: input.metadata.as_ref().and_then(|m| m.filename.clone()),
                description: input.metadata.as_ref().and_then(|m| m.description.clone()),
                tags: input.metadata.as_ref().map(|m| m.tags.clone().unwrap_or_default()).unwrap_or_default(),
                custom: input.metadata.as_ref().and_then(|m| m.custom.clone()),
            },
            url: String::new(),
        })
    }

    async fn blob_delete(
        &self,
        ctx: &Context<'_>,
        _key: String,
    ) -> Result<bool> {
        let _app = ctx.app()?;
        Ok(true)
    }

    // ============================================================
    // Auth
    // ============================================================

    async fn login(
        &self,
        ctx: &Context<'_>,
        _input: LoginInput,
    ) -> Result<AuthResult> {
        let _app = ctx.app()?;
        Err(FieldError::new("Invalid credentials"))
    }

    async fn logout(&self, ctx: &Context<'_>) -> Result<bool> {
        let _app = ctx.app()?;
        Ok(true)
    }

    async fn create_api_key(
        &self,
        ctx: &Context<'_>,
        _input: CreateApiKeyInput,
    ) -> Result<ApiKeyFull> {
        let _app = ctx.app()?;
        Err(FieldError::new("API key creation not supported"))
    }

    async fn revoke_api_key(
        &self,
        ctx: &Context<'_>,
        _id: Uuid,
    ) -> Result<bool> {
        let _app = ctx.app()?;
        Err(FieldError::new("API key not found"))
    }
}
