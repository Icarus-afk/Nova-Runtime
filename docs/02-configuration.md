# 02. Configuration

Nova Runtime is configured through a single `novad.toml` file, which defines all operational parameters for the daemon and its various subsystems. This document details the structure of `novad.toml`, how to manage configurations at runtime, and the validation rules applied.

## 1. `novad.toml` Structure

The `novad.toml` file is organized into 16 top-level sections, each corresponding to a specific area of the runtime. Below is an overview of each section with its default values and key parameters.

### General Configuration (`[general]`)

Core daemon settings.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `data_dir` | PathBuf | `/var/lib/novad` | Directory for all persistent data (databases, indexes, blobs). |
| `pid_file` | PathBuf | `/var/run/novad.pid` | Path to the PID file. |
| `max_connections` | `u32` | `1024` | Maximum number of concurrent client connections. |
| `shutdown_timeout_ms` | `u64` | `5000` | Timeout for graceful shutdown in milliseconds. |
| `startup_timeout_ms` | `u64` | `30000` | Timeout for daemon startup in milliseconds. |

### Storage Configuration (`[storage]`)

Settings for the internal storage engine, including WAL, caching, and LSM tree parameters.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `wal_dir` | PathBuf | `/var/lib/novad/wal` | Directory for Write-Ahead Log segments. |
| `wal_segment_size` | `u64` | `67108864` (64 MB) | Size of each WAL segment. |
| `fsync_policy` | String / Table | `"every_n_ms" = 100` | Controls when data is fsync'd to disk. Options: `"EveryWrite"`, `"Async"`, or `{ every_n_ms = <u64> }`. |
| `block_cache_size` | `u64` | `268435456` (256 MB) | Total size for the block cache. |
| `page_cache_size` | `u64` | `67108864` (64 MB) | Total size for the page cache. |
| `memtable_size` | `u64` | `67108864` (64 MB) | Maximum size of in-memory memtables before flushing to disk. |
| `max_blob_size` | `u64` | `10737418240` (10 GB) | Maximum size of a single blob that can be stored. |
| `compression` | String | `"Snappy"` | Compression algorithm for data blocks. Options: `"Snappy"`, `"Lz4"`, `"Zstd"`, `"None"`. |
| `bloom_filter_bits_per_key` | `u32` | `10` | Bits per key for Bloom filters. |
| `page_size` | `u16` | `8192` | Size of a storage page. Must be a power of 2 (4096, 8192, 16384, 32768). |
| `wal_page_size` | `u16` | `4096` | Size of a WAL page. |
| `btree_order` | `u8` | `4` | Order of B-trees. Must be between 2 and 32. |
| `lsm_max_level` | `u8` | `7` | Maximum level for LSM tree. Must be between 1 and 10. |
| `bloom_false_positive_rate` | `f64` | `0.01` | Desired false positive rate for Bloom filters. Must be > 0.0 and <= 0.1. |
| `write_buffer_size` | `u64` | `67108864` (64 MB) | Size of the write buffer. |
| `compaction_threads` | `u8` | `2` | Number of threads for background compaction. |

### Memory Configuration (`[memory]`)

Settings for runtime memory management.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `max_memory` | `u64` | `1073741824` (1 GB) | Total memory limit for the daemon in bytes. |
| `pressure_threshold_pct` | `u8` | `80` | Percentage of `max_memory` at which memory pressure starts. |
| `critical_threshold_pct` | `u8` | `95` | Percentage of `max_memory` at which memory is critical. |
| `emergency_reserve` | `u64` | `33554432` (32 MB) | Reserved memory for critical operations. |
| `gc_threshold_pct` | `u8` | `70` | Percentage of `max_memory` at which garbage collection is triggered. |

### Networking Configuration (`[networking]`)

