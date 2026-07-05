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
      if (err.detail) message = err.detail;
      if (err.message) message = err.message;
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
        cpu: { usage_percent: 0, load_avg_1m: 0, load_avg_5m: 0, load_avg_15m: 0, cores: 0, temperature_celsius: null },
        memory: {
          total_bytes: (mem.total_bytes as number) || 0,
          used_bytes: (mem.used_bytes as number) || 0,
          resident_bytes: 0, allocated_bytes: (mem.used_bytes as number) || 0,
          cache_bytes: 0, swap_used_bytes: 0, swap_total_bytes: 0,
        },
        disk: {
          data_path: '', total_bytes: (disk.total_bytes as number) || 0,
          used_bytes: (disk.used_bytes as number) || 0,
          free_bytes: (disk.free_bytes as number) || 0,
          fs_type: '', read_ops_per_sec: 0, write_ops_per_sec: 0,
          read_bytes_per_sec: 0, write_bytes_per_sec: 0, io_wait_percent: 0,
        },
        network: { rx_bytes_per_sec: 0, tx_bytes_per_sec: 0, rx_packets_per_sec: 0, tx_packets_per_sec: 0, connections_active: 0, connection_errors: 0, tcp_retransmit_percent: 0 },
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
        const docs = dataR.rows.map((row: unknown[], i: number) => ({
          id: String(offset + i + 1),
          collection,
          data: Object.fromEntries(names.map((c: string, j: number) => [c, row[j]])),
          created_at: 0,
          updated_at: 0,
          version: 1,
          size_bytes: 0,
        }) as import('../types').Document);
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

  queryDatabase: (query: { collection: string; filter?: Record<string, unknown>; limit?: number }) =>
    request<{ column_names?: string[]; columns: string[]; rows: unknown[][]; row_count: number; execution_time_ms: number }>('POST', '/sql/query', { query: query.collection, ...query.filter })
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
    request<{ data: string[]; total: number; pattern: string | null }>('GET', '/cache/keys')
      .then(r => ({
        data: (r.data || []).map(k => ({ key: k, value_size_bytes: 0, created_at: 0, expires_at: null, last_access_at: 0, access_count: 0, ttl_seconds: null } as unknown as import('../types').CacheEntry)),
        pagination: { page: _page, per_page: 100, total: r.total ?? 0, total_pages: 1 },
      }))
      .catch(() => {
        console.warn('getCacheKeys: backend unavailable');
        return { data: [], pagination: { page: _page, per_page: 20, total: 0, total_pages: 0 } };
      }),

  deleteCacheKey: (key: string) =>
    request<void>('DELETE', `/cache/${encodeURIComponent(key)}`)
      .catch(() => {
        console.warn('deleteCacheKey: backend unavailable');
        return undefined;
      }),

  clearCache: () =>
    Promise.resolve(undefined),

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
      }))
      .catch(() => {
        console.warn('publishMessage: backend unavailable');
        return {
          id: '', body, state: 'ready' as const, priority: priority ?? 0,
          enqueued_at: Date.now(), reserved_at: null, delayed_until: null,
          attempts: 0, error_count: 0, last_error: null, ttr_seconds: 0,
        };
      }),

  purgeQueue: (name: string) =>
    request<{ status: string }>('POST', `/queues/${name}/purge`)
      .then(() => ({ purged_count: -1 }))
      .catch(() => {
        console.warn('purgeQueue: backend unavailable');
        return { purged_count: 0 };
      }),

  deleteQueue: (name: string) =>
    request<void>('DELETE', `/queues/${name}`)
      .catch(() => {
        console.warn('deleteQueue: backend unavailable');
        return undefined;
      }),

  getJobs: () =>
    request<{ data: any[]; pagination: any }>('GET', '/scheduler/jobs')
      .then(r => (r.data || []).map(j => ({
        id: j.id ?? '',
        name: j.name ?? '',
        type: String(j.schedule_type ?? 'one_time').toLowerCase(),
        schedule: j.cron_expression ?? null,
        handler: '',
        payload: {},
        status: (j.state === 'Paused' || j.state === 'Cancelled' || j.state === 'Failed') ? 'paused' as const : 'active' as const,
        max_retries: j.retry_count ?? 0,
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

  getJobExecutions: (_jobId: string, page = 1) =>
    Promise.resolve({ data: [] as import('../types').JobExecution[], pagination: { page, per_page: 20, total: 0, total_pages: 0 } }),

  triggerJob: (jobId: string) =>
    request<{ status: string }>('POST', `/scheduler/jobs/${jobId}/trigger`)
      .then(() => ({
        id: '', job_id: jobId, status: 'running' as const,
        started_at: Date.now(), finished_at: null, duration_ms: null,
        result: null, error: null, retry_attempt: 0, trigger: 'manual' as const,
      }))
      .catch(() => {
        console.warn('triggerJob: backend unavailable');
        return {
          id: '', job_id: jobId, status: 'running' as const,
          started_at: Date.now(), finished_at: null, duration_ms: null,
          result: null, error: null, retry_attempt: 0, trigger: 'manual' as const,
        };
      }),

  pauseJob: (jobId: string) =>
    request<{ status: string }>('POST', `/scheduler/jobs/${jobId}/pause`)
      .then(() => ({
        id: jobId, name: '', type: 'once' as const, schedule: null, handler: '',
        payload: {}, status: 'paused' as const, max_retries: 0, retry_delay_seconds: 0,
        timeout_seconds: 0, created_at: 0, updated_at: 0, last_run_at: null,
        next_run_at: null, tags: [], concurrency_policy: 'allow' as const,
      }))
      .catch(() => {
        console.warn('pauseJob: backend unavailable');
        return {
          id: jobId, name: '', type: 'once' as const, schedule: null, handler: '',
          payload: {}, status: 'paused' as const, max_retries: 0, retry_delay_seconds: 0,
          timeout_seconds: 0, created_at: 0, updated_at: 0, last_run_at: null,
          next_run_at: null, tags: [], concurrency_policy: 'allow' as const,
        };
      }),

  resumeJob: (jobId: string) =>
    request<{ status: string }>('POST', `/scheduler/jobs/${jobId}/resume`)
      .then(() => ({
        id: jobId, name: '', type: 'once' as const, schedule: null, handler: '',
        payload: {}, status: 'active' as const, max_retries: 0, retry_delay_seconds: 0,
        timeout_seconds: 0, created_at: 0, updated_at: 0, last_run_at: null,
        next_run_at: null, tags: [], concurrency_policy: 'allow' as const,
      }))
      .catch(() => {
        console.warn('resumeJob: backend unavailable');
        return {
          id: jobId, name: '', type: 'once' as const, schedule: null, handler: '',
          payload: {}, status: 'active' as const, max_retries: 0, retry_delay_seconds: 0,
          timeout_seconds: 0, created_at: 0, updated_at: 0, last_run_at: null,
          next_run_at: null, tags: [], concurrency_policy: 'allow' as const,
        };
      }),

  deleteJob: (jobId: string) =>
    request<void>('DELETE', `/scheduler/jobs/${jobId}`)
      .catch(() => {
        console.warn('deleteJob: backend unavailable');
        return undefined;
      }),

  createJob: (job: { name: string; type: string; schedule?: string; handler: string; payload?: Record<string, unknown>; max_retries?: number }) =>
    request<{ id: string; name: string; status: string }>('POST', '/scheduler/jobs', { name: job.name, type: job.type === 'scheduled' ? 'cron' : job.type, schedule: job.schedule ?? '*/5 * * * *', max_retries: job.max_retries ?? 0 })
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
      }))
      .catch(() => {
        console.warn('createJob: backend unavailable');
        return {
          id: '',
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
        };
      }),

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
      .then(r => ({ hits: r.hits || [], total: r.total_hits || 0, execution_time_ms: r.execution_time_ms || 0, max_score: 0 } as import('../types').SearchResult))
      .catch(() => {
        console.warn('searchQuery: backend unavailable');
        return { hits: [], total: 0, execution_time_ms: 0, max_score: 0 };
      }),

  deleteIndex: (name: string) =>
    request<void>('DELETE', `/search/indexes/${name}`)
      .catch(() => {
        console.warn('deleteIndex: backend unavailable');
        return undefined;
      }),

  getBuckets: () =>
    request<{ data: any[] }>('GET', '/blobs')
      .then(r => (r.data || []).map(b => ({
        name: b.id ?? '',
        file_count: 1,
        total_size_bytes: b.size_bytes ?? 0,
        created_at: b.created_at ?? 0,
        last_modified_at: b.created_at ?? 0,
        allowed_mime_types: [],
        max_file_size_bytes: 0,
        versioning_enabled: false,
        public: false,
      } as import('../types').BucketInfo)))
      .catch(() => {
        console.warn('getBuckets: backend unavailable, returning empty');
        return [];
      }),

  getBucketObjects: (bucket: string, page = 1) =>
    request<{ data: any[] }>('GET', '/blobs')
      .then(r => ({
        data: (r.data || []).filter((b: any) => b.id === bucket).map((b: any) => ({
            key: b.id ?? '',
            size_bytes: b.size_bytes ?? 0,
            mime_type: b.content_type ?? '',
            etag: '',
            created_at: b.created_at ?? 0,
            last_modified_at: b.created_at ?? 0,
            version_id: null,
            metadata: {},
        } as unknown as import('../types').BlobObject)),
        pagination: { page, per_page: 20, total: (r.data || []).length, total_pages: 1 },
      }))
      .catch(() => {
        console.warn('getBucketObjects: backend unavailable, returning empty');
        return { data: [], pagination: { page, per_page: 20, total: 0, total_pages: 0 } };
      }),

  uploadBlob: async (bucket: string, file: File): Promise<{ id: string; size_bytes: number; content_type: string }> => {
    const formData = new FormData();
    formData.append('file', file);
    const url = `${baseUrl}/blobs`;
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
    request<{ data: import('../types').DashboardUser[] }>('GET', '/auth/users')
      .then(r => (r.data || []))
      .catch(() => {
        console.warn('getUsers: backend unavailable');
        return [];
      }),

  deleteUser: (id: string) =>
    request<void>('DELETE', `/auth/users/${id}`)
      .catch(() => {
        console.warn('deleteUser: backend unavailable');
        return undefined;
      }),

  getApiKeys: () =>
    request<{ data: import('../types').ApiKey[] }>('GET', '/auth/api-keys')
      .then(r => (r.data || []))
      .catch(() => {
        console.warn('getApiKeys: backend unavailable');
        return [];
      }),

  createApiKey: (name: string, role: string) =>
    request<import('../types').ApiKey & { full_key: string }>('POST', '/auth/api-keys', { name, permissions: [role], expires_at: null })
      .then(r => ({ ...r, role: role as import('../types').UserRole }))
      .catch(() => {
        console.warn('createApiKey: backend unavailable');
        return {
          id: '', name, key_prefix: '', role: role as import('../types').UserRole,
          permissions: [], created_at: Date.now(), last_used_at: null,
          expires_at: null, enabled: true, full_key: '',
        };
      }),

  deleteApiKey: (id: string) =>
    request<void>('DELETE', `/auth/api-keys/${id}`)
      .catch(() => {
        console.warn('deleteApiKey: backend unavailable');
        return undefined;
      }),

  getConfig: async () => {
    try {
      const res = await fetch('/runtime/config', { signal: AbortSignal.timeout(3000) });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const r = await res.json() as Record<string, unknown>;
      const entries: import('../types').ConfigEntry[] = [];
      const flatten = (obj: Record<string, unknown>, prefix = '') => {
        for (const [k, v] of Object.entries(obj)) {
          const key = prefix ? `${prefix}.${k}` : k;
          if (v !== null && typeof v === 'object' && !Array.isArray(v)) {
            flatten(v as Record<string, unknown>, key);
          } else {
            entries.push({ key, value: v, type: typeof v as import('../types').ConfigValueType, description: '', mutable: false, requires_restart: false, default_value: null });
          }
        }
      };
      flatten(r);
      return entries;
    } catch {
      console.warn('getConfig: backend unavailable');
      return [];
    }
  },

  getLogs: (_params: { levels?: string; subsystems?: string; search?: string; limit?: number; offset?: number; order?: string }) =>
    Promise.resolve({ entries: [] as import('../types').LogEntry[], total_count: 0, has_more: false }),

  getWsUrl: () => {
    const wsBase = baseUrl.replace(/^http/, 'ws');
    return `${wsBase}/logs/stream`;
  },
};
