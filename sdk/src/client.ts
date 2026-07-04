import fetch from 'cross-fetch';
import type { HealthStatus } from './types';
import { NovaError, Errors, createRetryPolicy, type RetryConfig, type RetryPolicy } from './errors';
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

class TokenAuthProvider implements AuthProvider {
  constructor(private token: string) {}

  async getToken(): Promise<string> {
    return this.token;
  }
}

class RefreshAuthProvider implements AuthProvider {
  private accessToken: string | null = null;
  private refreshTokenValue: string;
  private expiresAt: number = 0;
  private refreshPromise: Promise<boolean> | null = null;

  constructor(
    private config: { clientId: string; clientSecret: string },
    private httpClient: HttpClient,
    refreshToken?: string
  ) {
    this.refreshTokenValue = refreshToken ?? '';
  }

  async getToken(): Promise<string> {
    if (!this.accessToken || Date.now() >= this.expiresAt - 60000) {
      const refreshed = await this.performRefresh();
      if (!refreshed) {
        throw Errors.unauthorized('Unable to obtain access token');
      }
    }
    return this.accessToken!;
  }

  async onUnauthorized(): Promise<boolean> {
    return this.performRefresh();
  }

  private async performRefresh(): Promise<boolean> {
    if (this.refreshPromise) {
      return this.refreshPromise;
    }
    this.refreshPromise = this._doRefresh();
    try {
      return await this.refreshPromise;
    } finally {
      this.refreshPromise = null;
    }
  }

