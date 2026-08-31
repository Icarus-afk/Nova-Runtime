import {
  login, health, sqlQuery, sqlExecute, cacheSet, cacheGet,
  queueCreate, queuePublish, queuePoll,
  schedulerCreate, schedulerList,
  searchCreateIndex, searchAddDocs, searchQuery,
  blobUpload, blobList, createUser
} from './nova.js';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

async function ensureHealth() {
  for (let i = 0; i < 15; i++) {
    try { const h = await health(); if (h.status) return h; } catch {}
    console.log(`  Waiting for Nova at 8642... (${i + 1}/15)`);
    await sleep(1000);
  }
  throw new Error('Nova not reachable at http://127.0.0.1:8642 — run make dev from repo root first');
}

async function main() {
  console.log('┌─ Bloom Market — Fresh Project Seed');
  console.log('│  Demonstrates every Nova primitive in one flow');
  console.log('└────────────────────────────────────────\n');

  console.log('→ Health check');
  await ensureHealth();
  console.log('✓ Nova is up (http://127.0.0.1:8642/health)');

  // Bootstrapped user is always "admin" — password is NOVA_ADMIN_PASSWORD (here: Ehasan,123)
  // We log in as admin, then optionally create/use Ehasan as a separate marketplace user
  const ADMIN_USER = process.env.NOVA_USERNAME || 'admin';
  const ADMIN_PASS = process.env.NOVA_PASSWORD || process.env.NOVA_ADMIN_PASSWORD || 'Ehasan,123';
  try {
    await login(ADMIN_USER, ADMIN_PASS);
    console.log(`✓ Logged in as ${ADMIN_USER} (JWT acquired)\n`);
  } catch (e) {
    console.error(`\n✗ Login failed for ${ADMIN_USER}: ${e.message}\n`);
    console.error('  Nova was just reset with NOVA_ADMIN_PASSWORD=Ehasan,123');
    console.error('  Try:');
    console.error('    NOVA_USERNAME=admin NOVA_PASSWORD="Ehasan,123" npm run seed');
    console.error('  If you want Ehasan as login, first login as admin then create Ehasan:\n');
    throw e;
  }

  // ── AUTH: fresh users & API keys
  console.log('→ Auth: creating users');
  for (const [u, p, role] of [
    ['mira', 'mira1234', 'seller'],
    ['jonah', 'jonah1234', 'seller'],
    ['sasha', 'sasha1234', 'buyer'],
  ]) {
    try { await createUser(u, p, role); console.log(`  + user ${u} (${role})`); } catch (e) { console.log(`  · ${u} exists`); }
  }
  console.log('✓ Auth: 3 users (mira, jonah sellers; sasha buyer) + admin\n');

  // ── SQL: tables, JOINs, GROUP BY, ORDER BY
  console.log('→ SQL: creating tables');
  const ddl = [
    `DROP TABLE reviews`,
    `DROP TABLE orders`,
    `DROP TABLE listings`,
    `DROP TABLE sellers`,
    `CREATE TABLE sellers (sid INTEGER PRIMARY KEY, username TEXT, stall TEXT, bio TEXT)`,
    `CREATE TABLE listings (id INTEGER PRIMARY KEY, seller_id INTEGER, title TEXT, description TEXT, category TEXT, price REAL, stock INTEGER, created_at TEXT)`,
    `CREATE TABLE orders (oid INTEGER PRIMARY KEY, listing_id INTEGER, buyer TEXT, qty INTEGER, total REAL, status TEXT, created_at TEXT)`,
    `CREATE TABLE reviews (rid INTEGER PRIMARY KEY, listing_id INTEGER, author TEXT, rating INTEGER, body TEXT)`,
  ];
  for (const q of ddl) {
    try { await sqlExecute(q); } catch (e) { /* DROP may fail if not exists — ok for fresh */ }
  }
  console.log('✓ Tables: sellers, listings, orders, reviews');

  console.log('→ SQL: seeding sellers & listings');
  const sellers = [
    [1, 'mira', 'Mira’s Orchard', 'Stone fruit & honey, South Slope'],
    [2, 'jonah', 'Jonah Bakehouse', 'Sourdough & seasonal pastries, since 2019'],
  ];
  for (const [id, u, stall, bio] of sellers) {
    await sqlExecute(`INSERT INTO sellers (sid, username, stall, bio) VALUES (${id}, '${u}', '${stall}', '${bio.replace(/'/g, "''")}')`);
  }
  const listings = [
    [1, 1, 'Sun Gold Apricots — 1kg', 'Tree-ripened, picked this morning. Sweet-tart balance.', 'produce', 9.5, 18],
    [2, 1, 'Raw Wildflower Honey 500g', 'Unfiltered, from Mira’s hives. Floral, late summer.', 'pantry', 14.0, 12],
    [3, 1, 'Heirloom Tomatoes — mixed 750g', 'Brandywine + Cherokee Purple, dry-farmed.', 'produce', 8.0, 20],
    [4, 1, 'Pluot Jam — small batch', 'Pluot + vanilla, 220g jar. Great with cheese.', 'pantry', 7.5, 15],
    [5, 2, 'Seeded Sourdough', '50% whole wheat, 48h ferment. Crusty, nutty.', 'bakery', 6.5, 10],
    [6, 2, 'Cardamom Buns (6)', 'Kardemummabullar, with pearl sugar.', 'bakery', 12.0, 8],
    [7, 2, 'Miso Rye Loaf', 'Rye + barley miso, umami, soft crumb.', 'bakery', 7.0, 9],
    [8, 2, 'Seasonal Fruit Galette', 'Peach + apricot, butter crust, serves 4.', 'bakery', 18.0, 5],
    [9, 1, 'Basil Bunch + Strawberries', 'Genovese basil + 250g Mara des Bois.', 'produce', 6.0, 14],
    [10, 2, 'Olive & Herb Focaccia', 'Castelvetrano, rosemary, sea salt.', 'bakery', 8.5, 11],
    [11, 1, 'Dried Apricot Trail Mix 400g', 'Apricot, almond, toasted oat clusters.', 'pantry', 9.0, 16],
    [12, 2, 'Cinnamon Morning Buns (4)', 'Soft, brown sugar, overnight proof.', 'bakery', 10.0, 7],
  ];
  for (const [id, sid, title, desc, cat, price, stock] of listings) {
    await sqlExecute(`INSERT INTO listings (id, seller_id, title, description, category, price, stock, created_at) VALUES (${id}, ${sid}, '${title.replace(/'/g, "''")}', '${desc.replace(/'/g, "''")}', '${cat}', ${price}, ${stock}, '2026-08-31')`);
  }
  console.log(`✓ Listings: ${listings.length} (produce, bakery, pantry)`);

  console.log('→ SQL: orders + reviews');
  const orders = [
    [1, 5, 'sasha', 2, 13.0, 'paid'],
    [2, 1, 'sasha', 1, 9.5, 'paid'],
    [3, 6, 'mira', 1, 12.0, 'shipped'],
    [4, 8, 'jonah', 1, 18.0, 'paid'],
  ];
  for (const [oid, lid, buyer, qty, total, status] of orders) {
    await sqlExecute(`INSERT INTO orders (oid, listing_id, buyer, qty, total, status, created_at) VALUES (${oid}, ${lid}, '${buyer}', ${qty}, ${total}, '${status}', '2026-08-31')`);
  }
  const reviews = [
    [1, 5, 'sasha', 5, 'Best sourdough in the neighborhood. Crust is perfect.'],
    [2, 5, 'mira', 5, 'My customers ask for Jonah’s bread every Saturday.'],
    [3, 1, 'jonah', 4, 'Apricots were unreal — jam material.'],
    [4, 6, 'sasha', 5, 'Cardamom buns disappeared in 10 minutes.'],
    [5, 8, 'mira', 4, 'Galette fed the whole stall team.'],
  ];
  for (const [rid, lid, author, rating, body] of reviews) {
    await sqlExecute(`INSERT INTO reviews (rid, listing_id, author, rating, body) VALUES (${rid}, ${lid}, '${author}', ${rating}, '${body.replace(/'/g, "''")}')`);
  }
  console.log(`✓ Orders: ${orders.length}, Reviews: ${reviews.length}`);

  // Demonstrate JOIN + GROUP BY + ORDER BY alias (Nova SQL uses bare column names)
  console.log('\n→ SQL: demo queries (fresh project pattern)');
  const joinDemo = await sqlQuery(`SELECT title, stall FROM listings JOIN sellers ON seller_id = sid LIMIT 3`);
  console.log(`  JOIN listings->sellers: ${joinDemo.rows?.length ?? joinDemo.row_count ?? '?'} rows`);
  const groupDemo = await sqlQuery(`SELECT category, COUNT(*) as cnt FROM listings GROUP BY category HAVING COUNT(*) > 2 ORDER BY cnt DESC`);
  console.log(`  GROUP BY category HAVING cnt>2:`, groupDemo.rows ?? groupDemo);
  const orderDemo = await sqlQuery(`SELECT title, price FROM listings ORDER BY 2 DESC LIMIT 3`);
  console.log(`  ORDER BY ordinal (price desc):`, (orderDemo.rows ?? []).slice(0, 2).map(r => r.title ?? r[0]));
  console.log('✓ SQL: JOIN, GROUP BY/HAVING, ORDER BY alias/ordinal all work\n');

  // ── Search
  console.log('→ Search: indexing listings');
  await searchCreateIndex('listings_idx', [
    { name: 'title', type: 'text', analyzer: 'standard', boost: 2.0 },
    { name: 'description', type: 'text' },
    { name: 'category', type: 'keyword' },
  ]);
  const docs = listings.map(([id, , title, desc, cat]) => ({ id: String(id), title, description: desc, category: cat }));
  await searchAddDocs('listings_idx', docs);
  const sq = await searchQuery('listings_idx', 'sourdough', { limit: 3 });
  console.log(`✓ Search: listings_idx (${docs.length} docs) — query "sourdough" -> ${sq.hits?.length ?? sq.total_hits ?? '?'} hits`);

  // ── Blob
  console.log('\n→ Blob: uploading listing photos (namespace=bloom)');
  const fakeJpg = Buffer.from([0xff, 0xd8, 0xff, ...Buffer.from('FAKEJPEG' + 'x'.repeat(2048))]);
  const fakePng = Buffer.from('FAKEPNG' + 'y'.repeat(1536));
  for (const [name, buf, type] of [
    ['apricots.jpg', fakeJpg, 'image/jpeg'],
    ['sourdough.png', fakePng, 'image/png'],
    ['honey-label.txt', Buffer.from('Mira Orchard — ingredients: raw honey. 500g. Batch 42.'), 'text/plain'],
  ]) {
    const r = await blobUpload('bloom', buf, name, type);
    console.log(`  · ${name} -> ${String(r.id).slice(0, 12)} (${r.size_bytes ?? buf.length}B)`);
  }
  const bl = await blobList('bloom', 10);
  console.log(`✓ Blob: bloom namespace — ${bl.blobs?.length ?? bl.data?.length ?? '?'} files`);

  // ── Cache
  console.log('\n→ Cache: hot listings & carts');
  await cacheSet('bloom:trending', { top: [5, 6, 1], generated_at: new Date().toISOString() }, 60_000);
  await cacheSet('bloom:cart:sasha', { items: [{ listing_id: 5, qty: 2 }, { listing_id: 9, qty: 1 }], updated_at: Date.now() }, 30_000);
  for (const id of [5, 6, 1]) {
    const l = listings.find(x => x[0] === id);
    await cacheSet(`hot:listing:${id}`, { id, title: l[2], price: l[5] }, 30_000);
  }
  const trending = await cacheGet('bloom:trending');
  console.log(`✓ Cache: bloom:trending, bloom:cart:sasha, 3 hot listings (TTL 30-60s) — trending top=${trending?.value?.top ?? trending?.top}`);

  // ── Queue
  console.log('\n→ Queue: order processing + notifications');
  await queueCreate('bloom:orders', { durable: true });
  await queueCreate('bloom:notifications', { durable: true });
  await queuePublish('bloom:orders', { type: 'order.created', order_id: 1, buyer: 'sasha', total: 13.0 });
  await queuePublish('bloom:orders', { type: 'order.created', order_id: 2, buyer: 'sasha', total: 9.5 });
  await queuePublish('bloom:orders', { type: 'order.paid', order_id: 1 });
  await queuePublish('bloom:notifications', { type: 'ship', order_id: 3, eta: '2026-09-02' }, 4000);
  console.log('✓ Queue: bloom:orders (3), bloom:notifications (1 delayed 4s)');
  const polled = await queuePoll('bloom:orders', 2);
  console.log(`  poll bloom:orders (2) -> ${polled.messages?.length ?? polled.data?.length ?? '?'} messages`);

  // ── Scheduler
  console.log('\n→ Scheduler: daily digest + flash sale');
  try {
    await schedulerCreate({ name: 'bloom:daily-digest', type: 'cron', schedule: '0 9 * * *', timezone: 'UTC', action: { kind: 'digest', channel: 'email' }, enabled: true });
    console.log('  · cron bloom:daily-digest 0 9 * * * (UTC)');
  } catch (e) { console.log(`  · daily-digest exists: ${e.message.slice(0,60)}`); }
  try {
    await schedulerCreate({ name: 'bloom:flash-sale', type: 'interval', schedule: '30m', action: { kind: 'flash', discount: 15 }, enabled: true });
    console.log('  · interval bloom:flash-sale 30m');
  } catch (e) { console.log(`  · flash-sale exists: ${e.message.slice(0,60)}`); }
  try {
    const ls = await schedulerList();
    console.log(`✓ Scheduler: ${ls.data?.length ?? ls.jobs?.length ?? '?'} jobs`);
  } catch { console.log('✓ Scheduler: jobs created'); }

  console.log('\n┌─ Seed complete — Fresh project ready!');
  console.log('│  Every primitive exercised: SQL, Cache, Queue, Scheduler, Search, Blob, Auth');
  console.log('└────────────────────────────────────────');
  console.log('');
  console.log('  Try:');
  console.log('    SQL JOIN: curl -s http://127.0.0.1:8642/api/v1/sql/query -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d \'{"query":"SELECT listings.title, sellers.stall FROM listings JOIN sellers ON listings.seller_id=sellers.id"}\' | jq');
  console.log('    Search: curl -s http://127.0.0.1:8642/api/v1/search/indexes/listings_idx/query -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" -d \'{"query":"honey","limit":5}\' | jq');
  console.log('    UI: cd examples/bloom-market && npm run dev  -> http://localhost:3001');
  console.log('    Dashboard: http://127.0.0.1:5173 -> Database / Search / Blob / Queue / Scheduler');
}

main().catch(e => { console.error('\n✗ Seed failed:', e.message); console.error(e.stack?.slice(0, 800)); process.exit(1); });
