import type { HttpClient } from './client';
import type { HealthStatus, MetricsSnapshot, Connection, ConnectionInfo, PaginationInput } from './types';

export class RuntimeClient {
  constructor(
    private http: HttpClient
  ) {}

  async health(): Promise<HealthStatus> {
    const response = await this.http.get<HealthStatus>('/runtime/health');
    return response.data;
  }

  async getConfig(key?: string, subsystem?: string): Promise<Record<string, unknown>> {
    const response = await this.http.get<Record<string, unknown>>('/runtime/config', {
      query: { key, subsystem },
    });
    return response.data;
  }

  async updateConfig(key: string, value: unknown): Promise<Record<string, unknown>> {
    const response = await this.http.post<Record<string, unknown>>('/runtime/config', {
      key, value,
    });
    return response.data;
  }

  async getMetrics(options?: {
    since?: Date;
    resolution?: '1s' | '1m' | '5m' | '15m' | '1h';
  }): Promise<MetricsSnapshot> {
    const response = await this.http.get<MetricsSnapshot>('/runtime/metrics', {
      query: {
        since: options?.since?.toISOString(),
        resolution: options?.resolution,
      },
    });
    return response.data;
  }

  async getVersion(): Promise<{ version: string; buildCommit: string; buildDate: string }> {
    const response = await this.http.get<{ version: string; buildCommit: string; buildDate: string }>(
      '/runtime/version'
    );
    return response.data;
  }

  async listConnections(options?: {
    subsystem?: string;
    status?: string;
    pagination?: PaginationInput;
  }): Promise<Connection<ConnectionInfo>> {
    const response = await this.http.get<Connection<ConnectionInfo>>('/runtime/connections', {
      query: {
        subsystem: options?.subsystem,
        status: options?.status,
        ...(options?.pagination as Record<string, unknown> ?? {}),
      },
    });
    return response.data;
  }
}
