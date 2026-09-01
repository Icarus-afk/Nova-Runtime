# SDK — `@novaruntime/sdk`

Location: `sdk/` (TypeScript, `cross-fetch`).

> ⚠️ **Status: work in progress — the SDK is currently out of date with the REST API.**
>
> The Rust backend moved to a new `/api/v1` route layout (see `docs/api.md`), but the
> TypeScript SDK clients still target the **old** paths. Until it is ported, **do not use the
> SDK in new code** — use the small raw-fetch client pattern shown below (also what
> `examples/bloom-market` and the Dashboard use). See the “SDK vs current API” table
> at the bottom of this file.

## Recommended: raw-fetch client (verified, ~80 lines)

This is the pattern used and tested by `examples/bloom-market` (`examples/bloom-market/src/nova.js`).
Copy it into new projects:

```js
// src/nova.js
const API = 'http://127.0.0.1:8642/api/v1';

export let token = null;
export async function login(username = 'admin', password = null) {
  const pwd = password || process.env.NOVA_PASSWORD || process.env.NOVA_ADMIN_PASSWORD || '';
  const res = await fetch(`${API}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password: pwd }),
  });
  if (!res.ok) throw new Error(`login ${res.status}: ${await res.text()}`);
  token = (await res.json()).access_token;
  return token;
}

const auth = () => ({ 'Content-Type': 'application/json', Authorization: `Bearer ${token}` });

export const sqlQuery    = (query)           => fetch(`${API}/sql/query`,    { method: 'POST', headers: auth(), body: JSON.stringify({ query }) }).then(r => r.json());
export const cacheSet    = (key, value, ttlMs = 60000) => fetch(`${API}/cache/${key}`, { method: 'POST', headers: auth(), body: JSON.stringify({ value, ttl_ms: ttlMs }) }).then(r => r.json());
export const queuePublish = (name, body)     => fetch(`${API}/queues/${name}/messages`, { method: 'POST', headers: auth(), body: JSON.stringify({ messages: [{ body }] }) }).then(r => r.json());
export const searchQuery = (index, query, limit = 5) => fetch(`${API}/search/indexes/${index}/query`, { method: 'POST', headers: auth(), body: JSON.stringify({ query, limit }) }).then(r => r.json());
```

Key points (see `docs/api.md` for the full reference):

- **Auth first** — `POST /api/v1/auth/login` → `{ access_token }`. Nova has **no default
  password**: use `NOVA_ADMIN_PASSWORD` from server boot, or the random one logged at boot.
- **SQL responses are columnar** — `{ column_names, rows: [[…]] }`, not objects. Map with
  `rows.map(r => Object.fromEntries(column_names.map((c, i) => [c, r[i]])))`.
- **Bare column names only** — `SELECT a FROM t JOIN s ON a = sid` (no `t.a = s.sid`).
- **No `IF EXISTS`** — `CREATE TABLE` / `DROP TABLE` are plain; dropping a missing table errors.
- **Pagination is `limit`/`offset`** — not GraphQL-style cursor/edges.

## SDK install (once ported)

```bash
cd sdk && npm install && npm run build   # or make sdk
```

## SDK defaults

- `DEFAULT_CONFIG` → `http://127.0.0.1:8642/api/v1` (`host 127.0.0.1`, `port 8642`, `protocol http`, `basePath /api/v1`) — matches `novad`.
- `fromEnv()` reads `NOVA_URL` / `NOVA_API_URL`, falls back to the default above.
- Auth types in the SDK: `token`, `api-key`, `refresh` (`clientId/clientSecret` + `onUnauthorized` refresh), `none`.

## SDK clients — current (stale) surface

Sub-clients: `RuntimeClient`, `DatabaseClient`, `CacheClient`, `QueueClient`, `SchedulerClient`, `SearchClient`, `BlobClient`, `AuthClient`.

## SDK vs current API (mismatch table)

The SDK clients call these paths today:

| SDK method | SDK path today | Current REST API (`docs/api.md`) |
|---|---|---|
| `nova.db.query` | `POST /db/query` | `POST /api/v1/sql/query` |
| `nova.db.execute` | `POST /db/exec` | `POST /api/v1/sql/execute` |
| `nova.db.tables` | `GET /db/tables` | `GET /api/v1/sql/tables` |
| `nova.cache.set/get/del` | `/cache/multi-set`, `/cache/multi-get`, `/cache/multi-del` | `POST/GET /api/v1/cache/:key`, `POST /api/v1/cache/batch` |
| `nova.cache.flush` | `POST /cache/flush` | `POST /api/v1/cache/clear` (or `POST /api/v1/cache/:key` with empty value) |
| `nova.queue.*` | `/queue/*` | `/api/v1/queues/*` |
| `nova.blob.*` | `/blob/*` | `/api/v1/blobs/*` |
| `nova.auth.register` | `POST /auth/register` | `POST /api/v1/auth/users` (admin creates users) |
| `nova.auth.refresh` | `POST /auth/token/refresh` | `POST /api/v1/auth/refresh` |
| `nova.search.query` | `POST /search/:index/query` | `POST /api/v1/search/indexes/:name/query` |
| pagination | GraphQL-style `cursor` / `edges` | `limit` / `offset` |
| `nova.runtime.*` | `/runtime/*` | `/api/v1/runtime/*` (or `/health`, `/metrics`) — verify each |

The **Dashboard** (`dashboard/src/api/client.ts`) uses a correct hand-rolled client against the
current API — good reference for a future SDK port.