Defines how `novad` listens for connections.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `listen_address` | String | `"127.0.0.1"` | IP address to bind to. |
| `listen_port` | `u16` | `8642` | Port to listen on. |
| `tls_enabled` | `bool` | `false` | Enable/disable TLS. If true, `tls_cert_path` and `tls_key_path` are required. (Note: TLS is configured but not fully wired at the HTTP layer yet). |
| `tls_cert_path` | Option<PathBuf> | `None` | Path to the TLS certificate file. |
| `tls_key_path` | Option<PathBuf> | `None` | Path to the TLS private key file. |
| `unix_socket_path` | Option<PathBuf> | `None` | Path for a Unix domain socket. (Not implemented) |
| `tcp_nodelay` | `bool` | `true` | Enable/disable TCP_NODELAY. |
| `keepalive_secs` | `u64` | `30` | TCP keepalive duration in seconds. |
| `listeners` | `Vec<ListenerConfig>` | `[{ address = ":443", enabled = true }]` | Additional listeners (not fully implemented). |
| `timeouts` | `TimeoutConfig` | See below | HTTP/network timeout settings. |
| `rate_limiting` | `RateLimitingConfig` | See below | Network-level rate limiting (not fully implemented). |

**`[networking.timeouts]`**

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `read_timeout_ms` | `u64` | `30000` | Read timeout in milliseconds. |
| `write_timeout_ms` | `u64` | `60000` | Write timeout in milliseconds. |

**`[networking.rate_limiting]`** (Not fully wired into HTTP layer)

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `default_tokens_per_second` | `u32` | `1000` | Default tokens per second for rate limiters. |
| `default_burst_size` | `u32` | `2000` | Default burst size for rate limiters. |

### Logging Configuration (`[logging]`)

Controls logging output.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `level` | String | `"info"` | Log level (`trace`, `debug`, `info`, `warn`, `error`). |
| `format` | String | `"text"` | Log format (`"text"` or `"json"`). |
| `file` | Option<PathBuf> | `None` | Optional path to write logs to a file. |

### Subsystems Configuration (`[subsystems]`)

Enables or disables core subsystems.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `enable_sql` | `bool` | `true` | Enable the SQL subsystem. |
| `enable_cache` | `bool` | `true` | Enable the caching subsystem. |
| `enable_queue` | `bool` | `true` | Enable the message queue subsystem. |
| `enable_scheduler` | `bool` | `true` | Enable the job scheduler subsystem. |
| `enable_search` | `bool` | `true` | Enable the search subsystem. |
| `enable_blob` | `bool` | `true` | Enable the blob storage subsystem. |
| `enable_auth` | `bool` | `true` | Enable the authentication subsystem. |
| `enable_dashboard` | `bool` | `true` | Enable dashboard-related features (currently no specific routes). |

### Event Configuration (`[event]`)

Settings for the internal event bus.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `ordering_shards` | `u16` | `64` | Number of shards for event ordering. Must be a power of 2. |
| `default_queue_capacity` | `usize` | `1024` | Default capacity for event queues. Must be >= 64. |
| `default_max_retries` | `u32` | `3` | Default max retries for event processing. Must be <= 100. |
| `dlq_max_entries` | `u32` | `100000` | Max entries in the Dead Letter Queue. Must be <= 1,000,000. |

### Execution Configuration (`[execution]`)

