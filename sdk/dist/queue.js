"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.QueueClient = void 0;
const errors_1 = require("./errors");
function toQueueConnection(queues, pagination) {
    const edges = queues.map((node, idx) => ({
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
function toMessageConnection(messages, pagination) {
    const edges = messages.map((node, idx) => ({
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
function rawQueueToQueue(raw) {
    const r = raw;
    return {
        name: r.name,
        description: undefined,
        createdAt: '',
        updatedAt: '',
        messageCount: (r.total ?? (r.available ?? 0) + (r.in_flight ?? 0) + (r.delayed ?? 0)),
        messagesSent: 0,
        messagesReceived: 0,
        messagesDeleted: 0,
        messagesDeadLettered: 0,
        oldestMessageAgeMs: 0,
        config: {
            visibilityTimeoutMs: 30000,
            maxMessageSizeBytes: r.max_size ?? 1024 * 1024,
            messageRetentionMs: 7 * 24 * 60 * 60 * 1000,
            deadLetterMaxReceives: 5,
            deadLetterQueue: false,
            deliveryDelayMs: 0,
        },
    };
}
class QueueClient {
    constructor(http) {
        this.http = http;
    }
    async listQueues(options) {
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
        const response = await this.http.get('/queues', { query });
        const raw = response.data;
        let items = raw.data.map(rawQueueToQueue);
        if (options?.namePattern) {
            const reStr = '^' + options.namePattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\\\*/g, '.*').replace(/\\\?/g, '.') + '$';
            try {
                const re = new RegExp(reStr);
                items = items.filter((q) => re.test(q.name));
            }
            catch { /* ignore */ }
        }
        return toQueueConnection(items, raw.pagination);
    }
    async getQueue(name) {
        const response = await this.http.get(`/queues/${encodeURIComponent(name)}`);
        return rawQueueToQueue(response.data);
    }
    async createQueue(input) {
        // Backend expects { name, durable?, max_length?, max_message_size? }
        const body = { name: input.name };
        // Map enableDeadLetterQueue -> durable hint (dashboard uses durable)
        if (input.durable !== undefined)
            body.durable = input.durable;
        else if (input.enableDeadLetterQueue !== undefined)
            body.durable = true;
        if (input.max_length !== undefined)
            body.max_length = input.max_length;
        else if (input.maxLength !== undefined)
            body.max_length = input.maxLength;
        if (input.maxMessageSizeBytes !== undefined)
            body.max_message_size = input.maxMessageSizeBytes;
        else if (input.max_message_size !== undefined)
            body.max_message_size = input.max_message_size;
        const response = await this.http.post('/queues', body);
        const raw = response.data;
        // Response is { id, name, status, durable, max_length, max_message_size } — synthesize Queue
        return {
            name: raw.name ?? input.name,
            description: input.description,
            createdAt: new Date().toISOString(),
            updatedAt: new Date().toISOString(),
            messageCount: 0,
            messagesSent: 0,
            messagesReceived: 0,
            messagesDeleted: 0,
            messagesDeadLettered: 0,
            oldestMessageAgeMs: 0,
            config: {
                visibilityTimeoutMs: input.visibilityTimeoutMs ?? 30000,
                maxMessageSizeBytes: input.maxMessageSizeBytes ?? raw.max_message_size ?? 1024 * 1024,
                messageRetentionMs: input.messageRetentionMs ?? 7 * 24 * 60 * 60 * 1000,
                deadLetterMaxReceives: input.deadLetterMaxReceives ?? 5,
                deadLetterQueue: input.enableDeadLetterQueue ?? false,
                deliveryDelayMs: input.deliveryDelayMs ?? 0,
            },
        };
    }
    async deleteQueue(name, _force) {
        await this.http.delete(`/queues/${encodeURIComponent(name)}`);
    }
    async send(queueName, input) {
        const body = {
            messages: [
                {
                    body: input.body,
                    delay_ms: input.delayMs ?? input.delay_ms,
                },
            ],
        };
        const response = await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages`, body);
        const id = response.data.message_ids?.[0] ?? `msg_${Date.now()}`;
        return {
            id,
            body: input.body,
            contentType: input.contentType ?? 'application/json',
            sentAt: new Date().toISOString(),
            receiveCount: 0,
            attributes: {
                priority: input.priority ?? 'NORMAL',
                deduplicationId: input.deduplicationId,
                groupId: input.groupId,
                custom: input.attributes,
            },
        };
    }
    async sendBatch(queueName, inputs) {
        const body = {
            messages: inputs.map((i) => ({
                body: i.body,
                delay_ms: i.delayMs ?? i.delay_ms,
            })),
        };
        const response = await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages`, body);
        const ids = response.data.message_ids ?? inputs.map((_, idx) => `msg_${Date.now()}_${idx}`);
        return inputs.map((inp, idx) => ({
            id: ids[idx],
            body: inp.body,
            contentType: inp.contentType ?? 'application/json',
            sentAt: new Date().toISOString(),
            receiveCount: 0,
            attributes: {
                priority: inp.priority ?? 'NORMAL',
                deduplicationId: inp.deduplicationId,
                groupId: inp.groupId,
                custom: inp.attributes,
            },
        }));
    }
    async receive(queueName, options) {
        const count = options?.maxMessages ?? options?.count ?? 10;
        const v = options?.visibilityTimeoutMs ?? options?.visibility_timeout_ms;
        const body = { count };
        if (v !== undefined)
            body.visibility_timeout_ms = v;
        const response = await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages/poll`, body);
        return response.data.messages.map((m) => ({
            id: m.id,
            body: m.body,
            contentType: 'application/json',
            sentAt: new Date().toISOString(),
            firstReceivedAt: new Date().toISOString(),
            receiveCount: m.delivery_attempt ?? 1,
            visibilityTimeoutExpiresAt: undefined,
            attributes: { priority: 'NORMAL' },
        }));
    }
    // deleteMessage historically used DELETE — now map to POST .../ack
    async deleteMessage(queueName, messageId) {
        await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages/${encodeURIComponent(messageId)}/ack`);
    }
    // Explicit ack alias (dashboard uses this name)
    async ackMessage(queueName, messageId) {
        await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages/${encodeURIComponent(messageId)}/ack`);
    }
    // Poll alias for clarity
    async poll(queueName, options) {
        return this.receive(queueName, options);
    }
    async peek(_queueName, _options) {
        // No backend peek — return empty connection for prototype; alternatively could poll with visibility 0
        return toMessageConnection([], { offset: 0, limit: 0, total: 0, has_more: false });
    }
    async purge(queueName) {
        const response = await this.http.post(`/queues/${encodeURIComponent(queueName)}/purge`);
        return response.data.deleted ?? -1;
    }
    async listDLQ(_queueName, _options) {
        // No backend DLQ support — return empty
        return {
            edges: [],
            pageInfo: { hasNextPage: false, hasPreviousPage: false, startCursor: null, endCursor: null },
            totalCount: 0,
        };
    }
    async redriveDLQ(_queueName, _maxMessages) {
        // Not supported
        throw errors_1.Errors.notFound('redriveDLQ not supported by backend');
    }
    async getStats(queueName) {
        if (queueName) {
            const response = await this.http.get(`/queues/${encodeURIComponent(queueName)}/stats`);
            const raw = response.data;
            return {
                totalQueues: 1,
                totalMessages: raw.total_messages,
                totalMessagesSent: raw.messages_enqueued,
                totalMessagesReceived: raw.messages_dequeued,
                totalMessagesDeadLettered: raw.dlq_messages,
                avgQueueDepth: raw.total_messages,
                avgProcessingTimeMs: 0,
            };
        }
        // Aggregate stats for all queues not directly supported — sum per-queue
        const list = await this.listQueues({ first: 100 });
        let totalMessages = 0;
        let totalSent = 0;
        let totalReceived = 0;
        let totalDLQ = 0;
        for (const q of list.edges) {
            try {
                const s = await this.getStats(q.node.name);
                totalMessages += s.totalMessages;
                totalSent += s.totalMessagesSent;
                totalReceived += s.totalMessagesReceived;
                totalDLQ += s.totalMessagesDeadLettered;
            }
            catch { /* ignore */ }
        }
        return {
            totalQueues: list.totalCount,
            totalMessages,
            totalMessagesSent: totalSent,
            totalMessagesReceived: totalReceived,
            totalMessagesDeadLettered: totalDLQ,
            avgQueueDepth: list.totalCount ? totalMessages / list.totalCount : 0,
            avgProcessingTimeMs: 0,
        };
    }
    async *list(namePattern) {
        let offset = 0;
        const limit = 100;
        while (true) {
            const page = await this.listQueues({ first: limit, after: String(offset), namePattern });
            for (const edge of page.edges)
                yield edge.node;
            if (!page.pageInfo.hasNextPage)
                break;
            offset += limit;
        }
    }
}
exports.QueueClient = QueueClient;
//# sourceMappingURL=queue.js.map