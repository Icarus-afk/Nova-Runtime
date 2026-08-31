// Bloom Market — tiny Nova client (fresh project template)
// Copy this file to any new Node.js project to talk to Nova Runtime.
// No SDK needed — just fetch. For SDK, see sdk/src/index.ts
const BASE = process.env.NOVA_URL || 'http://127.0.0.1:8642';
const API = `${BASE}/api/v1`;
let token = null;

export async function login(username = 'admin', password = null) {
  // Fresh project: try passed password, then env, then known demo passwords
  const candidates = [
    password,
    process.env.NOVA_PASSWORD,
    process.env.NOVA_ADMIN_PASSWORD,
    'Ehasan,123',
    'admin123',
  ].filter(Boolean);
  // dedupe, keep order
  const tried = [...new Set(candidates)];
  let lastErr = null;
  for (const pwd of tried) {
    const res = await fetch(`${API}/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ username, password: pwd }),
    });
    if (res.ok) {
      const j = await res.json();
      token = j.access_token;
      if (tried[0] !== pwd) console.log(`  (logged in with password variant: ${pwd === 'admin123' ? 'admin123' : 'NOVA_ADMIN_PASSWORD'})`);
      return token;
    }
    lastErr = `login ${res.status}: ${await res.text()}`;
    if (res.status !== 401) break; // don't retry on 429 etc
  }
  throw new Error(lastErr || `login failed for ${username}`);
}

function headers() {
  const h = { 'Content-Type': 'application/json' };
  if (token) h['Authorization'] = `Bearer ${token}`;
  return h;
}

export async function health() {
  const r = await fetch(`${BASE}/health`);
  if (!r.ok) throw new Error(`health ${r.status}`);
  return r.json();
}

// SQL
export async function sqlQuery(query, params) {
  const res = await fetch(`${API}/sql/query`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ query, params }),
  });
  if (!res.ok) throw new Error(`sql query ${res.status}: ${await res.text()}`);
  return res.json();
}
export async function sqlExecute(query, params) {
  const res = await fetch(`${API}/sql/execute`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ query, params }),
  });
  if (!res.ok) throw new Error(`sql exec ${res.status}: ${await res.text()}`);
  return res.json();
}

// Cache
export async function cacheSet(key, value, ttlMs) {
  const res = await fetch(`${API}/cache/${encodeURIComponent(key)}`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ value, ttl_ms: ttlMs }),
  });
  if (!res.ok) throw new Error(`cache set ${res.status}: ${await res.text()}`);
  return res.json();
}
export async function cacheGet(key) {
  const res = await fetch(`${API}/cache/${encodeURIComponent(key)}`, { headers: headers() });
  if (res.status === 404) return null;
  if (!res.ok) throw new Error(`cache get ${res.status}`);
  return res.json();
}

// Queue
export async function queueCreate(name, opts = {}) {
  const res = await fetch(`${API}/queues`, { method: 'POST', headers: headers(), body: JSON.stringify({ name, ...opts }) });
  if (!res.ok && res.status !== 400) throw new Error(`queue create ${res.status}: ${await res.text()}`);
  return res.json().catch(() => ({}));
}
export async function queuePublish(queue, body, delayMs) {
  const res = await fetch(`${API}/queues/${encodeURIComponent(queue)}/messages`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ messages: [{ body, delay_ms: delayMs }] }),
  });
  if (!res.ok) throw new Error(`queue publish ${res.status}: ${await res.text()}`);
  return res.json();
}
export async function queuePoll(queue, count = 5, visibilityMs = 30000) {
  const res = await fetch(`${API}/queues/${encodeURIComponent(queue)}/messages/poll`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ count, visibility_timeout_ms: visibilityMs }),
  });
  if (!res.ok) throw new Error(`queue poll ${res.status}: ${await res.text()}`);
  return res.json();
}

// Scheduler
export async function schedulerCreate(job) {
  const res = await fetch(`${API}/scheduler/jobs`, { method: 'POST', headers: headers(), body: JSON.stringify(job) });
  if (!res.ok) throw new Error(`scheduler ${res.status}: ${await res.text()}`);
  return res.json();
}
export async function schedulerList() {
  const res = await fetch(`${API}/scheduler/jobs`, { headers: headers() });
  if (!res.ok) throw new Error(`scheduler list ${res.status}`);
  return res.json();
}

// Search
export async function searchCreateIndex(name, fields) {
  const res = await fetch(`${API}/search/indexes`, { method: 'POST', headers: headers(), body: JSON.stringify({ name, fields }) });
  if (!res.ok && res.status !== 400) throw new Error(`search create ${res.status}: ${await res.text()}`);
  return res.json().catch(() => ({}));
}
export async function searchAddDocs(index, documents) {
  const res = await fetch(`${API}/search/indexes/${encodeURIComponent(index)}/documents`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ documents }),
  });
  if (!res.ok) throw new Error(`search add ${res.status}: ${await res.text()}`);
  return res.json();
}
export async function searchQuery(index, query, opts = {}) {
  const res = await fetch(`${API}/search/indexes/${encodeURIComponent(index)}/query`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ query, limit: opts.limit, offset: opts.offset }),
  });
  if (!res.ok) throw new Error(`search query ${res.status}: ${await res.text()}`);
  return res.json();
}

// Blob (multipart)
export async function blobUpload(namespace, fileBuffer, filename, contentType) {
  const form = new FormData();
  const blob = new Blob([fileBuffer], { type: contentType });
  form.append('file', blob, filename);
  const h = {};
  if (token) h['Authorization'] = `Bearer ${token}`;
  const res = await fetch(`${API}/blobs?namespace=${encodeURIComponent(namespace)}`, { method: 'POST', headers: h, body: form });
  if (!res.ok) throw new Error(`blob ${res.status}: ${await res.text()}`);
  return res.json();
}
export async function blobList(namespace, limit = 20) {
  const res = await fetch(`${API}/blobs?namespace=${encodeURIComponent(namespace)}&limit=${limit}`, { headers: headers() });
  if (!res.ok) throw new Error(`blob list ${res.status}`);
  return res.json();
}

// Auth helpers
export async function createUser(username, password, role = 'viewer') {
  const res = await fetch(`${API}/auth/users`, {
    method: 'POST', headers: headers(), body: JSON.stringify({ username, password, role }),
  });
  if (!res.ok && res.status !== 400) throw new Error(`create user ${res.status}: ${await res.text()}`);
  return res.json().catch(() => ({}));
}
