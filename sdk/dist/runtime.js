"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.RuntimeClient = void 0;
class RuntimeClient {
    constructor(http) {
        this.http = http;
    }
    async health() {
        const response = await this.http.get('/runtime/health');
        return response.data;
    }
    async getConfig(key, subsystem) {
        const response = await this.http.get('/runtime/config', {
            query: { key, subsystem },
        });
        return response.data;
    }
    async updateConfig(key, value) {
        const response = await this.http.post('/runtime/config', {
            key, value,
        });
        return response.data;
    }
    async getMetrics(options) {
        const response = await this.http.get('/runtime/metrics', {
            query: {
                since: options?.since?.toISOString(),
                resolution: options?.resolution,
            },
        });
        return response.data;
    }
    async getVersion() {
        const response = await this.http.get('/runtime/version');
        return response.data;
    }
    async listConnections(options) {
        const response = await this.http.get('/runtime/connections', {
            query: {
                subsystem: options?.subsystem,
                status: options?.status,
                ...(options?.pagination ?? {}),
            },
        });
        return response.data;
    }
}
exports.RuntimeClient = RuntimeClient;
//# sourceMappingURL=runtime.js.map