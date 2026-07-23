# 04. REST API Reference

Nova Runtime provides a comprehensive REST API for managing and interacting with all subsystems. This document details all implemented endpoints, their methods, request/response formats, and examples.

## 1. Base URL

The API is served at `http://127.0.0.1:8642` by default. You can change the address and port in the `novad.toml` configuration file.

## 2. Authentication

Most endpoints require authentication via an API key or a JWT token. Include the API key or token in the `Authorization` header:

```http
Authorization: Bearer <API_KEY_OR_JWT>
```

## 3. Endpoints

### Admin Endpoints

#### `GET /health`

Returns the health status of the runtime.

**Response:**
```json
{
  "status": "healthy",
  "uptime_secs": 12345,
  "version": "X.Y.Z",
  "memory": {
    "total_bytes": 1073741824,
    "used_bytes": 123456789
  },
  "disk": {
    "total_bytes": 0,
    "used_bytes": 0,
    "free_bytes": 0
  },
  "subsystems": {
    "database": {"status": "healthy"},
    "cache": {"status": "healthy"},
    "queue": {"status": "healthy"},
    "scheduler": {"status": "healthy"},
    "search": {"status": "healthy"},
    "blob": {"status": "healthy"}
  }
}
```

#### `GET /ready`

Returns the readiness status of the runtime.

**Response:**
```json
{
  "status": "ready"
}
```

#### `GET /live`

Returns the liveness status of the runtime.

**Response:**
```json
{
  "status": "alive"
}
```

#### `GET /metrics`

Returns Prometheus-compatible metrics.

**Response:**
```
# HELP nova_uptime_secs Server uptime in seconds
# TYPE nova_uptime_secs gauge
nova_uptime_secs 12345

# HELP nova_operations_total Total operations processed
# TYPE nova_operations_total counter
nova_operations_total 42

# HELP nova_active_operations Currently in-flight operations
# TYPE nova_active_operations gauge
nova_active_operations 0

# HELP nova_queue_depth Current operation queue depth
# TYPE nova_queue_depth gauge
nova_queue_depth 0

# HELP nova_rate_limit_hits Total rate limit hits
# TYPE nova_rate_limit_hits counter
nova_rate_limit_hits 0

# HELP nova_circuit_breaker_opens Total circuit breaker state transitions
# TYPE nova_circuit_breaker_opens counter
nova_circuit_breaker_opens 0

# HELP nova_retry_attempts Total retry attempts
# TYPE nova_retry_attempts counter
nova_retry_attempts 0

# HELP nova_errors_total Total errors by category
# TYPE nova_errors_total counter
nova_errors_total{category="parse"} 0
nova_errors_total{category="validation"} 0
nova_errors_total{category="authorization"} 0
nova_errors_total{category="execution"} 0

# HELP nova_latency_avg_ns Average operation latency in nanoseconds
# TYPE nova_latency_avg_ns gauge
nova_latency_avg_ns 0
```

#### `GET /admin/config`

Returns the full runtime configuration.

