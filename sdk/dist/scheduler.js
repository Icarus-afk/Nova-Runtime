"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.SchedulerClient = void 0;
class SchedulerClient {
    constructor(http) {
        this.http = http;
    }
    async listJobs(options) {
        const response = await this.http.get('/scheduler/jobs', {
            query: { ...(options?.pagination ?? {}), state: options?.state, type: options?.type, tags: options?.tags?.join(',') },
        });
        return response.data;
    }
    async getJob(id) {
        const response = await this.http.get(`/scheduler/jobs/${id}`);
        return response.data;
    }
    async createJob(input) {
        const response = await this.http.post('/scheduler/jobs', input);
        return response.data;
    }
    async updateJob(id, input) {
        const response = await this.http.patch(`/scheduler/jobs/${id}`, input);
        return response.data;
    }
    async deleteJob(id) {
        await this.http.delete(`/scheduler/jobs/${id}`);
    }
    async pauseJob(id) {
        const response = await this.http.post(`/scheduler/jobs/${id}/pause`);
        return response.data;
    }
    async resumeJob(id) {
        const response = await this.http.post(`/scheduler/jobs/${id}/resume`);
        return response.data;
    }
    async triggerJob(id, input) {
        const response = await this.http.post(`/scheduler/jobs/${id}/trigger`, { input });
        return response.data;
    }
    async getJobHistory(jobId, options) {
        const response = await this.http.get(`/scheduler/jobs/${jobId}/history`, {
            query: {
                ...(options?.pagination ?? {}),
                status: options?.status,
                since: options?.since?.toISOString(),
                until: options?.until?.toISOString(),
            },
        });
        return response.data;
    }
    async getExecution(executionId) {
        const response = await this.http.get(`/scheduler/executions/${executionId}`);
        return response.data;
    }
    async cancelExecution(executionId) {
        await this.http.post(`/scheduler/executions/${executionId}/cancel`);
    }
    async retryExecution(executionId) {
        const response = await this.http.post(`/scheduler/executions/${executionId}/retry`);
        return response.data;
    }
    async getStats() {
        const response = await this.http.get('/scheduler/stats');
        return response.data;
    }
    async *listJobsIterator(options) {
        let cursor;
        let hasMore = true;
        while (hasMore) {
            const page = await this.listJobs({ ...options, pagination: { first: 100, after: cursor } });
            for (const edge of page.edges) {
                yield edge.node;
            }
            hasMore = page.pageInfo.hasNextPage;
            cursor = page.pageInfo.endCursor ?? undefined;
        }
    }
}
exports.SchedulerClient = SchedulerClient;
//# sourceMappingURL=scheduler.js.map