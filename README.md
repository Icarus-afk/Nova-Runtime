# Nova Runtime

> **Status: Implementation in Progress** — Phases 0–5 complete. 18 crates implemented with ~1,492 tests. See [Development Roadmap](docs/31-development-roadmap.md) for the full plan.

Nova Runtime is a lightweight backend runtime that collapses multiple infrastructure services into a single executable. It unifies database, cache, queue, scheduler, search, blob storage, authentication, and API runtime capabilities on commodity VPS hardware.

## Problem

Modern backend applications require a sprawl of infrastructure: PostgreSQL, Redis, RabbitMQ, Elasticsearch, S3, Auth0, and more. Each service adds operational complexity, deployment cost, failure modes, and resource overhead. On a small VPS, running even a subset of these services is impractical.

## Solution

Nova Runtime replaces this stack with a single `novad` binary — a unified runtime that provides database, caching, queuing, scheduling, full-text search, blob storage, authentication, and API serving from one process. Internally, it maintains modular subsystem boundaries with a shared execution pipeline, a single storage engine, a unified object model, and an event-driven architecture.

## Architecture Overview

```mermaid
graph TB
    subgraph "Client Interfaces"
        REST["REST API"]
        GRAPHQL["GraphQL"]
        CLI["novactl CLI"]
    end

    subgraph "Nova Runtime"
        NET["Networking Layer<br/>HTTP/1.1, WebSocket"]
        AUTH["Authentication & Security"]
        EXEC["Execution Engine"]
        EVENTS["Event System"]
        OBJ["Object Model"]
        STORE["Storage Engine<br/>(B-tree + LSM hybrid)"]

        subgraph "Subsystems"
            SQL["SQL Layer"]
            CACHE["Cache"]
            QUEUE["Queue"]
            SCHED["Scheduler"]
            SEARCH["Search"]
            BLOB["Blob Storage"]
        end
    end

    REST --> NET
    GRAPHQL --> NET
    CLI --> NET
    NET --> AUTH --> EXEC
    EXEC --> EVENTS
    EXEC --> OBJ
    OBJ --> STORE
    SQL --> OBJ
    CACHE --> OBJ
    QUEUE --> EVENTS
    SCHED --> EVENTS
    SEARCH --> OBJ
    BLOB --> STORE
    STORE --> DISK["Page Cache / WAL / SSTables"]
```

## Core Principles

| Principle | Description |
|-----------|-------------|
| **One Storage Engine** | All persistent state flows through a single storage engine — no subsystem owns its own persistence |
| **One Object Model** | Every subsystem reads and writes using a unified data representation |
| **One Event Model** | All state changes produce events; subsystems communicate through events, not direct calls |
| **One Execution Pipeline** | Every operation passes through a unified pipeline for consistent authorization, validation, and observability |
| **No Duplicated Persistence** | A given piece of data lives in exactly one place |
| **No Duplicated Business Logic** | Business logic lives in exactly one subsystem |
| **Correctness > Performance** | Never sacrifice correctness for speed |

## Documentation

The complete architecture is specified across 31 documents in [`docs/`](docs/). Each document is a standalone engineering specification covering purpose, architecture (with mermaid diagrams), data structures, algorithms, interfaces, failure modes, recovery strategy, performance considerations, security, and testing.

