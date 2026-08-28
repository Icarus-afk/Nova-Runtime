# SDK — `@novaruntime/sdk`

Location: `sdk/` (TypeScript, `cross-fetch`).

## Install

```bash
cd sdk && npm install && npm run build   # or make sdk
# publish: npm publish (access public, files: dist)
```

## Defaults

- `DEFAULT_CONFIG` → `http://127.0.0.1:8642/api/v1` (`host 127.0.0.1, port 8642, protocol http, basePath /api/v1`) — matches `novad` default. Previously `https://localhost:8443/v1` (broken).

## Usage

```ts
import { createClient, fromEnv } from '@novaruntime/sdk';
// or direct: import { createClient } from '../sdk/src/index';

const nova = fromEnv({ type: 'token', token: process.env.NOVA_TOKEN! });
// Reads NOVA_URL or NOVA_API_URL env (e.g. http://127.0.0.1:8642/api/v1), fallback to localhost default
// Alternately:
const nova2 = createClient({
  server: { host: '127.0.0.1', port: 8642, protocol: 'http', basePath: '/api/v1' },
  auth: { type: 'none' },
  retry: { maxRetries: 3 },
});

await nova.health();
await nova.db.query('SELECT 1');
await nova.cache.set('k', { v: 1 }, { ttlMs: 60000 });
await nova.queue.create('myq'); await nova.queue.send('myq', { hello: 1 });
```

See `examples/quickstart.ts` (`NOVA_URL=... npx tsx examples/quickstart.ts`) and `examples/quickstart.sh` (curl). Auth types: `token`, `api-key`, `refresh` (`clientId/clientSecret` + `onUnauthorized` refresh), `none`.

## Clients

- `FetchHttpClient` handles `Authorization: Bearer`, `X-Request-ID`, `Idempotency-Key`, retries with `createRetryPolicy`, `TOKEN_EXPIRED` refresh, timeout `AbortController`.
- Sub-clients: `RuntimeClient`, `DatabaseClient`, `CacheClient`, `QueueClient`, `SchedulerClient`, `SearchClient`, `BlobClient`, `AuthClient` (`crates/nova-api` 1:1).

## Build

`sdk/package.json` `build: tsc`, `test: jest`. `dist/` contains `*.js`+`*.d.ts`+`.map`.

Env file: `.env.example` (`NOVA_URL`, `NOVA_TOKEN`, `RUST_LOG`).
