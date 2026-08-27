# Architecture

## Principles

- **One storage** (`nova-storage`): BTree (hot, `page_cache_size`) + LSM (SSTable `zstd`/`snappy`, bloom `10 bits/key` + WAL `crc32c`) + `StorageEngineStore` adapter. All subsystems go through it — no per-subsystem DB.
- **One object model** (`nova-object`): unified `Value`/`Document` serialization (`rmp`/`json`).
- **One pipeline** (`nova-executor`): `PipelineExecutor` with `max_concurrent_ops`, `circuit_breaker`, `rate_limiter`, `idempotency_cache`. Every operation → `parse → validate → authorize → execute`.
- **Event bus** (`nova-event`): `EventBus` (crossbeam `bounded`, ordering shards, DLQ). SQL `SQLEngineMutationObserver` publishes `db.table.*` events.

```
Client (REST/GraphQL/CLI/Dashboard)
  → Networking (axum, cors_layer, auth_layer, request_logger)
  → Auth (bcrypt, DashMap sessions, BruteForce 5/min IP)
  → Executor (PipelineConfig)
  → Subsystems: SQL, Cache (HashMapBackend LRU), Queue (StorageQueueBackend, delayed/inflight DLQ scanner), Scheduler (TimeWheel), Search (IndexWriter BM25), Blob (ChunkManager dedup), Memory
  → StorageEngine (BTree memtable + SSTable LSM + WAL)
```

## Crates

- `nova-core`: `Key`/`Value`/`Page`/`Lsn` primitives, `StorageEngine` trait.
- `nova-config`: `Config` from `novad.toml` + `NOVA_*` env + `ConfigLoader::watch` + `SIGHUP`.
- `nova-memory`/`nova-storage`/`nova-object`/`nova-event`/`nova-security`/`nova-executor`.
- `nova-api`: `AdminState` (holds `Arc<Manager>` for each subsystem), `server::start_server` with `/health`/`/ready`/`/live`/`/metrics` + `auth_layer` (5/min IP) + `cors_layer` (allowlist `5173`/`8642`, preflight 204).
- `novad`: builds `MemoryManager`, `Store`, `EventBus`, `SQL`+`Queue`+`Cache`+`Blob`+`Search`+`Auth`+`Scheduler`, wires `SQLEngineMutationObserver`, spawns `QueueScanner` if `enable_scanners`.

## Key Fixes (2026-08)

- Bloom `wrapping_mul/rem` overflow → 122 storage tests green.
- BTree `get_from_page` `continue` on tombstone (was early `return None` breaking delete→reinsert).
- WAL `open` creates empty `.wal` + `GroupCommit` BTreeMap dedup.
- API no longer silent-drops `delay_ms`, `fields`, `offset`, `prefix`/`limit`, `expires_at`.

## Data Flow Example (SQL)

`POST /api/v1/sql/query {"query":"SELECT * FROM demo WHERE id=$1","params":[1],"limit":10}` → `routes/sql.rs` interpolates `$1` → `SQLEngine::execute` → `Lexer`→`Parser`→`Binder`→`LogicalPlanner`→ `build_executor` → `TableStore` scan → `RecordBatch` → `stats` + `truncated`.
