import type { HealthStatus } from './types';
import { type RetryConfig } from './errors';
import { RuntimeClient } from './runtime';
import { DatabaseClient } from './database';
import { CacheClient } from './cache';
import { QueueClient } from './queue';
import { SchedulerClient } from './scheduler';
import { SearchClient } from './search';
import { BlobClient } from './blob';
import { AuthClient } from './auth';
export type { ErrorCode, ErrorExtensions, NovaErrorParams, RetryConfig } from './errors';
export interface AuthProvider {
    getToken(): Promise<string>;
    onUnauthorized?(): Promise<boolean>;
    dispose?(): Promise<void>;
}
export interface HttpRequest {
    method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE' | 'HEAD' | 'OPTIONS';
    path: string;
    query?: Record<string, unknown>;
    headers?: Record<string, string>;
    body?: unknown;
    responseType?: 'json' | 'text' | 'stream' | 'buffer';
    timeoutMs?: number;
    signal?: AbortSignal;
    priority?: 'background' | 'normal' | 'high';
    idempotencyKey?: string;
}
export interface HttpResponse<T = unknown> {
    status: number;
    headers: Record<string, string>;
    data: T;
    requestId: string;
    durationMs: number;
}
export interface HttpClient {
    request<T = unknown>(req: HttpRequest): Promise<HttpResponse<T>>;
    get<T = unknown>(path: string, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    post<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    put<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    patch<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    delete<T = unknown>(path: string, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    dispose(): Promise<void>;
}
export declare class FetchHttpClient implements HttpClient {
    private config;
    private authProvider;
    private baseUrl;
    private defaultHeaders;
    private retryPolicy;
    private retryConfig;
    constructor(config: Required<NovaClientConfig>, authProvider: AuthProvider);
    request<T = unknown>(req: HttpRequest): Promise<HttpResponse<T>>;
    get<T = unknown>(path: string, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    post<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    put<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    patch<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    delete<T = unknown>(path: string, options?: Partial<HttpRequest>): Promise<HttpResponse<T>>;
    dispose(): Promise<void>;
    private buildUrl;
}
export interface ServerConfig {
    host?: string;
    port?: number;
    protocol?: 'http' | 'https';
    basePath?: string;
    timeout?: number;
}
export interface TransportConfig {
    maxConcurrent?: number;
    poolSize?: number;
    keepAliveMs?: number;
    userAgent?: string;
    defaultHeaders?: Record<string, string>;
}
export type AuthConfig = {
    type: 'token';
    token: string;
} | {
    type: 'api-key';
    apiKey: string;
    apiKeyName?: string;
} | {
    type: 'refresh';
    clientId: string;
    clientSecret: string;
} | {
    type: 'none';
};
export interface NovaClientConfig {
    server?: ServerConfig;
    auth: AuthConfig;
    transport?: TransportConfig;
    retry?: RetryConfig;
}
export declare class NovaClient {
    readonly runtime: RuntimeClient;
    readonly db: DatabaseClient;
    readonly cache: CacheClient;
    readonly queue: QueueClient;
    readonly scheduler: SchedulerClient;
    readonly search: SearchClient;
    readonly blob: BlobClient;
    readonly auth: AuthClient;
    private readonly config;
    private readonly httpClient;
    private readonly authProvider;
    constructor(config: NovaClientConfig);
    health(): Promise<HealthStatus>;
    dispose(): Promise<void>;
    private resolveConfig;
    private createAuthProvider;
}
export declare function createClient(config: NovaClientConfig): NovaClient;
