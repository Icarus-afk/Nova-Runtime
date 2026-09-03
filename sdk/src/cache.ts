import type { HttpClient } from './client';
import { NovaError } from './errors';
import type { Connection, PaginationInput, CacheMetrics } from './types';

interface RawCacheGet {
  key: string;
  value: unknown;
  ttl_remaining_ms: number | null;
}

interface RawCacheKeys {
  data: string[];
  pagination: { offset: number; limit: number; total: number; has_more: boolean };
  pattern: string | null;
  total?: number;
}

interface RawCacheStats {
  keys: number;
  hits: number;
  misses: number;
  hit_rate: number;
  memory_bytes: number;
  evictions: number;
}

function toCacheConnection(keys: string[], pagination: { offset: number; limit: number; total: number; has_more: boolean }): Connection<string> {
  const edges = keys.map((node, idx) => ({
    node,
    cursor: String(pagination.offset + idx + 1),
  }));
  return {
    edges,
    pageInfo: {
      hasNextPage: pagination.has_more,
      hasPreviousPage: pagination.offset > 0,
      startCursor: edges[0]?.cursor ?? null,
      endCursor: edges[edges.length - 1]?.cursor ?? null,
    },
    totalCount: pagination.total,
  };
}

export class CacheClient {
  constructor(
    private http: HttpClient
  ) {}

  async get<T = unknown>(key: string): Promise<T | null> {
    try {
      const response = await this.http.get<RawCacheGet>(`/cache/${encodeURIComponent(key)}`);
      return response.data.value as T;
    } catch (error) {
      if (error instanceof NovaError && error.code === 'NOT_FOUND') return null;
      throw error;
    }
  }

  async multiGet<T = unknown>(keys: string[]): Promise<Map<string, T | null>> {
    // No backend multi-get — fallback to parallel individual gets
    const entries = await Promise.all(
      keys.map(async (k) => {
        const v = await this.get<T>(k);
        return [k, v] as const;
      })
    );
    const map = new Map<string, T | null>();
    for (const [k, v] of entries) map.set(k, v);
    return map;
  }

  async set<T = unknown>(
    key: string,
    value: T,
    options?: { ttlMs?: number; ttl_ms?: number; nx?: boolean; ttlSeconds?: number }
  ): Promise<void> {
    const ttlMs = (options as any)?.ttl_ms ?? options?.ttlMs ?? ((options as any)?.ttlSeconds ? (options as any).ttlSeconds * 1000 : undefined);
    const body: Record<string, unknown> = { value };
    if (ttlMs !== undefined) body.ttl_ms = ttlMs;
    // nx not supported — ignored for prototype
    await this.http.post(`/cache/${encodeURIComponent(key)}`, body);
  }

  async multiSet<T = unknown>(entries: Array<{ key: string; value: T; ttlMs?: number; ttl_ms?: number }>): Promise<void> {
    // Backend batch expects POST /cache/batch with array of {key, value, ttl_ms}
    const payload = entries.map((e) => ({
      key: e.key,
      value: e.value,
      ttl_ms: (e as any).ttl_ms ?? (e as any).ttlMs ?? undefined,
    }));
    await this.http.post('/cache/batch', payload);
  }

  async del(key: string): Promise<boolean> {
    try {
      await this.http.delete(`/cache/${encodeURIComponent(key)}`);
      return true;
    } catch (error) {
      if (error instanceof NovaError && error.code === 'NOT_FOUND') return false;
      throw error;
    }
  }

  async multiDel(keys: string[]): Promise<number> {
    // No backend multi-del — fallback to looping deletes
    let deleted = 0;
    await Promise.all(
      keys.map(async (k) => {
        const ok = await this.del(k);
        if (ok) deleted++;
      })
    );
    return deleted;
  }

  async delPattern(pattern: string): Promise<number> {
    // No backend del-pattern — list keys with pattern then delete each
    const keysResp = await this.keys(pattern, { first: 1000 } as any);
    const keys = keysResp.edges.map((e) => e.node);
    // If pattern may need pagination, loop
    let allKeys = [...keys];
    // keys() already handles pagination size limit 1000; for larger sets, iterate via list helper
    if (keysResp.pageInfo.hasNextPage) {
      // fallback to enumerate via list()
      allKeys = [];
      for await (const k of this.list(pattern)) allKeys.push(k);
    }
    return this.multiDel(allKeys);
  }

