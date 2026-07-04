import type { HttpClient } from './client';
import type { Connection, PaginationInput, CacheMetrics } from './types';
export declare class CacheClient {
    private http;
    constructor(http: HttpClient);
    get<T = unknown>(key: string): Promise<T | null>;
    multiGet<T = unknown>(keys: string[]): Promise<Map<string, T | null>>;
    set<T = unknown>(key: string, value: T, options?: {
        ttlMs?: number;
        nx?: boolean;
    }): Promise<void>;
    multiSet<T = unknown>(entries: Array<{
        key: string;
        value: T;
        ttlMs?: number;
    }>): Promise<void>;
    del(key: string): Promise<boolean>;
    multiDel(keys: string[]): Promise<number>;
    delPattern(pattern: string): Promise<number>;
    keys(pattern?: string, options?: PaginationInput): Promise<Connection<string>>;
    ttl(key: string): Promise<number | null>;
    expire(key: string, ttlMs: number): Promise<boolean>;
    incr(key: string, amount?: number): Promise<number>;
    stats(): Promise<CacheMetrics>;
    flush(): Promise<number>;
    list(pattern?: string): AsyncIterable<string>;
}