| # | Document | What It Covers |
|---|----------|----------------|
| 01 | [Project Vision](docs/01-project-vision.md) | Mission, success criteria (10k ops/s target), system boundaries |
| 02 | [Core Principles](docs/02-core-principles.md) | 10 immutable design principles, trade-off hierarchy |
| 03 | [Glossary](docs/03-glossary.md) | 50+ defined terms, naming conventions, acronym registry |
| 04 | [Requirements Analysis](docs/04-requirements-analysis.md) | 89 functional requirements, MoSCoW prioritized, capacity planning |
| 05 | [Domain Model](docs/05-domain-model.md) | Document/Collection/Schema type system, validation, versioning |
| 06 | [High-Level Architecture](docs/06-high-level-architecture.md) | System block diagram, module dependencies, request lifecycle |
| 07 | [Runtime Architecture](docs/07-runtime-architecture.md) | Process model, thread pool, signal handling, graceful shutdown |
| 08 | [Storage Engine](docs/08-storage-engine.md) | Hybrid B-tree + LSM-tree, 4KB pages, WAL, compaction, MVCC |
| 09 | [Memory Model](docs/09-memory-model.md) | Arena/slab/page allocators, generational GC, memory budgeting |
| 10 | [Execution Engine](docs/10-execution-engine.md) | 6-stage pipeline, middleware chain, rate limiting, circuit breaker |
| 11 | [Event System](docs/11-event-system.md) | Pub-sub event bus, topic routing, delivery guarantees, backpressure |
| 12 | [Object Model](docs/12-object-model.md) | Type system (10 types), MessagePack serialization, schema evolution |
| 13 | [Networking](docs/13-networking.md) | TCP/TLS/Unix listeners, HTTP/1.1+2, WebSocket, gRPC, connection mgmt |
| 14 | [Configuration](docs/14-configuration.md) | Config struct, TOML-based loading, REST API for runtime changes |
| 15 | [Security](docs/15-security.md) | Threat model, AES-256-GCM at rest, TLS 1.3, audit logging, input validation |
| 16 | [Authentication](docs/16-authentication.md) | Password (bcrypt), API keys, JWT, RBAC, MFA |
| 17 | [Cache](docs/17-cache.md) | HashMap + TTL backends, LRU/LFU eviction, batch ops, TTL sweeper, event invalidation |
| 18 | [Queue](docs/18-queue.md) | FIFO/priority/delayed/DLQ, at-least-once, visibility timeout, consumer groups |
| 19 | [Scheduler](docs/19-scheduler.md) | Cron/delayed/one-shot jobs, time-wheel, DAG dependencies, retry (exp backoff) |
| 20 | [Search](docs/20-search.md) | BM25 scoring, inverted index, tokenization, fuzzy/boolean/phrase search |
| 21 | [Blob Storage](docs/21-blob-storage.md) | 1 MiB chunking, SHA-256 dedup, multipart upload, range requests |
| 22 | [SQL Layer](docs/22-sql-layer.md) | SQL subset (SELECT/JOIN/AGG/GROUP BY), recursive descent parser, iterator execution |
| 23 | [REST API](docs/23-rest-api.md) | REST endpoints by subsystem, health, config, WebSocket streaming |
| 24 | [GraphQL](docs/24-graphql.md) | Full SDL schema, DataLoader batching, subscriptions, complexity analysis |
| 25 | [CLI](docs/25-cli.md) | 52 subcommands across 11 command groups, shell completions |
| 26 | [SDK](docs/26-sdk.md) | TypeScript SDK with 9 typed clients, circuit breaker, auto-pagination |
| 27 | [Dashboard](docs/27-dashboard.md) | React SPA spec, wireframes, WebSocket live updates, component tree |
| 28 | [Testing Strategy](docs/28-testing-strategy.md) | Test pyramid (70/20/10), fuzzing, chaos engineering, CI pipeline |
| 29 | [Benchmark Strategy](docs/29-benchmark-strategy.md) | Latency/throughput/concurrency benchmarks, target numbers, regression detection |
| 30 | [Deployment](docs/30-deployment.md) | Docker setup, dev scripts, backup, monitoring, runbooks |
| 31 | [Development Roadmap](docs/31-development-roadmap.md) | 7-phase build plan, dependency graph, milestones |

## Key Design Decisions

**Single-node first.** Nova Runtime is designed as a single-node system. Clustering and replication are explicitly deferred to a future phase. This keeps the initial implementation achievable and avoids premature distribution complexity.

