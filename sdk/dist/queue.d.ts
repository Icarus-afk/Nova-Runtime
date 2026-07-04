import type { HttpClient } from './client';
import type { Queue, QueueMessage, QueueSendInput, QueueCreateInput, DeadLetterMessage, QueueStats, Connection, PaginationInput } from './types';
export declare class QueueClient {
    private http;
    constructor(http: HttpClient);
    listQueues(options?: PaginationInput & {
        namePattern?: string;
    }): Promise<Connection<Queue>>;
    getQueue(name: string): Promise<Queue>;
    createQueue(input: QueueCreateInput): Promise<Queue>;
    deleteQueue(name: string, force?: boolean): Promise<void>;
    send<T = unknown>(queueName: string, input: QueueSendInput<T>): Promise<QueueMessage<T>>;
    sendBatch<T = unknown>(queueName: string, inputs: QueueSendInput<T>[]): Promise<QueueMessage<T>[]>;
    receive<T = unknown>(queueName: string, options?: {
        maxMessages?: number;
        visibilityTimeoutMs?: number;
    }): Promise<QueueMessage<T>[]>;
    deleteMessage(queueName: string, messageId: string): Promise<void>;
    peek<T = unknown>(queueName: string, options?: PaginationInput): Promise<Connection<QueueMessage<T>>>;
    purge(queueName: string): Promise<number>;
    listDLQ<T = unknown>(queueName: string, options?: PaginationInput): Promise<Connection<DeadLetterMessage<T>>>;
    redriveDLQ(queueName: string, maxMessages?: number): Promise<number>;
    getStats(queueName?: string): Promise<QueueStats>;
    list(namePattern?: string): AsyncIterable<Queue>;
}
