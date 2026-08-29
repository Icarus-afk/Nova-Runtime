"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Errors = exports.NovaError = void 0;
exports.createRetryPolicy = createRetryPolicy;
class NovaError extends Error {
    constructor(params) {
        super(params.message);
        this.name = 'NovaError';
        this.code = params.code;
        this.httpStatus = params.httpStatus;
        this.extensions = params.extensions;
        this.retryable = params.retryable ?? false;
        this.requestId = params.requestId;
    }
    isRetryable() {
        return this.retryable;
    }
    retryAfterMs() {
        return this.extensions?.retryAfterMs;
    }
    toSummary() {
        return `[${this.code}] ${this.message}${this.requestId ? ` (req: ${this.requestId})` : ''}`;
    }
}
exports.NovaError = NovaError;
const statusCodeMap = {
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
exports.Errors = {
    notFound: (message, meta) => new NovaError({ code: 'NOT_FOUND', message, httpStatus: 404, ...meta }),
    unauthorized: (message = 'Authentication required') => new NovaError({ code: 'UNAUTHENTICATED', message, httpStatus: 401 }),
    tokenExpired: () => new NovaError({ code: 'TOKEN_EXPIRED', message: 'Access token has expired', httpStatus: 401, retryable: true }),
    rateLimited: (retryAfterMs) => new NovaError({
        code: 'RATE_LIMITED',
        message: 'Request rate limited',
        httpStatus: 429,
        retryable: true,
        extensions: { retryAfterMs },
    }),
    timeout: (message = 'Request timed out') => new NovaError({ code: 'TIMEOUT', message, retryable: true }),
    connectionError: (cause) => new NovaError({ code: 'CONNECTION_ERROR', message: `Connection error: ${cause}`, retryable: true }),
    circuitOpen: () => new NovaError({ code: 'CIRCUIT_OPEN', message: 'Circuit breaker is open, request rejected' }),
    fromHttpStatus: (status, body, requestId) => {
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
function createRetryPolicy(config) {
    return {
        shouldRetry(error, attempt) {
            if (attempt >= config.maxRetries)
                return false;
            if (!error.retryable)
                return false;
            if (error.httpStatus && !config.retryableStatuses.includes(error.httpStatus))
                return false;
            if (config.retryableErrors.length > 0 && !config.retryableErrors.includes(error.code))
                return false;
            return true;
        },
        getDelay(error, attempt) {
            const baseDelay = error.extensions?.retryAfterMs ?? config.baseDelayMs;
            let delay;
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
//# sourceMappingURL=errors.js.map