**Response:**
```json
{
  "general": {
    "data_dir": "/var/lib/novad",
    "pid_file": "/var/run/novad.pid",
    "max_connections": 1024,
    "shutdown_timeout_ms": 5000,
    "startup_timeout_ms": 30000
  },
  "storage": {
    "wal_dir": "/var/lib/novad/wal",
    "wal_segment_size": 67108864,
    "fsync_policy": {"every_n_ms": 100},
    "block_cache_size": 268435456,
    "page_cache_size": 67108864,
    "memtable_size": 67108864,
    "max_blob_size": 10737418240,
    "compression": "Snappy",
    "bloom_filter_bits_per_key": 10,
    "page_size": 8192,
    "wal_page_size": 4096,
    "btree_order": 4,
    "lsm_max_level": 7,
    "bloom_false_positive_rate": 0.01,
    "write_buffer_size": 67108864,
    "compaction_threads": 2
  },
  "memory": {
    "max_memory": 1073741824,
    "pressure_threshold_pct": 80,
    "critical_threshold_pct": 95,
    "emergency_reserve": 33554432,
    "gc_threshold_pct": 70
  },
  "networking": {
    "listen_address": "127.0.0.1",
    "listen_port": 8642,
    "tls_enabled": false,
    "tcp_nodelay": true,
    "keepalive_secs": 30,
    "listeners": [{"address": ":443", "enabled": true}],
    "timeouts": {"read_timeout_ms": 30000, "write_timeout_ms": 60000},
    "rate_limiting": {"default_tokens_per_second": 1000, "default_burst_size": 2000}
  },
  "logging": {
    "level": "info",
    "format": "text"
  },
  "subsystems": {
    "enable_sql": true,
    "enable_cache": true,
    "enable_queue": true,
    "enable_scheduler": true,
    "enable_search": true,
    "enable_blob": true,
    "enable_auth": true,
    "enable_dashboard": true
  },
  "event": {
    "ordering_shards": 64,
    "default_queue_capacity": 1024,
    "default_max_retries": 3,
    "dlq_max_entries": 100000
  },
  "execution": {
    "max_concurrent": 1024,
    "worker_threads": 4,
    "execution_timeout_ms": 30000,
    "max_concurrent_ops": 256,
    "pipeline_queue_depth": 1024,
    "default_operation_timeout_ms": 5000,
    "max_operation_timeout_ms": 60000,
    "rate_limit_default_per_sec": 1000,
    "rate_limit_global_per_sec": 10000,
    "rate_limit_global_burst": 20000,
    "rate_limit_user_per_sec": 100,
    "rate_limit_user_burst": 200,
    "rate_limit_ip_per_sec": 1000,
    "rate_limit_ip_burst": 2000,
    "circuit_breaker_threshold": 50,
    "circuit_breaker_window_ms": 10000,
    "circuit_breaker_half_open_timeout_ms": 10000,
    "circuit_breaker_success_threshold": 10,
    "audit_enabled": true,
    "audit_include_payloads": false,
    "audit_max_entry_size": 4096,
    "idempotency_key_ttl_secs": 86400,
    "max_idempotency_keys": 100000,
    "pipeline_max_retries": 3,
    "retry_base_delay_ms": 10,
    "retry_max_delay_ms": 1000
  },
  "auth": {
    "internal": {
      "password_policy": {
        "min_length": 8,
        "max_length": 128,
        "min_lowercase": 1,
        "min_uppercase": 1,
        "min_digits": 1,
        "min_special": 0
      },
      "lockout": {
        "max_attempts": 5,
        "duration_secs": 900
      },
      "bcrypt_cost": 12,
      "enable_brute_force_detection": true,
      "mfa": {
        "issuer": "Nova Runtime",
        "window": 1
      }
    },
    "session": {
      "ttl_seconds": 86400,
      "max_active_sessions": 100,
      "token_length_bytes": 32,
      "cache_size": 100000
    }
  },
  "security": {
    "encryption_at_rest": {
      "enabled": false
    }
  },
  "cache": {
    "max_size": 134217728,
    "default_ttl_secs": 300,
    "eviction_policy": "Lru",
    "backend_type": "HashMap"
  },
  "blob": {
    "chunk_size": 1048576,
    "max_blob_size": 10737418240,
    "gc_interval_secs": 3600,
    "gc_grace_period_secs": 86400,
    "data_dir": "./novad-blobs",
    "chunk_nesting_depth": 3
  },
  "search": {
    "default_limit": 10,
    "max_limit": 1000,
    "bm25_k1": 1.2,
    "bm25_b": 0.75,
    "fuzzy_max_distance": 2,
    "highlight_snippet_len": 150,
    "highlight_max_snippets": 3,
    "refresh_interval_ms": 1000,
    "merge_segment_threshold": 5
  },
  "sql": {
    "max_batch_size": 1024,
    "max_columns": 256,
    "default_limit": 1000
  },
  "queue": {
    "max_queues": 1000,
    "max_messages_per_queue": 10000,
    "max_message_size": 262144,
    "default_visibility_timeout_secs": 30,
    "message_ttl_secs": 86400,
    "max_receive_count": 3,
    "scanner_interval_ms": 1000,
    "backpressure_threshold": 0.9,
    "dlq_max_entries": 100000,
    "dlq_max_retries": 3,
    "enable_dlq": true,
    "enable_scanners": true
  },
  "scheduler": {
    "time_wheel_tick_ms": 100,
    "time_wheel_slots": 360,
    "priority_queue_tick_ms": 1000,
    "max_jobs_per_queue": 10000,
    "max_concurrent_jobs": 64,
    "default_job_timeout_secs": 300,
    "default_max_retries": 3,
    "default_retry_delay_secs": 10,
    "enable_startup_recovery": true,
    "enable_catch_up": true
  },
  "version": 1
}
```

