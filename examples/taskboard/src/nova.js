// Tiny Nova client — no SDK, just fetch to keep demo standalone
const BASE = process.env.NOVA_URL || 'http://127.0.0.1:8642';
const API = `${BASE}/api/v1`;
let token = null;

export async function login() {
  const res = await fetch(`${API}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username: 'admin', password: 'admin123' }),
  });
  if (!res.ok) throw new Error(`login ${res.status}`);
  const j = await res.json();
  token = j.access_token;
  return token;
}

function headers() {
  const h = { 'Content-Type': 'application/json' };
  if (token) h['Authorization'] = `Bearer ${token}`;
  return h;
}

export async function sqlQuery(query, params) {
  // params: optional $1 interpolation handled server-side
  const res = await fetch(`${API}/sql/query`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ query, params }),
  });
  if (!res.ok) throw new Error(`sql query ${res.status}: ${await res.text()}`);
  return res.json();
}

export async function sqlExecute(query, params) {
  const res = await fetch(`${API}/sql/execute`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ query, params }),
  });
  if (!res.ok) throw new Error(`sql exec ${res.status}: ${await res.text()}`);
  return res.json();
}

export async function cacheSet(key, value, ttlMs) {
  const res = await fetch(`${API}/cache/${encodeURIComponent(key)}`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ value, ttl_ms: ttlMs }),
  });
  if (!res.ok) throw new Error(`cache set ${res.status}`);
  return res.json();
}

export async function queueCreate(name, opts = {}) {
  const res = await fetch(`${API}/queues`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ name, ...opts }),
  });
  if (!res.ok && res.status !== 400) throw new Error(`queue create ${res.status}`);
  return res.json().catch(() => ({}));
}

export async function queuePublish(queue, body, delayMs) {
  const res = await fetch(`${API}/queues/${encodeURIComponent(queue)}/messages`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ messages: [{ body, delay_ms: delayMs }] }),
  });
  if (!res.ok) throw new Error(`queue publish ${res.status}`);
  return res.json();
}

export async function schedulerCreate(job) {
  const res = await fetch(`${API}/scheduler/jobs`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify(job),
  });
  if (!res.ok) throw new Error(`scheduler ${res.status}: ${await res.text()}`);
  return res.json();
}

export async function searchCreateIndex(name, fields) {
  const res = await fetch(`${API}/search/indexes`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ name, fields }),
  });
  if (!res.ok && res.status !== 400) throw new Error(`search create ${res.status}`);
  return res.json().catch(() => ({}));
}

export async function searchAddDocs(index, documents) {
  const res = await fetch(`${API}/search/indexes/${encodeURIComponent(index)}/documents`, {
    method: 'POST',
    headers: headers(),
    body: JSON.stringify({ documents }),
  });
  if (!res.ok) throw new Error(`search add ${res.status}`);
  return res.json();
}

export async function blobUpload(namespace, fileBuffer, filename, contentType) {
  const form = new FormData();
  const blob = new Blob([fileBuffer], { type: contentType });
  form.append('file', blob, filename);
  const h = {};
  if (token) h['Authorization'] = `Bearer ${token}`;
  const res = await fetch(`${API}/blobs?namespace=${encodeURIComponent(namespace)}`, {
    method: 'POST',
    headers: h,
    body: form,
  });
  if (!res.ok) throw new Error(`blob ${res.status}: ${await res.text()}`);
  return res.json();
}

export async function health() {
  const r = await fetch(`${BASE}/health`);
  return r.json();
}
