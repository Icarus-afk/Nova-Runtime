"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SearchClient = void 0;
class SearchClient {
    constructor(http) {
        this.http = http;
    }
    async search(index, query, options) {
        const response = await this.http.post(`/search/${encodeURIComponent(index)}/query`, {
            query,
            ...options,
            ...options?.pagination,
        });
        return response.data;
    }
    async suggest(index, prefix, options) {
        const response = await this.http.get(`/search/${encodeURIComponent(index)}/suggest`, {
            query: { prefix, ...options },
        });
        return response.data;
    }
    async listIndexes(options) {
        const response = await this.http.get('/search/indexes', {
            query: options,
        });
        return response.data;
    }
    async getIndex(name) {
        const response = await this.http.get(`/search/indexes/${encodeURIComponent(name)}`);
        return response.data;
    }
    async createIndex(input) {
        const response = await this.http.post('/search/indexes', input);
        return response.data;
    }
    async deleteIndex(name) {
        await this.http.delete(`/search/indexes/${encodeURIComponent(name)}`);
    }
    async indexDocument(index, document, id) {
        const response = await this.http.post(`/search/${encodeURIComponent(index)}/documents`, { document, id });
        return response.data;
    }
    async indexDocuments(index, documents) {
        const response = await this.http.post(`/search/${encodeURIComponent(index)}/documents/batch`, { documents });
        return response.data;
    }
    async deleteDocument(index, id) {
        await this.http.delete(`/search/${encodeURIComponent(index)}/documents/${id}`);
    }
    async getStats() {
        const response = await this.http.get('/search/stats');
        return response.data;
    }
    async *listIndexesIterator() {
        let cursor;
        let hasMore = true;
        while (hasMore) {
            const page = await this.listIndexes({ first: 100, after: cursor });
            for (const edge of page.edges) {
                yield edge.node;
            }
            hasMore = page.pageInfo.hasNextPage;
            cursor = page.pageInfo.endCursor ?? undefined;
        }
    }
}
exports.SearchClient = SearchClient;
//# sourceMappingURL=search.js.map