  async keys(pattern?: string, options?: PaginationInput & { limit?: number; offset?: number }): Promise<Connection<string>> {
    const query: Record<string, unknown> = {};
    if (pattern !== undefined) query.pattern = pattern;
    // Backend uses limit/offset ; SDK uses first/after cursor style — map
    if ((options as any)?.limit !== undefined) query.limit = (options as any).limit;
    else if (options?.first !== undefined) query.limit = options.first;
    if ((options as any)?.offset !== undefined) query.offset = (options as any).offset;
    else if (options?.after !== undefined) {
      const parsed = parseInt(options.after, 10);
      if (!Number.isNaN(parsed)) query.offset = parsed;
      else query.offset = 0;
    }
    // Include last/before alternative
    if (options?.last !== undefined && query.limit === undefined) query.limit = options.last;

    const response = await this.http.get<RawCacheKeys>('/cache/keys', { query });
    const raw = response.data;
    // Normalize pagination shape
    const pagination = raw.pagination ?? { offset: 0, limit: raw.data.length, total: raw.total ?? raw.data.length, has_more: false };
    return toCacheConnection(raw.data, {
      offset: pagination.offset ?? 0,
      limit: pagination.limit ?? raw.data.length,
      total: pagination.total ?? (raw.total ?? raw.data.length),
      has_more: pagination.has_more ?? false,
    });
  }

  async ttl(key: string): Promise<number | null> {
    // No dedicated /ttl endpoint — fetch via GET and read ttl_remaining_ms
    try {
      const response = await this.http.get<RawCacheGet>(`/cache/${encodeURIComponent(key)}`);
      return response.data.ttl_remaining_ms;
    } catch (error) {
      if (error instanceof NovaError && error.code === 'NOT_FOUND') return null;
      throw error;
    }
  }

  async expire(key: string, ttlMs: number): Promise<boolean> {
    // No /expire endpoint — re-set with same value but new ttl_ms
    const val = await this.get(key);
    if (val === null) return false;
    await this.set(key, val, { ttlMs });
    return true;
  }

  async incr(key: string, amount?: number): Promise<number> {
    // No /incr endpoint — get, increment, set
    const inc = amount ?? 1;
    const cur = await this.get<unknown>(key);
    let num = 0;
    if (typeof cur === 'number') num = cur;
    else if (typeof cur === 'string') num = parseFloat(cur) || 0;
    else if (cur === null || cur === undefined) num = 0;
    else num = Number(cur as any) || 0;
    const next = num + inc;
    await this.set(key, next as unknown as any);
    return next;
  }

  async stats(): Promise<CacheMetrics> {
    const response = await this.http.get<RawCacheStats>('/cache/stats');
    const raw = response.data;
    return {
      hits: raw.hits,
      misses: raw.misses,
      hitRate: raw.hit_rate,
      entries: raw.keys,
      memoryUsedBytes: raw.memory_bytes ?? 0,
      evictions: raw.evictions,
    };
  }

  async flush(): Promise<number> {
    // No /flush endpoint — enumerate and delete (matches dashboard clearCache)
    let deleted = 0;
    // Use list helper to handle pagination
    const allKeys: string[] = [];
    for await (const k of this.list('*')) allKeys.push(k);
    // Delete in batches to avoid huge concurrency
    const batchSize = 50;
    for (let i = 0; i < allKeys.length; i += batchSize) {
      const batch = allKeys.slice(i, i + batchSize);
      const n = await this.multiDel(batch);
      deleted += n;
    }
    return deleted;
  }

  async *list(pattern?: string): AsyncIterable<string> {
    let offset = 0;
    const limit = 100;
    while (true) {
      const page = await this.keys(pattern, { first: limit, after: String(offset) } as any);
      for (const edge of page.edges) yield edge.node;
      if (!page.pageInfo.hasNextPage) break;
      offset += limit;
    }
  }
}
