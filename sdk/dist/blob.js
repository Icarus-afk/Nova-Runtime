"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.BlobClient = void 0;
class BlobClient {
    constructor(http) {
        this.http = http;
    }
    async upload(key, content, options) {
        const body = {
            key,
            content,
            contentType: options?.contentType,
            contentEncoding: options?.contentEncoding,
            storageTier: options?.storageTier,
            expiresAt: options?.expiresAt?.toISOString(),
            metadata: options?.metadata,
            overwrite: options?.overwrite,
        };
        const response = await this.http.post('/blob/upload', body);
        return response.data;
    }
    async download(key, options) {
        const headers = {};
        if (options?.startByte !== undefined) {
            headers['Range'] = `bytes=${options.startByte}-${options.endByte ?? ''}`;
        }
        const response = await this.http.get(`/blob/${encodeURIComponent(key)}`, {
            responseType: 'text',
            headers,
            signal: options?.signal,
        });
        return response.data;
    }
    async del(key) {
        const response = await this.http.delete(`/blob/${encodeURIComponent(key)}`);
        return response.data.deleted;
    }
    async multiDel(keys) {
        const response = await this.http.post('/blob/multi-delete', { keys });
        return response.data.deleted;
    }
    async list(prefix, options) {
        const response = await this.http.get('/blob', {
            query: { prefix, delimiter: options?.delimiter, ...(options?.pagination ?? {}) },
        });
        return response.data;
    }
    async info(key) {
        const response = await this.http.get(`/blob/${encodeURIComponent(key)}/info`);
        return response.data;
    }
    async copy(source, destination) {
        const response = await this.http.post('/blob/copy', { source, destination });
        return response.data;
    }
    async move(source, destination) {
        const response = await this.http.post('/blob/move', { source, destination });
        return response.data;
    }
    async setTier(key, tier) {
        const response = await this.http.post(`/blob/${encodeURIComponent(key)}/tier`, { tier });
        return response.data;
    }
    async setExpiry(key, expiresAt) {
        const response = await this.http.post(`/blob/${encodeURIComponent(key)}/expiry`, { expiresAt: expiresAt.toISOString() });
        return response.data;
    }
    async removeExpiry(key) {
        const response = await this.http.delete(`/blob/${encodeURIComponent(key)}/expiry`);
        return response.data;
    }
    async getStats() {
        const response = await this.http.get('/blob/stats');
        return response.data;
    }
    async *listIterator(prefix, options) {
        let cursor;
        let hasMore = true;
        while (hasMore) {
            const page = await this.list(prefix, {
                ...options,
                pagination: { first: 100, after: cursor },
            });
            for (const edge of page.edges) {
                yield edge.node;
            }
            hasMore = page.pageInfo.hasNextPage;
            cursor = page.pageInfo.endCursor ?? undefined;
        }
    }
}
exports.BlobClient = BlobClient;
//# sourceMappingURL=blob.js.map