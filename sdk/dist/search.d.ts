import type { HttpClient } from './client';
import type { SearchIndex, SearchResponse, Suggestion, CreateIndexInput, SearchFilter, SearchSort, SearchStats, Connection, PaginationInput } from './types';
export declare class SearchClient {
    private http;
    constructor(http: HttpClient);
    search<T = Record<string, unknown>>(index: string, query: string, options?: {
        pagination?: PaginationInput;
        filters?: SearchFilter[];
        sort?: SearchSort;
        fields?: string[];
        highlight?: string[];
        minScore?: number;
        explain?: boolean;
        limit?: number;
        offset?: number;
    }): Promise<SearchResponse<T>>;
    suggest(_index: string, _prefix: string, _options?: {
        field?: string;
        size?: number;
    }): Promise<Suggestion[]>;
    listIndexes(options?: PaginationInput & {
        limit?: number;
        offset?: number;
    }): Promise<Connection<SearchIndex>>;
    getIndex(name: string): Promise<SearchIndex>;
    createIndex(input: CreateIndexInput): Promise<SearchIndex>;
    deleteIndex(name: string): Promise<void>;
    indexDocument<T = Record<string, unknown>>(index: string, document: T, id?: string): Promise<{
        id: string;
        indexed: boolean;
    }>;
    indexDocuments<T = Record<string, unknown>>(index: string, documents: Array<{
        id?: string;
        document: T;
    }>): Promise<{
        indexedCount: number;
        failedCount: number;
        errors?: string[];
    }>;
    addDocuments<T = Record<string, unknown>>(index: string, documents: T[]): Promise<{
        status: string;
        count: number;
    }>;
    deleteDocument(_index: string, _id: string): Promise<void>;
    getStats(): Promise<SearchStats>;
    getIndexStats(name: string): Promise<{
        num_docs: number;
        num_terms: number;
        field_count: number;
    }>;
    listIndexesIterator(): AsyncIterable<SearchIndex>;
}