**Hybrid B-tree + LSM-tree storage.** The storage engine uses a hybrid approach: B-tree for point reads on hot data, LSM-tree for write-heavy workloads and range scans. This provides balanced performance across diverse workloads without requiring separate engines.

**Event-driven communication.** Subsystems communicate through a shared event bus. A queue produces events when messages are enqueued/dequeued; the scheduler produces events when jobs execute; the SQL layer produces events on data mutations. Observability, audit logging, and future replication all consume the same event stream.

**Everything passes through the Execution Engine.** No operation bypasses the unified pipeline. This ensures every mutation is authorized, validated, logged, and audited. Individual subsystems implement their logic but never directly access storage or the network.

## Quick Start

### Prerequisites

- **Rust** 1.75+ (`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`)
- **Node.js** 18+ and **npm** (`sudo apt install nodejs npm` or from [nodejs.org](https://nodejs.org))
- **Port 8642** must be free (see [troubleshooting](#troubleshooting))

### One-Command Setup

```bash
bash scripts/setup.sh
```

This builds the backend, installs dashboard dependencies, and creates a default config file.

### Start Everything

```bash
bash scripts/dev.sh
```

This starts both the backend (`novad` on port 8642) and the dashboard dev server (Vite on port 5173).

| Service | URL | Default Credentials |
|---------|-----|-------------------|
| Backend API | `http://127.0.0.1:8642` | — |
| Dashboard | `http://127.0.0.1:5173` | `admin` / `admin123` |

### Manual Startup

```bash
# Terminal 1 — Backend
cargo build --bin novad
target/debug/novad

# Terminal 2 — Dashboard
cd dashboard && npm run dev

# Terminal 3 — Seed data (optional)
bash scripts/seed.sh
```

### Seed Data

The seed script populates all subsystems with test data:

```bash
bash scripts/seed.sh
```

| Subsystem | What Gets Created |
|-----------|------------------|
| SQL | 5 tables (users, products, orders, logs, events), 160 rows |
| Cache | 10 cache keys |
| Queue | 5 queues, 29 messages |
| Scheduler | 10 scheduled jobs |
| Search | 2 indexes, 25 documents |
| Blob | 8 blobs (text, JSON, CSV, YAML, XML, SQL) |
| Auth | 2 extra users + 2 API keys |

> **Idempotent**: Safe to re-run. Drops existing tables before recreating.

### Troubleshooting

#### Port 8642 already in use

```bash
# Check what's using it
ss -tlnp | grep 8642

# Kill it
fuser -k 8642/tcp

# Then retry
bash scripts/dev.sh
```

#### Port 5173 already in use

```bash
fuser -k 5173/tcp
```

#### Dashboard shows empty collections

SQL tables are in-memory only — they don't survive a restart. Re-run the seed script:

```bash
bash scripts/seed.sh
```

(If SQL persistence is enabled, tables survive restarts automatically.)

#### Login fails

The admin user is bootstrapped on first startup. If you're running a fresh instance with no `data/` directory, wait a few seconds for startup to complete before logging in.

### API Overview

All API routes are at `http://127.0.0.1:8642`:

#### System Endpoints

| Route | Method | Description |
|-------|--------|-------------|
| `/health` | GET | System health (status, uptime, memory, disk, subsystems) |
| `/ready` | GET | Readiness probe |
| `/live` | GET | Liveness probe |
| `/metrics` | GET | Prometheus-format metrics |
| `/admin/config` | GET/PUT | Runtime configuration (read all sections, or update with partial JSON) |
| `/admin/status` | GET | Pipeline status and metrics |
| `/runtime/status` | GET | Runtime subsystem health |
| `/runtime/info` | GET | Version and uptime |

#### Auth (`/api/v1/auth`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/auth/login` | POST | Login with username/password |
| `/api/v1/auth/refresh` | POST | Refresh JWT token |
| `/api/v1/auth/logout` | POST | Invalidate session |
| `/api/v1/auth/api-keys` | GET/POST | List/create API keys |
| `/api/v1/auth/api-keys/:id` | DELETE | Revoke an API key |
| `/api/v1/auth/users` | GET/POST | List/create users |
| `/api/v1/auth/users/:id` | GET/DELETE | Get/delete a user |
| `/api/v1/auth/users/:id/roles` | PUT | Update user roles |
| `/api/v1/auth/users/:id/password` | PUT | Change user password |

#### SQL (`/api/v1/sql`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/sql/query` | POST | Run SELECT query |
| `/api/v1/sql/execute` | POST | Run INSERT/UPDATE/DELETE/CREATE/DROP |
| `/api/v1/sql/tables` | GET | List tables |
| `/api/v1/sql/tables/:table/schema` | GET | Table schema |

#### Cache (`/api/v1/cache`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/cache/:key` | GET/POST/DELETE | Get/set/delete cache entry |
| `/api/v1/cache/keys` | GET | List cache keys |
| `/api/v1/cache/batch` | POST | Batch set cache entries |
| `/api/v1/cache/stats` | GET | Cache statistics |

#### Queue (`/api/v1/queues`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/queues/` | GET/POST | List queues / create queue |
| `/api/v1/queues/:name` | GET/DELETE | Get/delete queue |
| `/api/v1/queues/:name/messages` | POST | Publish message |
| `/api/v1/queues/:name/messages/poll` | POST | Poll messages |
| `/api/v1/queues/:name/messages/:id/ack` | POST | Acknowledge message |
| `/api/v1/queues/:name/purge` | POST | Purge queue |
| `/api/v1/queues/:name/stats` | GET | Queue statistics |

#### Scheduler (`/api/v1/scheduler`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/scheduler/jobs` | GET/POST | List/create jobs |
| `/api/v1/scheduler/jobs/:id` | GET/DELETE | Get/delete job |
| `/api/v1/scheduler/jobs/:id/trigger` | POST | Trigger job immediately |
| `/api/v1/scheduler/jobs/:id/pause` | POST | Pause a job |
| `/api/v1/scheduler/jobs/:id/resume` | POST | Resume a job |
| `/api/v1/scheduler/stats` | GET | Scheduler statistics |

#### Search (`/api/v1/search`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/search/indexes` | GET/POST | List/create indexes |
| `/api/v1/search/indexes/:name` | GET/DELETE | Get/delete index |
| `/api/v1/search/indexes/:name/documents` | POST | Index documents |
| `/api/v1/search/indexes/:name/query` | POST | Run search query |
| `/api/v1/search/indexes/:name/stats` | GET | Index statistics |

#### Blob (`/api/v1/blobs`)

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/blobs/` | GET/POST | List/upload blobs |
| `/api/v1/blobs/:id` | GET/DELETE | Download/delete blob |
| `/api/v1/blobs/:id/info` | GET | Blob metadata |
| `/api/v1/blobs/stats` | GET | Blob storage statistics |

#### WebSocket

| Route | Method | Description |
|-------|--------|-------------|
| `/api/v1/ws` | WebSocket | Real-time event streaming |

**Login flow:**

```bash
# Login
curl -X POST http://127.0.0.1:8642/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}'

# Response: {"token_type":"Bearer","access_token":"nova_sess_...","expires_in":3600}

# Use the token for subsequent requests
curl -H "Authorization: Bearer nova_sess_..." http://127.0.0.1:8642/api/v1/sql/tables
```

### SQL Examples

```bash
# Create table
curl -X POST http://127.0.0.1:8642/api/v1/sql/execute \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"query":"CREATE TABLE users (id Integer, name Text, email Text)"}'

# Insert
curl -X POST http://127.0.0.1:8642/api/v1/sql/execute \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"query":"INSERT INTO users VALUES (1, '\''Alice'\'', '\''alice@example.com'\'')"}'

# Query
curl -X POST http://127.0.0.1:8642/api/v1/sql/query \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"query":"SELECT * FROM users"}'

# List tables
curl http://127.0.0.1:8642/api/v1/sql/tables \
  -H "Authorization: Bearer $TOKEN"
```

### Configuration Management

Configuration can be read and updated at runtime via the API:

```bash
# View full config
curl http://127.0.0.1:8642/admin/config

# Update a setting (partial JSON merge)
curl -X PUT http://127.0.0.1:8642/admin/config \
  -H 'Content-Type: application/json' \
  -d '{"logging":{"level":"debug"}}'
```

Or via the CLI:

```bash
novactl config get logging.level
novactl config set logging.level debug
novactl config validate ./novad.toml
novactl config default  # print built-in default config
```

### CLI Overview

```bash
# Check backend health
curl http://127.0.0.1:8642/health | jq

# Quick CLI commands
novactl runtime status
novactl sql query "SELECT * FROM users LIMIT 5"
novactl cache stats
novactl queue list
novactl scheduler list
```

## Development Status

```
Phase 0: Foundations          ██████████ 100%  31 spec docs complete
Phase 1: Core Abstractions    ██████████ 100%  9 crates built, 85%+ code coverage
Phase 2: Runtime Core         ██████████ 100%  Execution Engine + novad alpha verified
Phase 3: Data Subsystems      ██████████ 100%  SQL, Cache, Search, Blob (172 tests)
Phase 4: Async Subsystems     ██████████ 100%  Queue, Scheduler, Auth (123 tests)
Phase 5: API & Tooling        ██████████ 100%  REST, GraphQL, CLI, Dashboard, config API
Phase 6: Hardening            ░░░░░░░░░░   0%
```

**Completed crates:**

| Crate | Key Components |
|-------|----------------|
| `nova-core` | PageId, Lsn, Key, Value, RuntimeError (16 variants), 7 traits |
| `nova-config` | 16-section Config, TOML loading, REST API for runtime changes, validation |
| `nova-memory` | Arena/Slab/PageAlloc/Budget/Pool, MemoryManager, GC |
| `nova-storage` | Page cache, WAL (11 record types), B+Tree, LSM, MVCC |
| `nova-object` | Value (32 variants), SchemaRegistry, MessagePack |
| `nova-event` | EventId (UUID v7), EventBus, sharded delivery, replay |
| `nova-security` | InputValidator, Encryption, RateLimiter, AuditLogger |
| `nova-cli` | 11 command groups, 52 subcommands, shell completions |
| `nova-executor` | 6-stage pipeline, middleware, circuit breaker |
| `nova-api` | HTTP server (axum), health/config/admin endpoints, subsystem routes |
| `nova-cache` | HashMap+Ttl backends, LRU/LFU, TTL sweeper |
| `nova-blob` | SHA-256 chunking, Merkle tree, dedup, GC |
| `nova-search` | BM25 scoring, Porter stemmer, query DSL |
| `nova-sql` | Full DML/DQL, GROUP BY, ORDER BY, constraints, MutationObserver |
| `nova-queue` | Pull-model, visibility timeout, DLQ, dedup |
| `nova-scheduler` | TimeWheel, CronSchedule, dependency validation |
| `nova-auth` | Password/API Key/JWT, RBAC, TOTP MFA, brute-force detection |
| `novad` | Subsystem wiring, graceful shutdown, SIGHUP config reload |

**Total: ~1,492 tests across 18 crates.**

## Target Hardware

| Tier | CPU | RAM | Disk | Expected Throughput |
|------|-----|-----|------|-------------------|
| Minimum | 1 core | 512 MB | 10 GB | 1k ops/s |
| Reference | 4 cores | 8 GB | 100 GB | 10k ops/s |
| Recommended | 8 cores | 32 GB | 500 GB | 50k ops/s |

## License

MIT
