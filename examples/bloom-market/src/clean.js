import { login, sqlExecute, queueCreate } from './nova.js';

async function main() {
  const u = process.env.NOVA_USERNAME || 'admin';
  const p = process.env.NOVA_PASSWORD || process.env.NOVA_ADMIN_PASSWORD || '';
  await login(u, p);
  console.log('→ Cleaning Bloom Market data...');
  for (const q of [
    `DROP TABLE IF EXISTS reviews`,
    `DROP TABLE IF EXISTS orders`,
    `DROP TABLE IF EXISTS listings`,
    `DROP TABLE IF EXISTS sellers`,
  ]) {
    try { await sqlExecute(q); console.log(`  · ${q.slice(0, 45)}`); } catch {}
  }
  // Queues / search / blobs are best inspected via dashboard; seed is idempotent via DROP
  console.log('✓ Clean done. Run npm run seed to recreate.');
}
main().catch(e => { console.error(e); process.exit(1); });
