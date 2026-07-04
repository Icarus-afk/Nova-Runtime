import type { HttpClient } from './client';
import type {
  Queue, QueueMessage, QueueSendInput, QueueCreateInput,
  DeadLetterMessage, QueueStats, Connection, PaginationInput
} from './types';

export class QueueClient {
  constructor(
    private http: HttpClient
  ) {}

  async listQueues(options?: PaginationInput & { namePattern?: string }): Promise<Connection<Queue>> {
    const response = await this.http.get<Connection<Queue>>('/queue', { query: { ...(options as Record<string, unknown> ?? {}) } });
    return response.data;
  }

  async getQueue(name: string): Promise<Queue> {
    const response = await this.http.get<Queue>(`/queue/${encodeURIComponent(name)}`);
    return response.data;
  }

  async createQueue(input: QueueCreateInput): Promise<Queue> {
    const response = await this.http.post<Queue>('/queue', input);
    return response.data;
  }

  async deleteQueue(name: string, force?: boolean): Promise<void> {
    await this.http.delete(`/queue/${encodeURIComponent(name)}`, { query: { force } });
  }

  async send<T = unknown>(queueName: string, input: QueueSendInput<T>): Promise<QueueMessage<T>> {
    const response = await this.http.post<QueueMessage<T>>(`/queue/${encodeURIComponent(queueName)}/messages`, input);
    return response.data;
  }

  async sendBatch<T = unknown>(queueName: string, inputs: QueueSendInput<T>[]): Promise<QueueMessage<T>[]> {
    const response = await this.http.post<QueueMessage<T>[]>(
      `/queue/${encodeURIComponent(queueName)}/messages/batch`, { messages: inputs }
    );
    return response.data;
  }

  async receive<T = unknown>(
    queueName: string,
    options?: {
      maxMessages?: number;
      visibilityTimeoutMs?: number;
    }
  ): Promise<QueueMessage<T>[]> {
    const response = await this.http.post<QueueMessage<T>[]>(
      `/queue/${encodeURIComponent(queueName)}/messages/receive`, options ?? {}
    );
    return response.data;
  }

  async deleteMessage(queueName: string, messageId: string): Promise<void> {
    await this.http.delete(
      `/queue/${encodeURIComponent(queueName)}/messages/${messageId}`
    );
  }

  async peek<T = unknown>(
    queueName: string,
    options?: PaginationInput
  ): Promise<Connection<QueueMessage<T>>> {
    const response = await this.http.get<Connection<QueueMessage<T>>>(
      `/queue/${encodeURIComponent(queueName)}/messages/peek`, { query: options as Record<string, unknown> }
    );
    return response.data;
  }

  async purge(queueName: string): Promise<number> {
    const response = await this.http.post<{ deleted: number }>(
      `/queue/${encodeURIComponent(queueName)}/purge`
    );
    return response.data.deleted;
  }

  async listDLQ<T = unknown>(
    queueName: string,
    options?: PaginationInput
  ): Promise<Connection<DeadLetterMessage<T>>> {
    const response = await this.http.get<Connection<DeadLetterMessage<T>>>(
      `/queue/${encodeURIComponent(queueName)}/dlq`, { query: options as Record<string, unknown> }
    );
    return response.data;
  }

  async redriveDLQ(queueName: string, maxMessages?: number): Promise<number> {
    const response = await this.http.post<{ redriven: number }>(
      `/queue/${encodeURIComponent(queueName)}/dlq/redrive`, { maxMessages }
    );
    return response.data.redriven;
  }

  async getStats(queueName?: string): Promise<QueueStats> {
    const path = queueName ? `/queue/${encodeURIComponent(queueName)}/stats` : '/queue/stats';
    const response = await this.http.get<QueueStats>(path);
    return response.data;
  }

  async *list(namePattern?: string): AsyncIterable<Queue> {
    let cursor: string | undefined;
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
