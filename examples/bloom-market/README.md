# Bloom Market — Fresh Project Template 100% on Nova Runtime

A neighborhood fresh-goods marketplace where **every byte lives in Nova** — no Postgres, no Redis, no S3. This is the **fresh project demo** (not a todo app): it shows how to use every Nova primitive cleanly in a new app.

| Nova Subsystem | How Bloom uses it |
|----------------|-------------------|
| **SQL** | `sellers`, `listings`, `orders`, `reviews` — `JOIN`, `GROUP BY`/`HAVING`, `ORDER BY` alias/ordinal |
| **Cache** | `bloom:trending`, `hot:listing:{id}`, `bloom:cart:{user}` with TTL |
| **Queue** | `bloom:orders` FIFO + `bloom:notifications` delayed (shipping ETA) |
| **Scheduler** | `bloom:daily-digest` cron `0 9 * * *` + `bloom:flash-sale` interval 30m |
| **Search** | `listings_idx` BM25 on title (boost 2x) + description + category |
| **Blob** | Listing photos in `bloom` namespace (SHA256 dedup, range) |
| **Auth** | `admin` + `mira`/`jonah` sellers + `sasha` buyer, JWT, RBAC |
| **Event** | Orders flow via queues — fulfillment decoupled |

## Fresh Project? Copy this pattern

In any new Node.js / TypeScript project:

```js
// src/nova.js — 80 lines, copy-paste from examples/bloom-market/src/nova.js
const API = 'http://127.0.0.1:8642/api/v1';
let token = (await fetch(`${API}/auth/login`, {
  method: 'POST', body: JSON.stringify({ username: 'admin', password: 'admin123' })
}).then(r=>r.json())).access_token;

// Then:
await fetch(`${API}/sql/query`, { method:'POST', headers:{Authorization:`Bearer ${token}`}, body: JSON.stringify({ query:'SELECT * FROM listings JOIN sellers ON listings.seller_id=sellers.id' }) })
await fetch(`${API}/cache/bloom:cart:sasha`, { method:'POST', headers:{Authorization:`Bearer ${token}`}, body: JSON.stringify({ value:{items:[{listing_id:5, qty:2}]}, ttl_ms:30000 }) })
await fetch(`${API}/queues/bloom:orders/messages`, { method:'POST', headers:{Authorization:`Bearer ${token}`}, body: JSON.stringify({ messages:[{body:{order_id:1}}] }) })
await fetch(`${API}/search/indexes/listings_idx/query`, { method:'POST', headers:{Authorization:`Bearer ${token}`}, body: JSON.stringify({ query:'honey', limit:5 }) })
```

Or use the SDK: `import { createClient } from '@novaruntime/sdk'` — see `examples/quickstart.ts`.

## Prereqs

```bash
# 1. Nova must be running
make dev          # from repo root → http://127.0.0.1:8642 + dashboard 5173
# or
docker compose up --build
```

## Run Demo

```bash
cd examples/bloom-market
npm install
npm run seed   # creates tables, 2 sellers, 12 listings, orders, cache, queues, scheduler, search index, blobs
npm run dev    # http://localhost:3001 — Marketplace UI

# View same data in Nova Dashboard:
# http://127.0.0.1:5173 → Database (listings table), Search (listings_idx), Blob (bloom namespace), Queue (bloom:orders)
```

## Seed what?

```
✓ Auth: 3 users (mira, jonah sellers; sasha buyer)
✓ Tables: sellers (2), listings (12), orders (4), reviews (5)
✓ SQL demo: JOIN, GROUP BY/HAVING, ORDER BY ordinal
✓ Search: listings_idx (12 docs) — query "sourdough" -> hits
✓ Blob: 3 files in bloom namespace
✓ Cache: bloom:trending, bloom:cart:sasha, 3 hot listings (TTL)
✓ Queue: bloom:orders (3), bloom:notifications (1 delayed)
✓ Scheduler: bloom:daily-digest (cron), bloom:flash-sale (interval)
```

## API (demo server :3001 proxies to Nova :8642)

- `GET /api/feed` — trending (Cache) + listings JOIN sellers (SQL) + stats GROUP BY
- `GET /api/listings?category=bakery` — filtered JOIN
- `GET /api/listings/:id` — detail + reviews + caches hot listing
- `POST /api/listings` — SQL insert (creates new listing)
- `GET /api/search?q=honey` — BM25 search via `listings_idx`
- `GET /api/cart/:user` / `POST /api/cart/:user` — cart is a Cache key
- `POST /api/orders` — SQL insert + Queue publish (order + notification) + cache pattern
- `GET /api/orders` — orders JOIN listings

All state is in Nova — restart Nova and data persists in `./data`.

## UI

- Grid of listings with category filter + BM25 search
- View → detail + reviews (SQL JOIN)
- Add to cart → Cache (`bloom:cart:{user}`)
- Checkout → SQL `orders` + Queue `bloom:orders` + delayed notification

## Clean

```bash
npm run clean  # DROP TABLE sellers/listings/orders/reviews
```

## Compared to Taskboard

- Taskboard = kanban for tasks — shows project/task/comment flow
- Bloom Market = marketplace for goods — shows seller/listing/order/review flow, more JOINs, more search, cart as cache, order queue, same primitives, different domain. Use whichever pattern fits your app.
