import { login, sqlExecute } from './nova.js';

async function main() {
  await fetch('http://127.0.0.1:8642/health').then(r => r.json()).catch(() => { throw new Error('Nova not running'); });
  await login();
  console.log('Cleaning NovaBoard data...');
  for (const q of [
    `DROP TABLE comments`,
    `DROP TABLE tasks`,
    `DROP TABLE projects`,
    `DROP TABLE users`,
  ]) {
    // Nova SQL has no DROP TABLE IF EXISTS — dropping a missing table errors; ignore that.
    try { await sqlExecute(q); console.log(`  ${q}`); } catch (e) { console.log(`  warn: ${e.message.slice(0,60)}`); }
  }
  // Purge queues, delete search index, blobs via direct fetch
  const loginRes = await fetch('http://127.0.0.1:8642/api/v1/auth/login', {
    method: 'POST', headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      username: process.env.NOVA_USERNAME || 'admin',
      password: process.env.NOVA_PASSWORD || process.env.NOVA_ADMIN_PASSWORD || '',
    }),
  });
  if (!loginRes.ok) throw new Error(`login ${loginRes.status}: ${await loginRes.text()}`);
  const token = (await loginRes.json()).access_token;
  const h = { Authorization: `Bearer ${token}` };
  for (const q of ['task-notifications', 'task-reminders']) {
    await fetch(`http://127.0.0.1:8642/api/v1/queues/${q}/purge`, { method: 'POST', headers: h }).catch(() => {});
    await fetch(`http://127.0.0.1:8642/api/v1/queues/${q}`, { method: 'DELETE', headers: h }).catch(() => {});
  }
  await fetch('http://127.0.0.1:8642/api/v1/search/indexes/tasks_idx', { method: 'DELETE', headers: h }).catch(() => {});
  // Blobs: list and delete in taskboard namespace
  const blobs = await fetch('http://127.0.0.1:8642/api/v1/blobs?namespace=taskboard', { headers: h }).then(r => r.json()).catch(() => ({ data: [] }));
  for (const b of blobs.data || []) {
    await fetch(`http://127.0.0.1:8642/api/v1/blobs/${b.id}`, { method: 'DELETE', headers: h }).catch(() => {});
  }
  console.log('✓ Clean done');
}
main().catch(e => { console.error(e); process.exit(1); });
