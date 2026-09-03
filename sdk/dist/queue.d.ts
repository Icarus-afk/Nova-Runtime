import type { HttpClient } from './client';
import type { Queue, QueueMessage, QueueSendInput, QueueCreateInput, DeadLetterMessage, QueueStats, Connection, PaginationInput } from './types';
export declare class QueueClient {
    private http;
    constructor(http: HttpClient);
    listQueues(options?: PaginationInput & {
        namePattern?: string;
        limit?: number;
        offset?: number;
    }): Promise<Connection<Queue>>;
    getQueue(name: string): Promise<Queue>;
    createQueue(input: QueueCreateInput): Promise<Queue>;
    deleteQueue(name: string, _force?: boolean): Promise<void>;
    send<T = unknown>(queueName: string, input: QueueSendInput<T>): Promise<QueueMessage<T>>;
    sendBatch<T = unknown>(queueName: string, inputs: QueueSendInput<T>[]): Promise<QueueMessage<T>[]>;
    receive<T = unknown>(queueName: string, options?: {
        maxMessages?: number;
        count?: number;
        visibilityTimeoutMs?: number;
        visibility_timeout_ms?: number;
    }): Promise<QueueMessage<T>[]>;
    deleteMessage(queueName: string, messageId: string): Promise<void>;
    ackMessage(queueName: string, messageId: string): Promise<void>;
    poll<T = unknown>(queueName: string, options?: {
        count?: number;
        visibilityTimeoutMs?: number;
    }): Promise<QueueMessage<T>[]>;
    peek<T = unknown>(_queueName: string, _options?: PaginationInput): Promise<Connection<QueueMessage<T>>>;
    purge(queueName: string): Promise<number>;
    listDLQ<T = unknown>(_queueName: string, _options?: PaginationInput): Promise<Connection<DeadLetterMessage<T>>>;
    redriveDLQ(_queueName: string, _maxMessages?: number): Promise<number>;
    getStats(queueName?: string): Promise<QueueStats>;
    list(namePattern?: string): AsyncIterable<Queue>;
}
