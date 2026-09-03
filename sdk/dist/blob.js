"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.BlobClient = void 0;
const errors_1 = require("./errors");
function toBlobConnection(entries, pagination) {
    const edges = entries.map((node, idx) => ({
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
class BlobClient {
    constructor(http) {
        this.http = http;
    }
    // Upload: POST /api/v1/blobs?namespace= multipart field 'file'
    // Keep legacy signature upload(key, content, options) where key is treated as namespace prefix or file identifier
    // For multipart we convert content string/Buffer/Blob to file
    async upload(key, content, options) {
        // Determine namespace: if key contains '/', first segment is namespace; otherwise use options.metadata?.custom?.namespace or "default"
        // Also allow key as filename hint via metadata.filename
        let namespace = 'default';
        let filenameHint = key;
        if (key.includes('/')) {
            const parts = key.split('/');
            namespace = parts[0] || 'default';
            filenameHint = parts.slice(1).join('/') || key;
        }
        // Allow override via options.metadata custom
        const customNs = options?.metadata?.namespace ?? options?.metadata?.bucket;
        if (customNs)
            namespace = String(customNs);
        // Build FormData
        const formData = new globalThis.FormData();
        let fileObj;
        let fileName = filenameHint || 'file';
        // Normalize content to Blob
        if (typeof content === 'string') {
            fileObj = new globalThis.Blob([content], { type: options?.contentType ?? 'application/octet-stream' });
        }
        else if (typeof globalThis.Buffer !== 'undefined' && content instanceof globalThis.Buffer) {
            fileObj = new globalThis.Blob([content], { type: options?.contentType ?? 'application/octet-stream' });
        }
        else if (content instanceof Uint8Array) {
            fileObj = new globalThis.Blob([content], { type: options?.contentType ?? 'application/octet-stream' });
        }
        else if (typeof globalThis.Blob !== 'undefined' && content instanceof globalThis.Blob) {
            fileObj = content;
            if (content.name)
                fileName = content.name;
        }
        else {
            // Fallback stringify
            const str = String(content);
            fileObj = new globalThis.Blob([str], { type: options?.contentType ?? 'text/plain' });
        }
        formData.append('file', fileObj, fileName);
        const response = await this.http.request({
            method: 'POST',
            path: '/blobs',
            query: { namespace },
            body: formData,
            headers: {}, // content-type removed via client detection
        });
        const raw = response.data;
        // Map to BlobMetadata (synthesize missing fields)
        return {
            key: raw.id,
            sizeBytes: raw.size_bytes,
            contentType: raw.content_type,
            contentEncoding: options?.contentEncoding,
            etag: raw.checksum_sha256,
            md5: '',
            sha256: raw.checksum_sha256,
            storageTier: options?.storageTier ?? 'HOT',
            createdAt: new Date(raw.created_at).toISOString(),
            updatedAt: new Date(raw.created_at).toISOString(),
            expiresAt: options?.expiresAt?.toISOString(),
            metadata: options?.metadata,
            url: `/api/v1/blobs/${raw.id}`,
        };
    }
    async download(key, options) {
        const headers = {};
        if (options?.startByte !== undefined) {
            headers['Range'] = `bytes=${options.startByte}-${options.endByte ?? ''}`;
        }
        // Backend returns raw bytes; try text first, fallback to buffer
        try {
            const response = await this.http.get(`/blobs/${encodeURIComponent(key)}`, {
                responseType: 'text',
                headers,
                signal: options?.signal,
            });
            return response.data;
        }
        catch (_e) {
            // If text fails, try buffer
            const response = await this.http.get(`/blobs/${encodeURIComponent(key)}`, {
                responseType: 'buffer',
                headers,
                signal: options?.signal,
            });
            const buf = response.data;
            if (globalThis.Buffer) {
                return globalThis.Buffer.from(buf).toString('utf-8');
            }
            return new TextDecoder().decode(buf);
        }
    }
    // Download as buffer helper
    async downloadBuffer(key, options) {
        const headers = {};
        if (options?.startByte !== undefined) {
            headers['Range'] = `bytes=${options.startByte}-${options.endByte ?? ''}`;
        }
        const response = await this.http.get(`/blobs/${encodeURIComponent(key)}`, {
            responseType: 'buffer',
            headers,
            signal: options?.signal,
        });
        return response.data;
    }
    async del(key) {
        try {
            await this.http.delete(`/blobs/${encodeURIComponent(key)}`);
            return true;
        }
        catch (error) {
            if (error instanceof errors_1.NovaError && error.code === 'NOT_FOUND')
                return false;
            throw error;
        }
    }
    async multiDel(keys) {
        // No backend multi-delete — loop
        let deleted = 0;
        await Promise.all(keys.map(async (k) => {
            const ok = await this.del(k);
            if (ok)
                deleted++;
        }));
        return deleted;
    }
    async list(prefix, options) {
        // Map SDK prefix/filter to backend query: namespace, prefix, limit, offset
        let namespace = options?.namespace ?? 'default';
        let pref = prefix;
        // If prefix looks like "namespace/rest" treat first segment as namespace
        if (prefix && prefix.includes('/') && !options?.namespace) {
            // Heuristic: if prefix contains '/', consider first part as namespace only when listing via prefix?
            // Keep as prefix; namespace remains default unless explicitly set.
            pref = prefix;
        }
        const query = { namespace };
        if (pref !== undefined)
            query.prefix = pref;
        // Pagination: backend uses limit/offset ; SDK uses first/after etc.
        let limit;
        let offset;
        if (options?.limit !== undefined)
            limit = options.limit;
        else if (options?.pagination?.first !== undefined)
            limit = options.pagination.first;
        if (options?.offset !== undefined)
            offset = options.offset;
        else if (options?.pagination?.after !== undefined) {
            const parsed = parseInt(options.pagination.after, 10);
            if (!Number.isNaN(parsed))
                offset = parsed;
        }
        if (limit !== undefined)
            query.limit = limit;
        if (offset !== undefined)
            query.offset = offset;
        // Filters not server-supported; pass through where possible
        const response = await this.http.get('/blobs', { query });
        const raw = response.data;
        let entries = (raw.data ?? []).map((item) => ({
            key: item.id,
            sizeBytes: item.size_bytes,
            contentType: item.content_type,
            storageTier: 'HOT',
            createdAt: new Date(item.created_at).toISOString(),
            etag: '',
            isPrefix: false,
        }));
        // Client-side filter if needed
        if (options?.filter?.contentType) {
            entries = entries.filter((e) => e.contentType === options.filter.contentType);
        }
        if (options?.filter?.minSizeBytes !== undefined) {
            entries = entries.filter((e) => e.sizeBytes >= options.filter.minSizeBytes);
        }
        if (options?.filter?.maxSizeBytes !== undefined) {
            entries = entries.filter((e) => e.sizeBytes <= options.filter.maxSizeBytes);
        }
        const fallbackPagination = { offset: offset ?? 0, limit: limit ?? entries.length, total: entries.length, has_more: false };
        const pagination = raw.pagination ?? fallbackPagination;
        return toBlobConnection(entries, pagination);
    }
    async info(key) {
        const response = await this.http.get(`/blobs/${encodeURIComponent(key)}/info`);
        const raw = response.data;
        return {
            key: raw.id,
            sizeBytes: raw.size_bytes,
            contentType: raw.content_type,
            etag: raw.checksum_sha256,
            md5: '',
            sha256: raw.checksum_sha256,
            storageTier: 'HOT',
            createdAt: new Date(raw.created_at).toISOString(),
            updatedAt: new Date(raw.created_at).toISOString(),
            metadata: raw.metadata,
            url: `/api/v1/blobs/${raw.id}`,
        };
    }
    async copy(_source, _destination) {
        // No backend copy — implement via download + upload for prototype
        const data = await this.downloadBuffer(_source);
        const buf = globalThis.Buffer ? globalThis.Buffer.from(data) : new Uint8Array(data);
        return this.upload(_destination, buf);
    }
    async move(source, destination) {
        const meta = await this.copy(source, destination);
        await this.del(source);
        return meta;
    }
    async setTier(key, tier) {
        // No backend setTier — return info with updated tier locally
        const meta = await this.info(key);
        return { ...meta, storageTier: tier };
    }
    async setExpiry(key, expiresAt) {
        const meta = await this.info(key);
        return { ...meta, expiresAt: expiresAt.toISOString() };
    }
    async removeExpiry(key) {
        const meta = await this.info(key);
        return { ...meta, expiresAt: undefined };
    }
    async getStats() {
        const response = await this.http.get('/blobs/stats');
        const raw = response.data;
        return {
            uploadsTotal: raw.total_blobs,
            downloadsTotal: 0,
            totalBlobs: raw.total_blobs,
            totalStorageBytes: raw.total_bytes,
        };
    }
    async *listIterator(prefix, options) {
        let offset = 0;
        const limit = 100;
        while (true) {
            const page = await this.list(prefix, {
                ...options,
                pagination: { first: limit, after: String(offset) },
            });
            for (const edge of page.edges)
                yield edge.node;
            if (!page.pageInfo.hasNextPage)
                break;
            offset += limit;
        }
    }
}
exports.BlobClient = BlobClient;
//# sourceMappingURL=blob.js.map