  private async _doRefresh(): Promise<boolean> {
    try {
      const response = await this.httpClient.request<{ access_token: string; refresh_token?: string; expires_in: number }>({
        method: 'POST',
        path: '/auth/token',
        body: {
          grant_type: 'client_credentials',
          client_id: this.config.clientId,
          client_secret: this.config.clientSecret,
          refresh_token: this.refreshTokenValue || undefined,
        },
      });
      this.accessToken = response.data.access_token;
      this.refreshTokenValue = response.data.refresh_token ?? this.refreshTokenValue;
      this.expiresAt = Date.now() + response.data.expires_in * 1000;
      return true;
    } catch {
      return false;
    }
  }
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

function generateId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 9)}`;
}

export class FetchHttpClient implements HttpClient {
  private baseUrl: string;
  private defaultHeaders: Record<string, string>;
  private retryPolicy: RetryPolicy;
  private retryConfig: Required<RetryConfig>;

  constructor(
    private config: Required<NovaClientConfig>,
    private authProvider: AuthProvider
  ) {
    const protocol = config.server.protocol || 'https';
    const host = config.server.host || 'localhost';
    const port = config.server.port || 8443;
    const basePath = config.server.basePath || '/v1';
    this.baseUrl = `${protocol}://${host}:${port}${basePath}`;
    this.defaultHeaders = {
      'Content-Type': 'application/json',
      'User-Agent': config.transport?.userAgent || '@novaruntime/sdk/0.1.0',
      ...config.transport?.defaultHeaders,
    };
    this.retryConfig = {
      maxRetries: config.retry?.maxRetries ?? 3,
      baseDelayMs: config.retry?.baseDelayMs ?? 1000,
      maxDelayMs: config.retry?.maxDelayMs ?? 30000,
      retryableStatuses: config.retry?.retryableStatuses ?? [429, 500, 502, 503, 504],
      retryableErrors: config.retry?.retryableErrors ?? [],
      strategy: config.retry?.strategy ?? 'exponential',
      jitterFactor: config.retry?.jitterFactor ?? 0.2,
    };
    this.retryPolicy = createRetryPolicy(this.retryConfig);
  }

  async request<T = unknown>(req: HttpRequest): Promise<HttpResponse<T>> {
    const timeoutMs = req.timeoutMs ?? this.config.server?.timeout ?? 30000;
    let attempt = 0;

    while (true) {
      const requestId = generateId();
      const token = await this.authProvider.getToken();
      const url = this.buildUrl(req.path, req.query);
      const headers: Record<string, string> = {
        ...this.defaultHeaders,
        'Authorization': `Bearer ${token}`,
        'X-Request-ID': requestId,
        ...req.headers,
      };

      if (req.idempotencyKey) {
        headers['Idempotency-Key'] = req.idempotencyKey;
      }

      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
      const signal = req.signal ? combineAbortSignals(req.signal, controller.signal) : controller.signal;

      const startTime = Date.now();

      try {
        const body = req.body !== undefined ? JSON.stringify(req.body) : undefined;
        const fetchInit: RequestInit & { headers: Record<string, string> } = {
          method: req.method,
          headers,
          signal,
        };
        if (body) {
          fetchInit.body = body;
        }

        const response = await fetch(url, fetchInit);
        clearTimeout(timeoutId);

        const durationMs = Date.now() - startTime;
        const responseHeaders: Record<string, string> = {};
        response.headers.forEach((value, key) => {
          responseHeaders[key] = value;
        });

        if (!response.ok) {
          let responseBody: any;
          try {
            responseBody = await response.json();
          } catch {
            responseBody = { message: await response.text() };
          }

          const error = Errors.fromHttpStatus(response.status, responseBody, requestId);

          if (error.code === 'TOKEN_EXPIRED' || (error.code === 'UNAUTHENTICATED' && this.authProvider.onUnauthorized)) {
            const refreshed = await this.authProvider.onUnauthorized!();
            if (refreshed) {
              attempt++;
              continue;
            }
          }

          if (this.retryPolicy.shouldRetry(error, attempt)) {
            const delay = this.retryPolicy.getDelay(error, attempt);
            await sleep(delay);
            attempt++;
            continue;
          }

          throw error;
        }

        let data: T;
        if (req.responseType === 'text') {
          data = (await response.text()) as unknown as T;
        } else if (req.responseType === 'buffer') {
          data = (await response.arrayBuffer()) as unknown as T;
        } else {
          data = await response.json() as T;
        }

        return {
          status: response.status,
          headers: responseHeaders,
          data,
          requestId,
          durationMs,
        };
      } catch (error: any) {
        clearTimeout(timeoutId);

        if (error instanceof NovaError) {
          throw error;
        }

        if (error?.name === 'AbortError') {
          const novaError = Errors.timeout('Request timed out');
          if (this.retryPolicy.shouldRetry(novaError, attempt)) {
            const delay = this.retryPolicy.getDelay(novaError, attempt);
            await sleep(delay);
            attempt++;
            continue;
          }
          throw novaError;
        }

        const novaError = Errors.connectionError(error?.message ?? 'Unknown error');
        if (this.retryPolicy.shouldRetry(novaError, attempt)) {
          const delay = this.retryPolicy.getDelay(novaError, attempt);
          await sleep(delay);
          attempt++;
          continue;
        }
        throw novaError;
      }
    }
  }

  async get<T = unknown>(path: string, options?: Partial<HttpRequest>): Promise<HttpResponse<T>> {
    return this.request<T>({ method: 'GET', path, ...options });
  }

  async post<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>> {
    return this.request<T>({ method: 'POST', path, body, ...options });
  }

  async put<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>> {
    return this.request<T>({ method: 'PUT', path, body, ...options });
  }

  async patch<T = unknown>(path: string, body?: unknown, options?: Partial<HttpRequest>): Promise<HttpResponse<T>> {
    return this.request<T>({ method: 'PATCH', path, body, ...options });
  }

  async delete<T = unknown>(path: string, options?: Partial<HttpRequest>): Promise<HttpResponse<T>> {
    return this.request<T>({ method: 'DELETE', path, ...options });
  }

  async dispose(): Promise<void> {
  }

  private buildUrl(path: string, query?: Record<string, unknown>): string {
    const url = `${this.baseUrl}/${path.replace(/^\//, '')}`;
    if (!query) return url;

    const params = new URLSearchParams();
    for (const [key, value] of Object.entries(query)) {
      if (value === undefined || value === null) continue;
      if (Array.isArray(value)) {
        for (const v of value) {
          params.append(key, v);
        }
      } else {
        params.set(key, String(value));
      }
    }

    const qs = params.toString();
    return qs ? `${url}?${qs}` : url;
  }
}