#### `PUT /admin/config`

Updates the runtime configuration. Accepts a partial JSON object, which is deeply merged with the existing configuration. The updated configuration is validated and applied in-memory.

**Request:**
```json
{
  "logging": {"level": "debug"},
  "networking": {"listen_port": 8080}
}
```

**Response:**
```json
{
  "general": {"data_dir": "/var/lib/novad", ...},
  "logging": {"level": "debug", "format": "text"},
  "networking": {"listen_address": "127.0.0.1", "listen_port": 8080, ...},
  ...
}
```

#### `GET /admin/status`

Returns the status of the execution pipeline.

**Response:**
```json
{
  "operations_total": 42,
  "active_operations": 0,
  "queue_depth": 0,
  "rate_limit_hits": 0,
  "circuit_opens": 0,
  "retries": 0,
  "parse_errors": 0,
  "validation_errors": 0,
  "authorization_errors": 0,
  "execution_errors": 0,
  "avg_latency_ns": 0
}
```

#### `GET /openapi.json`

Returns an OpenAPI 3.0.3 specification stub.

**Response:**
```json
{
  "openapi": "3.0.3",
  "info": {
    "title": "Nova Runtime API",
    "version": "X.Y.Z"
  },
  "paths": {}
}
```

#### `GET /runtime/status`

Alias for `/admin/status`.

#### `GET /runtime/info`

Returns runtime information.

**Response:**
```json
{
  "version": "X.Y.Z",
  "uptime_secs": 12345
}
```

#### `GET /runtime/config`

Alias for `/admin/config`.

### API v1 Endpoints

#### `POST /api/v1/sql/query`

Execute a SQL query.

**Request:**
```json
{
  "query": "SELECT * FROM users",
  "params": [],
  "limit": 1000,
  "format": "json"
}
```

**Response:**
```json
{
  "columns": ["id", "name", "email"],
  "column_names": ["id", "name", "email"],
  "types": ["integer", "text", "text"],
  "rows": [
    [1, "Alice", "alice@example.com"],
    [2, "Bob", "bob@example.com"]
  ],
  "row_count": 2,
  "truncated": false,
  "execution_time_ms": 12
}
```

#### `POST /api/v1/sql/execute`

Execute a SQL statement (INSERT/UPDATE/DELETE).

**Request:**
```json
{
  "query": "INSERT INTO users (name, email) VALUES ('Alice', 'alice@example.com')",
  "params": []
}
```

**Response:**
```json
{
  "affected_rows": 1,
  "execution_time_ms": 8
}
```

#### `GET /api/v1/sql/tables`

List all SQL tables.

**Response:**
```json
{
  "data": [
    {"name": "users", "document_count": 2},
    {"name": "products", "document_count": 5}
  ],
  "pagination": {"cursor": null, "limit": 50, "has_more": false}
}
```

#### `GET /api/v1/sql/tables/{table}/schema`

Get the schema of a SQL table.

**Response:**
```json
{
  "table": "users",
  "columns": [
    {"name": "id", "type": "Integer", "nullable": false, "is_primary_key": true, "unique": true},
    {"name": "name", "type": "String", "nullable": false, "is_primary_key": false, "unique": false},
    {"name": "email", "type": "String", "nullable": false, "is_primary_key": false, "unique": true}
  ]
}
```

#### `GET /api/v1/cache/{key}`

Get a cache entry.

**Response:**
```json
{
  "key": "my_key",
  "value": "my_value",
  "ttl_remaining_ms": null
}
```

#### `POST /api/v1/cache/{key}`

Set a cache entry.

**Request:**
```json
{
  "value": "my_value",
  "ttl_ms": 300000
}
```

**Response:**
```json
{
  "status": "set"
}
```

#### `DELETE /api/v1/cache/{key}`

Delete a cache entry.

**Response:**
```json
{
  "status": "deleted"
}
```

#### `POST /api/v1/cache/batch`

Set multiple cache entries in a batch.

**Request:**
```json
[
  {"key": "key1", "value": "value1", "ttl_ms": 300000},
  {"key": "key2", "value": "value2", "ttl_ms": 600000}
]
```

**Response:**
```json
{
  "status": "set",
  "count": 2
}
```

#### `GET /api/v1/cache/keys`

List cache keys.

**Response:**
```json
{
  "data": ["key1", "key2"],
  "total": 2,
  "pattern": null
}
```

