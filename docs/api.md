# API — REST & GraphQL

Base: `http://127.0.0.1:8642` (`NOVA_URL=http://127.0.0.1:8642/api/v1` for SDK).

## Auth

- `POST /api/v1/auth/login {"username","password","ttl_seconds?"}` → `{"token_type":"Bearer","access_token","expires_in"}` + IP rate limit 5/min → `429` with `retry_after_secs`. Auto-creates `admin/admin123` on first boot.
- `POST /api/v1/auth/refresh {"refresh_token"}` → new `access_token`.
- `POST /api/v1/auth/logout` `Authorization: Bearer <token>` → revokes.
- `POST/GET/DELETE /api/v1/auth/api-keys` `{"name","permissions","expires_at": "RFC3339|millis"}` → `expires_at` now honored (future check). `401` if missing `Bearer`.

All `/api/v1/*` except `/auth/login|/auth/refresh` + `/health|/ready|/live|/metrics|/openapi.json` require `Bearer`. See `crates/nova-api/src/middleware.rs:89`.

## SQL `/api/v1/sql`

- `POST /query {"query","params?": [val], "limit?", "format?": "json|csv|arrow"}` → interpolates `$1`/`$2` with escaping, validates `format`, truncates `rows` to `limit` with `truncated` flag. Returns `{"columns","column_names","types","rows","row_count","truncated","execution_time_ms","format"}`.
- `POST /execute` same `params` handling → `{"affected_rows"}`.
- `GET /tables`, `GET /tables/:table/schema`.

## Cache `/api/v1/cache`

- `GET /:key` → `{"key","value","ttl_remaining_ms"}`, `POST /:key {"value","ttl_ms?"}`, `DELETE /:key`, `POST /batch`, `GET /keys?pattern=` (glob `*`/`?` via `regex`), `GET /stats`.

## Queue `/api/v1/queues`

- `POST / {"name","durable?","max_length?","max_message_size?"}` → patches `IndividualQueueConfig` via `backend.update_queue`.
- `POST /:name/messages {"messages":[{"body","delay_ms?"}]}` → `delay_ms` (≤7d) sets `delay_until/visible_at` → delayed index.
- `POST /:name/messages/poll {"count?","visibility_timeout_ms?"}` → honors both (clamps `count` 1..100, `visibility` 12h).
- `POST /:name/messages/:id/ack`, `POST /:name/purge`, `GET /:name/stats`.

## Scheduler `/api/v1/scheduler`

- `POST /jobs {"name","type":"cron|interval|one_time","schedule?","timezone?","action?","max_retries?","retry_delay_ms?","enabled?"}` → validates `cron`, computes `next_after`, stores `timezone`+`action()->payload` in `tags`, `enabled=false → Paused`. Returns `id` (real `Uuid`, not `job_...` mock).
- `GET /jobs`, `GET /jobs/:id`, `DELETE /jobs/:id`, `POST /jobs/:id/{trigger,pause,resume}`, `GET /stats`.

## Search `/api/v1/search`

- Registry `OnceLock<RwLock<HashMap>>`. `POST /indexes {"name","fields?":[{"name","type":"text|keyword|integer|float|boolean","analyzer?","boost?"}]}` validates; `GET /indexes` lists registry; `DELETE /indexes/:name` 404 if missing.
- `POST /indexes/:name/documents {"documents":[{"id",...}]}` checks index exists, builds `IndexedDocument` correctly (was cloning bug).
- `POST /indexes/:name/query {"query","limit?","offset?"}` → `search` or `search_with_pagination` with `skip/take`.
- `GET /indexes/:name/stats`.

## Blob `/api/v1/blobs`

- `POST /?namespace=&bucket=` multipart (`name=file`) byte-parsed or raw `Bytes` with `Content-Type`; validates `namespace` not `..`/`/`, rejects empty, stores `SHA256` dedup chunks. Dashboard sends `?namespace=<bucket>`.
- `GET /?prefix=&limit=&namespace=` filters `starts_with`, `limit` 1..1000, `has_more`.
- `GET /:id`, `GET /:id/info`, `DELETE /:id`, `GET /stats`.

## GraphQL `/graphql`

- `GET` playground, `POST` query with `async-graphql` schema (`crates/nova-gql`). Shares same `AdminState` subsystems.

Errors: `ApiError` → `{"type":"about:blank","title","status","detail","instance","extra"}` with proper `StatusCode` mapping (499 → `BAD_REQUEST` fallback).

## Pagination

Uniform: `limit`/`offset` or `count`/`page`, returns `pagination:{cursor,limit,has_more}` or `total/pages`.

See `crates/nova-api/src/routes/*.rs` and `docs/04-rest-api-reference.md` legacy (now merged here).
