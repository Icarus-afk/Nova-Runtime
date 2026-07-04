export type ErrorCode = 'BAD_REQUEST' | 'VALIDATION_ERROR' | 'INVALID_SYNTAX' | 'INVALID_TYPE' | 'MISSING_FIELD' | 'INVALID_ARGUMENT' | 'UNAUTHENTICATED' | 'UNAUTHORIZED' | 'TOKEN_EXPIRED' | 'TOKEN_INVALID' | 'INSUFFICIENT_PERMISSIONS' | 'NOT_FOUND' | 'ALREADY_EXISTS' | 'CONFLICT' | 'RATE_LIMITED' | 'RESOURCE_EXHAUSTED' | 'INTERNAL_ERROR' | 'SUBSYSTEM_UNAVAILABLE' | 'TIMEOUT' | 'GATEWAY_TIMEOUT' | 'SERVICE_UNAVAILABLE' | 'CONNECTION_ERROR' | 'CONNECTION_TIMEOUT' | 'DNS_ERROR' | 'TLS_ERROR' | 'CIRCUIT_OPEN' | 'MAX_RETRIES_EXCEEDED' | 'INVALID_CONFIG' | 'STREAM_ERROR' | 'CANCELLED';
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
export declare class NovaError extends Error {
    readonly code: ErrorCode;
    readonly httpStatus?: number;
    readonly extensions?: ErrorExtensions;
    readonly retryable: boolean;
    readonly requestId?: string;
    constructor(params: NovaErrorParams);
    isRetryable(): boolean;
    retryAfterMs(): number | undefined;
    toSummary(): string;
}
export declare const Errors: {
    notFound: (message: string, meta?: Partial<NovaErrorParams>) => NovaError;
    unauthorized: (message?: string) => NovaError;
    tokenExpired: () => NovaError;
    rateLimited: (retryAfterMs: number) => NovaError;
    timeout: (message?: string) => NovaError;
    connectionError: (cause: string) => NovaError;
    circuitOpen: () => NovaError;
    fromHttpStatus: (status: number, body: any, requestId?: string) => NovaError;
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
export declare function createRetryPolicy(config: Required<RetryConfig>): RetryPolicy;