function combineAbortSignals(...signals: AbortSignal[]): AbortSignal {
  const controller = new AbortController();
  for (const signal of signals) {
    if (signal.aborted) {
      controller.abort();
      return controller.signal;
    }
    signal.addEventListener('abort', () => controller.abort(), { once: true });
  }
  return controller.signal;
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
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

export type AuthConfig =
  | { type: 'token'; token: string }
  | { type: 'api-key'; apiKey: string; apiKeyName?: string }
  | { type: 'refresh'; clientId: string; clientSecret: string }
  | { type: 'none' };

export interface NovaClientConfig {
  server?: ServerConfig;
  auth: AuthConfig;
  transport?: TransportConfig;
  retry?: RetryConfig;
}

export class NovaClient {
  public readonly runtime: RuntimeClient;
  public readonly db: DatabaseClient;
  public readonly cache: CacheClient;
  public readonly queue: QueueClient;
  public readonly scheduler: SchedulerClient;
  public readonly search: SearchClient;
  public readonly blob: BlobClient;
  public readonly auth: AuthClient;

  private readonly config: Required<NovaClientConfig>;
  private readonly httpClient: HttpClient;
  private readonly authProvider: AuthProvider;

  constructor(config: NovaClientConfig) {
    this.config = this.resolveConfig(config);
    this.authProvider = this.createAuthProvider();
    this.httpClient = new FetchHttpClient(this.config, this.authProvider);

    this.runtime = new RuntimeClient(this.httpClient);
    this.db = new DatabaseClient(this.httpClient);
    this.cache = new CacheClient(this.httpClient);
    this.queue = new QueueClient(this.httpClient);
    this.scheduler = new SchedulerClient(this.httpClient);
    this.search = new SearchClient(this.httpClient);
    this.blob = new BlobClient(this.httpClient);
    this.auth = new AuthClient(this.httpClient);
  }

  async health(): Promise<HealthStatus> {
    return this.runtime.health();
  }

  async dispose(): Promise<void> {
    await this.httpClient.dispose();
    await this.authProvider.dispose?.();
  }

  private resolveConfig(config: NovaClientConfig): Required<NovaClientConfig> {
    return deepMerge(DEFAULT_CONFIG, config) as Required<NovaClientConfig>;
  }

  private createAuthProvider(): AuthProvider {
    switch (this.config.auth.type) {
      case 'token':
        return new TokenAuthProvider(this.config.auth.token);
      case 'api-key':
        return new TokenAuthProvider(this.config.auth.apiKey);
      case 'refresh':
        return new RefreshAuthProvider(
          { clientId: this.config.auth.clientId, clientSecret: this.config.auth.clientSecret },
          this.httpClient
        );
      case 'none':
        return new TokenAuthProvider('');
    }
  }
}

const DEFAULT_CONFIG: Required<NovaClientConfig> = {
  server: { host: 'localhost', port: 8443, protocol: 'https', basePath: '/v1', timeout: 30000 },
  auth: { type: 'none' },
  transport: { maxConcurrent: 4, poolSize: 4, keepAliveMs: 30000, userAgent: '@novaruntime/sdk' },
  retry: { maxRetries: 3, baseDelayMs: 1000, maxDelayMs: 30000, retryableStatuses: [429, 500, 502, 503, 504], retryableErrors: [], strategy: 'exponential', jitterFactor: 0.2 },
};

function deepMerge<T>(target: any, source: any): T {
  const result = { ...target };
  for (const key of Object.keys(source)) {
    if (source[key] && typeof source[key] === 'object' && !Array.isArray(source[key])) {
      result[key] = deepMerge(result[key] || {}, source[key]);
    } else if (source[key] !== undefined) {
      result[key] = source[key];
    }
  }
  return result as T;
}

export function createClient(config: NovaClientConfig): NovaClient {
  return new NovaClient(config);
}
