import type { HttpClient } from './client';
import type { QueryResult, Connection, TableInfo, DatabaseMetrics, PaginationInput, CreateTableInput } from './types';
import { Errors } from './errors';

interface RawQueryResponse {
  columns: string[];
  column_names?: string[];
  types?: string[];
  rows: unknown[][];
  row_count: number;
  truncated?: boolean;
  execution_time_ms: number;
  format?: string;
}

interface RawExecuteResponse {
  affected_rows: number;
  execution_time_ms: number;
}

interface RawListTablesResponse {
  data: Array<{ name: string; document_count: number }>;
  pagination: { offset: number; limit: number; total: number; has_more: boolean };
}

interface RawTableSchema {
  table: string;
  columns: Array<{ name: string; type: string; nullable: boolean; is_primary_key: boolean; unique: boolean }>;
}

function toConnection<T>(data: T[], pagination: { offset: number; limit: number; total: number; has_more: boolean }): Connection<T> {
  const edges = data.map((node, idx) => ({
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
      limit?: number;
    }
  ): Promise<QueryResult<T>> {
    const body: Record<string, unknown> = {
      query: sql,
      params: params ?? undefined,
    };
    // Backend honors `limit` for truncation; map maxRows/fetchSize to limit if provided
    if (options?.limit !== undefined) body.limit = options.limit;
    else if (options?.maxRows !== undefined) body.limit = options.maxRows;

    const response = await this.http.post<RawQueryResponse>('/sql/query', body);
    const raw = response.data;

    const colNames: string[] = raw.column_names ?? raw.columns ?? [];
    const types: string[] = raw.types ?? [];

    // Build ColumnInfo[] for compatibility
    const columnInfos = colNames.map((name, i) => ({
      name,
      dataType: types[i] ?? 'text',
      nullable: true,
      primaryKey: false,
      defaultValue: null,
      comment: undefined,
    }));

    // Backend rows are unknown[][] ; map to array of objects for T = Record<string,unknown>
    // If T is generic object, produce objects; otherwise keep raw array form as fallback.
    let rowsAsObjects: unknown[];
    if (colNames.length > 0 && raw.rows.length > 0 && Array.isArray(raw.rows[0])) {
      rowsAsObjects = raw.rows.map((rowArr) => {
        const obj: Record<string, unknown> = {};
        const arr = rowArr as unknown[];
        colNames.forEach((c, idx) => {
          obj[c] = arr[idx];
        });
        return obj;
      });
    } else {
      rowsAsObjects = raw.rows as unknown[];
    }

    const mapped: any = {
      columns: columnInfos,
      // keep raw string column names also available via `column_names`
      column_names: colNames,
      types,
      rows: rowsAsObjects as T[],
      // preserve raw array rows under alternative key for callers expecting raw
      rawRows: raw.rows,
      rowCount: raw.row_count,
      row_count: raw.row_count,
      executionTimeMs: raw.execution_time_ms,
      execution_time_ms: raw.execution_time_ms,
      truncated: raw.truncated ?? false,
      warnings: undefined,
    };
    return mapped as QueryResult<T>;
  }

  async execute(
    sql: string,
    params?: unknown[],
    _options?: {
      timeoutMs?: number;
      dryRun?: boolean;
    }
  ): Promise<{ affectedRows: number; executionTimeMs: number; affected_rows?: number; execution_time_ms?: number; lastInsertedId?: string; warnings?: string[] }> {
    const response = await this.http.post<RawExecuteResponse>('/sql/execute', {
      query: sql,
      params: params ?? undefined,
    });
    const raw = response.data as any;
    // Some codepaths return row_count / executionTime variants; normalize
    const affected = raw.affected_rows ?? raw.affectedRows ?? raw.row_count ?? 0;
    const execMs = raw.execution_time_ms ?? raw.executionTimeMs ?? 0;
    return {
      affectedRows: affected,
      affected_rows: affected,
      executionTimeMs: execMs,
      execution_time_ms: execMs,
    };
  }

  async listTables(options?: {
    schema?: string;
    pattern?: string;
    pagination?: PaginationInput;
    limit?: number;
    offset?: number;
  }): Promise<Connection<TableInfo>> {
    // Backend uses limit/offset ; map from PaginationInput (first/after) or direct limit/offset
    let limit: number | undefined;
    let offset: number | undefined;
    if (options?.limit !== undefined) limit = options.limit;
    else if (options?.pagination?.first !== undefined) limit = options.pagination.first;
    if (options?.offset !== undefined) offset = options.offset;
    else if (options?.pagination?.after !== undefined) {
      const parsed = parseInt(options.pagination.after, 10);
      if (!Number.isNaN(parsed)) offset = parsed;
    }

    // schema / pattern filters are not supported server-side for tables; apply client-side if pattern given
    const query: Record<string, unknown> = {};
    if (limit !== undefined) query.limit = limit;
    if (offset !== undefined) query.offset = offset;

    const response = await this.http.get<RawListTablesResponse>('/sql/tables', { query });
    const raw = response.data;

    // Filter by pattern if provided (client-side)
    let filteredData = raw.data;
    if (options?.pattern) {
      const reStr = '^' + options.pattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\\\*/g, '.*').replace(/\\\?/g, '.') + '$';
      try {
        const re = new RegExp(reStr);
        filteredData = filteredData.filter((t) => re.test(t.name));
      } catch { /* ignore invalid pattern */ }
    }

    // Map to TableInfo[]
    const tableInfos: TableInfo[] = filteredData.map((t) => ({
      name: t.name,
      schema: options?.schema ?? 'public',
      columns: [],
      primaryKey: [],
      indexes: [],
      rowCount: t.document_count,
      sizeBytes: 0,
      createdAt: '',
      updatedAt: '',
    } as TableInfo));

    // If we filtered, pagination totals may be off — keep original pagination unless filtered
    const conn = toConnection<TableInfo>(tableInfos, raw.pagination);
    return conn;
  }

  async getTable(name: string): Promise<TableInfo> {
    const response = await this.http.get<RawTableSchema>(`/sql/tables/${encodeURIComponent(name)}/schema`);
    const raw = response.data;
    const columns = (raw.columns ?? []).map((c) => ({
      name: c.name,
      dataType: c.type,
      nullable: c.nullable,
      primaryKey: c.is_primary_key,
      defaultValue: null,
      comment: undefined,
    }));
    const primaryKey = (raw.columns ?? []).filter((c) => c.is_primary_key).map((c) => c.name);
    return {
      name: raw.table ?? name,
      schema: 'public',
      columns,
      primaryKey,
      indexes: [],
      rowCount: 0,
      sizeBytes: 0,
      createdAt: '',
      updatedAt: '',
    } as TableInfo;
  }

  async createTable(input: CreateTableInput): Promise<TableInfo> {
    // No dedicated REST endpoint — synthesize via SQL execute (matches dashboard)
    const cols = input.columns.map((c) => {
      const defaultClause = (c as any).defaultValue !== undefined && (c as any).defaultValue !== null
        ? (() => {
            const d = String((c as any).defaultValue).trim();
            if (/^-?\d+(\.\d+)?$/.test(d) || /^(TRUE|FALSE|NULL|CURRENT_TIMESTAMP)$/i.test(d)) return ` DEFAULT ${d}`;
            return ` DEFAULT '${d.replace(/'/g, "''")}'`;
          })()
        : '';
      const pk = (c as any).primaryKey ? ' PRIMARY KEY' : '';
      const uniq = (c as any).unique ? ' UNIQUE' : '';
      const nn = c.nullable === false ? ' NOT NULL' : '';
      return `${c.name} ${c.type}${pk}${uniq}${nn}${defaultClause}`;
    }).join(', ');
    const pkClause = input.primaryKey && input.primaryKey.length > 0 && !input.columns.some((c: any) => c.primaryKey)
      ? `, PRIMARY KEY (${input.primaryKey.join(', ')})`
      : '';
    const ifNotExists = input.ifNotExists ? 'IF NOT EXISTS ' : '';
    const sql = `CREATE TABLE ${ifNotExists}${input.name} (${cols}${pkClause})`;
    await this.execute(sql);
    // Return minimal TableInfo
    return {
      name: input.name,
      schema: 'public',
      columns: input.columns.map((c) => ({
        name: c.name,
        dataType: c.type,
        nullable: c.nullable ?? true,
        primaryKey: (c as any).primaryKey ?? false,
        defaultValue: (c as any).defaultValue ?? null,
      })),
      primaryKey: input.primaryKey ?? [],
      indexes: [],
      rowCount: 0,
      sizeBytes: 0,
      createdAt: new Date().toISOString(),
      updatedAt: new Date().toISOString(),
    } as unknown as TableInfo;
  }

  async dropTable(name: string, _ifExists?: boolean): Promise<void> {
    const clause = _ifExists ? 'IF EXISTS ' : '';
    await this.execute(`DROP TABLE ${clause}${name}`);
  }

  async explain(sql: string, params?: unknown[]): Promise<{ plan: unknown; estimatedRows: number; estimatedCost: number }> {
    // No backend explain route — fallback to query with EXPLAIN prefix or throw
    try {
      const result = await this.query(`EXPLAIN ${sql}`, params);
      return { plan: (result as any).rows, estimatedRows: (result as any).rowCount ?? 0, estimatedCost: 0 };
    } catch (e) {
      throw Errors.notFound(`EXPLAIN not supported for: ${sql} — ${String((e as Error).message ?? e)}`);
    }
  }

  async getStats(): Promise<DatabaseMetrics> {
    // No dedicated stats route — return placeholder derived from tables or throw minimal
    // Return zeroed metrics to keep prototype usable
    return {
      queriesTotal: 0,
      queriesPerSecond: 0,
      avgLatencyMs: 0,
      p50LatencyMs: 0,
      p95LatencyMs: 0,
      p99LatencyMs: 0,
      activeConnections: 0,
      cacheHitRate: 0,
    };
  }
}
