# API — REST & GraphQL

Base URL: `http://127.0.0.1:8642` (the SDK uses `NOVA_URL=http://127.0.0.1:8642/api/v1`).

All routes are versioned under `/api/v1/*`, except a small set of health/open endpoints. Response bodies are JSON; errors use RFC 7807 `application/problem+json`.

## Auth basics

Login is public:

- `POST /api/v1/auth/login {"username","password","ttl_seconds?"}` → `{"token_type":"Bearer","access_token","expires_in","refresh_token","refresh_expires_in"}`.

Every other route requires `Authorization: Bearer <token>` (**`/metrics` and `/graphql` included**).

Public (no auth) routes are exactly: `/health`, `/ready`, `/live`, `/openapi.json`, `/api/v1/auth/login`, `/api/v1/auth/refresh`. Anything OPTIONS (CORS preflight) is also allowed. See `crates/nova-api/src/middleware.rs:127`.

Limits & tokens:

- **Rate limit**: login is limited to **5 attempts/min per IP** → `429` with `retry_after_secs`.
- **Expiry**: tokens expire after `expires_in` seconds; `POST /api/v1/auth/refresh {"refresh_token"}` mints a new one.
- **Logout**: `POST /api/v1/auth/logout` (Bearer) revokes the session.
- **API keys**: `POST/GET/DELETE /api/v1/auth/api-keys` with `{"name","permissions","expires_at": "RFC3339|millis"}`. Send via `X-API-Key`; `expires_at` is enforced.
- **Users**: `POST/GET /api/v1/auth/users`, `GET/DELETE /api/v1/auth/users/:id`, `PUT /api/v1/auth/users/:id/roles`, `PUT /api/v1/auth/users/:id/password`.

> Every request in the curl examples below can carry `-H "Authorization: Bearer $TOKEN"` where `$TOKEN` comes from the login call above.

---

## SQL — `/api/v1/sql`

A real SQL engine with a deliberately small, predictable surface.

- `POST /api/v1/sql/query {"query","params?":[val],"limit?","format?":"json|csv|arrow"}` → interpolates `$1`/`$2` with escaping, validates `format`, truncates `rows` to `limit` with a `truncated` flag. Response: `{"columns","column_names","types","rows","row_count","truncated","execution_time_ms","format"}`.
- `POST /api/v1/sql/execute` — same `params` handling; returns `{"affected_rows"}` (for CREATE/INSERT/UPDATE/DELETE).
- `GET /api/v1/sql/tables`
- `GET /api/v1/sql/tables/:table/schema`

### SQL syntax rules (important)

- **Bare column names only.** Qualified `table.col` is a parse error (`expected From, got Dot`).
- **No `CREATE TABLE IF NOT EXISTS`**, no **`DROP TABLE IF EXISTS`**. Plain statements only; a `DROP TABLE` for a missing table errors, so guard it in code.
- **JOINs** flatten both tables' columns — join keys must be **distinctly named** across the two joined tables. The idiomatic pattern is distinct PKs:

  ```sql
  CREATE TABLE sellers (sid INTEGER PRIMARY KEY, username TEXT, stall TEXT, bio TEXT);
  CREATE TABLE listings (id INTEGER PRIMARY KEY, seller_id INTEGER, title TEXT, price REAL);
  SELECT title, stall FROM listings JOIN sellers ON seller_id = sid;
  ```

- Supported: `SELECT`, `WHERE`, `JOIN` (inner/cross), `GROUP BY` / `HAVING`, `ORDER BY` (alias or ordinal), aggregation (`COUNT`, `AVG`, `MAX`, `MIN`, `SUM`), `INSERT`, `UPDATE`, `DELETE`, parameter binding `$1`.

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/sql/query \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"query":"SELECT title, stall FROM listings JOIN sellers ON seller_id = sid WHERE price > 5 ORDER BY 2 DESC","limit":10}'
```

---

## Cache — `/api/v1/cache`

- `GET /:key` → `{"key","value","ttl_remaining_ms"}` (404 when absent)
- `POST /:key {"value","ttl_ms?"}` — set (TTL in ms; omitted = default from config)
- `DELETE /:key`
- `POST /batch` — set/get/delete several keys at once
- `GET /keys?pattern=` — glob `*`/`?` matching
- `GET /stats`

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/cache/hello \
  -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json' \
  -d '{"value":{"msg":"world"},"ttl_ms":60000}'
curl -s http://127.0.0.1:8642/api/v1/cache/hello -H "Authorization: Bearer $TOKEN"
```

---

## Queue — `/api/v1/queues`

Durable FIFO queues with delayed delivery and visibility timeout (poll-style consumers).