Controls the pipeline executor and general operation processing.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `max_concurrent` | `u32` | `1024` | Max concurrent operations. |
| `worker_threads` | `u32` | `4` | Number of worker threads for the pipeline. |
| `execution_timeout_ms` | `u64` | `30000` | Max execution timeout for an operation in milliseconds. |
| `max_concurrent_ops` | `u32` | `256` | Max concurrent operations in the pipeline. |
| `pipeline_queue_depth` | `u32` | `1024` | Depth of the pipeline's internal queue. |
| `default_operation_timeout_ms` | `u64` | `5000` | Default timeout for individual operations. |
| `max_operation_timeout_ms` | `u64` | `60000` | Maximum allowed timeout for an operation. |
| `rate_limit_default_per_sec` | `u64` | `1000` | Default rate limit (per second). |
| `rate_limit_global_per_sec` | `u64` | `10000` | Global rate limit (per second). |
| `rate_limit_global_burst` | `u64` | `20000` | Global rate limit burst capacity. |
| `rate_limit_user_per_sec` | `u64` | `100` | Per-user rate limit (per second). |
| `rate_limit_user_burst` | `u64` | `200` | Per-user rate limit burst capacity. |
| `rate_limit_ip_per_sec` | `u64` | `1000` | Per-IP rate limit (per second). |
| `rate_limit_ip_burst` | `u64` | `2000` | Per-IP rate limit burst capacity. |
| `circuit_breaker_threshold` | `u64` | `50` | Failure rate percentage to open circuit breaker. |
| `circuit_breaker_window_ms` | `u64` | `10000` | Time window for circuit breaker failure rate. |
| `circuit_breaker_half_open_timeout_ms` | `u64` | `10000` | Timeout before half-opening circuit breaker. |
| `circuit_breaker_success_threshold` | `u64` | `10` | Number of successful calls to close circuit breaker. |
| `audit_enabled` | `bool` | `true` | Enable/disable auditing of operations. |
| `audit_include_payloads` | `bool` | `false` | Include request/response payloads in audit logs. |
| `audit_max_entry_size` | `u32` | `4096` | Max size of an audit log entry. |
| `idempotency_key_ttl_secs` | `u64` | `86400` (24h) | TTL for idempotency keys. |
| `max_idempotency_keys` | `u32` | `100000` | Max number of idempotency keys to store. |
| `pipeline_max_retries` | `u8` | `3` | Max retries for pipeline operations. |
| `retry_base_delay_ms` | `u64` | `10` | Base delay for retries in milliseconds. |
| `retry_max_delay_ms` | `u64` | `1000` | Max delay for retries in milliseconds. |

### Authentication Configuration (`[auth]`)

Manages settings for users, sessions, and security policies.

**`[auth.internal]`**

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `password_policy` | `PasswordPolicy` | See below | Password strength requirements. |
| `lockout` | `LockoutConfig` | See below | Account lockout settings. |
| `bcrypt_cost` | `u32` | `12` | Cost factor for bcrypt password hashing. |
| `enable_brute_force_detection` | `bool` | `true` | Enable detection of brute force attacks. |
| `mfa` | `MfaConfig` | See below | Multi-Factor Authentication settings (not implemented). |

**`[auth.internal.password_policy]`**

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `min_length` | `u8` | `8` | Minimum password length. |
| `max_length` | `u8` | `128` | Maximum password length. |
| `min_lowercase` | `u8` | `1` | Minimum lowercase characters. |
| `min_uppercase` | `u8` | `1` | Minimum uppercase characters. |
| `min_digits` | `u8` | `1` | Minimum digits. |
| `min_special` | `u8` | `0` | Minimum special characters. |

**`[auth.internal.lockout]`**

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `max_attempts` | `u8` | `5` | Max failed login attempts before lockout. |
| `duration_secs` | `u64` | `900` (15 min) | Lockout duration in seconds. |

**`[auth.internal.mfa]`** (Not Implemented)

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `issuer` | String | `"Nova Runtime"` | MFA issuer name. |
| `window` | `u8` | `1` | Time window for TOTP (in 30-sec intervals). |

**`[auth.session]`**

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `ttl_seconds` | `u32` | `86400` (24h) | Session TTL (Time To Live) in seconds. |
| `max_active_sessions` | `u32` | `100` | Max active sessions per user. |
| `token_length_bytes` | `usize` | `32` | Length of session tokens in bytes. |
| `cache_size` | `usize` | `100000` | Size of the session cache. |

### Security Configuration (`[security]`)

Overall security settings.

**`[security.encryption_at_rest]`** (Not Implemented)

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `enabled` | `bool` | `false` | Enable/disable encryption of data at rest. |

### Cache Configuration (`[cache]`)

