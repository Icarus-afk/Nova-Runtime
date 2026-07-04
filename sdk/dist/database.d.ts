import type { HttpClient } from './client';
import type { QueryResult, Connection, TableInfo, DatabaseMetrics, PaginationInput, CreateTableInput } from './types';
export declare class DatabaseClient {
    private http;
    constructor(http: HttpClient);
    query<T = Record<string, unknown>>(sql: string, params?: unknown[], options?: {
        timeoutMs?: number;
        maxRows?: number;
        fetchSize?: number;
    }): Promise<QueryResult<T>>;
    execute(sql: string, params?: unknown[], options?: {
        timeoutMs?: number;
        dryRun?: boolean;
    }): Promise<{
        affectedRows: number;
        executionTimeMs: number;
        lastInsertedId?: string;
        warnings?: string[];
    }>;
    listTables(options?: {
        schema?: string;
        pattern?: string;
        pagination?: PaginationInput;
    }): Promise<Connection<TableInfo>>;
    getTable(name: string): Promise<TableInfo>;
    createTable(input: CreateTableInput): Promise<TableInfo>;
    dropTable(name: string, ifExists?: boolean): Promise<void>;
    explain(sql: string, params?: unknown[]): Promise<{
        plan: unknown;
        estimatedRows: number;
        estimatedCost: number;
    }>;
    getStats(): Promise<DatabaseMetrics>;
}