- `POST / {"name","durable?","max_length?","max_message_size?"}` → creates (or patches) a queue.
- `GET /` → list queues.
- `GET/DELETE /:name` → inspect / delete.
- `POST /:name/messages {"messages":[{"body","delay_ms?"}]}` → publish. `delay_ms` (≤7 days) defers delivery (a message becomes visible only after the delay).
- `POST /:name/messages/poll {"count?","visibility_timeout_ms?"}` → consume. `count` clamped to 1..100, `visibility_timeout_ms` up to 12h. Polled messages are temporarily invisible until you ACK them.
- `POST /:name/messages/:id/ack` → confirm done.
- `POST /:name/purge` → drop all messages.
- `GET /:name/stats`

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/queues/myq -H "Authorization: Bearer $TOKEN" -H 'Content-Type: application/json'
curl -s -X POST http://127.0.0.1:8642/api/v1/queues/myq/messages -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"messages":[{"body":{"order_id":1},"delay_ms":0}]}'
curl -s -X POST http://127.0.0.1:8642/api/v1/queues/myq/messages/poll -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"count":5,"visibility_timeout_ms":30000}'
```

---

## Scheduler — `/api/v1/scheduler`

Cron, interval, and one-shot jobs that fire an HTTP `action` payload or a queue publish.

- `POST /jobs {"name","type":"cron|interval|one_time","schedule?","timezone?","action?","max_retries?","retry_delay_ms?","enabled?"}`
  - `cron`: `schedule` is a 5-field cron expression (e.g. `0 9 * * *`).
  - `interval`: `schedule` is minutes.
  - `one_time`: `schedule` is an ISO timestamp.
  - `enabled=false` creates a paused job.
  - Returns the job `id` (a real UUID).
- `GET /jobs`, `GET /jobs/:id`, `DELETE /jobs/:id`
- `POST /jobs/:id/trigger|pause|resume`
- `GET /stats`

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/scheduler/jobs -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"daily-digest","type":"cron","schedule":"0 9 * * *","action":{"url":"http://worker/run"}}'
```

---

## Search — `/api/v1/search`

BM25 full-text index. You create an index (with a schema), add documents, then query.

- `POST /indexes {"name","fields?":[{"name","type":"text|keyword|integer|float|boolean","analyzer?","boost?"}]}` — create/validate schema.
- `GET /indexes` — list registry.
- `GET/DELETE /indexes/:name` — inspect / remove (404 if missing).
- `POST /indexes/:name/documents {"documents":[{"id",...}]}` — add/overwrite docs.
- `POST /indexes/:name/query {"query","limit?","offset?"}` → `{"hits":[{"id","score","source"}],"total_hits",...}`.
- `GET /indexes/:name/stats`

```bash
curl -s -X POST http://127.0.0.1:8642/api/v1/search/indexes/honey_idx -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"honey_idx","fields":[{"name":"title","type":"text","boost":2.0},{"name":"body","type":"text"}]}'
curl -s -X POST http://127.0.0.1:8642/api/v1/search/indexes/honey_idx/query -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -d '{"query":"wildflower","limit":5}'
```

---

## Blob — `/api/v1/blobs`

Binary storage with namespace scoping and SHA256 content dedup. Everything is addressed by returned blob `id`.

- `POST /?namespace=&bucket=` — multipart (`name=file`) or raw `Bytes` with `Content-Type`; `namespace` must not contain `/` or `..`; empty bodies rejected. Returns the blob id(s).
- `GET /?prefix=&limit=&namespace=` — list, `starts_with` filter, `limit` 1..1000, `has_more` pagination.
- `GET /:id` — fetch bytes.
- `GET /:id/info` — metadata (size, sha256, namespace).
- `DELETE /:id`
- `GET /stats`

```bash
curl -s -X POST "http://127.0.0.1:8642/api/v1/blobs?namespace=images" \
  -H "Authorization: Bearer $TOKEN" -F "file=@photo.jpg"
```

---

## GraphQL — `/graphql`

`async-graphql` schema served by `crates/nova-gql`, sharing the same subsystem state as REST. `GET` opens the playground; `POST` runs queries. **Requires a Bearer token.**

---

## Error responses

All API errors use RFC 7807:

```json
{"type":"about:blank","title":"Bad Request","status":400,"detail":"...","instance":"...","extra":null}
```

`status` correctly maps to `StatusCode` (unmapped codes fall back to `500 BAD_REQUEST`).

## Pagination

Uniform: `limit`/`offset` or `count`/`page`; list-style endpoints return `pagination:{cursor,limit,has_more}` or `total/pages` alongside the items.

---

## Reference

Route implementations live in `crates/nova-api/src/routes/*.rs` (one file per subsystem): `sql.rs`, `cache.rs`, `queue.rs`, `scheduler.rs`, `search.rs`, `blob.rs`, `auth.rs`, `http.rs`. The GraphQL schema is in `crates/nova-gql/`. An OpenAPI document is served at `/openapi.json`.