Settings for the internal caching subsystem.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `max_size` | `usize` | `134217728` (128 MB) | Maximum cache size in bytes. |
| `default_ttl_secs` | `u64` | `300` (5 min) | Default Time To Live for cache entries in seconds. |
| `eviction_policy` | String | `"Lru"` | Eviction policy (`"Lru"`, `"Lfu"`, `"Ttl"`, `"LruWithTtl"`, `"NoEviction"`). |
| `backend_type` | String | `"HashMap"` | Backend storage for cache (`"HashMap"`, `"Redis"`). Currently only `HashMap` is implemented. |
| `redis_url` | Option<String> | `None` | URL for Redis backend (e.g., `redis://127.0.0.1:6379/`). |

### Blob Configuration (`[blob]`)

Settings for the blob storage subsystem.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `chunk_size` | `usize` | `1048576` (1 MB) | Size of data chunks for blobs. |
| `max_blob_size` | `u64` | `10737418240` (10 GB) | Maximum size of a single blob. |
| `gc_interval_secs` | `u64` | `3600` (1h) | Garbage collection interval in seconds. |
| `gc_grace_period_secs` | `u64` | `86400` (24h) | Grace period before deleting unreferenced chunks. |
| `data_dir` | String | `./novad-blobs` | Directory for blob data. |
| `chunk_nesting_depth` | `usize` | `3` | Directory nesting depth for chunk storage. |

### Search Configuration (`[search]`)

Settings for the full-text search engine.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `default_limit` | `usize` | `10` | Default number of results per search query. |
| `max_limit` | `usize` | `1000` | Maximum number of results per search query. |
| `bm25_k1` | `f64` | `1.2` | BM25 parameter k1. |
| `bm25_b` | `f64` | `0.75` | BM25 parameter b. |
| `fuzzy_max_distance` | `u8` | `2` | Maximum Levenshtein distance for fuzzy matching. |
| `highlight_snippet_len` | `usize` | `150` | Length of text snippets for highlighting. |
| `highlight_max_snippets` | `usize` | `3` | Maximum number of snippets to return. |
| `refresh_interval_ms` | `u64` | `1000` | Index refresh interval in milliseconds. |
| `merge_segment_threshold` | `usize` | `5` | Threshold for merging search segments. |

### SQL Configuration (`[sql]`)

Settings for the SQL query engine.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `max_batch_size` | `usize` | `1024` | Maximum number of statements in a single batch. |
| `max_columns` | `usize` | `256` | Maximum number of columns in a query result. |
| `default_limit` | `usize` | `1000` | Default row limit for `SELECT` queries. |

### Queue Configuration (`[queue]`)

Settings for the message queuing subsystem.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `max_queues` | `usize` | `1000` | Maximum number of queues that can be created. |
| `max_messages_per_queue` | `usize` | `10000` | Maximum messages a single queue can hold. |
| `max_message_size` | `usize` | `262144` (256 KB) | Maximum size of a single message in bytes. |
| `default_visibility_timeout_secs` | `u32` | `30` | Default timeout for consumed messages to remain invisible. |
| `message_ttl_secs` | `u32` | `86400` (24h) | Time To Live for messages in a queue. |
| `max_receive_count` | `u32` | `3` | Max times a message can be received before moving to DLQ. |
| `scanner_interval_ms` | `u64` | `1000` | Interval for background queue scanners (e.g., for DLQ). |
| `backpressure_threshold` | `f64` | `0.9` | Queue fullness ratio to trigger backpressure. Must be between 0.0 and 1.0. |
| `dlq_max_entries` | `usize` | `100000` | Maximum entries in a Dead Letter Queue. |
| `dlq_max_retries` | `u32` | `3` | Max retries before a message is permanently dropped from DLQ. |
| `enable_dlq` | `bool` | `true` | Enable/disable Dead Letter Queues. |
| `enable_scanners` | `bool` | `true` | Enable/disable background queue scanners. |

### Scheduler Configuration (`[scheduler]`)

Settings for the job scheduling subsystem.

