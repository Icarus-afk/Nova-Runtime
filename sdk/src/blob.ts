import type { HttpClient } from './client';
import type {
  BlobMetadata, BlobUploadInput, BlobDownloadOptions,
  BlobListEntry, BlobFilter, BlobMetrics, StorageTier,
  Connection, PaginationInput
} from './types';

export class BlobClient {
  constructor(
    private http: HttpClient
  ) {}

  async upload(
    key: string,
    content: string,
    options?: Omit<BlobUploadInput, 'key' | 'content'>
  ): Promise<BlobMetadata> {
    const body: Record<string, unknown> = {
      key,
      content,
      contentType: options?.contentType,
      contentEncoding: options?.contentEncoding,
      storageTier: options?.storageTier,
      expiresAt: options?.expiresAt?.toISOString(),
      metadata: options?.metadata,
      overwrite: options?.overwrite,
    };

    const response = await this.http.post<BlobMetadata>('/blob/upload', body);
    return response.data;
  }

  async download(key: string, options?: BlobDownloadOptions): Promise<string> {
    const headers: Record<string, string> = {};
    if (options?.startByte !== undefined) {
      headers['Range'] = `bytes=${options.startByte}-${options.endByte ?? ''}`;
    }
    const response = await this.http.get<string>(`/blob/${encodeURIComponent(key)}`, {
      responseType: 'text',
      headers,
      signal: options?.signal,
    });
    return response.data;
  }

  async del(key: string): Promise<boolean> {
    const response = await this.http.delete<{ deleted: boolean }>(`/blob/${encodeURIComponent(key)}`);
    return response.data.deleted;
  }

  async multiDel(keys: string[]): Promise<number> {
    const response = await this.http.post<{ deleted: number }>('/blob/multi-delete', { keys });
    return response.data.deleted;
  }

  async list(
    prefix?: string,
    options?: {
      delimiter?: string;
      pagination?: PaginationInput;
      filter?: BlobFilter;
    }
  ): Promise<Connection<BlobListEntry>> {
    const response = await this.http.get<Connection<BlobListEntry>>('/blob', {
      query: { prefix, delimiter: options?.delimiter, ...(options?.pagination as Record<string, unknown> ?? {}) },
    });
    return response.data;
  }

  async info(key: string): Promise<BlobMetadata> {
    const response = await this.http.get<BlobMetadata>(`/blob/${encodeURIComponent(key)}/info`);
    return response.data;
  }

  async copy(source: string, destination: string): Promise<BlobMetadata> {
    const response = await this.http.post<BlobMetadata>('/blob/copy', { source, destination });
    return response.data;
  }

  async move(source: string, destination: string): Promise<BlobMetadata> {
    const response = await this.http.post<BlobMetadata>('/blob/move', { source, destination });
    return response.data;
  }

  async setTier(key: string, tier: StorageTier): Promise<BlobMetadata> {
    const response = await this.http.post<BlobMetadata>(`/blob/${encodeURIComponent(key)}/tier`, { tier });
    return response.data;
  }

  async setExpiry(key: string, expiresAt: Date): Promise<BlobMetadata> {
    const response = await this.http.post<BlobMetadata>(
      `/blob/${encodeURIComponent(key)}/expiry`, { expiresAt: expiresAt.toISOString() }
    );
    return response.data;
  }

  async removeExpiry(key: string): Promise<BlobMetadata> {
    const response = await this.http.delete<BlobMetadata>(`/blob/${encodeURIComponent(key)}/expiry`);
    return response.data;
  }

  async getStats(): Promise<BlobMetrics> {
    const response = await this.http.get<BlobMetrics>('/blob/stats');
    return response.data;
  }

  async *listIterator(prefix?: string, options?: {
    delimiter?: string;
    filter?: BlobFilter;
  }): AsyncIterable<BlobListEntry> {
    let cursor: string | undefined;
    let hasMore = true;
    while (hasMore) {
      const page = await this.list(prefix, {
        ...options,
        pagination: { first: 100, after: cursor },
      });
      for (const edge of page.edges) {
        yield edge.node;
      }
      hasMore = page.pageInfo.hasNextPage;
      cursor = page.pageInfo.endCursor ?? undefined;
    }
  }
}
