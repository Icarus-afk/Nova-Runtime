import type { HttpClient } from './client';
import type { QueryResult, Connection, TableInfo, DatabaseMetrics, PaginationInput, CreateTableInput } from './types';

export class DatabaseClient {
  constructor(
    private http: HttpClient
  ) {}

  async query<T = Record<string, unknown>>(
    sql: string,
    params?: unknown[],
    options?: {
      timeoutMs?: number;
      maxRows?: number;
      fetchSize?: number;
    }
  ): Promise<QueryResult<T>> {
    const response = await this.http.post<QueryResult<T>>('/db/query', {
      query: sql,
      params: params ?? [],
      ...options,
    });
    return response.data;
  }

  async execute(
    sql: string,
    params?: unknown[],
    options?: {
      timeoutMs?: number;
      dryRun?: boolean;
    }
  ): Promise<{ affectedRows: number; executionTimeMs: number; lastInsertedId?: string; warnings?: string[] }> {
    const response = await this.http.post<{ affectedRows: number; executionTimeMs: number; lastInsertedId?: string; warnings?: string[] }>('/db/exec', {
      query: sql,
      params: params ?? [],
      ...options,
    });
    return response.data;
  }

  async listTables(options?: {
    schema?: string;
    pattern?: string;
    pagination?: PaginationInput;
  }): Promise<Connection<TableInfo>> {
    const response = await this.http.get<Connection<TableInfo>>('/db/tables', {
      query: { ...(options?.pagination as Record<string, unknown> ?? {}), schema: options?.schema, pattern: options?.pattern },
    });
    return response.data;
  }

  async getTable(name: string): Promise<TableInfo> {
    const response = await this.http.get<TableInfo>(`/db/tables/${encodeURIComponent(name)}`);
    return response.data;
  }

  async createTable(input: CreateTableInput): Promise<TableInfo> {
    const response = await this.http.post<TableInfo>('/db/tables', input);
    return response.data;
  }

  async dropTable(name: string, ifExists?: boolean): Promise<void> {
    await this.http.delete(`/db/tables/${encodeURIComponent(name)}`, {
      query: { ifExists },
    });
  }

  async explain(sql: string, params?: unknown[]): Promise<{ plan: unknown; estimatedRows: number; estimatedCost: number }> {
    const response = await this.http.post<{ plan: unknown; estimatedRows: number; estimatedCost: number }>('/db/explain', { query: sql, params: params ?? [] });
    return response.data;
  }

  async getStats(): Promise<DatabaseMetrics> {
    const response = await this.http.get<DatabaseMetrics>('/db/stats');
    return response.data;
  }
}