#### `GET /api/v1/cache/stats`

Get cache statistics.

**Response:**
```json
{
  "keys": 123,
  "hits": 456,
  "misses": 789,
  "hit_rate": 0.36,
  "memory_bytes": 0,
  "evictions": 12
}
```

#### `POST /api/v1/queues/`

Create a queue.

**Request:**
```json
{
  "name": "myqueue",
  "durable": true,
  "max_length": 10000
}
```

**Response:**
```json
{
  "id": "q_myqueue",
  "name": "myqueue",
  "status": "created"
}
```

#### `GET /api/v1/queues/`

List all queues.

**Response:**
```json
{
  "data": [
    {
      "name": "myqueue",
      "queue_type": "standard",
      "available": 0,
      "in_flight": 0,
      "delayed": 0,
      "total": 0,
      "paused": false
    }
  ],
  "pagination": {"cursor": null, "limit": 100, "has_more": false}
}
```

#### `GET /api/v1/queues/{name}`

Get a queue.

**Response:**
```json
{
  "name": "myqueue",
  "queue_type": "standard",
  "max_size": 10000,
  "paused": false
}
```

#### `DELETE /api/v1/queues/{name}`

Delete a queue.

**Response:**
```json
{
  "status": "deleted"
}
```

#### `POST /api/v1/queues/{name}/messages`

Publish messages to a queue.

**Request:**
```json
{
  "messages": [
    {"body": "Hello, world!"},
    {"body": "Another message", "delay_ms": 5000}
  ]
}
```

**Response:**
```json
{
  "published_count": 2,
  "message_ids": ["msg_123", "msg_456"]
}
```

#### `POST /api/v1/queues/{name}/messages/poll`

Poll messages from a queue.

**Request:**
```json
{
  "count": 5,
  "visibility_timeout_ms": 30000
}
```

**Response:**
```json
{
  "messages": [
    {
      "id": "msg_123",
      "body": "Hello, world!",
      "receipt_handle": "abc123",
      "delivery_attempt": 1
    }
  ],
  "message_count": 1
}
```

#### `POST /api/v1/queues/{name}/messages/{id}/ack`

Acknowledge a message.

**Response:**
```json
{
  "status": "acknowledged"
}
```

#### `POST /api/v1/queues/{name}/purge`

Purge a queue.

**Response:**
```json
{
  "status": "purged"
}
```

#### `GET /api/v1/queues/{name}/stats`

Get queue statistics.

**Response:**
```json
{
  "available_messages": 0,
  "in_flight_messages": 0,
  "delayed_messages": 0,
  "total_messages": 0,
  "dlq_messages": 0,
  "messages_enqueued": 0,
  "messages_dequeued": 0
}
```

#### `POST /api/v1/scheduler/jobs`

Create a job.

**Request:**
```json
{
  "name": "myjob",
  "type": "cron",
  "schedule": "0 0 * * *",
  "action": {"command": "echo 'Hello, world!'"},
  "max_retries": 3,
  "retry_delay_ms": 10000,
  "enabled": true
}
```

**Response:**
```json
{
  "id": "job_123",
  "name": "myjob",
  "status": "created",
  "next_run_at": "2023-01-01T00:00:00Z"
}
```

#### `GET /api/v1/scheduler/jobs`

List all jobs.

**Response:**
```json
{
  "data": [
    {
      "id": "job_123",
      "name": "myjob",
      "schedule_type": "cron",
      "state": "pending",
      "next_run_at": "2023-01-01T00:00:00Z",
      "last_run_at": null,
      "retry_count": 0
    }
  ],
  "pagination": {"cursor": null, "limit": 100, "has_more": false}
}
```

#### `GET /api/v1/scheduler/jobs/{id}`

Get a job.

**Response:**
```json
{
  "id": "job_123",
  "name": "myjob",
  "schedule_type": "cron",
  "state": "pending",
  "next_run_at": "2023-01-01T00:00:00Z",
  "last_run_at": null,
  "max_retries": 3,
  "retry_count": 0
}
```

#### `DELETE /api/v1/scheduler/jobs/{id}`

Delete a job.

**Response:**
```json
{
  "status": "deleted"
}
```

#### `POST /api/v1/scheduler/jobs/{id}/trigger`

Trigger a job.

**Response:**
```json
{
  "status": "triggered"
}
```

#### `POST /api/v1/scheduler/jobs/{id}/pause`

Pause a job.