| Key | Type | Default | Description |
| :-- | :--- | :------ | :---------- |
| `time_wheel_tick_ms` | `u64` | `100` | Time wheel tick interval in milliseconds. |
| `time_wheel_slots` | `usize` | `360` | Number of slots in the time wheel. Must be > 0. |
| `priority_queue_tick_ms` | `u64` | `1000` | Priority queue tick interval in milliseconds. |
| `max_jobs_per_queue` | `usize` | `10000` | Maximum number of jobs per internal queue. |
| `max_concurrent_jobs` | `u32` | `64` | Maximum number of jobs running concurrently. Must be > 0. |
| `default_job_timeout_secs` | `u32` | `300` (5 min) | Default timeout for a job execution. |
| `default_max_retries` | `u32` | `3` | Default maximum retries for a failed job. |
| `default_retry_delay_secs` | `u32` | `10` | Default delay between retries. |
| `enable_startup_recovery` | `bool` | `true` | Enable recovery of pending jobs on startup. |
| `enable_catch_up` | `bool` | `true` | Enable catching up on missed job schedules after downtime. |

## 2. Runtime Configuration Management

Nova Runtime supports dynamic configuration updates through its REST API and CLI, as well as hot-reloads via `SIGHUP`.

### Via REST API (`/admin/config`)

*   **`GET /admin/config`**: Retrieves the full current runtime configuration as a JSON object.
    ```bash
    curl http://127.0.0.1:8642/admin/config
    ```

*   **`PUT /admin/config`**: Updates the runtime configuration. This endpoint accepts a partial JSON object, which is deeply merged with the existing configuration. The updated configuration is then validated and applied in-memory without requiring a daemon restart.
    ```bash
    curl -X PUT -H "Content-Type: application/json" \
         -d '{"logging": {"level": "debug"}, "networking": {"listen_port": 8080}}' \
         http://127.0.0.1:8642/admin/config
    ```
    If validation fails, a `422 Unprocessable Entity` error is returned with details on the invalid fields.

### Via CLI (`novactl config`)

`novactl` provides commands to interact with the runtime configuration API:

*   **`novactl config get <KEY>`**: Retrieves a specific configuration value by its dot-separated key (e.g., `logging.level`).
    ```bash
    novactl config get logging.level
    # Output: info
    ```

*   **`novactl config set <KEY> <VALUE>`**: Sets a configuration value at runtime using the `PUT /admin/config` API. Values are automatically converted to the correct type based on the schema.
    ```bash
    novactl config set logging.level debug
    novactl config set networking.listen_port 8080
    novactl config set subsystems.enable_search false
    ```

*   **`novactl config validate <PATH>`**: Validates a TOML configuration file against the schema, reporting any errors.
    ```bash
    novactl config validate ./bad_config.toml
    ```

*   **`novactl config default`**: Prints the built-in default configuration in TOML format to stdout.
    ```bash
    novactl config default > novad.toml
    ```

### Via SIGHUP (Hot-Reload)

As described in the [Getting Started](01-getting-started.md) guide, `novad` can reload its configuration from the original file on disk by receiving a `SIGHUP` signal. This is useful for applying changes made directly to the `novad.toml` file without restarting the entire daemon process.

```bash
kill -SIGHUP <novad_PID>
```

## 3. Configuration Validation

Nova Runtime performs extensive validation on its configuration to ensure operational stability. Validation occurs at daemon startup, during `PUT /admin/config` API calls, and when `novactl config validate` is run. Key validation rules include:

*   **Range Checks:** Values like port numbers, memory limits, and timeouts are within acceptable ranges.
*   **Format Checks:** Ensuring values like `fsync_policy` or `compression` adhere to allowed enums or formats.
*   **Dependency Checks:** E.g., `tls_cert_path` and `tls_key_path` are required if `tls_enabled` is true.
*   **Logical Consistency:** E.g., `pressure_threshold_pct` must be less than `critical_threshold_pct`.

Any validation failures will prevent the configuration from being applied and will be reported via logs or API responses.