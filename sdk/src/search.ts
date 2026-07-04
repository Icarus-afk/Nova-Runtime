import type { HttpClient } from './client';
import type {
  SearchIndex, SearchResponse, Suggestion, CreateIndexInput,
  SearchFilter, SearchSort, SearchStats, Connection, PaginationInput
} from './types';

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
    }
  ): Promise<SearchResponse<T>> {
    const response = await this.http.post<SearchResponse<T>>(
      `/search/${encodeURIComponent(index)}/query`, {
        query,
        ...options,
        ...options?.pagination,
      }
    );
    return response.data;
  }

  async suggest(
    index: string,
    prefix: string,
    options?: { field?: string; size?: number }
  ): Promise<Suggestion[]> {
    const response = await this.http.get<Suggestion[]>(
      `/search/${encodeURIComponent(index)}/suggest`, {
        query: { prefix, ...options },
      }
    );
    return response.data;
  }

  async listIndexes(options?: PaginationInput): Promise<Connection<SearchIndex>> {
    const response = await this.http.get<Connection<SearchIndex>>('/search/indexes', {
      query: options as Record<string, unknown>,
    });
    return response.data;
  }

  async getIndex(name: string): Promise<SearchIndex> {
    const response = await this.http.get<SearchIndex>(`/search/indexes/${encodeURIComponent(name)}`);
    return response.data;
  }

  async createIndex(input: CreateIndexInput): Promise<SearchIndex> {
    const response = await this.http.post<SearchIndex>('/search/indexes', input);
    return response.data;
  }

  async deleteIndex(name: string): Promise<void> {
    await this.http.delete(`/search/indexes/${encodeURIComponent(name)}`);
  }

  async indexDocument<T = Record<string, unknown>>(
    index: string,
    document: T,
    id?: string
  ): Promise<{ id: string; indexed: boolean }> {
    const response = await this.http.post<{ id: string; indexed: boolean }>(
      `/search/${encodeURIComponent(index)}/documents`, { document, id }
    );
    return response.data;
  }

  async indexDocuments<T = Record<string, unknown>>(
    index: string,
    documents: Array<{ id?: string; document: T }>
  ): Promise<{ indexedCount: number; failedCount: number; errors?: string[] }> {
    const response = await this.http.post<{ indexedCount: number; failedCount: number; errors?: string[] }>(
      `/search/${encodeURIComponent(index)}/documents/batch`, { documents }
    );
    return response.data;
  }

  async deleteDocument(index: string, id: string): Promise<void> {
    await this.http.delete(`/search/${encodeURIComponent(index)}/documents/${id}`);
  }

  async getStats(): Promise<SearchStats> {
    const response = await this.http.get<SearchStats>('/search/stats');
    return response.data;
  }

  async *listIndexesIterator(): AsyncIterable<SearchIndex> {
    let cursor: string | undefined;
    let hasMore = true;
    while (hasMore) {
      const page = await this.listIndexes({ first: 100, after: cursor });
      for (const edge of page.edges) {
        yield edge.node;
      }
      hasMore = page.pageInfo.hasNextPage;
      cursor = page.pageInfo.endCursor ?? undefined;
    }
  }
}