**Response:**
```json
{
  "status": "paused"
}
```

#### `POST /api/v1/scheduler/jobs/{id}/resume`

Resume a job.

**Response:**
```json
{
  "status": "resumed"
}
```

#### `GET /api/v1/scheduler/stats`

Get scheduler statistics.

**Response:**
```json
{
  "jobs_pending": 1,
  "jobs_running": 0,
  "jobs_completed": 0,
  "jobs_failed": 0,
  "jobs_cancelled": 0,
  "total_scheduled": 1,
  "total_executed": 0,
  "total_failures": 0
}
```

#### `POST /api/v1/search/indexes`

Create a search index.

**Request:**
```json
{
  "name": "myindex",
  "fields": [
    {"name": "title", "type": "text", "analyzer": "standard", "boost": 1.0},
    {"name": "content", "type": "text", "analyzer": "english", "boost": 2.0}
  ]
}
```

**Response:**
```json
{
  "id": "idx_myindex",
  "name": "myindex",
  "status": "created"
}
```

#### `GET /api/v1/search/indexes`

List all search indexes.

**Response:**
```json
{
  "data": [],
  "pagination": {"cursor": null, "limit": 50, "has_more": false}
}
```

#### `GET /api/v1/search/indexes/{name}`

Get a search index.

**Response:**
```json
{
  "name": "myindex",
  "num_docs": 0,
  "num_terms": 0,
  "field_count": 0
}
```

#### `DELETE /api/v1/search/indexes/{name}`

Delete a search index.

**Response:**
```json
{
  "status": "deleted"
}
```

#### `POST /api/v1/search/indexes/{name}/documents`

Index documents.

**Request:**
```json
{
  "documents": [
    {"id": "doc1", "title": "Hello", "content": "World"},
    {"id": "doc2", "title": "Foo", "content": "Bar"}
  ]
}
```

**Response:**
```json
{
  "status": "indexed",
  "count": 2
}
```

#### `POST /api/v1/search/indexes/{name}/query`

Search an index.

**Request:**
```json
{
  "query": "hello world",
  "limit": 10,
  "offset": 0
}
```

**Response:**
```json
{
  "hits": [
    {
      "id": "doc1",
      "score": 0.95,
      "source": {"title": "Hello", "content": "World"}
    }
  ],
  "total_hits": 1,
  "execution_time_ms": 12
}
```

#### `GET /api/v1/search/indexes/{name}/stats`

Get search index statistics.

**Response:**
```json
{
  "num_docs": 0,
  "num_terms": 0,
  "field_count": 0
}
```

#### `POST /api/v1/blobs/`

Upload a blob.

**Request:**
```
Headers:
  Content-Type: application/octet-stream

Body:
  (Binary data)
```

**Response:**
```json
{
  "id": "blob_123",
  "size_bytes": 12345,
  "content_type": "application/octet-stream",
  "checksum_sha256": "a1b2c3...",
  "created_at": "2023-01-01T00:00:00Z"
}
```

#### `GET /api/v1/blobs/`

List blobs.

**Response:**
```json
{
  "data": [
    {
      "id": "blob_123",
      "filename": "blob_123",
      "size_bytes": 12345,
      "content_type": "application/octet-stream",
      "created_at": "2023-01-01T00:00:00Z"
    }
  ],
  "pagination": {"cursor": null, "limit": 50, "has_more": false}
}
```

#### `GET /api/v1/blobs/{id}`

Download a blob.

**Response:**
```
Headers:
  Content-Type: application/octet-stream
  X-Blob-Size: 12345
  X-Blob-Checksum-SHA256: a1b2c3...

Body:
  (Binary data)
```

#### `DELETE /api/v1/blobs/{id}`

Delete a blob.

**Response:**
```json
{
  "status": "deleted"
}
```

#### `GET /api/v1/blobs/{id}/info`

Get blob metadata.

**Response:**
```json
{
  "id": "blob_123",
  "size_bytes": 12345,
  "content_type": "application/octet-stream",
  "checksum_sha256": "a1b2c3...",
  "created_at": "2023-01-01T00:00:00Z",
  "metadata": {}
}
```

#### `GET /api/v1/blobs/stats`

Get blob statistics.

**Response:**
```json
{
  "total_blobs": 123,
  "total_bytes": 4567890,
  "total_chunks": 789,
  "unique_chunks": 456,
  "active_uploads": 0,
  "namespaces": 1
}
```

