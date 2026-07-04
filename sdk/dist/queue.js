"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.QueueClient = void 0;
class QueueClient {
    constructor(http) {
        this.http = http;
    }
    async listQueues(options) {
        const response = await this.http.get('/queue', { query: { ...(options ?? {}) } });
        return response.data;
    }
    async getQueue(name) {
        const response = await this.http.get(`/queue/${encodeURIComponent(name)}`);
        return response.data;
    }
    async createQueue(input) {
        const response = await this.http.post('/queue', input);
        return response.data;
    }
    async deleteQueue(name, force) {
        await this.http.delete(`/queue/${encodeURIComponent(name)}`, { query: { force } });
    }
    async send(queueName, input) {
        const response = await this.http.post(`/queue/${encodeURIComponent(queueName)}/messages`, input);
        return response.data;
    }
    async sendBatch(queueName, inputs) {
        const response = await this.http.post(`/queue/${encodeURIComponent(queueName)}/messages/batch`, { messages: inputs });
        return response.data;
    }
    async receive(queueName, options) {
        const response = await this.http.post(`/queue/${encodeURIComponent(queueName)}/messages/receive`, options ?? {});
        return response.data;
    }
    async deleteMessage(queueName, messageId) {
        await this.http.delete(`/queue/${encodeURIComponent(queueName)}/messages/${messageId}`);
    }
    async peek(queueName, options) {
        const response = await this.http.get(`/queue/${encodeURIComponent(queueName)}/messages/peek`, { query: options });
        return response.data;
    }
    async purge(queueName) {
        const response = await this.http.post(`/queue/${encodeURIComponent(queueName)}/purge`);
        return response.data.deleted;
    }
    async listDLQ(queueName, options) {
        const response = await this.http.get(`/queue/${encodeURIComponent(queueName)}/dlq`, { query: options });
        return response.data;
    }
    async redriveDLQ(queueName, maxMessages) {
        const response = await this.http.post(`/queue/${encodeURIComponent(queueName)}/dlq/redrive`, { maxMessages });
        return response.data.redriven;
    }
    async getStats(queueName) {
        const path = queueName ? `/queue/${encodeURIComponent(queueName)}/stats` : '/queue/stats';
        const response = await this.http.get(path);
        return response.data;
    }
    async *list(namePattern) {
        let cursor;
        let hasMore = true;
        while (hasMore) {
            const page = await this.listQueues({ first: 100, after: cursor, namePattern });
            for (const edge of page.edges) {
                yield edge.node;
            }
            hasMore = page.pageInfo.hasNextPage;
            cursor = page.pageInfo.endCursor ?? undefined;
        }
    }
}
exports.QueueClient = QueueClient;
//# sourceMappingURL=queue.js.map