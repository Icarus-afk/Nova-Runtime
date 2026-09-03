import type { HttpClient } from './client';
import { NovaError } from './errors';
import type {
  BlobMetadata, BlobUploadInput, BlobDownloadOptions,
  BlobListEntry, BlobFilter, BlobMetrics, StorageTier,
  Connection, PaginationInput
} from './types';

interface RawBlobUpload {
  id: string;
  size_bytes: number;
  content_type: string;
  checksum_sha256: string;
  created_at: number;
}

interface RawBlobListItem {
  id: string;
  filename: string;
  size_bytes: number;
  content_type: string;
  created_at: number;
}

interface RawBlobList {
  data: RawBlobListItem[];
  pagination: { offset: number; limit: number; total: number; has_more: boolean };
}

interface RawBlobInfo {
  id: string;
  size_bytes: number;
  content_type: string;
  checksum_sha256: string;
  created_at: number;
  metadata: Record<string, string>;
}

interface RawBlobStats {
  total_blobs: number;
  total_bytes: number;
  total_chunks: number;
  unique_chunks: number;
  active_uploads: number;
  namespaces: string[];
}

function toBlobConnection(entries: BlobListEntry[], pagination: { offset: number; limit: number; total: number; has_more: boolean }): Connection<BlobListEntry> {
  const edges = entries.map((node, idx) => ({
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

export class BlobClient {
  constructor(
    private http: HttpClient
  ) {}

  // Upload: POST /api/v1/blobs?namespace= multipart field 'file'
  // Keep legacy signature upload(key, content, options) where key is treated as namespace prefix or file identifier
  // For multipart we convert content string/Buffer/Blob to file
  async upload(
    key: string,
    content: string | Buffer | Blob | Uint8Array,
    options?: Omit<BlobUploadInput, 'key' | 'content'>
  ): Promise<BlobMetadata> {
    // Determine namespace: if key contains '/', first segment is namespace; otherwise use options.metadata?.custom?.namespace or "default"
    // Also allow key as filename hint via metadata.filename
    let namespace = 'default';
    let filenameHint = key;
    if (key.includes('/')) {
      const parts = key.split('/');
      namespace = parts[0] || 'default';
      filenameHint = parts.slice(1).join('/') || key;
    }
    // Allow override via options.metadata custom
    const customNs = (options?.metadata as any)?.namespace ?? (options?.metadata as any)?.bucket;
    if (customNs) namespace = String(customNs);

    // Build FormData
    const formData = new (globalThis as any).FormData();
    let fileObj: any;
    let fileName = filenameHint || 'file';
    // Normalize content to Blob
    if (typeof content === 'string') {
      fileObj = new (globalThis as any).Blob([content], { type: options?.contentType ?? 'application/octet-stream' });
    } else if (typeof (globalThis as any).Buffer !== 'undefined' && (content as any) instanceof (globalThis as any).Buffer) {
      fileObj = new (globalThis as any).Blob([content as any], { type: options?.contentType ?? 'application/octet-stream' });
    } else if (content instanceof Uint8Array) {
      fileObj = new (globalThis as any).Blob([content], { type: options?.contentType ?? 'application/octet-stream' });
    } else if (typeof (globalThis as any).Blob !== 'undefined' && content instanceof (globalThis as any).Blob) {
      fileObj = content;
      if ((content as any).name) fileName = (content as any).name;
    } else {
      // Fallback stringify
      const str = String(content);
      fileObj = new (globalThis as any).Blob([str], { type: options?.contentType ?? 'text/plain' });
    }
    formData.append('file', fileObj, fileName);

    const response = await this.http.request<RawBlobUpload>({
      method: 'POST',
      path: '/blobs',
      query: { namespace },
      body: formData as any,
      headers: {}, // content-type removed via client detection
    });

    const raw = response.data;
    // Map to BlobMetadata (synthesize missing fields)
    return {
      key: raw.id,
      sizeBytes: raw.size_bytes,
      contentType: raw.content_type,
      contentEncoding: options?.contentEncoding,
      etag: raw.checksum_sha256,
      md5: '',
      sha256: raw.checksum_sha256,
      storageTier: options?.storageTier ?? 'HOT',
      createdAt: new Date(raw.created_at).toISOString(),
      updatedAt: new Date(raw.created_at).toISOString(),
      expiresAt: options?.expiresAt?.toISOString(),
      metadata: options?.metadata,
      url: `/api/v1/blobs/${raw.id}`,
    } as BlobMetadata;
  }

  async download(key: string, options?: BlobDownloadOptions): Promise<string> {
    const headers: Record<string, string> = {};
    if (options?.startByte !== undefined) {
      headers['Range'] = `bytes=${options.startByte}-${options.endByte ?? ''}`;
    }
    // Backend returns raw bytes; try text first, fallback to buffer
    try {
      const response = await this.http.get<string>(`/blobs/${encodeURIComponent(key)}`, {
        responseType: 'text',
        headers,
        signal: options?.signal,
      });
      return response.data;
    } catch (_e) {
      // If text fails, try buffer
      const response = await this.http.get<ArrayBuffer>(`/blobs/${encodeURIComponent(key)}`, {
        responseType: 'buffer',
        headers,
        signal: options?.signal,
      });
      const buf = response.data as unknown as ArrayBuffer;
      if ((globalThis as any).Buffer) {
        return (globalThis as any).Buffer.from(buf as any).toString('utf-8');
      }
      return new TextDecoder().decode(buf as ArrayBuffer);
    }
  }

  // Download as buffer helper
  async downloadBuffer(key: string, options?: BlobDownloadOptions): Promise<ArrayBuffer> {
    const headers: Record<string, string> = {};
    if (options?.startByte !== undefined) {
      headers['Range'] = `bytes=${options.startByte}-${options.endByte ?? ''}`;
    }
    const response = await this.http.get<ArrayBuffer>(`/blobs/${encodeURIComponent(key)}`, {
      responseType: 'buffer',
      headers,
      signal: options?.signal,
    });
    return response.data;
  }

  async del(key: string): Promise<boolean> {
    try {
      await this.http.delete(`/blobs/${encodeURIComponent(key)}`);
      return true;
    } catch (error) {
      if (error instanceof NovaError && error.code === 'NOT_FOUND') return false;
      throw error;
    }
  }

  async multiDel(keys: string[]): Promise<number> {
    // No backend multi-delete — loop
    let deleted = 0;
    await Promise.all(keys.map(async (k) => {
      const ok = await this.del(k);
      if (ok) deleted++;
    }));
    return deleted;
  }

  async list(
    prefix?: string,
    options?: {
      delimiter?: string;
      pagination?: PaginationInput;
      filter?: BlobFilter;
      namespace?: string;
      limit?: number;
      offset?: number;
    }
  ): Promise<Connection<BlobListEntry>> {
    // Map SDK prefix/filter to backend query: namespace, prefix, limit, offset
    let namespace = (options as any)?.namespace ?? 'default';
    let pref = prefix;
    // If prefix looks like "namespace/rest" treat first segment as namespace
    if (prefix && prefix.includes('/') && !(options as any)?.namespace) {
      // Heuristic: if prefix contains '/', consider first part as namespace only when listing via prefix?
      // Keep as prefix; namespace remains default unless explicitly set.
      pref = prefix;
    }
    const query: Record<string, unknown> = { namespace };
    if (pref !== undefined) query.prefix = pref;
    // Pagination: backend uses limit/offset ; SDK uses first/after etc.
    let limit: number | undefined;
    let offset: number | undefined;
    if (options?.limit !== undefined) limit = options.limit;
    else if (options?.pagination?.first !== undefined) limit = options.pagination.first;
    if (options?.offset !== undefined) offset = options.offset;
    else if (options?.pagination?.after !== undefined) {
      const parsed = parseInt(options.pagination.after, 10);
      if (!Number.isNaN(parsed)) offset = parsed;
    }
    if (limit !== undefined) query.limit = limit;
    if (offset !== undefined) query.offset = offset;

    // Filters not server-supported; pass through where possible
    const response = await this.http.get<RawBlobList>('/blobs', { query });
    const raw = response.data;
    let entries: BlobListEntry[] = (raw.data ?? []).map((item) => ({
      key: item.id,
      sizeBytes: item.size_bytes,
      contentType: item.content_type,
      storageTier: 'HOT' as StorageTier,
      createdAt: new Date(item.created_at).toISOString(),
      etag: '',
      isPrefix: false,
    }));
    // Client-side filter if needed
    if (options?.filter?.contentType) {
      entries = entries.filter((e) => e.contentType === options.filter!.contentType);
    }
    if (options?.filter?.minSizeBytes !== undefined) {
      entries = entries.filter((e) => e.sizeBytes >= options.filter!.minSizeBytes!);
    }
    if (options?.filter?.maxSizeBytes !== undefined) {
      entries = entries.filter((e) => e.sizeBytes <= options.filter!.maxSizeBytes!);
    }
    const fallbackPagination = { offset: offset ?? 0, limit: limit ?? entries.length, total: entries.length, has_more: false };
    const pagination = raw.pagination ?? fallbackPagination;
    return toBlobConnection(entries, pagination);
  }

  async info(key: string): Promise<BlobMetadata> {
    const response = await this.http.get<RawBlobInfo>(`/blobs/${encodeURIComponent(key)}/info`);
    const raw = response.data;
    return {
      key: raw.id,
      sizeBytes: raw.size_bytes,
      contentType: raw.content_type,
      etag: raw.checksum_sha256,
      md5: '',
      sha256: raw.checksum_sha256,
      storageTier: 'HOT',
      createdAt: new Date(raw.created_at).toISOString(),
      updatedAt: new Date(raw.created_at).toISOString(),
      metadata: raw.metadata as any,
      url: `/api/v1/blobs/${raw.id}`,
    } as BlobMetadata;
  }

  async copy(_source: string, _destination: string): Promise<BlobMetadata> {
    // No backend copy — implement via download + upload for prototype
    const data = await this.downloadBuffer(_source);
      const buf = (globalThis as any).Buffer ? (globalThis as any).Buffer.from(data) : new Uint8Array(data as ArrayBuffer);
    return this.upload(_destination, buf);
  }

  async move(source: string, destination: string): Promise<BlobMetadata> {
    const meta = await this.copy(source, destination);
    await this.del(source);
    return meta;
  }

  async setTier(key: string, tier: StorageTier): Promise<BlobMetadata> {
    // No backend setTier — return info with updated tier locally
    const meta = await this.info(key);
    return { ...meta, storageTier: tier };
  }

  async setExpiry(key: string, expiresAt: Date): Promise<BlobMetadata> {
    const meta = await this.info(key);
    return { ...meta, expiresAt: expiresAt.toISOString() };
  }

  async removeExpiry(key: string): Promise<BlobMetadata> {
    const meta = await this.info(key);
    return { ...meta, expiresAt: undefined };
  }

  async getStats(): Promise<BlobMetrics> {
    const response = await this.http.get<RawBlobStats>('/blobs/stats');
    const raw = response.data;
    return {
      uploadsTotal: raw.total_blobs,
      downloadsTotal: 0,
      totalBlobs: raw.total_blobs,
      totalStorageBytes: raw.total_bytes,
    };
  }

  async *listIterator(prefix?: string, options?: {
    delimiter?: string;
    filter?: BlobFilter;
    namespace?: string;
  }): AsyncIterable<BlobListEntry> {
    let offset = 0;
    const limit = 100;
    while (true) {
      const page = await this.list(prefix, {
        ...options,
        pagination: { first: limit, after: String(offset) },
      });
      for (const edge of page.edges) yield edge.node;
      if (!page.pageInfo.hasNextPage) break;
      offset += limit;
    }
  }
}
