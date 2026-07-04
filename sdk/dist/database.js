"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.DatabaseClient = void 0;
class DatabaseClient {
    constructor(http) {
        this.http = http;
    }
    async query(sql, params, options) {
        const response = await this.http.post('/db/query', {
            query: sql,
            params: params ?? [],
            ...options,
        });
        return response.data;
    }
    async execute(sql, params, options) {
        const response = await this.http.post('/db/exec', {
            query: sql,
            params: params ?? [],
            ...options,
        });
        return response.data;
    }
    async listTables(options) {
        const response = await this.http.get('/db/tables', {
            query: { ...(options?.pagination ?? {}), schema: options?.schema, pattern: options?.pattern },
        });
        return response.data;
    }
    async getTable(name) {
        const response = await this.http.get(`/db/tables/${encodeURIComponent(name)}`);
        return response.data;
    }
    async createTable(input) {
        const response = await this.http.post('/db/tables', input);
        return response.data;
    }
    async dropTable(name, ifExists) {
        await this.http.delete(`/db/tables/${encodeURIComponent(name)}`, {
            query: { ifExists },
        });
    }
    async explain(sql, params) {
        const response = await this.http.post('/db/explain', { query: sql, params: params ?? [] });
        return response.data;
    }
    async getStats() {
        const response = await this.http.get('/db/stats');
        return response.data;
    }
}
exports.DatabaseClient = DatabaseClient;
//# sourceMappingURL=database.js.map