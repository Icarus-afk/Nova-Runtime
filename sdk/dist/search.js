"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SearchClient = void 0;
const errors_1 = require("./errors");
function toIndexConnection(indexes, pagination) {
    const edges = indexes.map((node, idx) => ({
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
class SearchClient {
    constructor(http) {
        this.http = http;
    }
    async search(index, query, options) {
        // Map pagination: first/after or limit/offset
        let limit;
        let offset;
        if (options?.limit !== undefined)
            limit = options.limit;
        else if (options?.pagination?.first !== undefined)
            limit = options.pagination.first;
        else if (options?.first !== undefined)
            limit = options.first;
        if (options?.offset !== undefined)
            offset = options.offset;
        else if (options?.pagination?.after !== undefined) {
            const parsed = parseInt(options.pagination.after, 10);
            if (!Number.isNaN(parsed))
                offset = parsed;
        }
        const body = { query };
        if (limit !== undefined)
            body.limit = limit;
        if (offset !== undefined)
            body.offset = offset;
        const response = await this.http.post(`/search/indexes/${encodeURIComponent(index)}/query`, body);
        const raw = response.data;
        const edges = (raw.hits ?? []).map((h) => ({
            node: (h.source ?? h.fields ?? {}),
            cursor: h.id,
            score: h.score,
            highlight: undefined,
        }));
        return {
            edges,
            pageInfo: {
                hasNextPage: (raw.offset ?? 0) + (raw.limit ?? edges.length) < raw.total_hits,
                hasPreviousPage: (raw.offset ?? 0) > 0,
                startCursor: edges[0]?.cursor ?? null,
                endCursor: edges[edges.length - 1]?.cursor ?? null,
            },
            totalCount: raw.total_hits,
            maxScore: edges.length ? Math.max(...edges.map((e) => e.score)) : 0,
            tookMs: raw.execution_time_ms ?? 0,
            aggregations: undefined,
        };
    }
    async suggest(_index, _prefix, _options) {
        // No backend suggest route — return empty for prototype
        return [];
    }
    async listIndexes(options) {
        const query = {};
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
        }
        if (options?.last !== undefined && query.limit === undefined)
            query.limit = options.last;
        const response = await this.http.get('/search/indexes', { query });
        const raw = response.data;
        const indexes = (raw.data ?? []).map((idx) => ({
            name: idx.name,
            documentCount: idx.doc_count ?? 0,
            sizeBytes: 0,
            fieldCount: idx.field_count ?? (idx.fields?.length ?? 0),
            analyzer: 'standard',
            createdAt: idx.created_at ? new Date(idx.created_at).toISOString() : new Date().toISOString(),
            updatedAt: idx.created_at ? new Date(idx.created_at).toISOString() : new Date().toISOString(),
            fields: (idx.fields ?? []).map((f) => ({
                name: f.name,
                type: String(f.type ?? 'TEXT').toUpperCase(),
                searchable: true,
                sortable: false,
                facetable: false,
                stored: true,
                analyzer: f.analyzer,
                boost: f.boost ?? 1,
            })),
        }));
        return toIndexConnection(indexes, raw.pagination);
    }
    async getIndex(name) {
        const response = await this.http.get(`/search/indexes/${encodeURIComponent(name)}`);
        const raw = response.data;
        return {
            name: raw.name,
            documentCount: raw.num_docs ?? 0,
            sizeBytes: 0,
            fieldCount: raw.field_count ?? (raw.fields?.length ?? 0),
            analyzer: 'standard',
            createdAt: raw.created_at ? new Date(raw.created_at).toISOString() : new Date().toISOString(),
            updatedAt: raw.created_at ? new Date(raw.created_at).toISOString() : new Date().toISOString(),
            fields: (raw.fields ?? []).map((f) => ({
                name: f.name,
                type: String(f.type ?? 'TEXT').toUpperCase(),
                searchable: true,
                sortable: false,
                facetable: false,
                stored: true,
                analyzer: f.analyzer,
                boost: f.boost ?? 1,
            })),
        };
    }
    async createIndex(input) {
        // Backend expects { name, fields?: [{name, type, analyzer?, boost?}] }
        // Map field type to lowercase as backend validates lower-case set
        const fields = input.fields ?? input.fieldsInput ?? [];
        const mappedFields = fields.map((f) => ({
            name: f.name,
            type: String(f.type ?? 'text').toLowerCase(),
            analyzer: f.analyzer,
            boost: f.boost,
        }));
        const body = { name: input.name };
        if (mappedFields.length > 0)
            body.fields = mappedFields;
        if (input.analyzer)
            body.fields = mappedFields; // ignore top-level analyzer for compat
        const response = await this.http.post('/search/indexes', body);
        const raw = response.data;
        return {
            name: raw.name ?? input.name,
            documentCount: 0,
            sizeBytes: 0,
            fieldCount: mappedFields.length,
            analyzer: input.analyzer ?? 'standard',
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            fields: mappedFields.map((f) => ({
                name: f.name,
                type: String(f.type).toUpperCase(),
                searchable: f.searchable ?? true,
                sortable: f.sortable ?? false,
                facetable: f.facetable ?? false,
                stored: f.stored ?? true,
                analyzer: f.analyzer,
                boost: f.boost ?? 1,
            })),
        };
    }
    async deleteIndex(name) {
        await this.http.delete(`/search/indexes/${encodeURIComponent(name)}`);
    }
    async indexDocument(index, document, id) {
        const docWithId = { ...document };
        if (id)
            docWithId.id = id;
        const response = await this.http.post(`/search/indexes/${encodeURIComponent(index)}/documents`, { documents: [docWithId] });
        return { id: id ?? docWithId.id ?? String(Date.now()), indexed: response.data.count > 0 };
    }
    async indexDocuments(index, documents) {
        const payloadDocs = documents.map((d) => {
            const obj = { ...d.document };
            if (d.id)
                obj.id = d.id;
            return obj;
        });
        const response = await this.http.post(`/search/indexes/${encodeURIComponent(index)}/documents`, { documents: payloadDocs });
        return { indexedCount: response.data.count, failedCount: 0 };
    }
    // Alternative batch signature where caller passes raw documents[]
    async addDocuments(index, documents) {
        const response = await this.http.post(`/search/indexes/${encodeURIComponent(index)}/documents`, { documents });
        return response.data;
    }
    async deleteDocument(_index, _id) {
        // No backend route for per-document delete
        throw errors_1.Errors.notFound('deleteDocument not supported by backend — delete and re-create index');
    }
    async getStats() {
        // No global stats route — aggregate from listIndexes
        const conn = await this.listIndexes({ first: 100 });
        let totalDocuments = 0;
        for (const idx of conn.edges)
            totalDocuments += idx.node.documentCount;
        return {
            totalIndexes: conn.totalCount,
            totalDocuments,
            totalSizeBytes: 0,
            avgIndexTimeMs: 0,
            avgQueryTimeMs: 0,
            p95QueryTimeMs: 0,
            queriesTotal: 0,
            indexingTotal: totalDocuments,
        };
    }
    async getIndexStats(name) {
        const response = await this.http.get(`/search/indexes/${encodeURIComponent(name)}/stats`);
        return response.data;
    }
    async *listIndexesIterator() {
        let offset = 0;
        const limit = 100;
        while (true) {
            const page = await this.listIndexes({ first: limit, after: String(offset) });
            for (const edge of page.edges)
                yield edge.node;
            if (!page.pageInfo.hasNextPage)
                break;
            offset += limit;
        }
    }
}
exports.SearchClient = SearchClient;
//# sourceMappingURL=search.js.map