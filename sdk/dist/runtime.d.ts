import type { HttpClient } from './client';
import type { HealthStatus, MetricsSnapshot, Connection, ConnectionInfo, PaginationInput } from './types';
export declare class RuntimeClient {
    private http;
    constructor(http: HttpClient);
    health(): Promise<HealthStatus>;
    getConfig(key?: string, subsystem?: string): Promise<Record<string, unknown>>;
    updateConfig(key: string, value: unknown): Promise<Record<string, unknown>>;
    getMetrics(options?: {
        since?: Date;
        resolution?: '1s' | '1m' | '5m' | '15m' | '1h';
    }): Promise<MetricsSnapshot>;
    getVersion(): Promise<{
        version: string;
        buildCommit: string;
        buildDate: string;
    }>;
    listConnections(options?: {
        subsystem?: string;
        status?: string;
        pagination?: PaginationInput;
    }): Promise<Connection<ConnectionInfo>>;
}
