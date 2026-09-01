import { login, sqlQuery, sqlExecute, cacheSet, queueCreate, queuePublish, schedulerCreate, searchCreateIndex, searchAddDocs, blobUpload, health } from './nova.js';

const sleep = (ms) => new Promise(r => setTimeout(r, ms));

async function ensureHealth() {
  for (let i = 0; i < 15; i++) {
    try {
      const h = await health();
      if (h.status) return;
    } catch {}
    console.log(`Waiting for Nova at 8642... (${i + 1}/15)`);
    await sleep(1000);
  }
  throw new Error('Nova not reachable at http://127.0.0.1:8642 — run `make dev` first');
}

async function main() {
  console.log('→ Checking Nova health...');
  await ensureHealth();
  console.log('✓ Nova is up');
  await login();
  console.log('✓ Logged in as admin');

  // --- SQL ---
  console.log('\n→ SQL: creating tables...');
  // Nova SQL has no DROP TABLE IF EXISTS — use plain DROP (missing table errors, that's fine)
  for (const t of ['comments', 'tasks', 'projects', 'users']) {
    try { await sqlExecute(`DROP TABLE ${t}`); } catch {}
  }
  for (const q of [
    `CREATE TABLE users (id INTEGER PRIMARY KEY, username TEXT, email TEXT, role TEXT)`,
    `CREATE TABLE projects (id INTEGER PRIMARY KEY, name TEXT, color TEXT, description TEXT)`,
    `CREATE TABLE tasks (id INTEGER PRIMARY KEY, project_id INTEGER, title TEXT, description TEXT, status TEXT, priority INTEGER, assignee TEXT, due_at TEXT, created_at TEXT)`,
    `CREATE TABLE comments (id INTEGER PRIMARY KEY, task_id INTEGER, author TEXT, body TEXT, created_at TEXT)`,
  ]) {
    try { await sqlExecute(q); } catch (e) { console.log(`  sql warn: ${e.message.slice(0,120)}`); }
  }
  console.log('✓ Tables created');

  const users = [
    [1, 'alice', 'alice@nova.local', 'admin'],
    [2, 'bob', 'bob@nova.local', 'operator'],
    [3, 'carol', 'carol@nova.local', 'viewer'],
    [4, 'dave', 'dave@nova.local', 'operator'],
    [5, 'eve', 'eve@nova.local', 'viewer'],
  ];
  for (const [id, username, email, role] of users) {
    await sqlExecute(`INSERT INTO users (id, username, email, role) VALUES (${id}, '${username}', '${email}', '${role}')`);
  }
  console.log('✓ Users: 5');

  const projects = [
    [1, 'Website Redesign', '#4cc9f0', 'New marketing site'],
    [2, 'Mobile App', '#10b981', 'iOS & Android'],
    [3, 'Ops & Infra', '#f59e0b', 'CI/CD and monitoring'],
  ];
  for (const [id, name, color, desc] of projects) {
    await sqlExecute(`INSERT INTO projects (id, name, color, description) VALUES (${id}, '${name}', '${color}', '${desc}')`);
  }
  console.log('✓ Projects: 3');

  const tasks = [
    [1, 1, 'Design homepage', 'Figma mockups for new homepage', 'todo', 0, 'alice', '2026-09-10'],
    [2, 1, 'Implement hero', 'React + Tailwind hero section', 'in_progress', 1, 'bob', '2026-09-12'],
    [3, 1, 'Write copy', 'Finalize headline and CTA', 'todo', 1, 'carol', '2026-09-08'],
    [4, 1, 'SEO audit', 'Check meta, sitemap', 'done', 2, 'alice', '2026-08-20'],
    [5, 1, 'Analytics setup', 'GA4 + Plausible', 'todo', 1, 'dave', '2026-09-15'],
    [6, 1, 'Blog migration', 'Move 50 posts to new CMS', 'in_progress', 1, 'eve', '2026-09-20'],
    [7, 2, 'Auth flow', 'JWT + refresh, biometric', 'in_progress', 0, 'bob', '2026-09-05'],
    [8, 2, 'Offline sync', 'CRDT for offline tasks', 'todo', 0, 'alice', '2026-09-18'],
    [9, 2, 'Push notifications', 'FCM/APNs', 'todo', 2, 'dave', '2026-09-22'],
    [10, 2, 'App store screenshots', '6.5" and 12.9"', 'todo', 1, 'carol', '2026-09-11'],
    [11, 2, 'TestFlight beta', 'Invite 20 testers', 'done', 1, 'bob', '2026-08-28'],
    [12, 2, 'Performance', 'Reduce cold start <1s', 'in_progress', 0, 'alice', '2026-09-14'],
    [13, 3, 'CI pipeline', 'GitHub Actions + cache', 'done', 0, 'dave', '2026-08-25'],
    [14, 3, 'K8s deploy', 'Helm chart for novad', 'in_progress', 1, 'dave', '2026-09-09'],
    [15, 3, 'Monitoring', 'Prometheus + Grafana', 'todo', 1, 'eve', '2026-09-13'],
    [16, 3, 'Backups', 'Daily WAL + S3', 'todo', 2, 'alice', '2026-09-16'],
    [17, 1, 'A/B test pricing', 'Test $29 vs $49', 'todo', 2, 'carol', '2026-09-19'],
    [18, 2, 'Accessibility audit', 'WCAG 2.1 AA', 'todo', 1, 'carol', '2026-09-07'],
    [19, 3, 'Incident runbook', 'Write on-call guide', 'done', 1, 'eve', '2026-08-22'],
    [20, 1, 'Newsletter template', 'MJML responsive', 'in_progress', 2, 'bob', '2026-09-06'],
    [21, 2, 'Deep linking', 'Universal links', 'todo', 1, 'alice', '2026-09-17'],
    [22, 3, 'Cost report', 'August infra spend', 'todo', 2, 'dave', '2026-09-04'],
    [23, 1, 'User testing', '5 moderated sessions', 'todo', 0, 'carol', '2026-09-21'],
    [24, 2, 'Store review', 'Reply to 20 reviews', 'done', 2, 'eve', '2026-08-30'],
  ];
  for (const [id, pid, title, desc, status, prio, assignee, due] of tasks) {
    await sqlExecute(
      `INSERT INTO tasks (id, project_id, title, description, status, priority, assignee, due_at, created_at) VALUES (${id}, ${pid}, '${title.replace(/'/g, "''")}', '${desc.replace(/'/g, "''")}', '${status}', ${prio}, '${assignee}', '${due}', '2026-08-28')`
    );
  }
  console.log('✓ Tasks: 24');

  const comments = [
    [1, 1, 'alice', 'Need brand colors first'],
    [2, 2, 'bob', 'Hero is 80% done, needs review'],
    [3, 7, 'carol', 'JWT expiry 15m ok?'],
    [4, 12, 'alice', 'Profiling shows 1.2s, need <1s'],
    [5, 14, 'dave', 'Helm values done'],
    [6, 1, 'bob', 'Mockups linked in Figma'],
    [7, 7, 'eve', 'Refresh token rotation added'],
    [8, 8, 'alice', 'CRDT lib chosen: Automerge'],
    [9, 15, 'dave', 'Grafana dashboards imported'],
    [10, 2, 'carol', 'Waiting on copy'],
    [11, 20, 'bob', 'MJML looks great'],
    [12, 3, 'carol', 'Headline v2 is better'],
  ];
  for (const [id, tid, author, body] of comments) {
    await sqlExecute(`INSERT INTO comments (id, task_id, author, body, created_at) VALUES (${id}, ${tid}, '${author}', '${body.replace(/'/g, "''")}', '2026-08-28')`);
  }
  console.log('✓ Comments: 12');

  // --- Cache ---
  console.log('\n→ Cache: hot keys...');
  await cacheSet('board:stats', { projects: 3, tasks: 24, dueSoon: 5 }, 60_000);
  for (const t of tasks.slice(0, 5)) {
    await cacheSet(`hot:task:${t[0]}`, { id: t[0], title: t[2], priority: t[5] }, 30_000);
  }
  console.log('✓ Cache: board:stats + 5 hot tasks (TTL)');

  // --- Queue ---
  console.log('\n→ Queue: notifications...');
  await queueCreate('task-notifications', { durable: true });
  await queueCreate('task-reminders', { durable: true });
  await queuePublish('task-notifications', { type: 'task.created', task_id: 1, by: 'alice' });
  await queuePublish('task-notifications', { type: 'task.moved', task_id: 2, from: 'todo', to: 'in_progress', by: 'bob' });
  await queuePublish('task-notifications', { type: 'comment.added', task_id: 1, by: 'bob' });
  await queuePublish('task-reminders', { type: 'due', task_id: 3, due: '2026-09-08' }, 5000); // delayed 5s
  await queuePublish('task-reminders', { type: 'due', task_id: 8, due: '2026-09-18' }, 10000);
  console.log('✓ Queue: task-notifications (3), task-reminders (2 delayed)');

  // --- Scheduler ---
  console.log('\n→ Scheduler: cron + due jobs...');
  try {
    await schedulerCreate({ name: 'due-reminder', type: 'cron', schedule: '*/5 * * * *', handler: 'notify-due', payload: { channel: 'slack' }, max_retries: 3 });
    console.log('✓ Scheduler: due-reminder (cron */5)');
  } catch (e) { console.log(`  scheduler warn: ${e.message.slice(0,80)}`); }
  try {
    await schedulerCreate({ name: 'task-due-7', type: 'once', schedule: '0 9 * * *', handler: 'task-due', payload: { task_id: 7 }, max_retries: 2 });
    console.log('✓ Scheduler: task-due-7 (one-shot)');
  } catch (e) { console.log(`  scheduler warn: ${e.message.slice(0,80)}`); }

  // --- Search ---
  console.log('\n→ Search: tasks_idx...');
  await searchCreateIndex('tasks_idx', [
    { name: 'title', type: 'text', analyzer: 'standard', boost: 2 },
    { name: 'description', type: 'text' },
    { name: 'status', type: 'keyword' },
  ]);
  const docs = tasks.map(([id, pid, title, desc, status]) => ({ id: String(id), title, description: desc, status, project_id: pid }));
  await searchAddDocs('tasks_idx', docs);
  console.log('✓ Search: tasks_idx (24 docs)');

  // --- Blob ---
  console.log('\n→ Blob: attachments...');
  const samples = [
    { name: 'homepage-mock.png', data: Buffer.from('FAKEPNG' + 'x'.repeat(1024)), type: 'image/png' },
    { name: 'spec.pdf', data: Buffer.from('%PDF-FAKE' + 'y'.repeat(2048)), type: 'application/pdf' },
    { name: 'notes.txt', data: Buffer.from('Meeting notes: need to finalize homepage by Sep 10'), type: 'text/plain' },
  ];
  for (const s of samples) {
    const r = await blobUpload('taskboard', s.data, s.name, s.type);
    console.log(`  blob ${s.name} -> ${r.id.slice(0,8)} (${r.size_bytes}B)`);
  }

  console.log('\n✓ Seed complete! View at:');
  console.log('  SQL: curl -s http://127.0.0.1:8642/api/v1/sql/query -H "Content-Type: application/json" -d \'{"query":"SELECT * FROM tasks LIMIT 5"}\' | jq');
  console.log('  Search: curl -s http://127.0.0.1:8642/api/v1/search/indexes/tasks_idx/query -H "Content-Type: application/json" -d \'{"query":"homepage","limit":5}\' | jq');
  console.log('  Board UI: cd examples/taskboard && npm run dev (http://localhost:3000)');
  console.log('  Dashboard: http://127.0.0.1:5173 → Database/Search/Blob/Queue/Scheduler');
}

main().catch(e => { console.error('Seed failed:', e); process.exit(1); });
