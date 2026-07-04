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
    }): Promise<SearchResponse<T>>;
    suggest(index: string, prefix: string, options?: {
        field?: string;
        size?: number;
    }): Promise<Suggestion[]>;
    listIndexes(options?: PaginationInput): Promise<Connection<SearchIndex>>;
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
    deleteDocument(index: string, id: string): Promise<void>;
    getStats(): Promise<SearchStats>;
    listIndexesIterator(): AsyncIterable<SearchIndex>;
}
