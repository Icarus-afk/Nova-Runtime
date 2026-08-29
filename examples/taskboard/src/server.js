import express from 'express';
import cors from 'cors';
import path from 'path';
import { fileURLToPath } from 'url';
import { login, sqlQuery, sqlExecute, cacheSet } from './nova.js';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const app = express();
app.use(cors());
app.use(express.json({ limit: '2mb' }));
app.use(express.static(path.join(__dirname, '../public')));

let token = null;
async function ensureAuth() {
  if (!token) token = await login();
  return token;
}

const parseRows = (r) => {
  const names = r.column_names || r.columns || [];
  return (r.rows || []).map(row => Object.fromEntries(names.map((k, i) => [k, row[i]])));
};

// Board
app.get('/api/board', async (req, res) => {
  try {
    await ensureAuth();
    const [projectsRes, tasksRes, usersRes] = await Promise.all([
      sqlQuery('SELECT * FROM projects'),
      sqlQuery('SELECT * FROM tasks'),
      sqlQuery('SELECT * FROM users').catch(() => ({ rows: [], column_names: [] })),
    ]);
    const projects = parseRows(projectsRes);
    const tasks = parseRows(tasksRes);
    const users = parseRows(usersRes);
    try { await cacheSet('board:stats', { projects: projects.length, tasks: tasks.length, users: users.length }, 30000); } catch {}
    res.json({ projects, tasks, users, cached: false });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/projects', async (req, res) => {
  try { await ensureAuth(); const r = await sqlQuery('SELECT * FROM projects'); res.json({ data: parseRows(r) }); } catch (e) { res.status(500).json({ error: e.message }); }
});

app.post('/api/projects', async (req, res) => {
  try {
    await ensureAuth();
    const { name, color, description } = req.body;
    if (!name) return res.status(400).json({ error: 'name required' });
    const id = Date.now() % 100000;
    const c = color || '#4cc9f0';
    await sqlExecute(`INSERT INTO projects (id, name, color, description) VALUES (${id}, '${name.replace(/'/g, "''")}', '${c}', '${String(description||'').replace(/'/g,"''")}')`);
    res.json({ id, name });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.delete('/api/projects/:id', async (req, res) => {
  try { await ensureAuth(); await sqlExecute(`DELETE FROM projects WHERE id = ${req.params.id}`); res.json({ status: 'deleted' }); } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/users', async (req, res) => {
  try { await ensureAuth(); const r = await sqlQuery('SELECT * FROM users'); res.json({ data: parseRows(r) }); } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/tasks', async (req, res) => {
  try {
    await ensureAuth();
    let q = 'SELECT * FROM tasks';
    const { status, project_id, assignee, priority, search } = req.query;
    const conds = [];
    if (status) conds.push(`status = '${status}'`);
    if (project_id) conds.push(`project_id = ${project_id}`);
    if (assignee) conds.push(`assignee = '${assignee}'`);
    if (priority) conds.push(`priority = ${priority}`);
    if (conds.length) q += ' WHERE ' + conds.join(' AND ');
    if (search) {
      // Use Nova Search if available, fallback to LIKE
      try {
        await ensureAuth();
        const tok = await ensureAuth();
        const sr = await fetch('http://127.0.0.1:8642/api/v1/search/indexes/tasks_idx/query', {
          method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${tok}` },
          body: JSON.stringify({ query: String(search), limit: 50 }),
        }).then(r => r.json());
        const ids = (sr.hits || []).map((h) => h.id).join(',');
        if (ids) q += (conds.length ? ' AND ' : ' WHERE ') + `id IN (${ids})`;
      } catch {}
    }
    const r = await sqlQuery(q);
    res.json({ data: parseRows(r) });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/tasks/:id', async (req, res) => {
  try {
    await ensureAuth();
    const r = await sqlQuery(`SELECT * FROM tasks WHERE id = ${req.params.id}`);
    const data = parseRows(r);
    if (!data.length) return res.status(404).json({ error: 'not found' });
    const task = data[0];
    // Comments
    let comments = [];
    try {
      const cr = await sqlQuery(`SELECT * FROM comments WHERE task_id = ${req.params.id} ORDER BY id`);
      comments = parseRows(cr);
    } catch {}
    // Attachments via blob? List blobs where key contains task id? For demo, return empty
    res.json({ task, comments });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.post('/api/tasks', async (req, res) => {
  try {
    await ensureAuth();
    const { title, description, project_id, priority, assignee, due_at, status } = req.body;
    if (!title) return res.status(400).json({ error: 'title required' });
    const id = Date.now() % 100000;
    await sqlExecute(`INSERT INTO tasks (id, project_id, title, description, status, priority, assignee, due_at, created_at) VALUES (${id}, ${project_id || 1}, '${title.replace(/'/g, "''")}', '${String(description||'').replace(/'/g,"''")}', '${status||'todo'}', ${priority ?? 1}, '${assignee||'alice'}', '${due_at||'2026-09-30'}', '2026-08-28')`);
    try { await fetch('http://127.0.0.1:8642/api/v1/cache/board:stats', { method: 'DELETE', headers: { Authorization: `Bearer ${token}` } }); } catch {}
    try { await fetch('http://127.0.0.1:8642/api/v1/queues/task-notifications/messages', { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }, body: JSON.stringify({ messages: [{ body: { type: 'task.created', task_id: id, title } }] }) }); } catch {}
    try { await fetch('http://127.0.0.1:8642/api/v1/search/indexes/tasks_idx/documents', { method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` }, body: JSON.stringify({ documents: [{ id: String(id), title, description }] }) }); } catch {}
    res.json({ id, status: 'created' });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.put('/api/tasks/:id', async (req, res) => {
  try {
    await ensureAuth();
    const { title, description, status, priority, assignee, due_at, project_id } = req.body;
    const sets = [];
    if (title !== undefined) sets.push(`title = '${String(title).replace(/'/g,"''")}'`);
    if (description !== undefined) sets.push(`description = '${String(description).replace(/'/g,"''")}'`);
    if (status !== undefined) sets.push(`status = '${status}'`);
    if (priority !== undefined) sets.push(`priority = ${priority}`);
    if (assignee !== undefined) sets.push(`assignee = '${assignee}'`);
    if (due_at !== undefined) sets.push(`due_at = '${due_at}'`);
    if (project_id !== undefined) sets.push(`project_id = ${project_id}`);
    if (!sets.length) return res.status(400).json({ error: 'no fields' });
    await sqlExecute(`UPDATE tasks SET ${sets.join(', ')} WHERE id = ${req.params.id}`);
    res.json({ status: 'updated' });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.put('/api/tasks/:id/move', async (req, res) => {
  try {
    await ensureAuth();
    const { status } = req.body;
    if (!['todo','in_progress','done'].includes(status)) return res.status(400).json({ error: 'invalid status' });
    await sqlExecute(`UPDATE tasks SET status = '${status}' WHERE id = ${req.params.id}`);
    res.json({ status });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.delete('/api/tasks/:id', async (req, res) => {
  try { await ensureAuth(); await sqlExecute(`DELETE FROM tasks WHERE id = ${req.params.id}`); await sqlExecute(`DELETE FROM comments WHERE task_id = ${req.params.id}`); res.json({ status: 'deleted' }); } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/tasks/:id/comments', async (req, res) => {
  try { await ensureAuth(); const r = await sqlQuery(`SELECT * FROM comments WHERE task_id = ${req.params.id} ORDER BY id`); res.json({ data: parseRows(r) }); } catch (e) { res.status(500).json({ error: e.message }); }
});

app.post('/api/tasks/:id/comments', async (req, res) => {
  try {
    await ensureAuth();
    const { author, body } = req.body;
    if (!body) return res.status(400).json({ error: 'body required' });
    const id = Date.now() % 100000;
    await sqlExecute(`INSERT INTO comments (id, task_id, author, body, created_at) VALUES (${id}, ${req.params.id}, '${(author||'alice').replace(/'/g,"''")}', '${String(body).replace(/'/g,"''")}', '2026-08-28')`);
    res.json({ id });
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/search', async (req, res) => {
  try {
    await ensureAuth();
    const q = String(req.query.q || '');
    if (!q) return res.json({ hits: [], total: 0 });
    const tok = await ensureAuth();
    const r = await fetch('http://127.0.0.1:8642/api/v1/search/indexes/tasks_idx/query', {
      method: 'POST', headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${tok}` },
      body: JSON.stringify({ query: q, limit: 20 }),
    }).then(r => r.json());
    res.json(r);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

app.get('/api/queue', async (req, res) => {
  try {
    await ensureAuth();
    const tok = await ensureAuth();
    const qs = await fetch('http://127.0.0.1:8642/api/v1/queues', { headers: { Authorization: `Bearer ${tok}` } }).then(r => r.json()).catch(() => ({ data: [] }));
    res.json(qs);
  } catch (e) { res.status(500).json({ error: e.message }); }
});

const PORT = process.env.PORT || 3000;
app.listen(PORT, () => {
  console.log(`NovaBoard demo at http://localhost:${PORT}`);
  console.log(`Board API at http://localhost:${PORT}/api/board (proxies to Nova 8642)`);
});
