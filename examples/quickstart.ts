/**
 * Nova Runtime — 30-second quickstart (raw fetch, no SDK — SDK is currently out of sync)
 * Run after starting novad (http://127.0.0.1:8642):
 *   NOVA_USERNAME=admin NOVA_PASSWORD="your-password" npx tsx examples/quickstart.ts
 *
 * Auth: Nova bootstraps the 'admin' user with the password you pass as
 * NOVA_ADMIN_PASSWORD when starting novad (or prints a random one in the log).
 * There is no hardcoded default password.
 */
const BASE = process.env.NOVA_URL || 'http://127.0.0.1:8642/api/v1';

async function login() {
  const username = process.env.NOVA_USERNAME || 'admin';
  const password = process.env.NOVA_PASSWORD || process.env.NOVA_ADMIN_PASSWORD || '';
  if (!password) throw new Error('set NOVA_PASSWORD (or NOVA_ADMIN_PASSWORD) — Nova has no default');
  const r = await fetch(`${BASE}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });
  if (!r.ok) throw new Error(`login ${r.status}: ${await r.text()}`);
  const { access_token } = await r.json();
  console.log(`→ logged in as ${username}`);
  return access_token;
}

const post = (path: string, body: unknown, token: string) =>
  fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  }).then(r => r.json());

async function main() {
  const token = await login();

  // SQL — create, insert, query. NOTE: responses are {column_names, rows} — map to objects.
  console.log('\n→ SQL: create table, insert, query');
  await post('/sql/execute', { query: 'DROP TABLE demo' }, token).catch(() => {});
  console.log('  create:', await post('/sql/execute', { query: 'CREATE TABLE demo (id INTEGER PRIMARY KEY, name TEXT)' }, token));
  console.log('  insert:', await post('/sql/execute', { query: "INSERT INTO demo (id, name) VALUES (1, 'alice')" }, token));
  const q = await post('/sql/query', { query: 'SELECT * FROM demo' }, token);
  console.log('  query:', q.rows.map((r: unknown[]) => Object.fromEntries(q.column_names.map((c: string, i: number) => [c, r[i]]))));

  // Cache
  console.log('\n→ Cache: set/get');
  console.log('  set:', await post(`/cache/hello`, { value: { msg: 'world' }, ttl_ms: 60000 }, token));
  const v = await fetch(`${BASE}/cache/hello`, { headers: { Authorization: `Bearer ${token}` } }).then(r => r.json());
  console.log('  get:', v);

  // Queue — create + publish + poll
  console.log('\n→ Queue: create + publish + poll');
  const qname = `demo-${Date.now()}`;
  console.log('  create:', await post('/queues', { name: qname }, token));
  console.log('  publish:', await post(`/queues/${qname}/messages`, { messages: [{ body: { hello: 'queue' } }] }, token));
  console.log('  poll:', await post(`/queues/${qname}/messages/poll`, { ack: true }, token));

  // Auth — we already have a token; prove it works by listing users
  console.log('\n→ Auth: verify token by listing users');
  const users = await fetch(`${BASE}/auth/users`, { headers: { Authorization: `Bearer ${token}` } }).then(r => r.json());
  console.log('  users:', JSON.stringify(users).slice(0, 200));

  console.log('\n✓ quickstart done — see docs/getting-started.md for the full guide');
}

main().catch(e => {
  console.error('quickstart failed:', e);
  process.exit(1);
});