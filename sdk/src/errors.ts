export type ErrorCode =
  | 'BAD_REQUEST' | 'VALIDATION_ERROR' | 'INVALID_SYNTAX' | 'INVALID_TYPE'
  | 'MISSING_FIELD' | 'INVALID_ARGUMENT'
  | 'UNAUTHENTICATED' | 'UNAUTHORIZED' | 'TOKEN_EXPIRED' | 'TOKEN_INVALID'
  | 'INSUFFICIENT_PERMISSIONS'
  | 'NOT_FOUND' | 'ALREADY_EXISTS' | 'CONFLICT' | 'RATE_LIMITED'
  | 'RESOURCE_EXHAUSTED'
  | 'INTERNAL_ERROR' | 'SUBSYSTEM_UNAVAILABLE' | 'TIMEOUT'
  | 'GATEWAY_TIMEOUT' | 'SERVICE_UNAVAILABLE'
  | 'CONNECTION_ERROR' | 'CONNECTION_TIMEOUT' | 'DNS_ERROR' | 'TLS_ERROR'
  | 'CIRCUIT_OPEN' | 'MAX_RETRIES_EXCEEDED' | 'INVALID_CONFIG'
  | 'STREAM_ERROR' | 'CANCELLED';

export interface ErrorExtensions {
  subsystem?: string;
  retryAfterMs?: number;
  internalCode?: string;
  fields?: string[];
  details?: string[];
  suggestion?: string;
}

export interface NovaErrorParams {
  code: ErrorCode;
  message: string;
  httpStatus?: number;
  extensions?: ErrorExtensions;
  requestId?: string;
  retryable?: boolean;
}

export class NovaError extends Error {
  public readonly code: ErrorCode;
  public readonly httpStatus?: number;
  public readonly extensions?: ErrorExtensions;
  public readonly retryable: boolean;
  public readonly requestId?: string;

  constructor(params: NovaErrorParams) {
    super(params.message);
    this.name = 'NovaError';
    this.code = params.code;
    this.httpStatus = params.httpStatus;
    this.extensions = params.extensions;
    this.retryable = params.retryable ?? false;
    this.requestId = params.requestId;
  }

  public isRetryable(): boolean {
    return this.retryable;
  }

  public retryAfterMs(): number | undefined {
    return this.extensions?.retryAfterMs;
  }

  public toSummary(): string {
    return `[${this.code}] ${this.message}${this.requestId ? ` (req: ${this.requestId})` : ''}`;
  }
}

const statusCodeMap: Record<number, ErrorCode> = {
  400: 'BAD_REQUEST',
  401: 'UNAUTHENTICATED',
  403: 'INSUFFICIENT_PERMISSIONS',
  404: 'NOT_FOUND',
  409: 'CONFLICT',
  422: 'VALIDATION_ERROR',
  429: 'RATE_LIMITED',
  500: 'INTERNAL_ERROR',
  502: 'SUBSYSTEM_UNAVAILABLE',
  503: 'SERVICE_UNAVAILABLE',
  504: 'GATEWAY_TIMEOUT',
};

export const Errors = {
  notFound: (message: string, meta?: Partial<NovaErrorParams>) =>
    new NovaError({ code: 'NOT_FOUND', message, httpStatus: 404, ...meta }),

  unauthorized: (message = 'Authentication required') =>
    new NovaError({ code: 'UNAUTHENTICATED', message, httpStatus: 401 }),

  tokenExpired: () =>
    new NovaError({ code: 'TOKEN_EXPIRED', message: 'Access token has expired', httpStatus: 401, retryable: true }),

  rateLimited: (retryAfterMs: number) =>
    new NovaError({
      code: 'RATE_LIMITED',
      message: 'Request rate limited',
      httpStatus: 429,
      retryable: true,
      extensions: { retryAfterMs },
    }),

  timeout: (message = 'Request timed out') =>
    new NovaError({ code: 'TIMEOUT', message, retryable: true }),

  connectionError: (cause: string) =>
    new NovaError({ code: 'CONNECTION_ERROR', message: `Connection error: ${cause}`, retryable: true }),

  circuitOpen: () =>
    new NovaError({ code: 'CIRCUIT_OPEN', message: 'Circuit breaker is open, request rejected' }),

  fromHttpStatus: (status: number, body: any, requestId?: string): NovaError => {
    const code = statusCodeMap[status] ?? 'INTERNAL_ERROR';
    const msg = body?.error?.message ?? body?.message ?? `HTTP ${status}`;
    return new NovaError({
      code,
      message: msg,
      httpStatus: status,
      extensions: body?.error?.extensions,
      requestId,
      retryable: status >= 500 || status === 429,
    });
  },
};

export interface RetryConfig {
  maxRetries?: number;
  baseDelayMs?: number;
  maxDelayMs?: number;
  retryableStatuses?: number[];
  retryableErrors?: ErrorCode[];
  strategy?: 'exponential' | 'linear' | 'fixed';
  jitterFactor?: number;
}

export interface RetryPolicy {
  shouldRetry(error: NovaError, attempt: number): boolean;
  getDelay(error: NovaError, attempt: number): number;
}

export function createRetryPolicy(config: Required<RetryConfig>): RetryPolicy {
  return {
    shouldRetry(error: NovaError, attempt: number): boolean {
      if (attempt >= config.maxRetries) return false;
      if (!error.retryable) return false;
      if (error.httpStatus && !config.retryableStatuses.includes(error.httpStatus)) return false;
      if (config.retryableErrors.length > 0 && !config.retryableErrors.includes(error.code)) return false;
      return true;
    },

    getDelay(error: NovaError, attempt: number): number {
      const baseDelay = error.extensions?.retryAfterMs ?? config.baseDelayMs;
      let delay: number;
      switch (config.strategy) {
        case 'linear':
          delay = baseDelay * (attempt + 1);
          break;
        case 'fixed':
          delay = baseDelay;
          break;
        case 'exponential':
        default:
          delay = baseDelay * Math.pow(2, attempt);
          break;
      }
      delay = Math.min(delay, config.maxDelayMs);
      const jitter = delay * config.jitterFactor * (Math.random() * 2 - 1);
      return Math.max(0, delay + jitter);
    },
  };
}
