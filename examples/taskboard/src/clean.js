import { login, sqlExecute } from './nova.js';

async function main() {
  await fetch('http://127.0.0.1:8642/health').then(r => r.json()).catch(() => { throw new Error('Nova not running'); });
  await login();
  console.log('Cleaning NovaBoard data...');
  for (const q of [
    `DROP TABLE IF EXISTS comments`,
    `DROP TABLE IF EXISTS tasks`,
    `DROP TABLE IF EXISTS projects`,
    `DROP TABLE IF EXISTS users`,
  ]) {
    try { await sqlExecute(q); console.log(`  ${q}`); } catch (e) { console.log(`  warn: ${e.message.slice(0,60)}`); }
  }
  // Purge queues, delete search index, blobs via direct fetch
  const token = await fetch('http://127.0.0.1:8642/api/v1/auth/login', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ username: 'admin', password: 'admin123' }) }).then(r => r.json()).then(j => j.access_token);
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
