const DEFAULT_BASE_URL = '/api/v1/dashboard';

let baseUrl = DEFAULT_BASE_URL;
let authToken: string | null = null;

export function setBaseUrl(url: string) {
  baseUrl = url;
}

export function setToken(token: string | null) {
  authToken = token;
}

export function getToken(): string | null {
  return authToken;
}

class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
  }
}

async function request<T>(
  method: string,
  path: string,
  body?: unknown,
  params?: Record<string, string | number | boolean | undefined>
): Promise<T> {
  let url = `${baseUrl}${path}`;
  if (params) {
    const searchParams = new URLSearchParams();
    for (const [key, value] of Object.entries(params)) {
      if (value !== undefined) {
        searchParams.set(key, String(value));
      }
    }
    const qs = searchParams.toString();
    if (qs) url += `?${qs}`;
  }

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };
  if (authToken) {
    headers['Authorization'] = `Bearer ${authToken}`;
  }

  const res = await fetch(url, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });

  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    try {
      const err = await res.json();
      if (err.error) message = err.error;
      if (err.message) message = err.message;
    } catch {}
    throw new ApiError(message, res.status);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

export const api = {
  login: (username: string, password: string) =>
    request<{ session_id: string; token: string; expires_at: number; user: { id: string; username: string; email: string; role: string } }>('POST', '/auth/login', { username, password }),

  getSystemHealth: () => request<import('../types').SystemHealth>('GET', '/system/health'),

  getCollections: () => request<import('../types').CollectionInfo[]>('GET', '/database/collections'),

  getDocuments: (collection: string, page = 1, perPage = 20) =>
    request<{ data: import('../types').Document[]; pagination: import('../types').PaginationInfo }>('GET', `/database/collections/${collection}/docs`, undefined, { page, per_page: perPage }),

  queryDatabase: (query: { collection: string; filter?: Record<string, unknown>; limit?: number }) =>
    request<import('../types').QueryResult>('POST', '/database/query', query),

  getCacheStats: () => request<import('../types').CacheStats>('GET', '/cache/stats'),

  getCacheKeys: (search?: string, page = 1) =>
    request<{ data: import('../types').CacheEntry[]; pagination: import('../types').PaginationInfo }>('GET', '/cache/keys', undefined, { search, page }),

  deleteCacheKey: (key: string) => request<void>('DELETE', `/cache/keys/${encodeURIComponent(key)}`),

  clearCache: () => request<void>('POST', '/cache/clear'),

  getQueues: () => request<import('../types').QueueInfo[]>('GET', '/queue'),

  getQueueMessages: (name: string, page = 1, state?: string) =>
    request<{ data: import('../types').QueueMessage[]; pagination: import('../types').PaginationInfo }>('GET', `/queue/${name}/messages`, undefined, { page, state }),

  publishMessage: (queue: string, body: string, priority?: number, delaySeconds?: number) =>
    request<import('../types').QueueMessage>('POST', `/queue/${queue}/messages`, { body, priority, delay_seconds: delaySeconds }),

  purgeQueue: (name: string) => request<{ purged_count: number }>('POST', `/queue/${name}/purge`),

  deleteQueue: (name: string) => request<void>('DELETE', `/queue/${name}`),

  getJobs: () => request<import('../types').JobInfo[]>('GET', '/scheduler/jobs'),

  getJobExecutions: (jobId: string, page = 1) =>
    request<{ data: import('../types').JobExecution[]; pagination: import('../types').PaginationInfo }>('GET', `/scheduler/jobs/${jobId}/executions`, undefined, { page }),

  triggerJob: (jobId: string) => request<import('../types').JobExecution>('POST', `/scheduler/jobs/${jobId}/trigger`),

  pauseJob: (jobId: string) => request<import('../types').JobInfo>('POST', `/scheduler/jobs/${jobId}/pause`),

  resumeJob: (jobId: string) => request<import('../types').JobInfo>('POST', `/scheduler/jobs/${jobId}/resume`),

  deleteJob: (jobId: string) => request<void>('DELETE', `/scheduler/jobs/${jobId}`),

  createJob: (job: { name: string; type: string; schedule?: string; handler: string; payload?: Record<string, unknown>; max_retries?: number }) =>
    request<import('../types').JobInfo>('POST', '/scheduler/jobs', job),

  getIndexes: () => request<import('../types').IndexInfo[]>('GET', '/search/indexes'),

  searchQuery: (index: string, query: string, page = 1) =>
    request<import('../types').SearchResult>('POST', `/search/indexes/${index}/query`, { index, query, limit: 10, skip: (page - 1) * 10 }),

  deleteIndex: (name: string) => request<void>('DELETE', `/search/indexes/${name}`),

  getBuckets: () => request<import('../types').BucketInfo[]>('GET', '/blob/buckets'),

  getBucketObjects: (bucket: string, page = 1) =>
    request<{ data: import('../types').BlobObject[]; pagination: import('../types').PaginationInfo }>('GET', `/blob/buckets/${bucket}/objects`, undefined, { page }),

  getUsers: () => request<import('../types').DashboardUser[]>('GET', '/users'),

  deleteUser: (id: string) => request<void>('DELETE', `/users/${id}`),

  getApiKeys: () => request<import('../types').ApiKey[]>('GET', '/api-keys'),

  createApiKey: (name: string, role: string) =>
    request<import('../types').ApiKey & { full_key: string }>('POST', '/api-keys', { name, role }),

  deleteApiKey: (id: string) => request<void>('DELETE', `/api-keys/${id}`),

  getConfig: () => request<import('../types').ConfigEntry[]>('GET', '/config'),

  getLogs: (params: { levels?: string; subsystems?: string; search?: string; limit?: number; offset?: number; order?: string }) =>
    request<{ entries: import('../types').LogEntry[]; total_count: number; has_more: boolean }>('GET', '/logs', undefined, params),

  getWsUrl: () => {
    const wsBase = baseUrl.replace(/^http/, 'ws');
    return `${wsBase}/logs/stream`;
  },
};
