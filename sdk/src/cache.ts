import type { HttpClient } from './client';
import { NovaError } from './errors';
import type { Connection, PaginationInput, CacheMetrics } from './types';

export class CacheClient {
  constructor(
    private http: HttpClient
  ) {}

  async get<T = unknown>(key: string): Promise<T | null> {
    try {
      const response = await this.http.get<{ value: T }>(`/cache/${encodeURIComponent(key)}`);
      return response.data.value;
    } catch (error) {
      if (error instanceof NovaError && error.code === 'NOT_FOUND') return null;
      throw error;
    }
  }

  async multiGet<T = unknown>(keys: string[]): Promise<Map<string, T | null>> {
    const response = await this.http.post<Array<{ key: string; value: T | null }>>('/cache/multi-get', { keys });
    const map = new Map<string, T | null>();
    for (const entry of response.data) {
      map.set(entry.key, entry.value);
    }
    return map;
  }

  async set<T = unknown>(
    key: string,
    value: T,
    options?: { ttlMs?: number; nx?: boolean }
  ): Promise<void> {
    await this.http.post(`/cache/${encodeURIComponent(key)}`, {
      value,
      ttlMs: options?.ttlMs,
      nx: options?.nx,
    });
  }

  async multiSet<T = unknown>(entries: Array<{ key: string; value: T; ttlMs?: number }>): Promise<void> {
    await this.http.post('/cache/multi-set', { entries });
  }

  async del(key: string): Promise<boolean> {
    const response = await this.http.delete<{ deleted: boolean }>(`/cache/${encodeURIComponent(key)}`);
    return response.data.deleted;
  }

  async multiDel(keys: string[]): Promise<number> {
    const response = await this.http.post<{ deleted: number }>('/cache/multi-del', { keys });
    return response.data.deleted;
  }

  async delPattern(pattern: string): Promise<number> {
    const response = await this.http.post<{ deleted: number }>('/cache/del-pattern', { pattern });
    return response.data.deleted;
  }

  async keys(pattern?: string, options?: PaginationInput): Promise<Connection<string>> {
    const response = await this.http.get<Connection<string>>('/cache/keys', {
      query: { pattern, ...(options as Record<string, unknown> ?? {}) },
    });
    return response.data;
  }

  async ttl(key: string): Promise<number | null> {
    const response = await this.http.get<{ ttlMs: number | null }>(`/cache/${encodeURIComponent(key)}/ttl`);
    return response.data.ttlMs;
  }

  async expire(key: string, ttlMs: number): Promise<boolean> {
    const response = await this.http.post<{ updated: boolean }>(`/cache/${encodeURIComponent(key)}/expire`, { ttlMs });
    return response.data.updated;
  }

  async incr(key: string, amount?: number): Promise<number> {
    const response = await this.http.post<{ value: number }>(`/cache/${encodeURIComponent(key)}/incr`, {
      amount: amount ?? 1,
    });
    return response.data.value;
  }

  async stats(): Promise<CacheMetrics> {
    const response = await this.http.get<CacheMetrics>('/cache/stats');
    return response.data;
  }

  async flush(): Promise<number> {
    const response = await this.http.post<{ deleted: number }>('/cache/flush');
    return response.data.deleted;
  }

  async *list(pattern?: string): AsyncIterable<string> {
    let cursor: string | undefined;
    let hasMore = true;
    while (hasMore) {
      const page = await this.keys(pattern, { first: 100, after: cursor });
      for (const edge of page.edges) {
        yield edge.node;
      }
      hasMore = page.pageInfo.hasNextPage;
      cursor = page.pageInfo.endCursor ?? undefined;
    }
  }
}
