import type { HttpClient } from './client';
import type {
  Queue, QueueMessage, QueueSendInput, QueueCreateInput,
  DeadLetterMessage, QueueStats, Connection, PaginationInput
} from './types';
import { Errors } from './errors';

interface RawQueueListItem {
  name: string;
  queue_type: string;
  available: number;
  in_flight: number;
  delayed: number;
  total: number;
  paused: boolean;
  max_size?: number;
}

interface RawQueueListResponse {
  data: RawQueueListItem[];
  pagination: { offset: number; limit: number; total: number; has_more: boolean };
}

interface RawQueueGet {
  name: string;
  queue_type: string;
  max_size: number;
  paused: boolean;
}

interface RawPublishResponse {
  published_count: number;
  message_ids: string[];
}

interface RawPollResponse {
  messages: Array<{ id: string; body: unknown; receipt_handle?: string; delivery_attempt?: number; visibility_timeout_ms?: number }>;
  message_count: number;
}

interface RawQueueStats {
  available_messages: number;
  in_flight_messages: number;
  delayed_messages: number;
  total_messages: number;
  dlq_messages: number;
  messages_enqueued: number;
  messages_dequeued: number;
}

function toQueueConnection(queues: Queue[], pagination: { offset: number; limit: number; total: number; has_more: boolean }): Connection<Queue> {
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

function toMessageConnection<T>(messages: QueueMessage<T>[], pagination: { offset: number; limit: number; total: number; has_more: boolean }): Connection<QueueMessage<T>> {
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

function rawQueueToQueue(raw: RawQueueListItem | RawQueueGet): Queue {
  const r: any = raw;
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
  } as Queue;
}

export class QueueClient {
  constructor(
    private http: HttpClient
  ) {}

  async listQueues(options?: PaginationInput & { namePattern?: string; limit?: number; offset?: number }): Promise<Connection<Queue>> {
    const query: Record<string, unknown> = {};
    if ((options as any)?.limit !== undefined) query.limit = (options as any).limit;
    else if (options?.first !== undefined) query.limit = options.first;
    if ((options as any)?.offset !== undefined) query.offset = (options as any).offset;
    else if (options?.after !== undefined) {
      const parsed = parseInt(options.after, 10);
      if (!Number.isNaN(parsed)) query.offset = parsed;
    }
    if (options?.last !== undefined && query.limit === undefined) query.limit = options.last;
    const response = await this.http.get<RawQueueListResponse>('/queues', { query });
    const raw = response.data;
    let items: Queue[] = raw.data.map(rawQueueToQueue);
    if (options?.namePattern) {
      const reStr = '^' + options.namePattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/\\\*/g, '.*').replace(/\\\?/g, '.') + '$';
      try {
        const re = new RegExp(reStr);
        items = items.filter((q) => re.test(q.name));
      } catch { /* ignore */ }
    }
    return toQueueConnection(items, raw.pagination);
  }

  async getQueue(name: string): Promise<Queue> {
    const response = await this.http.get<RawQueueGet>(`/queues/${encodeURIComponent(name)}`);
    return rawQueueToQueue(response.data);
  }

  async createQueue(input: QueueCreateInput): Promise<Queue> {
    // Backend expects { name, durable?, max_length?, max_message_size? }
    const body: Record<string, unknown> = { name: input.name };
    // Map enableDeadLetterQueue -> durable hint (dashboard uses durable)
    if ((input as any).durable !== undefined) body.durable = (input as any).durable;
    else if (input.enableDeadLetterQueue !== undefined) body.durable = true;
    if ((input as any).max_length !== undefined) body.max_length = (input as any).max_length;
    else if ((input as any).maxLength !== undefined) body.max_length = (input as any).maxLength;
    if (input.maxMessageSizeBytes !== undefined) body.max_message_size = input.maxMessageSizeBytes;
    else if ((input as any).max_message_size !== undefined) body.max_message_size = (input as any).max_message_size;

    const response = await this.http.post<any>('/queues', body);
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
    } as Queue;
  }

  async deleteQueue(name: string, _force?: boolean): Promise<void> {
    await this.http.delete(`/queues/${encodeURIComponent(name)}`);
  }

  async send<T = unknown>(queueName: string, input: QueueSendInput<T>): Promise<QueueMessage<T>> {
    const body = {
      messages: [
        {
          body: input.body,
          delay_ms: input.delayMs ?? (input as any).delay_ms,
        },
      ],
    };
    const response = await this.http.post<RawPublishResponse>(`/queues/${encodeURIComponent(queueName)}/messages`, body);
    const id = response.data.message_ids?.[0] ?? `msg_${Date.now()}`;
    return {
      id,
      body: input.body as T,
      contentType: input.contentType ?? 'application/json',
      sentAt: new Date().toISOString(),
      receiveCount: 0,
      attributes: {
        priority: input.priority ?? 'NORMAL',
        deduplicationId: input.deduplicationId,
        groupId: input.groupId,
        custom: input.attributes,
      },
    } as QueueMessage<T>;
  }

  async sendBatch<T = unknown>(queueName: string, inputs: QueueSendInput<T>[]): Promise<QueueMessage<T>[]> {
    const body = {
      messages: inputs.map((i) => ({
        body: i.body,
        delay_ms: i.delayMs ?? (i as any).delay_ms,
      })),
    };
    const response = await this.http.post<RawPublishResponse>(`/queues/${encodeURIComponent(queueName)}/messages`, body);
    const ids: string[] = response.data.message_ids ?? inputs.map((_, idx) => `msg_${Date.now()}_${idx}`);
    return inputs.map((inp, idx) => ({
      id: ids[idx],
      body: inp.body as T,
      contentType: inp.contentType ?? 'application/json',
      sentAt: new Date().toISOString(),
      receiveCount: 0,
      attributes: {
        priority: inp.priority ?? 'NORMAL',
        deduplicationId: inp.deduplicationId,
        groupId: inp.groupId,
        custom: inp.attributes,
      },
    } as QueueMessage<T>));
  }

  async receive<T = unknown>(
    queueName: string,
    options?: {
      maxMessages?: number;
      count?: number;
      visibilityTimeoutMs?: number;
      visibility_timeout_ms?: number;
    }
  ): Promise<QueueMessage<T>[]> {
    const count = options?.maxMessages ?? (options as any)?.count ?? 10;
    const v = options?.visibilityTimeoutMs ?? (options as any)?.visibility_timeout_ms;
    const body: Record<string, unknown> = { count };
    if (v !== undefined) body.visibility_timeout_ms = v;
    const response = await this.http.post<RawPollResponse>(
      `/queues/${encodeURIComponent(queueName)}/messages/poll`, body
    );
    return response.data.messages.map((m) => ({
      id: m.id,
      body: m.body as T,
      contentType: 'application/json',
      sentAt: new Date().toISOString(),
      firstReceivedAt: new Date().toISOString(),
      receiveCount: m.delivery_attempt ?? 1,
      visibilityTimeoutExpiresAt: undefined,
      attributes: { priority: 'NORMAL' as const },
    } as QueueMessage<T>));
  }

  // deleteMessage historically used DELETE — now map to POST .../ack
  async deleteMessage(queueName: string, messageId: string): Promise<void> {
    await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages/${encodeURIComponent(messageId)}/ack`);
  }

  // Explicit ack alias (dashboard uses this name)
  async ackMessage(queueName: string, messageId: string): Promise<void> {
    await this.http.post(`/queues/${encodeURIComponent(queueName)}/messages/${encodeURIComponent(messageId)}/ack`);
  }

  // Poll alias for clarity
  async poll<T = unknown>(queueName: string, options?: { count?: number; visibilityTimeoutMs?: number }): Promise<QueueMessage<T>[]> {
    return this.receive<T>(queueName, options as any);
  }

  async peek<T = unknown>(
    _queueName: string,
    _options?: PaginationInput
  ): Promise<Connection<QueueMessage<T>>> {
    // No backend peek — return empty connection for prototype; alternatively could poll with visibility 0
    return toMessageConnection<T>([], { offset: 0, limit: 0, total: 0, has_more: false });
  }

  async purge(queueName: string): Promise<number> {
    const response = await this.http.post<{ status: string; deleted?: number }>(
      `/queues/${encodeURIComponent(queueName)}/purge`
    );
    return (response.data as any).deleted ?? -1;
  }

  async listDLQ<T = unknown>(
    _queueName: string,
    _options?: PaginationInput
  ): Promise<Connection<DeadLetterMessage<T>>> {
    // No backend DLQ support — return empty
    return {
      edges: [],
      pageInfo: { hasNextPage: false, hasPreviousPage: false, startCursor: null, endCursor: null },
      totalCount: 0,
    };
  }

  async redriveDLQ(_queueName: string, _maxMessages?: number): Promise<number> {
    // Not supported
    throw Errors.notFound('redriveDLQ not supported by backend');
  }

  async getStats(queueName?: string): Promise<QueueStats> {
    if (queueName) {
      const response = await this.http.get<RawQueueStats>(`/queues/${encodeURIComponent(queueName)}/stats`);
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
    const list = await this.listQueues({ first: 100 } as any);
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
      } catch { /* ignore */ }
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

  async *list(namePattern?: string): AsyncIterable<Queue> {
    let offset = 0;
    const limit = 100;
    while (true) {
      const page = await this.listQueues({ first: limit, after: String(offset), namePattern } as any);
      for (const edge of page.edges) yield edge.node;
      if (!page.pageInfo.hasNextPage) break;
      offset += limit;
    }
  }
}
