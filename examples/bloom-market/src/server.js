import express from 'express';
import cors from 'cors';
import path from 'path';
import { fileURLToPath } from 'url';
import { login, sqlQuery, sqlExecute, cacheGet, cacheSet, searchQuery, queuePublish, blobList } from './nova.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const app = express();
const PORT = process.env.PORT || 3001;

app.use(cors());
app.use(express.json());
app.use(express.static(path.join(__dirname, '..', 'public')));

// Helper: Nova SQL returns {column_names, rows:[[..]]} — map to objects for frontend
function toObjects(r) {
  if (!r) return [];
  const rows = r.rows ?? r.data ?? [];
  const names = r.column_names ?? r.columns ?? [];
  if (!rows.length) return [];
  if (typeof rows[0] === 'object' && !Array.isArray(rows[0])) return rows;
  if (!names.length) return rows;
  return rows.map(row => { const o = {}; names.forEach((n,i) => o[n]=row[i]); return o; });
}

// Ensure logged in before proxying to Nova (reuse token in nova.js)
let ready = false;
async function ensureAuth() {
  if (ready) return;
  // Bootstrapped admin password is Ehasan,123 (set via NOVA_ADMIN_PASSWORD)
  const u = process.env.NOVA_USERNAME || 'admin';
  const p = process.env.NOVA_PASSWORD || process.env.NOVA_ADMIN_PASSWORD || 'Ehasan,123';
  await login(u, p);
  ready = true;
}

