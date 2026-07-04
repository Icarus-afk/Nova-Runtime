"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CacheClient = void 0;
const errors_1 = require("./errors");
class CacheClient {
    constructor(http) {
        this.http = http;
    }
    async get(key) {
        try {
            const response = await this.http.get(`/cache/${encodeURIComponent(key)}`);
            return response.data.value;
        }
        catch (error) {
            if (error instanceof errors_1.NovaError && error.code === 'NOT_FOUND')
                return null;
            throw error;
        }
    }
    async multiGet(keys) {
        const response = await this.http.post('/cache/multi-get', { keys });
        const map = new Map();
        for (const entry of response.data) {
            map.set(entry.key, entry.value);
        }
        return map;
    }
    async set(key, value, options) {
        await this.http.post(`/cache/${encodeURIComponent(key)}`, {
            value,
            ttlMs: options?.ttlMs,
            nx: options?.nx,
        });
    }
    async multiSet(entries) {
        await this.http.post('/cache/multi-set', { entries });
    }
    async del(key) {
        const response = await this.http.delete(`/cache/${encodeURIComponent(key)}`);
        return response.data.deleted;
    }
    async multiDel(keys) {
        const response = await this.http.post('/cache/multi-del', { keys });
        return response.data.deleted;
    }
    async delPattern(pattern) {
        const response = await this.http.post('/cache/del-pattern', { pattern });
        return response.data.deleted;
    }
    async keys(pattern, options) {
        const response = await this.http.get('/cache/keys', {
            query: { pattern, ...(options ?? {}) },
        });
        return response.data;
    }
    async ttl(key) {
        const response = await this.http.get(`/cache/${encodeURIComponent(key)}/ttl`);
        return response.data.ttlMs;
    }
    async expire(key, ttlMs) {
        const response = await this.http.post(`/cache/${encodeURIComponent(key)}/expire`, { ttlMs });
        return response.data.updated;
    }
    async incr(key, amount) {
        const response = await this.http.post(`/cache/${encodeURIComponent(key)}/incr`, {
            amount: amount ?? 1,
        });
        return response.data.value;
    }
    async stats() {
        const response = await this.http.get('/cache/stats');
        return response.data;
    }
    async flush() {
        const response = await this.http.post('/cache/flush');
        return response.data.deleted;
    }
    async *list(pattern) {
        let cursor;
        let hasMore = true;
        while (hasMore) {
            const page = await this.keys(pattern, { first: 100, after: cursor });
            for (const edge of page.edges) {
                yield edge.node;
            }
            hasMore = page.pageInfo.hasNextPage;
            cursor = page.pageInfo.endCursor ?? undefined;
        }
    }
}
exports.CacheClient = CacheClient;
//# sourceMappingURL=cache.js.map