import type { HttpClient } from './client';
import type {
  SearchIndex, SearchResponse, Suggestion, CreateIndexInput,
  SearchFilter, SearchSort, SearchStats, Connection, PaginationInput
} from './types';
import { Errors } from './errors';

interface RawListIndexesResponse {
  data: Array<{ name: string; fields?: any[]; doc_count?: number; field_count?: number; created_at?: number }>;
  pagination: { offset: number; limit: number; total: number; has_more: boolean };
}

interface RawSearchResponse {
  hits: Array<{ id: string; score: number; source?: Record<string, unknown>; fields?: Record<string, unknown> }>;
  total_hits: number;
  offset?: number;
  limit?: number;
  execution_time_ms?: number;
}

interface RawIndexGet {
  name: string;
  fields: any[];
  num_docs: number;
  num_terms: number;
  field_count: number;
  created_at?: number;
}

function toIndexConnection(indexes: SearchIndex[], pagination: { offset: number; limit: number; total: number; has_more: boolean }): Connection<SearchIndex> {
  const edges = indexes.map((node, idx) => ({
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

export class SearchClient {
  constructor(
    private http: HttpClient
  ) {}

  async search<T = Record<string, unknown>>(
    index: string,
    query: string,
    options?: {
      pagination?: PaginationInput;
      filters?: SearchFilter[];
      sort?: SearchSort;
      fields?: string[];
      highlight?: string[];
      minScore?: number;
      explain?: boolean;
      limit?: number;
      offset?: number;
    }
  ): Promise<SearchResponse<T>> {
    // Map pagination: first/after or limit/offset
    let limit: number | undefined;
    let offset: number | undefined;
    if (options?.limit !== undefined) limit = options.limit;
    else if (options?.pagination?.first !== undefined) limit = options.pagination.first;
    else if ((options as any)?.first !== undefined) limit = (options as any).first;

    if (options?.offset !== undefined) offset = options.offset;
    else if (options?.pagination?.after !== undefined) {
      const parsed = parseInt(options.pagination.after, 10);
      if (!Number.isNaN(parsed)) offset = parsed;
    }

    const body: Record<string, unknown> = { query };
    if (limit !== undefined) body.limit = limit;
    if (offset !== undefined) body.offset = offset;

    const response = await this.http.post<RawSearchResponse>(
      `/search/indexes/${encodeURIComponent(index)}/query`, body
    );
    const raw = response.data;
    const edges = (raw.hits ?? []).map((h) => ({
      node: (h.source ?? h.fields ?? {}) as T,
      cursor: h.id,
      score: h.score,
      highlight: undefined,
    }));
    return {
      edges,
      pageInfo: {
        hasNextPage: (raw.offset ?? 0) + (raw.limit ?? edges.length) < raw.total_hits,
        hasPreviousPage: (raw.offset ?? 0) > 0,
        startCursor: edges[0]?.cursor ?? null,
        endCursor: edges[edges.length - 1]?.cursor ?? null,
      },
      totalCount: raw.total_hits,
      maxScore: edges.length ? Math.max(...edges.map((e) => e.score)) : 0,
      tookMs: raw.execution_time_ms ?? 0,
      aggregations: undefined,
    };
  }

  async suggest(
    _index: string,
    _prefix: string,
    _options?: { field?: string; size?: number }
  ): Promise<Suggestion[]> {
    // No backend suggest route — return empty for prototype
    return [];
  }

  async listIndexes(options?: PaginationInput & { limit?: number; offset?: number }): Promise<Connection<SearchIndex>> {
    const query: Record<string, unknown> = {};
    if ((options as any)?.limit !== undefined) query.limit = (options as any).limit;
    else if (options?.first !== undefined) query.limit = options.first;
    if ((options as any)?.offset !== undefined) query.offset = (options as any).offset;
    else if (options?.after !== undefined) {
      const parsed = parseInt(options.after, 10);
      if (!Number.isNaN(parsed)) query.offset = parsed;
    }
    if (options?.last !== undefined && query.limit === undefined) query.limit = options.last;

    const response = await this.http.get<RawListIndexesResponse>('/search/indexes', { query });
    const raw = response.data;
    const indexes: SearchIndex[] = (raw.data ?? []).map((idx) => ({
      name: idx.name,
      documentCount: idx.doc_count ?? 0,
      sizeBytes: 0,
      fieldCount: idx.field_count ?? (idx.fields?.length ?? 0),
      analyzer: 'standard',
      createdAt: idx.created_at ? new Date(idx.created_at).toISOString() : new Date().toISOString(),
      updatedAt: idx.created_at ? new Date(idx.created_at).toISOString() : new Date().toISOString(),
      fields: (idx.fields ?? []).map((f: any) => ({
        name: f.name,
        type: String(f.type ?? 'TEXT').toUpperCase() as any,
        searchable: true,
        sortable: false,
        facetable: false,
        stored: true,
        analyzer: f.analyzer,
        boost: f.boost ?? 1,
      })),
    }));
    return toIndexConnection(indexes, raw.pagination);
  }

  async getIndex(name: string): Promise<SearchIndex> {
    const response = await this.http.get<RawIndexGet>(`/search/indexes/${encodeURIComponent(name)}`);
    const raw = response.data;
    return {
      name: raw.name,
      documentCount: raw.num_docs ?? 0,
      sizeBytes: 0,
      fieldCount: raw.field_count ?? (raw.fields?.length ?? 0),
      analyzer: 'standard',
      createdAt: raw.created_at ? new Date(raw.created_at).toISOString() : new Date().toISOString(),
      updatedAt: raw.created_at ? new Date(raw.created_at).toISOString() : new Date().toISOString(),
      fields: (raw.fields ?? []).map((f: any) => ({
        name: f.name,
        type: String(f.type ?? 'TEXT').toUpperCase() as any,
        searchable: true,
        sortable: false,
        facetable: false,
        stored: true,
        analyzer: f.analyzer,
        boost: f.boost ?? 1,
      })),
    };
  }

  async createIndex(input: CreateIndexInput): Promise<SearchIndex> {
    // Backend expects { name, fields?: [{name, type, analyzer?, boost?}] }
    // Map field type to lowercase as backend validates lower-case set
    const fields = (input as any).fields ?? (input as any).fieldsInput ?? [];
    const mappedFields = (fields as any[]).map((f: any) => ({
      name: f.name,
      type: String(f.type ?? 'text').toLowerCase(),
      analyzer: f.analyzer,
      boost: f.boost,
    }));
    const body: Record<string, unknown> = { name: input.name };
    if (mappedFields.length > 0) body.fields = mappedFields;
    if ((input as any).analyzer) body.fields = mappedFields; // ignore top-level analyzer for compat
    const response = await this.http.post<any>('/search/indexes', body);
    const raw = response.data;
    return {
      name: raw.name ?? input.name,
      documentCount: 0,
      sizeBytes: 0,
      fieldCount: mappedFields.length,
      analyzer: (input as any).analyzer ?? 'standard',
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      fields: mappedFields.map((f: any) => ({
        name: f.name,
        type: String(f.type).toUpperCase() as any,
        searchable: f.searchable ?? true,
        sortable: f.sortable ?? false,
        facetable: f.facetable ?? false,
        stored: f.stored ?? true,
        analyzer: f.analyzer,
        boost: f.boost ?? 1,
      })),
    };
  }

  async deleteIndex(name: string): Promise<void> {
    await this.http.delete(`/search/indexes/${encodeURIComponent(name)}`);
  }

  async indexDocument<T = Record<string, unknown>>(
    index: string,
    document: T,
    id?: string
  ): Promise<{ id: string; indexed: boolean }> {
    const docWithId: any = { ...(document as any) };
    if (id) docWithId.id = id;
    const response = await this.http.post<{ status: string; index: string; count: number }>(
      `/search/indexes/${encodeURIComponent(index)}/documents`, { documents: [docWithId] }
    );
    return { id: id ?? docWithId.id ?? String(Date.now()), indexed: response.data.count > 0 };
  }

  async indexDocuments<T = Record<string, unknown>>(
    index: string,
    documents: Array<{ id?: string; document: T }>
  ): Promise<{ indexedCount: number; failedCount: number; errors?: string[] }> {
    const payloadDocs = documents.map((d) => {
      const obj: any = { ...(d.document as any) };
      if (d.id) obj.id = d.id;
      return obj;
    });
    const response = await this.http.post<{ status: string; index: string; count: number }>(
      `/search/indexes/${encodeURIComponent(index)}/documents`, { documents: payloadDocs }
    );
    return { indexedCount: response.data.count, failedCount: 0 };
  }

  // Alternative batch signature where caller passes raw documents[]
  async addDocuments<T = Record<string, unknown>>(
    index: string,
    documents: T[]
  ): Promise<{ status: string; count: number }> {
    const response = await this.http.post<{ status: string; index: string; count: number }>(
      `/search/indexes/${encodeURIComponent(index)}/documents`, { documents }
    );
    return response.data;
  }

  async deleteDocument(_index: string, _id: string): Promise<void> {
    // No backend route for per-document delete
    throw Errors.notFound('deleteDocument not supported by backend — delete and re-create index');
  }

  async getStats(): Promise<SearchStats> {
    // No global stats route — aggregate from listIndexes
    const conn = await this.listIndexes({ first: 100 } as any);
    let totalDocuments = 0;
    for (const idx of conn.edges) totalDocuments += idx.node.documentCount;
    return {
      totalIndexes: conn.totalCount,
      totalDocuments,
      totalSizeBytes: 0,
      avgIndexTimeMs: 0,
      avgQueryTimeMs: 0,
      p95QueryTimeMs: 0,
      queriesTotal: 0,
      indexingTotal: totalDocuments,
    };
  }

  async getIndexStats(name: string): Promise<{ num_docs: number; num_terms: number; field_count: number }> {
    const response = await this.http.get<{ num_docs: number; num_terms: number; field_count: number }>(`/search/indexes/${encodeURIComponent(name)}/stats`);
    return response.data;
  }

  async *listIndexesIterator(): AsyncIterable<SearchIndex> {
    let offset = 0;
    const limit = 100;
    while (true) {
      const page = await this.listIndexes({ first: limit, after: String(offset) } as any);
      for (const edge of page.edges) yield edge.node;
      if (!page.pageInfo.hasNextPage) break;
      offset += limit;
    }
  }
}
