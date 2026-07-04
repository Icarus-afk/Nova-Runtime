export interface Connection<T, C = Cursor> {
    edges: Edge<T, C>[];
    pageInfo: PageInfo;
    totalCount: number;
}
export interface Edge<T, C = Cursor> {
    node: T;
    cursor: C;
}
export interface PageInfo {
    hasNextPage: boolean;
    hasPreviousPage: boolean;
    startCursor: Cursor | null;
    endCursor: Cursor | null;
}
export type Cursor = string;
export interface PaginationInput {
    first?: number;
    after?: Cursor;
    last?: number;
    before?: Cursor;
}
export interface SortInput {
    field: string;
    direction: 'ASC' | 'DESC';
}
export interface QueryResult<T = Record<string, unknown>> {
    columns: ColumnInfo[];
    rows: T[];
    rowCount: number;
    executionTimeMs: number;
    warnings?: string[];
}
export interface ColumnInfo {
    name: string;
    dataType: string;
    nullable: boolean;
    primaryKey: boolean;
    defaultValue: unknown;
    comment?: string;
}
export interface TableInfo {
    name: string;
    schema: string;
    columns: ColumnInfo[];
    primaryKey: string[];
    indexes: IndexInfo[];
    rowCount: number;
    sizeBytes: number;
    createdAt: string;
    updatedAt: string;
}
export interface IndexInfo {
    name: string;
    columns: string[];
    unique: boolean;
    primary: boolean;
    indexType: 'BTREE' | 'HASH' | 'GIN' | 'GIANT' | 'FULLTEXT';
}
export interface HealthStatus {
    status: 'healthy' | 'degraded' | 'unhealthy';
    uptimeSeconds: number;
    version: string;
    subsystems: SubsystemHealth[];
    lastStartup: string;
}
export interface SubsystemHealth {
    name: string;
    status: 'healthy' | 'degraded' | 'unhealthy';
    latencyMs: number;
    lastError?: string;
    lastChecked: string;
}
export interface MetricsSnapshot {
    collectedAt: string;
    timeRange: {
        start: string;
        end: string;
    };
    system: SystemMetrics;
    subsystems: SubsystemMetrics;
}
export interface SystemMetrics {
    cpuUsagePercent: number;
    memoryUsageBytes: number;
    memoryTotalBytes: number;
    diskUsageBytes: number;
    diskTotalBytes: number;
    networkBytesIn: number;
    networkBytesOut: number;
    openFileDescriptors: number;
    connections: number;
}
export interface SubsystemMetrics {
    database?: DatabaseMetrics;
    cache?: CacheMetrics;
    queue?: QueueMetrics;
    scheduler?: SchedulerMetrics;
    search?: SearchMetrics;
    blob?: BlobMetrics;
    graphql?: GraphQLMetrics;
}
export interface DatabaseMetrics {
    queriesTotal: number;
    queriesPerSecond: number;
    avgLatencyMs: number;
    p50LatencyMs: number;
    p95LatencyMs: number;
    p99LatencyMs: number;
    activeConnections: number;
    cacheHitRate: number;
}
export interface CacheMetrics {
    hits: number;
    misses: number;
    hitRate: number;
    entries: number;
    memoryUsedBytes: number;
    evictions: number;
}
export interface QueueMetrics {
    messagesSent: number;
    messagesReceived: number;
    messagesDeleted: number;
    messagesDeadLettered: number;
    queuesCount: number;
    totalMessages: number;
    avgLatencyMs: number;
}
export interface SchedulerMetrics {
    jobsExecuted: number;
    jobsFailed: number;
    activeJobs: number;
    avgExecutionTimeMs: number;
    successRate: number;
}
export interface SearchMetrics {
    queriesTotal: number;
    indexingTotal: number;
    avgQueryLatencyMs: number;
    indexesCount: number;
    documentsIndexed: number;
}
export interface BlobMetrics {
    uploadsTotal: number;
    downloadsTotal: number;
    totalBlobs: number;
    totalStorageBytes: number;
}
export interface GraphQLMetrics {
    queriesTotal: number;
    mutationsTotal: number;
    subscriptionsTotal: number;
    queriesRejected: number;
    avgResolutionTimeMs: number;
    activeSubscriptions: number;
}
export interface BlobMetadata {
    key: string;
    sizeBytes: number;
    contentType: string;
    contentEncoding?: string;
    etag: string;
    md5: string;
    sha256: string;
    storageTier: StorageTier;
    createdAt: string;
    updatedAt: string;
    expiresAt?: string;
    metadata?: BlobUserMetadata;
    url: string;
}
export interface BlobUserMetadata {
    filename?: string;
    description?: string;
    tags?: string[];
    custom?: Record<string, unknown>;
}
export type StorageTier = 'HOT' | 'WARM' | 'COLD';
export interface BlobUploadInput {
    key: string;
    content: Buffer | ReadableStream | Blob | string;
    contentType?: string;
    contentEncoding?: string;
    storageTier?: StorageTier;
    expiresAt?: Date;
    metadata?: BlobUserMetadata;
    overwrite?: boolean;
    onProgress?: (progress: UploadProgress) => void;
}
export interface UploadProgress {
    bytesUploaded: number;
    totalBytes: number;
    percentage: number;
    speedBytesPerSecond: number;
    etaMs: number;
}
export interface BlobDownloadOptions {
    outputPath?: string;
    startByte?: number;
    endByte?: number;
    onProgress?: (progress: DownloadProgress) => void;
    signal?: AbortSignal;
}
export interface DownloadProgress {
    bytesDownloaded: number;
    totalBytes: number;
    percentage: number;
    speedBytesPerSecond: number;
}
export interface BlobFilter {
    contentType?: string;
    storageTier?: StorageTier;
    minSizeBytes?: number;
    maxSizeBytes?: number;
    createdAfter?: Date;
    createdBefore?: Date;
    tags?: string[];
}
export interface BlobListEntry {
    key: string;
    sizeBytes: number;
    contentType: string;
    storageTier: StorageTier;
    createdAt: string;
    etag: string;
    isPrefix: boolean;
}
export interface Queue {
    name: string;
    description?: string;
    createdAt: string;
    updatedAt: string;
    messageCount: number;
    messagesSent: number;
    messagesReceived: number;
    messagesDeleted: number;
    messagesDeadLettered: number;
    oldestMessageAgeMs: number;
    config: QueueConfig;
}
export interface QueueConfig {
    visibilityTimeoutMs: number;
    maxMessageSizeBytes: number;
    messageRetentionMs: number;
    deadLetterMaxReceives: number;
    deadLetterQueue: boolean;
    deliveryDelayMs: number;
}
export interface QueueMessage<T = unknown> {
    id: string;
    body: T;
    contentType: string;
    sentAt: string;
    firstReceivedAt?: string;
    receiveCount: number;
    visibilityTimeoutExpiresAt?: string;
    delayUntil?: string;
    attributes: MessageAttributes;
}
export interface MessageAttributes {
    priority: 'LOW' | 'NORMAL' | 'HIGH' | 'CRITICAL';
    deduplicationId?: string;
    groupId?: string;
    sender?: string;
    custom?: Record<string, unknown>;
}
export interface QueueSendInput<T = unknown> {
    body: T;
    contentType?: string;
    delayMs?: number;
    priority?: 'LOW' | 'NORMAL' | 'HIGH' | 'CRITICAL';
    deduplicationId?: string;
    groupId?: string;
    attributes?: Record<string, unknown>;
}
export interface QueueCreateInput {
    name: string;
    description?: string;
    visibilityTimeoutMs?: number;
    maxMessageSizeBytes?: number;
    messageRetentionMs?: number;
    deadLetterMaxReceives?: number;
    enableDeadLetterQueue?: boolean;
    deliveryDelayMs?: number;
}
export interface DeadLetterMessage<T = unknown> {
    id: string;
    originalMessage: QueueMessage<T>;
    deadLetteredAt: string;
    reason: string;
    receiveCount: number;
    originalQueue: string;
}
export interface QueueStats {
    totalQueues: number;
    totalMessages: number;
    totalMessagesSent: number;
    totalMessagesReceived: number;
    totalMessagesDeadLettered: number;
    avgQueueDepth: number;
    avgProcessingTimeMs: number;
}
export interface Job {
    id: string;
    name: string;
    description?: string;
    type: 'CRON' | 'SCHEDULED_ONCE' | 'EVENT_DRIVEN';
    state: 'ACTIVE' | 'PAUSED' | 'COMPLETED' | 'FAILED' | 'CANCELLED';
    schedule?: CronExpression;
    maxRetries: number;
    retryCount: number;
    timeoutMs: number;
    createdAt: string;
    updatedAt: string;
    lastExecutedAt?: string;
    lastError?: string;
    nextExecutionAt?: string;
    tags?: string[];
    input?: unknown;
    metadata: JobMetadata;
}
export interface CronExpression {
    expression: string;
    description: string;
    timezone: string;
    nextFireTimes: string[];
}
export interface JobMetadata {
    totalExecutions: number;
    successfulExecutions: number;
    failedExecutions: number;
    avgDurationMs: number;
    lastExecutionId?: string;
}
export interface JobExecution {
    id: string;
    jobId: string;
    jobName: string;
    status: ExecutionStatus;
    startedAt: string;
    completedAt?: string;
    durationMs: number;
    retryAttempt: number;
    trigger: ExecutionTrigger;
    input?: unknown;
    output?: unknown;
    error?: ExecutionError;
    logs?: ExecutionLogEntry[];
}
export type ExecutionStatus = 'PENDING' | 'RUNNING' | 'SUCCESS' | 'FAILED' | 'SKIPPED' | 'TIMEOUT' | 'CANCELLED';
export type ExecutionTrigger = 'SCHEDULED' | 'MANUAL' | 'EVENT' | 'RETRY';
export interface ExecutionError {
    message: string;
    code: string;
    stackTrace?: string;
    subsystem?: string;
}
export interface ExecutionLogEntry {
    timestamp: string;
    level: string;
    message: string;
    metadata?: Record<string, unknown>;
}
export interface CreateJobInput {
    name: string;
    description?: string;
    type: 'CRON' | 'SCHEDULED_ONCE' | 'EVENT_DRIVEN';
    schedule?: string;
    maxRetries?: number;
    timeoutMs?: number;
    tags?: string[];
    input?: unknown;
    startAt?: Date;
}
export interface UpdateJobInput {
    name?: string;
    description?: string;
    schedule?: string;
    maxRetries?: number;
    timeoutMs?: number;
    tags?: string[];
    input?: unknown;
    state?: 'ACTIVE' | 'PAUSED' | 'CANCELLED';
}
export interface SchedulerStats {
    totalJobs: number;
    activeJobs: number;
    pausedJobs: number;
    failedJobs: number;
    completedJobs: number;
    executionsTotal: number;
    executionsToday: number;
    avgExecutionTimeMs: number;
    p95ExecutionTimeMs: number;
    successRate: number;
    triggersFiredTotal: number;
}
export interface SearchIndex {
    name: string;
    documentCount: number;
    sizeBytes: number;
    fieldCount: number;
    analyzer: string;
    createdAt: string;
    updatedAt: string;
    fields: IndexField[];
}
export interface IndexField {
    name: string;
    type: 'TEXT' | 'KEYWORD' | 'INTEGER' | 'FLOAT' | 'BOOLEAN' | 'DATE' | 'OBJECT' | 'ARRAY';
    searchable: boolean;
    sortable: boolean;
    facetable: boolean;
    stored: boolean;
    analyzer?: string;
    boost: number;
}
export interface SearchResult<T = Record<string, unknown>> {
    id: string;
    index: string;
    document: T;
    score: number;
    highlight?: HighlightResult;
}
export interface HighlightResult {
    fragments: string[];
    field: string;
}
export interface SearchOptions {
    filters?: SearchFilter[];
    sort?: SearchSort;
    fields?: string[];
    highlight?: string[];
    minScore?: number;
    explain?: boolean;
    analyzer?: string;
}
export interface SearchFilter {
    field: string;
    operator: FilterOperator;
    value: unknown;
}
export type FilterOperator = 'EQ' | 'NEQ' | 'GT' | 'GTE' | 'LT' | 'LTE' | 'IN' | 'NOT_IN' | 'EXISTS' | 'NOT_EXISTS' | 'RANGE' | 'PREFIX' | 'WILDCARD' | 'REGEX';
export interface SearchSort {
    field: string;
    direction: 'ASC' | 'DESC';
}
export interface SearchAggregations {
    terms?: TermAggregation[];
    ranges?: RangeAggregation[];
    dateHistogram?: DateHistogramBucket[];
}
export interface TermAggregation {
    field: string;
    buckets: TermBucket[];
}
export interface TermBucket {
    key: string;
    docCount: number;
}
export interface RangeAggregation {
    field: string;
    buckets: RangeBucket[];
}
export interface RangeBucket {
    from?: number;
    to?: number;
    docCount: number;
}
export interface DateHistogramBucket {
    key: string;
    docCount: number;
}
export interface SearchStats {
    totalIndexes: number;
    totalDocuments: number;
    totalSizeBytes: number;
    avgIndexTimeMs: number;
    avgQueryTimeMs: number;
    p95QueryTimeMs: number;
    queriesTotal: number;
    indexingTotal: number;
}
export interface Suggestion {
    text: string;
    score: number;
    frequency: number;
}
export interface CreateIndexInput {
    name: string;
    fields: IndexFieldInput[];
    analyzer?: string;
}
export interface IndexFieldInput {
    name: string;
    type: IndexFieldType;
    searchable?: boolean;
    sortable?: boolean;
    facetable?: boolean;
    stored?: boolean;
    analyzer?: string;
    boost?: number;
}
export type IndexFieldType = 'TEXT' | 'KEYWORD' | 'INTEGER' | 'FLOAT' | 'BOOLEAN' | 'DATE' | 'OBJECT' | 'ARRAY';
export interface SearchResponse<T> {
    edges: Array<{
        node: T;
        cursor: string;
        score: number;
        highlight?: HighlightResult;
    }>;
    pageInfo: PageInfo;
    totalCount: number;
    maxScore: number;
    tookMs: number;
    aggregations?: SearchAggregations;
}
export interface AuthResult {
    accessToken: string;
    refreshToken: string;
    expiresIn: number;
    tokenType: string;
    user: User;
}
export interface User {
    id: string;
    username: string;
    email: string;
    displayName: string;
    roles: Role[];
    status: string;
    emailVerified: boolean;
    createdAt: string;
    updatedAt: string;
    lastLoginAt?: string;
}
export interface Role {
    name: string;
    description: string;
    permissions: string[];
    isSystem: boolean;
    createdAt: string;
}
export interface ApiKey {
    id: string;
    name: string;
    keyPrefix: string;
    permissions: string[];
    roles?: string[];
    expiresAt?: string;
    lastUsedAt?: string;
    createdAt: string;
    isActive: boolean;
}
export interface ApiKeyFull {
    apiKey: ApiKey;
    rawKey: string;
}
export interface RegisterInput {
    username: string;
    email: string;
    password: string;
    displayName: string;
}
export interface ConnectionInfo {
    id: string;
    principalId?: string;
    protocol: string;
    connectedAt: string;
    lastActivity: string;
    remoteAddress: string;
    subsystem: string;
    status: string;
}
export interface CreateTableInput {
    name: string;
    columns: CreateColumnInput[];
    primaryKey?: string[];
    indexes?: CreateIndexInput[];
    ifNotExists?: boolean;
}
export interface CreateColumnInput {
    name: string;
    type: string;
    nullable?: boolean;
    defaultValue?: unknown;
    primaryKey?: boolean;
    unique?: boolean;
}
export interface CreateIndexInput {
    name: string;
    columns: string[];
    unique?: boolean;
    type?: 'BTREE' | 'HASH' | 'GIN' | 'FULLTEXT';
}
export interface Page<T> {
    items: T[];
    cursor?: string;
    hasMore: boolean;
}