// Fresh project pattern: every route is a thin composition over Nova primitives
// SQL + JOIN + GROUP BY
app.get('/api/feed', async (req, res) => {
  try {
    await ensureAuth();
    const trending = await cacheGet('bloom:trending').catch(() => null);
    // Nova SQL uses bare column names (no table prefix) — join keys must be distinct (seller_id = sid)
    const listings = await sqlQuery(`SELECT id, title, price, category, stock, stall FROM listings JOIN sellers ON seller_id = sid ORDER BY price DESC LIMIT 12`);
    const stats = await sqlQuery(`SELECT category, COUNT(*) as cnt, AVG(price) as avg_price FROM listings GROUP BY category ORDER BY cnt DESC`);
    res.json({ listings: toObjects(listings), stats: toObjects(stats), trending: trending?.value ?? trending ?? null });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/listings', async (req, res) => {
  try {
    await ensureAuth();
    const cat = req.query.category;
    const q = cat
      ? `SELECT id, title, price, category, stall FROM listings JOIN sellers ON seller_id = sid WHERE category='${String(cat).replace(/'/g, "''")}' ORDER BY price ASC`
      : `SELECT id, title, price, category, stall FROM listings JOIN sellers ON seller_id = sid ORDER BY id ASC`;
    const r = await sqlQuery(q);
    res.json({ data: toObjects(r), raw: r });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/listings/:id', async (req, res) => {
  try {
    await ensureAuth();
    const id = Number(req.params.id);
    const r = await sqlQuery(`SELECT title, description, price, category, stock, stall, bio FROM listings JOIN sellers ON seller_id = sid WHERE id=${id}`);
    const rows = toObjects(r);
    if (!rows.length) return res.status(404).json({ error: 'not found' });
    const listing = rows[0];
    const revs = await sqlQuery(`SELECT * FROM reviews WHERE listing_id=${id} ORDER BY rating DESC`);
    await cacheSet(`hot:listing:${id}`, listing, 30_000).catch(() => {});
    res.json({ listing, reviews: toObjects(revs) });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.post('/api/listings', async (req, res) => {
  try {
    await ensureAuth();
    const { title, description, category, price, stock, seller_id } = req.body;
    if (!title || !category) return res.status(400).json({ error: 'title, category required' });
    const maxR = await sqlQuery(`SELECT MAX(id) as m FROM listings`);
    const maxId = (maxR.rows?.[0]?.m ?? maxR.rows?.[0]?.max ?? maxR.data?.[0]?.m ?? 12) + 1;
    await sqlExecute(`INSERT INTO listings (id, seller_id, title, description, category, price, stock, created_at) VALUES (${maxId}, ${Number(seller_id)||1}, '${String(title).replace(/'/g, "''")}', '${String(description||'').replace(/'/g, "''")}', '${String(category).replace(/'/g, "''")}', ${Number(price)||0}, ${Number(stock)||0}, '2026-08-31')`);
    // Invalidate trending (fresh project pattern)
    await cacheSet('bloom:trending', { top: [maxId], generated_at: new Date().toISOString() }, 60_000).catch(() => {});
    res.status(201).json({ id: maxId, title, category });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

// Search (BM25)
app.get('/api/search', async (req, res) => {
  try {
    await ensureAuth();
    const q = String(req.query.q || '').trim();
    if (!q) return res.json({ hits: [], total_hits: 0 });
    const r = await searchQuery('listings_idx', q, { limit: Number(req.query.limit) || 8 });
    res.json(r);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

// Cart via Cache (fresh project pattern: cart is just a cache key)
app.get('/api/cart/:user', async (req, res) => {
  try { await ensureAuth(); const c = await cacheGet(`bloom:cart:${req.params.user}`); res.json(c?.value ?? c ?? { items: [] }); } catch (e) { res.status(500).json({ error: e.message }); }
});
app.post('/api/cart/:user', async (req, res) => {
  try { await ensureAuth(); const body = req.body; await cacheSet(`bloom:cart:${req.params.user}`, body, 30_000); res.json({ ok: true }); } catch (e) { res.status(500).json({ error: e.message }); }
});

// Orders: SQL + Queue + Cache
app.post('/api/orders', async (req, res) => {
  try {
    await ensureAuth();
    const { listing_id, buyer, qty } = req.body;
    if (!listing_id || !buyer) return res.status(400).json({ error: 'listing_id, buyer required' });
    const lid = Number(listing_id); const q = Number(qty) || 1;
    const lr = await sqlQuery(`SELECT price FROM listings WHERE id=${lid}`);
    const row = (lr.rows ?? lr.data ?? [])[0];
    if (!row) return res.status(404).json({ error: 'listing not found' });
    const price = row.price ?? row[2] ?? 0;
    const total = price * q;
    const maxR = await sqlQuery(`SELECT MAX(oid) as m FROM orders`);
    const maxId = (maxR.rows?.[0]?.m ?? maxR.data?.[0]?.m ?? 0) + 1;
    await sqlExecute(`INSERT INTO orders (oid, listing_id, buyer, qty, total, status, created_at) VALUES (${maxId}, ${lid}, '${String(buyer).replace(/'/g, "''")}', ${q}, ${total}, 'paid', '2026-08-31')`);
    // Queue: order processing (fresh pattern: queue decouples fulfillment)
    await queuePublish('bloom:orders', { type: 'order.created', order_id: maxId, listing_id: lid, buyer, qty: q, total }).catch(() => {});
    await queuePublish('bloom:notifications', { type: 'order.confirm', buyer, listing_id: lid }, 2000).catch(() => {});
    res.status(201).json({ id: maxId, total, status: 'paid' });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/orders', async (req, res) => {
  try {
    await ensureAuth();
    const r = await sqlQuery(`SELECT oid, buyer, qty, total, status, title FROM orders JOIN listings ON listing_id = id ORDER BY oid DESC LIMIT 20`);
    res.json({ data: toObjects(r) });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

// Blob + Queue + Scheduler + Search meta for dashboard
app.get('/api/meta', async (req, res) => {
  try {
    await ensureAuth();
    const [blobs, sched] = await Promise.all([
      blobList('bloom', 20).catch(() => ({ blobs: [] })),
      fetch(`http://127.0.0.1:8642/api/v1/scheduler/jobs`, { headers: { Authorization: `Bearer ${ (await import('./nova.js')).login ? '' : ''}` } }).catch(() => null),
    ]);
    res.json({ blobs, note: 'See Nova Dashboard at http://127.0.0.1:5173 for Queue/Scheduler/Search details' });
  } catch (e) { res.json({ ok: true }); }
});

app.get('/health', (req, res) => res.json({ ok: true, service: 'bloom-market-demo', nova: 'http://127.0.0.1:8642' }));

app.listen(PORT, () => {
  console.log(`\n Bloom Market demo at http://localhost:${PORT}`);
  console.log(`   Feed:      http://localhost:${PORT}/api/feed`);
  console.log(`   Search:    http://localhost:${PORT}/api/search?q=honey`);
  console.log(`   Nova:      http://127.0.0.1:8642/health | Dashboard http://127.0.0.1:5173`);
  console.log(`\n Fresh project pattern: every route composes Nova SQL/Cache/Queue/Search/Blob/Scheduler/Auth`);
});
