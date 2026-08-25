/**
 * Nova Runtime — 30-second quickstart (SDK)
 * Run after `make dev` (novad at 127.0.0.1:8642):
 *   NOVA_URL=http://127.0.0.1:8642/api/v1 npx tsx examples/quickstart.ts
 * Or:
 *   npm i -g tsx && tsx examples/quickstart.ts
 */
import { createClient, fromEnv } from '../sdk/src/index';

// Works with no config — defaults to http://127.0.0.1:8642/api/v1
const nova = process.env.NOVA_URL
  ? fromEnv({ type: 'none' })
  : createClient({
      server: { host: '127.0.0.1', port: 8642, protocol: 'http', basePath: '/api/v1' },
      auth: { type: 'none' }, // dev auto-creates admin/admin123; use token type for prod
    });

async function main() {
  console.log('→ health:', await nova.health().catch(e => ({ error: e.message })));

  // SQL — create, insert, query (uses real SQL engine)
  console.log('\n→ SQL: create table, insert, query');
  try {
    await nova.db.execute(`CREATE TABLE IF NOT EXISTS demo (id INTEGER PRIMARY KEY, name TEXT)`);
    // The SDK DatabaseClient maps to /api/v1/sql — use query helper
    const res = await fetch('http://127.0.0.1:8642/api/v1/sql/execute', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ query: `INSERT INTO demo (id, name) VALUES (1, 'alice')` }),
    }).then(r => r.json());
    console.log('  insert:', res);
    const q = await fetch('http://127.0.0.1:8642/api/v1/sql/query', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ query: 'SELECT * FROM demo' }),
    }).then(r => r.json());
    console.log('  query:', q.rows ?? q);
  } catch (e: any) {
    console.log('  (sql demo skipped:', e.message, ')');
  }

  // Cache
  console.log('\n→ Cache: set/get');
  try {
    await nova.cache.set('hello', { msg: 'world' }, { ttlMs: 60000 });
    const v = await nova.cache.get('hello');
    console.log('  get(hello):', v);
  } catch (e: any) {
    console.log('  cache error:', e.message);
  }

  // Queue
  console.log('\n→ Queue: create + publish + poll');
  try {
    const qname = `demo-${Date.now()}`;
    await nova.queue.create(qname);
    await nova.queue.send(qname, { hello: 'queue' });
    const polled = await nova.queue.receive(qname, { maxMessages: 1 });
    console.log('  polled:', polled);
  } catch (e: any) {
    console.log('  queue error:', e.message);
  }

  // Auth (if novad running, admin/admin123 exists)
  console.log('\n→ Auth: login as admin (if enabled)');
  try {
    const token = await fetch('http://127.0.0.1:8642/api/v1/auth/login', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username: 'admin', password: 'admin123' }),
    }).then(r => r.json());
    console.log('  login:', token.token_type ? 'ok' : token);
  } catch (e: any) {
    console.log('  login error (auth may be disabled):', e.message);
  }

  console.log('\n✓ quickstart done — see sdk/src/*.ts and docs/ for next steps');
}

main().catch(e => {
  console.error('quickstart failed:', e);
  process.exit(1);
});