### Auth Endpoints

#### `POST /api/v1/auth/login`

Authenticate a user.

**Request:**
```json
{
  "username": "admin",
  "password": "admin123",
  "ttl_seconds": 3600
}
```

**Response:**
```json
{
  "token_type": "Bearer",
  "access_token": "abc123...",
  "expires_in": 3600,
  "refresh_token": null,
  "refresh_expires_in": null
}
```

#### `POST /api/v1/auth/refresh`

Refresh a session.

**Request:**
```json
{
  "refresh_token": "abc123..."
}
```

**Response:**
```json
{
  "token_type": "Bearer",
  "access_token": "def456...",
  "expires_in": 3600
}
```

#### `POST /api/v1/auth/logout`

Invalidate a session.

**Response:**
```json
{
  "status": "logged_out"
}
```

#### `POST /api/v1/auth/api-keys`

Create an API key.

**Request:**
```json
{
  "name": "myapp",
  "permissions": ["read", "write"],
  "expires_at": "2023-12-31"
}
```

**Response:**
```json
{
  "id": "key_123",
  "name": "myapp",
  "key": "abc123...",
  "prefix": "nova_sk_123",
  "permissions": ["read", "write"],
  "created_at": "2023-01-01T00:00:00Z"
}
```

#### `GET /api/v1/auth/api-keys`

List API keys.

**Response:**
```json
{
  "data": [
    {
      "id": "key_123",
      "name": "myapp",
      "prefix": "nova_sk_123",
      "permissions": ["read", "write"],
      "created_at": "2023-01-01T00:00:00Z",
      "expires_at": "2023-12-31T00:00:00Z",
      "enabled": true
    }
  ],
  "pagination": {"cursor": null, "limit": 50, "has_more": false}
}
```

#### `DELETE /api/v1/auth/api-keys/{id}`

Revoke an API key.

**Response:**
```json
{
  "status": "revoked",
  "id": "key_123"
}
```

#### `POST /api/v1/auth/users`

Create a user.

**Request:**
```json
{
  "username": "alice",
  "password": "password123",
  "roles": ["user"]
}
```

**Response:**
```json
{
  "id": "user_123",
  "username": "alice",
  "roles": ["user"],
  "status": "created"
}
```

#### `GET /api/v1/auth/users`

List users.

**Response:**
```json
{
  "data": [
    {
      "id": "user_123",
      "username": "alice",
      "roles": ["user"],
      "created_at": "2023-01-01T00:00:00Z"
    }
  ],
  "pagination": {"cursor": null, "limit": 50, "has_more": false}
}
```

#### `GET /api/v1/auth/users/{id}`

Get a user.

**Response:**
```json
{
  "id": "user_123",
  "username": "alice",
  "roles": ["user"],
  "created_at": "2023-01-01T00:00:00Z"
}
```

#### `DELETE /api/v1/auth/users/{id}`

Delete a user.

**Response:**
```json
{
  "status": "deleted"
}
```

#### `PUT /api/v1/auth/users/{id}/roles`

Update user roles.

**Request:**
```json
{
  "roles": ["admin", "user"]
}
```

**Response:**
```json
{
  "status": "updated"
}
```

#### `PUT /api/v1/auth/users/{id}/password`

Change a user's password.

**Request:**
```json
{
  "password": "newpassword123"
}
```

**Response:**
```json
{
  "status": "updated"
}
```

## 4. WebSocket Endpoint

#### `GET /api/v1/ws`

Establish a WebSocket connection for real-time event streaming.

## 5. GraphQL Endpoint

#### `GET /graphql`

Open the GraphQL Playground interface.

#### `POST /graphql`

Execute a GraphQL query or mutation.

**Request:**
```json
{
  "query": "query { version }",
  "variables": {}
}
```

**Response:**
```json
{
  "data": {
    "version": "X.Y.Z"
  }
}
```

## 6. Error Responses

| Status Code | Error Type | Description |
| :---------- | :--------- | :---------- |
| 400 | `bad_request` | Invalid request parameters or body. |
| 401 | `unauthorized` | Missing or invalid authentication. |
| 403 | `forbidden` | Insufficient permissions. |
| 404 | `not_found` | Resource not found. |
| 422 | `validation_failed` | Configuration validation failed. |
| 500 | `internal` | Internal server error.

All error responses include a JSON body with an `error` field:

```json
{
  "error": "error_type",
  "detail": "Error message"
}
```