# Production Roadmap

Forward-looking engineering plan for making Nova Runtime production-grade. This is a **design document** — it describes what to build and why, not what exists today. Each section is scoped, prioritised, and grounded in the current architecture so a contributor can pick it up without re-deriving context.

Status legend: 🟢 planned-first · 🟡 planned-later · ⚪ considered

---

## 1. Multiple Database Support

Today Nova is a single embedded storage engine (`nova-storage`: BTree memtable + LSM SSTable + WAL, adapted via `StorageEngineStore`). Every subsystem shares it. "Multiple DB support" means two different things, both worth building — in this order.

### 1.1 Multiple embedded data instances (multiple databases / namespaces)

🟢 **Problem.** One `novad` owns one data directory. A common SaaS need is several isolated databases on one node (per-team, per-tenant, per-environment) without running N daemons.

**Design.** Introduce a `db` layer above the storage engine:

```
AdminState
  └─ DatabaseRegistry           (nova-db / new crate)
       ├─ "default" -> Database { engine: StorageEngineStore, event_bus, catalog }
       ├─ "tenant-a" -> Database { ... }
       └─ "tenant-b" -> Database { ... }
```

- Each `Database` owns its own `MemoryManager` budget, WAL, table catalog (`information_schema`), search index registry, and event-bus shard partition.
- REST routes gain a `?database=` / `X-Nova-Database` selector. When omitted, `default` is used. `nova`-prefixed system metadata stays in `default`.
- `novactl db create/list/drop/rename`, `database` scope on the SDK and dashboard.
- Resource isolation: a per-database `max_bytes` / `max_tables` guard (reuse the existing memory manager's accounting).

**Why this shape.** It reuses the proven single-engine core (no new storage code), gives hard isolation, and keeps one process — matching the project's "one binary" philosophy. It is the natural prerequisite for 1.3.

### 1.2 External backend adapters (plug-in storage)

🟡 **Problem.** Some workloads want Postgres/MySQL/SQLite as the durable store, or object storage for blobs, rather than the embedded engine.

**Design.** Add a `Backend` trait behind `StorageEngineStore` so an instance can be backed by:

- **embedded** (current, default),
- **SQLite** — simplest external option, single-file, ships in-tree (rusqlite),
- **PostgreSQL / MySQL** — via a thin `sqlx`/`tokio-postgres` adapter exposing the same `Key`/`Value`/table API,
- **object storage** (`S3`/`MinIO`) as a blob/`nova-blob` namespace target.

**Constraints.** The unified `nova-core` `StorageEngine` trait is the seam. Each adapter only needs to satisfy that contract; subsystems don't change. Ship **SQLite first** (zero-infra, drops into CI/demo), then Postgres. Mark embedded as still the recommended default.

**Trade-off to document:** external backends buy interoperability and scaled-out durability but lose the tight fusion of engine+WAL+event bus that Nova's performance and transactions rely on. Keep embedded as the default; treat external as an escape hatch, not the centrepiece.

### 1.3 Read replicas & horizontal scale-out

🟡 **Problem.** One `novad` is a single point of write throughput. Production apps often need to scale reads.

**Design.**

- **Snapshot read replicas**: `novad` can **seed + attach** to a primary and serve read-only `/api/v1/sql` + `/api/v1/search` + `/api/v1/cache` GETs from a local copy, applied incrementally from a replication log (see §3 backups / §4 recovery — the WAL-archiving mechanism is the same one used to ship change data).
- **Consistency model**: reads on a replica are *eventually consistent* by default; MVP reads only tag `/health`/`/ready` with a `replication_lag_secs` gauge.
- **Writes stay on the primary** in v1. Distributed consensus (RAFT) is explicitly **out of scope** and documented as a non-goal — see §6.

**MVP acceptance:** `novad replica --primary http://primary:8642 --dir ./replica` boots a read copy, applies WAL deltas, serves read-only routes, and exposes `replication_lag_secs`.

---

## 2. Schema Migrations

🟢 **Problem.** Today there is no schema versioning: `CREATE TABLE` / `DROP TABLE` are applied immediately and there is no record of a schema's evolution. Teams need versioned, ordered, reversible schema changes applied atomically and repeatably across environments.

**Design — a first-class `novactl migrations` command + embedded runner.**

- **Migration format** (files in `<data_dir>/migrations/` or an overridable dir):

  ```
  migrations/
    0001_create_users.up.sql      CREATE TABLE users (id INTEGER PRIMARY KEY AUTO_INCREMENT, email TEXT UNIQUE NOT NULL, ...);
    0001_create_users.down.sql    DROP TABLE users;
    0002_add_roles.up.sql         ALTER TABLE users ADD COLUMN role TEXT DEFAULT 'viewer';
    0002_add_roles.down.sql       ...
  ```

  `.up` / `.down` pairs give forward + rollback. Plain SQL — the engine already parses the dialect; no new DSL.

- **Version ledger** `schema_migrations (version, name, checksum, applied_at, applied_by)` stored inside the database (`default`). `checksum` (SHA-256 of the file) detects a file that changed after being applied — mismatch is a hard error, never a silent re-run.

- **Commands:**

  ```bash
  novactl migrations init        # create migrations/ + schema_migrations table
  novactl migrations new add_roles   # scaffold 0003_add_roles.{up,down}.sql
  novactl migrations status      # applied/pending table
  novactl migrations up          # apply all pending, in order, transactionally
  novactl migrations down 1      # roll back one step (runs .down)
  novactl migrations up --db tenant-a
  ```

- **Atomicity**: each migration runs in a single transaction where the engine supports DDL+DDL (no interleaved commits). `autocommit` DDL paths are detected and warned about.

- **Startup guard**: on boot, if pending migrations exist and `auto_migrate=false` (default), `novad` refuses reads against the affected tables and logs `run 'novactl migrations up'`. `auto_migrate=true` applies them before serving.

**Dashboard:** a Migration page (status list, `up`/`down`, per-db) reusing the same client verbs the CLI exposes.

---

## 3. Backups

🟢 **Problem.** No backup story exists. `data/` is a live directory; copying it while running can produce a torn snapshot.

Three complementary layers, in order of effort:

### 3.1 Logical backup (`.sql` dump) — 🟢 first

- `novactl backup dump --out backup_20260101.sql` emits a full logical dump: `CREATE TABLE`, `PRAGMA`-style metadata, then batched `INSERT`s, ordered to satisfy FK/join-key ordering.
- Round-trips through `novactl backup restore --in backup_20260101.sql` against an empty database.
- Portable across embedded/SQLite/Postgres (§1.2) — the interchange format is plain SQL.
- No coordination required; safe on a live node (we read consistent per-table batches).

### 3.2 Physical snapshot + WAL archiving (point-in-time recovery) — 🟡

- **Consistent snapshot**: `novactl backup snapshot` takes a copy of the SSTable set + memtable checkpoint while the engine continues to run. Reuse the existing `Lsn` (log sequence number) fencing so the snapshot is internally consistent.
- **WAL archive**: continuously ship committed WAL segments (already `crc32c`-framed) to an archive location (local dir, S3, MinIO) — `max_wal_archive_age_secs`.
- **PITR restore**: `novactl backup restore --to-time 20260101T00:00:00Z` applies the base snapshot + replayed WAL up to a target `Lsn`/timestamp.
- This is the same change-log ingredient as §1.3 replicas — build it once, consume it twice.

### 3.3 Scheduled, encrypted, verified backups — 🟡

- `[backup]` config section: `schedule` (cron), `retention_days`, `target` (dir / `s3://`), `encryption_key` (age/openssl AES-GCM; key via env `NOVA_BACKUP_KEY`, never in `novad.toml`).
- **Verify**: a checksum manifest + a periodic restore-into-temp smoke test (`novactl backup verify`).
- Dashboard **Backups** page: last backup, next run, status, one-click back up / restore.

**Bug bar:** a restore must be runnable in an empty container from nothing but the backup artifact — no `data/` left intact.

---

## 4. Monitoring, Observability & Alerting

🟢 **Problem.** `/health`, `/metrics`, and `/health` subsystem status exist, but there is no SLO tracking, alerting, or rich tracing.

- **Metrics parity with a real catalog**: expose Prometheus text at `/metrics` covering per-subsystem counters already tracked internally (cache hit/miss, queue depths, scheduler `total_executed`/failures, SQL query latency percentiles, storage LSM level sizes, WAL fsync latency) + `REPLICATION_LAG`, `MIGRATION_LAG`, `BACKUP_LAST_SUCCESS`.
- **Structured traces**: correlate a request across REST → executor → storage with a `trace_id`/`span_id` (the log engine already has these fields; surface them in `/metrics` histograms and dashboard Logs with trace-view).
- **Alert rules** (`novad.toml [alerts]`): thresholds on metrics → webhook/Slack/email/PagerDuty channel (reuse the notification-channel model already in the alert/config types).
- **SLOs**: define the default burn-rate budgets (e.g. availability 99.9%, p99 SQL < 50ms) and a `status` page endpoint for them.

---

## 5. Operational Hardening (pro, lower-risk items)

- 🟡 **Encryption at rest** for WAL + SSTables (`[storage] encryption_key`), layered cleanly so it does not slow the hot path (page-cache holds plaintext).
- 🟡 **Full audit log**: append-only `audit.log` for auth events (login, key issue/revoke, user/role change) with a `novactl audit tail`. Pairs with the Auth subsystem already centralising identity.
- 🟢 **Role improvements for the dashboard/API**: fix RBAC so a session's roles/permissions drive real UI gating, and make API-key creation correctly honour the admin policy (see the 403 issue below).
- 🟢 **Multi-tenancy RBAC**: per-database access (`tenant-a:read`, `tenant-a:write`) once §1.1 ships — RBAC engine supports user+permission, wired to databases.
- ⚪ **TLS mTLS** for the replication/replica channel (§1.3), distinct from client-facing TLS.

---

## 6. Developer & User Experience

🟢 **Problem.** Nova is feature-rich but the *feel* of using it — the dashboard, the CLI, error messages, the SDK — is where new users stumble. This section is about making the existing surface **obviously usable and impossible to misuse**, and is deliberately scoped so improvements carry **near-zero risk of new bugs or edge cases**. Every item here is either a *presentation-layer* change or a *guardrail* that fails safely — none of them alter core engine semantics.

**The golden rule for this whole section:** no change that touches the storage engine, transaction semantics, or data formats without a full review. All DX/UX work below is confined to the API client, CLI printing, and dashboard — where a regression is visible immediately and reversible.

### 6.1 One canonical API client (kills drift at the source)

- Today `dashboard/src/api/client.ts` is the de-facto source of truth, but the SDK (`@novaruntime/sdk`) and `examples/bloom-market/src/nova.js` are out of sync (`docs/sdk.md` documents the mismatch). Every divergence is a place where a user hits a wrong-shaped response.
- **Move:** promote `client.ts` to the single shared client (or a generated `openapi.json` client) and have the SDK re-export it. This removes entire classes of "it worked in the dashboard but not the SDK" bugs before they are written.
- **Risk:** low — it is a refactor of *call* sites, not engine code. Guard with the existing `tsc --noEmit` + `npm run build` in CI.

### 6.2 Surface the real data, stop faking it

- Several dashboard pages currently render **synthetic placeholder data** when the backend is quiet (e.g. Logs falls back to fabricated entries; some metric cards show hardcoded `0`s for fields the API doesn't return).
- **Move:** remove fabricated content. Render an honest empty state ("no entries yet") plus the exact `note`/`detail` the server returned. Fabricated data actively misleads during onboarding — it looks alive when it isn't.
- **Risk:** none to correctness; it only changes what's displayed. It also makes the RBAC and API-shape bugs (like the 403 below) visible instead of masked.

### 6.3 Read-only introspection helpers

- **Move:** a `novactl doctor` / `setup` self-check that runs against a live `novad` and prints, in one pass: version, health, migration status, backup freshness, auth (can a given key log in?), and the shape of key endpoints. Model it on the existing `cargo`-style "here's what's wrong and the exact command to fix it" output.
- Analogy: `docker info`, `kubectl get --help`, `git status`. This collapses the whole troubleshooting loop (which today lives in `docs/troubleshooting`) into one command.

### 6.4 Guardrail the pitfalls that already cause real 403s/misuse

- **Fix the known admin-policy bug first** (see Related Notes): API-key creation must succeed for `admin`. Until "admin can administer" is actually true, every auth-facing page reads as broken.
- **Fail loud, not silent.** Where the API returns an empty list because of an overscoped request or a miss, show *why*: "no results for `foo` (namespace `bar` is empty)" rather than a bare empty table.
- **Confirm destructive actions** already exist; extend the same confirm/undo pattern to queue purge, bucket delete, and migration `down` in the dashboard.

### 6.5 Tighter CLI ergonomics without new commands

- Improve **output defaults**: `--output=table` for humans, `json` for scripts — already supported; pick the right default per command so piping never surprises.
- **Better errors**: when `novactl` hits the network it should distinguish *"server unreachable"* vs *"auth rejected"* vs *"resource missing"* in one line, mirroring `ApiError`'s `status`/`detail`/`title` shape.
- **Progress + exit codes**: non-zero exit on failure (already the norm), and no invisible hangs — add the same health-wait the Makefile uses to long-running verbs.

### 6.6 Dashboard usability (CSS/UX only, no data risk)

- Empty states with a concrete next action ("Upload a file to create a namespace", "Run a migration").
- Consistent loading skeletons (already partly there via `MetricCard`), clearer disabled states, and a global error banner that surfaces the server's `detail` instead of a generic "failed".
- Tooltips on unfamiliar terms (LSM, memtable, DLQ, SSTable) so ops/users aren't alienated.

### 6.7 Onboarding that reflects today's reality

- The README's 30-second start is good; extend a **"first 10 minutes"** walkthrough that drives a user through the real `make dev` flow and shows each of the 8 subsystems lighting up against the *live* server — seeding demo data through the API (not hardcoded), so the screens show genuine output.
- Move the "SDK out of sync" caveat out of first-class onboarding into §6.1's fix.

### 6.8 Guardrails for the migration/backup features (§2, §3)

- Once migrations ship, DX guards prevent the classic foot-guns: refuse `migrations up` on a `PRODUCTION` database without `--confirm`, surface a **dry-run** (`novactl migrations up --dry-run`) and show a diff of what will run, and make `down` require an explicit count.
- Backups get the same treatment: a restore always runs into a **temporary empty dir** by default and reports the target path, so a careless restore can never clobber a live database.

**Acceptance for §6 as a whole:** a new user can go `make dev` → dashboard → perform one action per subsystem → understand every screen, *without* consulting the docs, and never see fabricated data or a cryptic 403.

---

## 7. Non-Goals (explicitly deferred)

- **Distributed consensus (RAFT/Paxos)** for multi-writer HA. Nova's design goal is a *single primary*; HA means replicas + fast failover + WAL replay, not shared-mutable multi-master. Revisit only if the product pivots.
- **Cross-database distributed transactions.** Atomicity is per-database. Cross-db writes are the application's job (outbox / compensating writes) — document this contract.
- **A new query DSL.** Migrations and backups speak existing SQL. No new schema language.

---

## § Priority Order (suggested)

| Rank | Item | Why first |
|------|------|-----------|
| 0 | **§6.4 auth fix (403)** | Nothing is usable if "admin" can't administer; unblocks every auth screen. |
| 1 | **§6 DX / UX (§6.1–6.3)** | Highest trust-per-effort; low bug risk; improves every other feature's onboarding. |
| 2 | **§2 Schema Migrations** | No project is production-safe without versioned schema; unlocks teams, CI, rollbacks. |
| 3 | **§3.1 Logical backup** | Immediate safety net; small, portable, no infra. |
| 4 | **§1.1 Multi-database (embedded instances)** | Isolation + tenancy; reuses core; prerequisite for scaling. |
| 5 | **§3.2 Physical snapshot + WAL archive / PITR** | The change-data primitive that also powers §1.3 replicas. |
| 6 | **§4 Observability/alerting** | Makes the rest trustworthy in production. |
| 7 | **§1.2 external backends, then §1.3 replicas** | Higher effort; depend on WAL-archive and adapter seams. |
| 8 | **§5 hardening** | Incremental confidence (at-rest, audit). |

---

## Related Current-State Notes

- Auth: creating an API key currently returns **403 "Admin role required"** even for the `admin` user (session roles are not flowing through to the RBAC gate in `crates/nova-api/src/middleware.rs`). This is a known bug to fix *before* any multi-tenancy RBAC work in §5.
- `novactl db …` already exists as a command namespace (see `docs/cli.md`) — extend it with `db create/list/drop` per §1.1 rather than adding a new surface.
- The log engine already carries `trace_id`/`span_id` — §4 tracing builds on existing fields rather than introducing new ones.
