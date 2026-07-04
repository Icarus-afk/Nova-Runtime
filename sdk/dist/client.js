"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
exports.NovaClient = exports.FetchHttpClient = void 0;
exports.createClient = createClient;
const cross_fetch_1 = __importDefault(require("cross-fetch"));
const errors_1 = require("./errors");
const runtime_1 = require("./runtime");
const database_1 = require("./database");
const cache_1 = require("./cache");
const queue_1 = require("./queue");
const scheduler_1 = require("./scheduler");
const search_1 = require("./search");
const blob_1 = require("./blob");
const auth_1 = require("./auth");
class TokenAuthProvider {
    constructor(token) {
        this.token = token;
    }
    async getToken() {
        return this.token;
    }
}
class RefreshAuthProvider {
    constructor(config, httpClient, refreshToken) {
        this.config = config;
        this.httpClient = httpClient;
        this.accessToken = null;
        this.expiresAt = 0;
        this.refreshPromise = null;
        this.refreshTokenValue = refreshToken ?? '';
    }
    async getToken() {
        if (!this.accessToken || Date.now() >= this.expiresAt - 60000) {
            const refreshed = await this.performRefresh();
            if (!refreshed) {
                throw errors_1.Errors.unauthorized('Unable to obtain access token');
            }
        }
        return this.accessToken;
    }
    async onUnauthorized() {
        return this.performRefresh();
    }
    async performRefresh() {
        if (this.refreshPromise) {
            return this.refreshPromise;
        }
        this.refreshPromise = this._doRefresh();
        try {
            return await this.refreshPromise;
        }
        finally {
            this.refreshPromise = null;
        }
    }
    async _doRefresh() {
        try {
            const response = await this.httpClient.request({
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
        }
        catch {
            return false;
        }
    }
}
function generateId() {
    return `${Date.now().toString(36)}-${Math.random().toString(36).substring(2, 9)}`;
}
class FetchHttpClient {
    constructor(config, authProvider) {
        this.config = config;
        this.authProvider = authProvider;
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
        this.retryPolicy = (0, errors_1.createRetryPolicy)(this.retryConfig);
    }
    async request(req) {
        const timeoutMs = req.timeoutMs ?? this.config.server?.timeout ?? 30000;
        let attempt = 0;
        while (true) {
            const requestId = generateId();
            const token = await this.authProvider.getToken();
            const url = this.buildUrl(req.path, req.query);
            const headers = {
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
                const fetchInit = {
                    method: req.method,
                    headers,
                    signal,
                };
                if (body) {
                    fetchInit.body = body;
                }
                const response = await (0, cross_fetch_1.default)(url, fetchInit);
                clearTimeout(timeoutId);
                const durationMs = Date.now() - startTime;
                const responseHeaders = {};
                response.headers.forEach((value, key) => {
                    responseHeaders[key] = value;
                });
                if (!response.ok) {
                    let responseBody;
                    try {
                        responseBody = await response.json();
                    }
                    catch {
                        responseBody = { message: await response.text() };
                    }
                    const error = errors_1.Errors.fromHttpStatus(response.status, responseBody, requestId);
                    if (error.code === 'TOKEN_EXPIRED' || (error.code === 'UNAUTHENTICATED' && this.authProvider.onUnauthorized)) {
                        const refreshed = await this.authProvider.onUnauthorized();
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
                let data;
                if (req.responseType === 'text') {
                    data = (await response.text());
                }
                else if (req.responseType === 'buffer') {
                    data = (await response.arrayBuffer());
                }
                else {
                    data = await response.json();
                }
                return {
                    status: response.status,
                    headers: responseHeaders,
                    data,
                    requestId,
                    durationMs,
                };
            }
            catch (error) {
                clearTimeout(timeoutId);
                if (error instanceof errors_1.NovaError) {
                    throw error;
                }
                if (error?.name === 'AbortError') {
                    const novaError = errors_1.Errors.timeout('Request timed out');
                    if (this.retryPolicy.shouldRetry(novaError, attempt)) {
                        const delay = this.retryPolicy.getDelay(novaError, attempt);
                        await sleep(delay);
                        attempt++;
                        continue;
                    }
                    throw novaError;
                }
                const novaError = errors_1.Errors.connectionError(error?.message ?? 'Unknown error');
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
    async get(path, options) {
        return this.request({ method: 'GET', path, ...options });
    }
    async post(path, body, options) {
        return this.request({ method: 'POST', path, body, ...options });
    }
    async put(path, body, options) {
        return this.request({ method: 'PUT', path, body, ...options });
    }
    async patch(path, body, options) {
        return this.request({ method: 'PATCH', path, body, ...options });
    }
    async delete(path, options) {
        return this.request({ method: 'DELETE', path, ...options });
    }
    async dispose() {
    }
    buildUrl(path, query) {
        const url = `${this.baseUrl}/${path.replace(/^\//, '')}`;
        if (!query)
            return url;
        const params = new URLSearchParams();
        for (const [key, value] of Object.entries(query)) {
            if (value === undefined || value === null)
                continue;
            if (Array.isArray(value)) {
                for (const v of value) {
                    params.append(key, v);
                }
            }
            else {
                params.set(key, String(value));
            }
        }
        const qs = params.toString();
        return qs ? `${url}?${qs}` : url;
    }
}
exports.FetchHttpClient = FetchHttpClient;
function combineAbortSignals(...signals) {
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
function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}
class NovaClient {
    constructor(config) {
        this.config = this.resolveConfig(config);
        this.authProvider = this.createAuthProvider();
        this.httpClient = new FetchHttpClient(this.config, this.authProvider);
        this.runtime = new runtime_1.RuntimeClient(this.httpClient);
        this.db = new database_1.DatabaseClient(this.httpClient);
        this.cache = new cache_1.CacheClient(this.httpClient);
        this.queue = new queue_1.QueueClient(this.httpClient);
        this.scheduler = new scheduler_1.SchedulerClient(this.httpClient);
        this.search = new search_1.SearchClient(this.httpClient);
        this.blob = new blob_1.BlobClient(this.httpClient);
        this.auth = new auth_1.AuthClient(this.httpClient);
    }
    async health() {
        return this.runtime.health();
    }
    async dispose() {
        await this.httpClient.dispose();
        await this.authProvider.dispose?.();
    }
    resolveConfig(config) {
        return deepMerge(DEFAULT_CONFIG, config);
    }
    createAuthProvider() {
        switch (this.config.auth.type) {
            case 'token':
                return new TokenAuthProvider(this.config.auth.token);
            case 'api-key':
                return new TokenAuthProvider(this.config.auth.apiKey);
            case 'refresh':
                return new RefreshAuthProvider({ clientId: this.config.auth.clientId, clientSecret: this.config.auth.clientSecret }, this.httpClient);
            case 'none':
                return new TokenAuthProvider('');
        }
    }
}
exports.NovaClient = NovaClient;
const DEFAULT_CONFIG = {
    server: { host: 'localhost', port: 8443, protocol: 'https', basePath: '/v1', timeout: 30000 },
    auth: { type: 'none' },
    transport: { maxConcurrent: 4, poolSize: 4, keepAliveMs: 30000, userAgent: '@novaruntime/sdk' },
    retry: { maxRetries: 3, baseDelayMs: 1000, maxDelayMs: 30000, retryableStatuses: [429, 500, 502, 503, 504], retryableErrors: [], strategy: 'exponential', jitterFactor: 0.2 },
};
function deepMerge(target, source) {
    const result = { ...target };
    for (const key of Object.keys(source)) {
        if (source[key] && typeof source[key] === 'object' && !Array.isArray(source[key])) {
            result[key] = deepMerge(result[key] || {}, source[key]);
        }
        else if (source[key] !== undefined) {
            result[key] = source[key];
        }
    }
    return result;
}
function createClient(config) {
    return new NovaClient(config);
}
//# sourceMappingURL=client.js.map