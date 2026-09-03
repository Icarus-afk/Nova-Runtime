"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.CacheClient = void 0;
const errors_1 = require("./errors");
function toCacheConnection(keys, pagination) {
    const edges = keys.map((node, idx) => ({
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
        // No backend multi-get — fallback to parallel individual gets
        const entries = await Promise.all(keys.map(async (k) => {
            const v = await this.get(k);
            return [k, v];
        }));
        const map = new Map();
        for (const [k, v] of entries)
            map.set(k, v);
        return map;
    }
    async set(key, value, options) {
        const ttlMs = options?.ttl_ms ?? options?.ttlMs ?? (options?.ttlSeconds ? options.ttlSeconds * 1000 : undefined);
        const body = { value };
        if (ttlMs !== undefined)
            body.ttl_ms = ttlMs;
        // nx not supported — ignored for prototype
        await this.http.post(`/cache/${encodeURIComponent(key)}`, body);
    }
    async multiSet(entries) {
        // Backend batch expects POST /cache/batch with array of {key, value, ttl_ms}
        const payload = entries.map((e) => ({
            key: e.key,
            value: e.value,
            ttl_ms: e.ttl_ms ?? e.ttlMs ?? undefined,
        }));
        await this.http.post('/cache/batch', payload);
    }
    async del(key) {
        try {
            await this.http.delete(`/cache/${encodeURIComponent(key)}`);
            return true;
        }
        catch (error) {
            if (error instanceof errors_1.NovaError && error.code === 'NOT_FOUND')
                return false;
            throw error;
        }
    }
    async multiDel(keys) {
        // No backend multi-del — fallback to looping deletes
        let deleted = 0;
        await Promise.all(keys.map(async (k) => {
            const ok = await this.del(k);
            if (ok)
                deleted++;
        }));
        return deleted;
    }
    async delPattern(pattern) {
        // No backend del-pattern — list keys with pattern then delete each
        const keysResp = await this.keys(pattern, { first: 1000 });
        const keys = keysResp.edges.map((e) => e.node);
        // If pattern may need pagination, loop
        let allKeys = [...keys];
        // keys() already handles pagination size limit 1000; for larger sets, iterate via list helper
        if (keysResp.pageInfo.hasNextPage) {
            // fallback to enumerate via list()
            allKeys = [];
            for await (const k of this.list(pattern))
                allKeys.push(k);
        }
        return this.multiDel(allKeys);
    }
    async keys(pattern, options) {
        const query = {};
        if (pattern !== undefined)
            query.pattern = pattern;
        // Backend uses limit/offset ; SDK uses first/after cursor style — map
        if (options?.limit !== undefined)
            query.limit = options.limit;
        else if (options?.first !== undefined)
            query.limit = options.first;
        if (options?.offset !== undefined)
            query.offset = options.offset;
        else if (options?.after !== undefined) {
            const parsed = parseInt(options.after, 10);
            if (!Number.isNaN(parsed))
                query.offset = parsed;
            else
                query.offset = 0;
        }
        // Include last/before alternative
        if (options?.last !== undefined && query.limit === undefined)
            query.limit = options.last;
        const response = await this.http.get('/cache/keys', { query });
        const raw = response.data;
        // Normalize pagination shape
        const pagination = raw.pagination ?? { offset: 0, limit: raw.data.length, total: raw.total ?? raw.data.length, has_more: false };
        return toCacheConnection(raw.data, {
            offset: pagination.offset ?? 0,
            limit: pagination.limit ?? raw.data.length,
            total: pagination.total ?? (raw.total ?? raw.data.length),
            has_more: pagination.has_more ?? false,
        });
    }
    async ttl(key) {
        // No dedicated /ttl endpoint — fetch via GET and read ttl_remaining_ms
        try {
            const response = await this.http.get(`/cache/${encodeURIComponent(key)}`);
            return response.data.ttl_remaining_ms;
        }
        catch (error) {
            if (error instanceof errors_1.NovaError && error.code === 'NOT_FOUND')
                return null;
            throw error;
        }
    }
    async expire(key, ttlMs) {
        // No /expire endpoint — re-set with same value but new ttl_ms
        const val = await this.get(key);
        if (val === null)
            return false;
        await this.set(key, val, { ttlMs });
        return true;
    }
    async incr(key, amount) {
        // No /incr endpoint — get, increment, set
        const inc = amount ?? 1;
        const cur = await this.get(key);
        let num = 0;
        if (typeof cur === 'number')
            num = cur;
        else if (typeof cur === 'string')
            num = parseFloat(cur) || 0;
        else if (cur === null || cur === undefined)
            num = 0;
        else
            num = Number(cur) || 0;
        const next = num + inc;
        await this.set(key, next);
        return next;
    }
    async stats() {
        const response = await this.http.get('/cache/stats');
        const raw = response.data;
        return {
            hits: raw.hits,
            misses: raw.misses,
            hitRate: raw.hit_rate,
            entries: raw.keys,
            memoryUsedBytes: raw.memory_bytes ?? 0,
            evictions: raw.evictions,
        };
    }
    async flush() {
        // No /flush endpoint — enumerate and delete (matches dashboard clearCache)
        let deleted = 0;
        // Use list helper to handle pagination
        const allKeys = [];
        for await (const k of this.list('*'))
            allKeys.push(k);
        // Delete in batches to avoid huge concurrency
        const batchSize = 50;
        for (let i = 0; i < allKeys.length; i += batchSize) {
            const batch = allKeys.slice(i, i + batchSize);
            const n = await this.multiDel(batch);
            deleted += n;
        }
        return deleted;
    }
    async *list(pattern) {
        let offset = 0;
        const limit = 100;
        while (true) {
            const page = await this.keys(pattern, { first: limit, after: String(offset) });
            for (const edge of page.edges)
                yield edge.node;
            if (!page.pageInfo.hasNextPage)
                break;
            offset += limit;
        }
    }
}
exports.CacheClient = CacheClient;
//# sourceMappingURL=cache.js.map