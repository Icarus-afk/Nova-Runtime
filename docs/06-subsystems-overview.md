# 06. Subsystems Overview

Nova Runtime is composed of several integrated subsystems, each providing specific functionality. This document provides a high-level overview of each subsystem, its purpose, and key features.

## 1. Core Subsystems

### Memory Manager

**Purpose:** Manages in-memory resources, including allocation, garbage collection, and memory pressure handling.

**Key Features:**
*   Configurable memory limits and thresholds.
*   Automatic garbage collection when memory usage exceeds thresholds.
*   Emergency reserve for critical operations.

**Configuration:** `memory` section in `novad.toml`.

### Storage Engine

**Purpose:** Provides persistent storage for all data, including databases, indexes, and blobs.

**Key Features:**
*   Transactional storage with Write-Ahead Logging (WAL).
*   Configurable caching, compression, and fsync policies.
*   Supports B-trees, LSM trees, and Bloom filters.

**Configuration:** `storage` section in `novad.toml`.

### Pipeline Executor

**Purpose:** Executes operations with support for rate limiting, circuit breaking, retries, and idempotency.

**Key Features:**
*   Configurable concurrency and queue depth.
*   Rate limiting at global, user, and IP levels.
*   Circuit breaker for fault tolerance.
*   Idempotency keys for safe retries.

**Configuration:** `execution` section in `novad.toml`.

### Event Bus

**Purpose:** Enables asynchronous communication between subsystems via events.

**Key Features:**
*   Ordered event processing with configurable sharding.
*   Dead Letter Queue (DLQ) for failed events.
*   Background retry mechanism.

**Configuration:** `event` section in `novad.toml`.

## 2. Data Subsystems

### SQL Engine

**Purpose:** Provides a SQL-compatible interface for querying and manipulating structured data.

**Key Features:**
*   Supports SELECT, INSERT, UPDATE, DELETE, and CREATE TABLE.
*   Batch operations and parameterized queries.
*   Schema inspection and table management.

**Configuration:** `sql` section in `novad.toml`.

**API Endpoints:**
*   `POST /api/v1/sql/query`
*   `POST /api/v1/sql/execute`
*   `GET /api/v1/sql/tables`
*   `GET /api/v1/sql/tables/{table}/schema`

### Cache Manager

**Purpose:** Provides in-memory caching for frequently accessed data.

**Key Features:**
*   Multiple eviction policies (LRU, LFU, TTL).
*   Configurable cache size and TTL.
*   Batch operations for efficiency.

**Configuration:** `cache` section in `novad.toml`.

**API Endpoints:**
*   `GET /api/v1/cache/{key}`
*   `POST /api/v1/cache/{key}`
*   `DELETE /api/v1/cache/{key}`
*   `POST /api/v1/cache/batch`
*   `GET /api/v1/cache/keys`
*   `GET /api/v1/cache/stats`

### Queue Manager

**Purpose:** Manages message queues for asynchronous processing.

**Key Features:**
*   Multiple queue types (standard, delayed).
*   Dead Letter Queue (DLQ) for failed messages.
*   Configurable visibility timeouts and message TTL.

**Configuration:** `queue` section in `novad.toml`.

**API Endpoints:**
*   `POST /api/v1/queues/`
*   `GET /api/v1/queues/`
*   `GET /api/v1/queues/{name}`
*   `DELETE /api/v1/queues/{name}`
*   `POST /api/v1/queues/{name}/messages`
*   `POST /api/v1/queues/{name}/messages/poll`
*   `POST /api/v1/queues/{name}/messages/{id}/ack`
*   `POST /api/v1/queues/{name}/purge`
*   `GET /api/v1/queues/{name}/stats`

### Scheduler Manager

**Purpose:** Manages scheduled jobs and recurring tasks.

**Key Features:**
*   Cron and interval-based scheduling.
*   Job retries and failure handling.
*   Manual triggering and pause/resume.

**Configuration:** `scheduler` section in `novad.toml`.

**API Endpoints:**
*   `POST /api/v1/scheduler/jobs`
*   `GET /api/v1/scheduler/jobs`
*   `GET /api/v1/scheduler/jobs/{id}`
*   `DELETE /api/v1/scheduler/jobs/{id}`
*   `POST /api/v1/scheduler/jobs/{id}/trigger`
*   `POST /api/v1/scheduler/jobs/{id}/pause`
*   `POST /api/v1/scheduler/jobs/{id}/resume`
*   `GET /api/v1/scheduler/stats`

### Search Manager

**Purpose:** Provides full-text search capabilities over indexed data.

**Key Features:**
*   BM25 ranking algorithm.
*   Fuzzy matching and highlighting.
*   Configurable analyzers and boosts.

**Configuration:** `search` section in `novad.toml`.

**API Endpoints:**
*   `POST /api/v1/search/indexes`
*   `GET /api/v1/search/indexes`
*   `GET /api/v1/search/indexes/{name}`
*   `DELETE /api/v1/search/indexes/{name}`
*   `POST /api/v1/search/indexes/{name}/documents`
*   `POST /api/v1/search/indexes/{name}/query`
*   `GET /api/v1/search/indexes/{name}/stats`

### Blob Manager

**Purpose:** Manages binary large objects (blobs) such as files and media.

**Key Features:**
*   Chunked storage for large files.
*   Configurable garbage collection.
*   Metadata and content type support.

**Configuration:** `blob` section in `novad.toml`.

**API Endpoints:**
*   `POST /api/v1/blobs/`
*   `GET /api/v1/blobs/`
*   `GET /api/v1/blobs/{id}`
*   `DELETE /api/v1/blobs/{id}`
*   `GET /api/v1/blobs/{id}/info`
*   `GET /api/v1/blobs/stats`

## 3. System Subsystems

### Auth Manager

**Purpose:** Handles authentication, authorization, and session management.

**Key Features:**
*   User and API key management.
*   JWT-based sessions.
*   Password policies and lockout.

**Configuration:** `auth` section in `novad.toml`.

**API Endpoints:**
*   `POST /api/v1/auth/login`
*   `POST /api/v1/auth/refresh`
*   `POST /api/v1/auth/logout`
*   `POST /api/v1/auth/api-keys`
*   `GET /api/v1/auth/api-keys`
*   `DELETE /api/v1/auth/api-keys/{id}`
*   `POST /api/v1/auth/users`
*   `GET /api/v1/auth/users`
*   `GET /api/v1/auth/users/{id}`
*   `DELETE /api/v1/auth/users/{id}`
*   `PUT /api/v1/auth/users/{id}/roles`
*   `PUT /api/v1/auth/users/{id}/password`

### Security Manager

**Purpose:** Manages security policies and encryption.

**Key Features:**
*   Encryption at rest (not implemented).
*   TLS configuration (not fully wired).

**Configuration:** `security` section in `novad.toml`.

## 4. CLI Commands

Each subsystem is also exposed via the `novactl` CLI. Refer to the [CLI Reference](03-cli-reference.md) for details on available commands.

## 5. Notes

*   All subsystems are enabled by default but can be disabled via the `subsystems` section in `novad.toml`.
*   Subsystems communicate via the internal event bus and shared storage engine.
*   Configuration changes for most subsystems can be applied at runtime via the `/admin/config` API or `novactl config set`.