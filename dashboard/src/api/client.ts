const DEFAULT_BASE_URL = '/api/v1';

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

function mapScheduleType(t: unknown): import('../types').JobType {
  switch (String(t ?? 'OneTime').toLowerCase()) {
    case 'cron':
      return 'cron';
    case 'interval':
      return 'interval';
    case 'onetime':
      return 'once';
    default:
      return 'once';
  }
}

function mapUserRole(permissions: unknown): import('../types').UserRole {
  if (Array.isArray(permissions)) {
    const role = permissions.find((p) => p === 'admin' || p === 'operator' || p === 'viewer');
    if (role) return role as import('../types').UserRole;
  }
  return 'operator';
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

  if (res.status === 401) {
    // Token invalid/expired — clear and redirect to login
    authToken = null;
    localStorage.removeItem('nova_token');
    if (typeof window !== 'undefined' && window.location.pathname !== '/login') {
      window.location.href = '/login';
    }
  }
  if (!res.ok) {
    let message = `HTTP ${res.status}`;
    try {
      const err = await res.json();
      if (err.error) message = err.error;
      if (err.detail) message = err.detail;
      if (err.message) message = err.message;
      if (err.title) message = err.title;
    } catch {}
    throw new ApiError(message, res.status);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

export const api = {
  login: async (username: string, password: string): Promise<{ session_id: string; token: string; expires_at: number; user: { id: string; username: string; email: string; role: string } }> => {
    const result = await request<{ access_token: string; token_type: string; expires_in: number }>('POST', '/auth/login', { username, password });
    const token = result.access_token;
    setToken(token);
    localStorage.setItem('nova_token', token);
    return {
        session_id: token,
        token,
        expires_at: Date.now() + (result.expires_in * 1000),
        user: { id: '', username, email: `${username}@nova.local`, role: 'admin' },
    };
  },

  getSystemHealth: async () => {
    try {
      const res = await fetch('/health', { signal: AbortSignal.timeout(3000) });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json() as Record<string, unknown>;

      const mem = (data.memory as Record<string, unknown>) || {};
      const disk = (data.disk as Record<string, unknown>) || {};
      const cpu = (data.cpu as Record<string, unknown>) || {};
      const network = (data.network as Record<string, unknown>) || {};
      const rawSubsystems = (data.subsystems as Record<string, unknown>) || {};

      const subsystems: import('../types').SubsystemStatus[] = Object.entries(rawSubsystems).map(([name, info]) => {
        const s = info as Record<string, unknown>;
        return {
          name,
          status: (s.status as import('../types').HealthStatus) || 'degraded',
          uptime_seconds: 0,
          metrics: {},
          last_error: null,
          last_error_time: null,
        };
      });

      return {
        status: (data.status as import('../types').HealthStatus) || 'degraded',
        uptime_seconds: (data.uptime_secs as number) || 0,
        version: (data.version as string) || '',
        cpu: {
          usage_percent: (cpu.usage_percent as number) ?? 0,
          load_avg_1m: (cpu.load_avg_1m as number) ?? 0,
          load_avg_5m: (cpu.load_avg_5m as number) ?? 0,
          load_avg_15m: (cpu.load_avg_15m as number) ?? 0,
          cores: (cpu.cores as number) ?? 0,
          temperature_celsius: (cpu.temperature_celsius as number | null) ?? null,
        },
        memory: {
          total_bytes: (mem.total_bytes as number) || 0,
          used_bytes: (mem.used_bytes as number) || 0,
          resident_bytes: (mem.resident_bytes as number) ?? (mem.used_bytes as number) ?? 0,
          allocated_bytes: (mem.allocated_bytes as number) ?? (mem.used_bytes as number) ?? 0,
          cache_bytes: (mem.cache_bytes as number) ?? 0,
          swap_used_bytes: (mem.swap_used_bytes as number) ?? 0,
          swap_total_bytes: (mem.swap_total_bytes as number) ?? 0,
        },
        disk: {
          data_path: (disk.data_path as string) || '',
          total_bytes: (disk.total_bytes as number) || 0,
          used_bytes: (disk.used_bytes as number) || 0,
          free_bytes: (disk.free_bytes as number) || 0,
          fs_type: (disk.fs_type as string) || '',
          read_ops_per_sec: (disk.read_ops_per_sec as number) ?? 0,
          write_ops_per_sec: (disk.write_ops_per_sec as number) ?? 0,
          read_bytes_per_sec: (disk.read_bytes_per_sec as number) ?? 0,
          write_bytes_per_sec: (disk.write_bytes_per_sec as number) ?? 0,
          io_wait_percent: (disk.io_wait_percent as number) ?? 0,
        },
        network: {
          rx_bytes_per_sec: (network.rx_bytes_per_sec as number) ?? 0,
          tx_bytes_per_sec: (network.tx_bytes_per_sec as number) ?? 0,
          rx_packets_per_sec: (network.rx_packets_per_sec as number) ?? 0,
          tx_packets_per_sec: (network.tx_packets_per_sec as number) ?? 0,
          connections_active: (network.connections_active as number) ?? (network.request_rate as number) ?? 0,
          connection_errors: (network.connection_errors as number) ?? 0,
          tcp_retransmit_percent: (network.tcp_retransmit_percent as number) ?? 0,
        },
        subsystems,
        last_checked: Date.now(),
      } as import('../types').SystemHealth;
    } catch {
      console.warn('getSystemHealth: backend unavailable');
      return null as unknown as import('../types').SystemHealth;
    }
  },

  getCollections: () =>
    request<{ data: { name: string; document_count: number }[] }>('GET', '/sql/tables')
      .then(r => (r.data || []).map((t) => ({ name: t.name, document_count: t.document_count, total_size_bytes: 0, average_document_size_bytes: 0, index_count: 0, created_at: 0, last_updated_at: 0 } as unknown as import('../types').CollectionInfo)))
      .catch(() => {
        console.warn('getCollections: backend unavailable');
        return [];
      }),

  getDocuments: (collection: string, page = 1, perPage = 20) => {
    const offset = (page - 1) * perPage;
    const dataPromise = request<{ column_names?: string[]; columns: string[]; rows: unknown[][]; row_count: number }>('POST', '/sql/query', { query: `SELECT * FROM ${collection} LIMIT ${perPage} OFFSET ${offset}` });
    const countPromise = request<{ rows: unknown[][]; row_count: number }>('POST', '/sql/query', { query: `SELECT COUNT(*) FROM ${collection}` });
    return Promise.all([dataPromise, countPromise])
      .then(([dataR, countR]) => {
        const names: string[] = dataR.column_names || dataR.columns;
        const idIdx = names.indexOf('id');
        const docs = dataR.rows.map((row: unknown[], i: number) => {
          const id = idIdx >= 0 && row[idIdx] !== null && row[idIdx] !== undefined
            ? String(row[idIdx])
            : String(offset + i + 1);
          return {
            id,
            collection,
            data: Object.fromEntries(names.map((c: string, j: number) => [c, row[j]])),
            created_at: 0,
            updated_at: 0,
            version: 1,
            size_bytes: 0,
          } as import('../types').Document;
        });
        const total = countR.row_count > 0 ? Number(countR.rows[0][0]) : 0;
        return {
          data: docs,
          pagination: { page, per_page: perPage, total, total_pages: Math.ceil(total / perPage) || 1 },
        };
      })
      .catch(() => {
        console.warn(`getDocuments(${collection}): backend unavailable`);
        return { data: [] as import('../types').Document[], pagination: { page, per_page: perPage, total: 0, total_pages: 0 } };
      });
  },

  queryDatabase: (query: { collection: string; filter?: Record<string, unknown>; limit?: number; params?: unknown[] }) =>
    request<{ column_names?: string[]; columns: string[]; rows: unknown[][]; row_count: number; execution_time_ms: number }>('POST', '/sql/query', { query: query.collection, params: query.params, limit: query.limit, ...query.filter })
      .then(r => {
        const names = r.column_names || r.columns;
        return { documents: r.rows.map((row, i) => ({ id: `${i}`, collection: '', data: Object.fromEntries(names.map((c, j) => [c, row[j]])), created_at: 0, updated_at: 0, version: 1, size_bytes: 0 })), total_count: r.row_count, execution_time_ms: r.execution_time_ms, warning: null } as unknown as import('../types').QueryResult;
      })
      .catch(() => {
        console.warn('queryDatabase: backend unavailable');
        return { documents: [], total_count: null, execution_time_ms: 0, warning: null };
      }),

  getCacheStats: () =>
    request<{ keys: number; hits: number; misses: number; evictions: number; hit_rate: number; memory_bytes: number }>('GET', '/cache/stats')
      .then(r => ({
        hit_count: r.hits,
        miss_count: r.misses,
        hit_ratio: r.hit_rate,
        total_entries: r.keys,
        current_size_bytes: r.memory_bytes,
        max_size_bytes: 0,
        eviction_count: r.evictions,
        ttl_expired_count: 0,
        oldest_entry_age_seconds: 0,
        newest_entry_age_seconds: 0,
      }))
      .catch(() => {
        console.warn('getCacheStats: backend unavailable');
        return {
          hit_count: 0, miss_count: 0, hit_ratio: 0, total_entries: 0,
          current_size_bytes: 0, max_size_bytes: 0, eviction_count: 0,
          ttl_expired_count: 0, oldest_entry_age_seconds: 0, newest_entry_age_seconds: 0,
        };
      }),

  getCacheKeys: (_search?: string, _page = 1) =>
    request<{ data: string[]; total: number; pattern: string | null }>('GET', '/cache/keys', undefined, { pattern: _search || undefined })
      .then(r => ({
        data: (r.data || []).map(k => ({ key: k, value_size_bytes: 0, created_at: 0, expires_at: null, last_access_at: 0, access_count: 0, ttl_seconds: null } as unknown as import('../types').CacheEntry)),
        pagination: { page: _page, per_page: 100, total: r.total ?? 0, total_pages: 1 },
      }))
      .catch(() => {
        console.warn('getCacheKeys: backend unavailable');
        return { data: [], pagination: { page: _page, per_page: 20, total: 0, total_pages: 0 } };
      }),

  deleteCacheKey: (key: string) =>
    request<void>('DELETE', `/cache/${encodeURIComponent(key)}`),

  clearCache: async () => {
    // Backend has no /cache/clear route — enumerate keys and delete each.
    const r = await request<{ data: string[] }>('GET', '/cache/keys', undefined, { pattern: '*' });
    const keys = r.data || [];
    await Promise.all(keys.map((k) => request<void>('DELETE', `/cache/${encodeURIComponent(k)}`).catch(() => {})));
  },

  getQueues: () =>
    request<{ data: any[]; pagination: any }>('GET', '/queues')
      .then(r => (r.data || []).map(q => ({
        name: q.name ?? '',
        message_count: (q.available ?? 0) + (q.in_flight ?? 0) + (q.delayed ?? 0),
        ready_count: q.available ?? 0,
        reserved_count: q.in_flight ?? 0,
        delayed_count: q.delayed ?? 0,
        buried_count: 0,
        dead_letter_count: 0,
        enqueue_rate_per_sec: 0,
        dequeue_rate_per_sec: 0,
        average_message_size_bytes: 0,
        oldest_message_age_seconds: 0,
        created_at: 0,
        max_length: 0,
        dead_letter_queue: null,
        visibility_timeout_seconds: 0,
        retention_seconds: 0,
      } as import('../types').QueueInfo)))
      .catch(() => {
        console.warn('getQueues: backend unavailable');
        return [];
      }),

  getQueueMessages: (name: string, _page = 1, _state?: string) =>
    request<{ messages: any[]; message_count: number }>('POST', `/queues/${name}/messages/poll`, { count: 20 })
      .then(r => ({
        data: (r.messages || []).map(m => ({
          id: m.id ?? '',
          body: typeof m.body === 'string' ? m.body : JSON.stringify(m.body),
          state: 'ready' as const,
          priority: 0,
          enqueued_at: Date.now(),
          reserved_at: null,
          delayed_until: null,
          attempts: m.delivery_attempt ?? 0,
          error_count: 0,
          last_error: null,
          ttr_seconds: 0,
        } as import('../types').QueueMessage)),
        pagination: { page: _page, per_page: 20, total: r.message_count ?? 0, total_pages: 1 },
      }))
      .catch(() => {
        console.warn('getQueueMessages: backend unavailable');
        return { data: [], pagination: { page: _page, per_page: 20, total: 0, total_pages: 0 } };
      }),

  publishMessage: (queue: string, body: string, priority?: number, delaySeconds?: number) =>
    request<{ published_count: number; message_ids: string[] }>('POST', `/queues/${queue}/messages`, { messages: [{ body: (() => { try { return JSON.parse(body); } catch { return body; } })(), delay_ms: delaySeconds ? delaySeconds * 1000 : undefined }] })
      .then(r => ({
        id: r.message_ids?.[0] ?? '',
        body,
        state: 'ready' as const,
        priority: priority ?? 0,
        enqueued_at: Date.now(),
        reserved_at: null,
        delayed_until: null,
        attempts: 0,
        error_count: 0,
        last_error: null,
        ttr_seconds: 0,
      })),

  purgeQueue: (name: string) =>
    request<{ status: string }>('POST', `/queues/${name}/purge`)
      .then(() => ({ purged_count: -1 })),

  deleteQueue: (name: string) =>
    request<void>('DELETE', `/queues/${name}`),

  getJobs: () =>
    request<{ data: any[]; pagination: any }>('GET', '/scheduler/jobs')
      .then(r => (r.data || []).map(j => ({
        id: j.id ?? '',
        name: j.name ?? '',
        type: mapScheduleType(j.schedule_type),
        schedule: null,
        handler: '',
        payload: {},
        status: (j.state === 'Paused' || j.state === 'Cancelled' || j.state === 'Failed') ? 'paused' as const : 'active' as const,
        max_retries: 0,
        retry_delay_seconds: 0,
        timeout_seconds: 0,
        created_at: 0,
        updated_at: 0,
        last_run_at: j.last_run_at ?? null,
        next_run_at: j.next_run_at ?? null,
        tags: [],
        concurrency_policy: 'allow' as const,
      } as import('../types').JobInfo)))
      .catch(() => {
        console.warn('getJobs: backend unavailable');
        return [];
      }),

  getSchedulerStats: () =>
    request<{ jobs_pending: number; jobs_running: number; jobs_completed: number; jobs_failed: number; jobs_cancelled: number; total_scheduled: number; total_executed: number; total_failures: number }>('GET', '/scheduler/stats'),

  triggerJob: (jobId: string) =>
    request<{ status: string }>('POST', `/scheduler/jobs/${jobId}/trigger`)
      .then(() => ({
        id: '', job_id: jobId, status: 'running' as const,
        started_at: Date.now(), finished_at: null, duration_ms: null,
        result: null, error: null, retry_attempt: 0, trigger: 'manual' as const,
      })),

  pauseJob: (jobId: string) =>
    request<{ status: string }>('POST', `/scheduler/jobs/${jobId}/pause`)
      .then(() => ({
        id: jobId, name: '', type: 'once' as const, schedule: null, handler: '',
        payload: {}, status: 'paused' as const, max_retries: 0, retry_delay_seconds: 0,
        timeout_seconds: 0, created_at: 0, updated_at: 0, last_run_at: null,
        next_run_at: null, tags: [], concurrency_policy: 'allow' as const,
      })),

  resumeJob: (jobId: string) =>
    request<{ status: string }>('POST', `/scheduler/jobs/${jobId}/resume`)
      .then(() => ({
        id: jobId, name: '', type: 'once' as const, schedule: null, handler: '',
        payload: {}, status: 'active' as const, max_retries: 0, retry_delay_seconds: 0,
        timeout_seconds: 0, created_at: 0, updated_at: 0, last_run_at: null,
        next_run_at: null, tags: [], concurrency_policy: 'allow' as const,
      })),

  deleteJob: (jobId: string) =>
    request<void>('DELETE', `/scheduler/jobs/${jobId}`),

  createJob: (job: { name: string; type: string; schedule?: string; handler?: string; payload?: Record<string, unknown>; max_retries?: number }) => {
    const body: Record<string, unknown> = {
      name: job.name,
      type: job.type === 'interval' ? 'interval' : job.type === 'once' ? 'once' : 'cron',
      max_retries: job.max_retries ?? 0,
    };
    if (job.type !== 'interval' && job.type !== 'once') body.schedule = job.schedule || '*/5 * * * *';
    if (job.payload && Object.keys(job.payload).length > 0) body.action = job.payload;
    return request<{ id: string; name: string; status: string }>('POST', '/scheduler/jobs', body)
      .then(r => ({
        id: r.id,
        name: job.name,
        type: job.type as import('../types').JobType,
        schedule: job.schedule ?? null,
        handler: job.handler,
        payload: job.payload ?? {},
        status: 'active' as const,
        max_retries: job.max_retries ?? 0,
        retry_delay_seconds: 0,
        timeout_seconds: 0,
        created_at: Date.now(),
        updated_at: Date.now(),
        last_run_at: null,
        next_run_at: null,
        tags: [],
        concurrency_policy: 'allow' as const,
      }));
  },

  getIndexes: () =>
    request<{ data: any[]; pagination: any }>('GET', '/search/indexes')
      .then(r => (r.data || []).map(idx => ({
        name: idx.name ?? '',
        document_count: idx.doc_count ?? 0,
        index_size_bytes: 0,
        field_count: idx.field_count ?? 0,
        query_count: 0,
        average_query_time_ms: 0,
      } as import('../types').IndexInfo)))
      .catch(() => {
        console.warn('getIndexes: backend unavailable');
        return [];
      }),

  searchQuery: (index: string, query: string, page = 1) =>
    request<{ hits: any[]; total_hits: number; execution_time_ms: number }>('POST', `/search/indexes/${index}/query`, { query, limit: 10, offset: (page - 1) * 10 })
      .then(r => ({
        hits: (r.hits || []).map(h => ({ id: h.id, score: h.score ?? 0, fields: h.source ?? h.fields ?? {} } as import('../types').SearchHit)),
        total: r.total_hits || 0,
        execution_time_ms: r.execution_time_ms || 0,
        max_score: 0,
      } as import('../types').SearchResult))
      .catch(() => {
        console.warn('searchQuery: backend unavailable');
        return { hits: [], total: 0, execution_time_ms: 0, max_score: 0 };
      }),

  deleteIndex: (name: string) =>
    request<void>('DELETE', `/search/indexes/${name}`),

  getBuckets: async (): Promise<import('../types').BucketInfo[]> => {
    try {
      const stats = await request<{ total_blobs: number; namespaces?: string[] }>('GET', '/blobs/stats');
      const fromStats = stats.namespaces || [];
      const namespaces = ['default', ...fromStats.filter(n => n !== 'default')];
      const buckets: import('../types').BucketInfo[] = [];
      for (const ns of namespaces) {
        try {
          const r = await request<{ data: any[]; pagination?: { total?: number } }>('GET', '/blobs', undefined, { namespace: ns, limit: 1 });
          const data = r.data || [];
          buckets.push({
            name: ns,
            file_count: r.pagination?.total ?? data.length,
            total_size_bytes: data.reduce((s: number, b: any) => s + (b.size_bytes ?? 0), 0),
            created_at: data[0]?.created_at ?? Date.now(),
            last_modified_at: data[0]?.created_at ?? Date.now(),
            allowed_mime_types: [],
            max_file_size_bytes: 0,
            versioning_enabled: false,
            public: false,
          } as import('../types').BucketInfo);
        } catch {}
      }
      return buckets;
    } catch {
      console.warn('getBuckets: backend unavailable');
      return [];
    }
  },

  getBucketObjects: async (bucket: string, page = 1) => {
    try {
      const r = await request<{ data: any[]; pagination?: any }>('GET', '/blobs', undefined, { namespace: bucket, limit: 100 } as any);
      const data = (r.data || []).map((b: any) => ({
        key: b.id ?? b.key ?? '',
        size_bytes: b.size_bytes ?? 0,
        mime_type: b.content_type ?? b.mime_type ?? 'application/octet-stream',
        etag: b.checksum_sha256 ?? b.etag ?? '',
        created_at: b.created_at ?? 0,
        last_modified_at: b.created_at ?? b.last_modified_at ?? 0,
        version_id: null,
        metadata: b.metadata || {},
      } as unknown as import('../types').BlobObject));
      // Simple pagination slice
      const perPage = 20;
      const start = (page - 1) * perPage;
      const paged = data.slice(start, start + perPage);
      return {
        data: paged,
        pagination: { page, per_page: perPage, total: data.length, total_pages: Math.max(1, Math.ceil(data.length / perPage)) },
      };
    } catch {
      console.warn('getBucketObjects: backend unavailable');
      return { data: [], pagination: { page, per_page: 20, total: 0, total_pages: 0 } };
    }
  },

  uploadBlob: async (bucket: string, file: File): Promise<{ id: string; size_bytes: number; content_type: string }> => {
    const formData = new FormData();
    formData.append('file', file);
    const url = `${baseUrl}/blobs?namespace=${encodeURIComponent(bucket)}`;
    const headers: Record<string, string> = {};
    if (authToken) {
        headers['Authorization'] = `Bearer ${authToken}`;
    }
    // Don't set Content-Type for multipart - browser sets it with boundary
    const res = await fetch(url, {
        method: 'POST',
        headers,
        body: formData,
    });
    if (res.status === 401) {
        authToken = null;
        localStorage.removeItem('nova_token');
        if (typeof window !== 'undefined' && window.location.pathname !== '/login') {
            window.location.href = '/login';
        }
    }
    if (!res.ok) {
        let message = `HTTP ${res.status}`;
        try {
            const err = await res.json();
            if (err.error) message = err.error;
            if (err.detail) message = err.detail;
            if (err.message) message = err.message;
        } catch {}
        throw new ApiError(message, res.status);
    }
    return res.json();
  },

  getUsers: () =>
    request<{ data: any[] }>('GET', '/auth/users')
      .then(r => (r.data || []).map((u: any) => ({
        id: u.id ?? '',
        username: u.username ?? '',
        email: '',
        role: (Array.isArray(u.roles) ? u.roles[0] : undefined) ?? 'viewer' as import('../types').UserRole,
        mfa_enabled: false,
        created_at: u.created_at ?? 0,
        last_login_at: null,
        enabled: true,
      } as import('../types').DashboardUser)))
      .catch(() => {
        console.warn('getUsers: backend unavailable');
        return [];
      }),

  deleteUser: (id: string) =>
    request<void>('DELETE', `/auth/users/${id}`),

  getApiKeys: () =>
    request<{ data: any[] }>('GET', '/auth/api-keys')
      .then(r => (r.data || []).map((k: any) => ({
        id: k.id ?? '',
        name: k.name ?? '',
        key_prefix: k.prefix ?? '',
        role: mapUserRole(k.permissions),
        permissions: k.permissions ?? [],
        created_at: k.created_at ?? 0,
        last_used_at: null,
        expires_at: k.expires_at ?? null,
        enabled: k.enabled ?? true,
      } as import('../types').ApiKey)))
      .catch(() => {
        console.warn('getApiKeys: backend unavailable');
        return [];
      }),

  createApiKey: (name: string, role: string) =>
    request<any>('POST', '/auth/api-keys', { name, permissions: [role], expires_at: null })
      .then(r => ({
        id: r.id ?? '',
        name: r.name ?? name,
        key_prefix: r.prefix ?? '',
        role: role as import('../types').UserRole,
        permissions: r.permissions ?? [role],
        created_at: r.created_at ?? 0,
        last_used_at: null,
        expires_at: r.expires_at ?? null,
        enabled: true,
        full_key: r.key ?? '',
      } as import('../types').ApiKey & { full_key: string })),

  deleteApiKey: (id: string) =>
    request<void>('DELETE', `/auth/api-keys/${id}`),

  getConfig: async () => {
    try {
      // Use authenticated request — /admin/config is protected, /runtime/config also protected
      // We need to bypass baseUrl (/api/v1) for admin routes, so use raw fetch with token
      const token = getToken() || localStorage.getItem('nova_token') || '';
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
      };
      if (token) headers['Authorization'] = `Bearer ${token}`;
      // Try /admin/config first (full config with validation), fallback to /runtime/config
      let res = await fetch('/admin/config', { headers, signal: AbortSignal.timeout(3000) });
      if (!res.ok) {
        res = await fetch('/runtime/config', { headers, signal: AbortSignal.timeout(3000) });
      }
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const r = await res.json() as Record<string, unknown>;
      const entries: import('../types').ConfigEntry[] = [];
      const flatten = (obj: Record<string, unknown>, prefix = '') => {
        for (const [k, v] of Object.entries(obj)) {
          const key = prefix ? `${prefix}.${k}` : k;
          if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
            flatten(v as Record<string, unknown>, key);
          } else {
            // Infer mutable/requires_restart based on key
            const isMutable = !['general.data_dir', 'networking.tls_cert_path', 'networking.tls_key_path'].includes(key) && !key.includes('data_dir');
            entries.push({ key, value: v, type: typeof v as import('../types').ConfigValueType, description: `Current value for ${key}`, mutable: isMutable, requires_restart: !isMutable, default_value: null });
          }
        }
      };
      flatten(r);
      return entries;
    } catch (e) {
      console.warn('getConfig: backend unavailable', e);
      throw e instanceof Error ? e : new Error(String(e));
    }
  },

  getLogs: async (_params: { levels?: string; subsystems?: string; search?: string; limit?: number; offset?: number; order?: string }) => {
    // No REST log history endpoint — logs are only available via WebSocket /ws live stream.
    // Return empty so the UI shows the empty state with a hint to use Live Stream.
    return { entries: [] as import('../types').LogEntry[], total_count: 0, has_more: false };
  },

  getWsUrl: () => {
    return `${baseUrl}/ws`;
  },

  // === SQL / Database extended CRUD ===
  executeSql: (query: string, params?: unknown[]) =>
    request<{ affected_rows?: number; row_count?: number; execution_time_ms: number }>('POST', '/sql/execute', { query, params }),

  getTableSchema: (table: string) =>
    request<{ table: string; columns: Array<{ name: string; type: string; nullable: boolean; is_primary_key: boolean; unique: boolean }> }>('GET', `/sql/tables/${encodeURIComponent(table)}/schema`),

  createTable: (name: string, columns: Array<{ name: string; type: string; nullable?: boolean; primaryKey?: boolean; unique?: boolean; autoIncrement?: boolean; default?: string }>) => {
    const cols = columns.map(c => {
      const defaultClause = c.default ? (() => {
        const d = c.default.trim();
        if (/^-?\d+(\.\d+)?$/.test(d) || /^(TRUE|FALSE|NULL|CURRENT_TIMESTAMP)$/i.test(d)) return ` DEFAULT ${d}`;
        return ` DEFAULT '${String(d).replace(/'/g, "''")}'`;
      })() : '';
      return `${c.name} ${c.type}${c.primaryKey ? ' PRIMARY KEY' : ''}${c.unique ? ' UNIQUE' : ''}${c.autoIncrement ? ' AUTO_INCREMENT' : ''}${c.nullable === false ? ' NOT NULL' : ''}${defaultClause}`;
    }).join(', ');
    return request('POST', '/sql/execute', { query: `CREATE TABLE ${name} (${cols})` });
  },

  deleteTable: (name: string) =>
    request('POST', '/sql/execute', { query: `DROP TABLE ${name}` }),

  insertDocument: (table: string, data: Record<string, unknown>) => {
    const keys = Object.keys(data);
    const values = keys.map(k => {
      const v = data[k];
      if (typeof v === 'string') return `'${String(v).replace(/'/g, "''")}'`;
      if (v === null || v === undefined) return 'NULL';
      return String(v);
    });
    return request('POST', '/sql/execute', { query: `INSERT INTO ${table} (${keys.join(', ')}) VALUES (${values.join(', ')})` });
  },

  deleteDocument: (table: string, where: string) =>
    request('POST', '/sql/execute', { query: `DELETE FROM ${table} WHERE ${where}` }),

  // === Cache extended ===
  setCacheKey: (key: string, value: unknown, ttlSeconds?: number) =>
    request('POST', `/cache/${encodeURIComponent(key)}`, { value, ttl_ms: ttlSeconds ? ttlSeconds * 1000 : undefined }),

  getCacheEntry: (key: string) =>
    request<{ key: string; value: unknown; ttl_remaining_ms: number | null }>('GET', `/cache/${encodeURIComponent(key)}`),

  // === Queue extended ===
  createQueue: (name: string, opts?: { durable?: boolean; max_length?: number; max_message_size?: number }) =>
    request('POST', '/queues', { name, durable: opts?.durable, max_length: opts?.max_length, max_message_size: opts?.max_message_size }),

  getQueue: (name: string) =>
    request<{ name: string; queue_type: string; max_size: number; paused: boolean }>('GET', `/queues/${encodeURIComponent(name)}`),

  getQueueStats: (name: string) =>
    request<{ available_messages: number; in_flight_messages: number; delayed_messages: number; total_messages: number; dlq_messages: number; messages_enqueued: number; messages_dequeued: number }>('GET', `/queues/${encodeURIComponent(name)}/stats`),

  ackMessage: (queue: string, id: string) =>
    request('POST', `/queues/${encodeURIComponent(queue)}/messages/${encodeURIComponent(id)}/ack`),

  // === Scheduler extended ===
  getJob: (id: string) =>
    request<any>('GET', `/scheduler/jobs/${encodeURIComponent(id)}`)
      .then(j => ({
        id: j.id ?? '',
        name: j.name ?? '',
        type: mapScheduleType(j.schedule_type),
        schedule: null,
        handler: '',
        payload: {},
        status: (j.state === 'Paused' || j.state === 'Cancelled') ? 'paused' as const : 'active' as const,
        max_retries: j.max_retries ?? 0,
        retry_delay_seconds: 0,
        timeout_seconds: 0,
        created_at: 0,
        updated_at: 0,
        last_run_at: j.last_run_at ?? null,
        next_run_at: j.next_run_at ?? null,
        tags: [],
        concurrency_policy: 'allow' as const,
      }) as unknown as import('../types').JobInfo),

  updateJob: (id: string, data: Partial<{ name: string; schedule: string; enabled: boolean }>) =>
    request('PUT', `/scheduler/jobs/${encodeURIComponent(id)}`, data),

  // === Search extended ===
  createIndex: (name: string, fields?: Array<{ name: string; type: string; analyzer?: string; boost?: number }>) =>
    request('POST', '/search/indexes', { name, fields }),

  addDocuments: (index: string, documents: unknown[]) =>
    request('POST', `/search/indexes/${encodeURIComponent(index)}/documents`, { documents }),

  getIndex: (name: string) =>
    request<{ name: string; num_docs: number; num_terms: number; field_count: number }>('GET', `/search/indexes/${encodeURIComponent(name)}`),

  getIndexStats: (name: string) =>
    request<{ num_docs: number; num_terms: number; field_count: number }>('GET', `/search/indexes/${encodeURIComponent(name)}/stats`),

  // === Blob extended ===
  deleteBlob: (id: string) =>
    request('DELETE', `/blobs/${encodeURIComponent(id)}`),

  getBlobInfo: (id: string) =>
    request<{ id: string; size_bytes: number; content_type: string; checksum_sha256: string; created_at: number; metadata: Record<string,string> }>('GET', `/blobs/${encodeURIComponent(id)}/info`),

  getBlobStats: () =>
    request<{ total_blobs: number; total_bytes: number; total_chunks: number; unique_chunks: number; active_uploads: number; namespaces: string[] }>('GET', '/blobs/stats'),

  createBucket: (name: string) =>
    Promise.reject(new Error('Buckets map to namespaces; upload to a new namespace to create one')),

  deleteBucket: async (name: string) => {
    // No backend DELETE-by-namespace route — delete every object in the namespace.
    let offset = 0;
    const per = 100;
    while (true) {
      const r = await request<{ data: any[] }>('GET', '/blobs', undefined, { namespace: name, limit: per, offset });
      const ids = (r.data || []).map((b: any) => b.id).filter(Boolean);
      await Promise.all(ids.map((id: string) => request<void>('DELETE', `/blobs/${encodeURIComponent(id)}`).catch(() => {})));
      if (ids.length < per) break;
      offset += per;
    }
  },

  // === Auth extended ===
  createUser: (data: { username: string; password: string; roles?: string[] }) =>
    request<{ id: string; username: string; roles: string[] }>('POST', '/auth/users', data),

  getUser: (id: string) =>
    request<{ id: string; username: string; roles: string[]; created_at: number }>('GET', `/auth/users/${encodeURIComponent(id)}`),

  updateUserRoles: (id: string, roles: string[]) =>
    request('PUT', `/auth/users/${encodeURIComponent(id)}/roles`, { roles }),

  changePassword: (id: string, current_password: string, new_password: string) =>
    request('PUT', `/auth/users/${encodeURIComponent(id)}/password`, { current_password, new_password }),

  // === Config ===
  updateConfig: (patch: Record<string, unknown>) =>
    request('PUT', '/admin/config', patch),

  getAdminConfig: () =>
    request<Record<string, unknown>>('GET', '/admin/config'),
};
