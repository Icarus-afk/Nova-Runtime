export type HealthStatus = 'healthy' | 'degraded' | 'critical';

export type UserRole = 'admin' | 'operator' | 'viewer';

export type LogLevel = 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'fatal';

export type MetricType = 'counter' | 'gauge' | 'histogram';

export type AggregationFn = 'avg' | 'sum' | 'min' | 'max' | 'p50' | 'p90' | 'p99';

export type JobType = 'cron' | 'once' | 'interval';

export type JobStatus = 'active' | 'paused' | 'disabled' | 'completed' | 'failed';

export type ExecutionStatus = 'running' | 'success' | 'failed' | 'timeout' | 'cancelled';

export type ExecutionTrigger = 'scheduled' | 'manual' | 'retry';

export type ConcurrencyPolicy = 'allow' | 'skip' | 'queue';

export type MessageState = 'ready' | 'reserved' | 'delayed' | 'buried' | 'dead_letter';

export type FieldType = 'text' | 'keyword' | 'integer' | 'float' | 'boolean' | 'date' | 'geo_point';

export type ConfigValueType = 'string' | 'number' | 'boolean' | 'array' | 'object' | 'duration' | 'size';

export type AlertCondition = 'above' | 'below' | 'equals' | 'changes' | 'absence';

export type AlertSeverity = 'info' | 'warning' | 'critical';

export type NotificationChannelType = 'email' | 'slack' | 'webhook' | 'pager_duty' | 'discord' | 'telegram';

export interface SystemHealth {
  status: HealthStatus;
  uptime_seconds: number;
  version: string;
  cpu: CpuInfo;
  memory: MemoryInfo;
  disk: DiskInfo;
  network: NetworkInfo;
  subsystems: SubsystemStatus[];
  last_checked: number;
}

export interface CpuInfo {
  usage_percent: number;
  load_avg_1m: number;
  load_avg_5m: number;
  load_avg_15m: number;
  cores: number;
  temperature_celsius: number | null;
}

export interface MemoryInfo {
  total_bytes: number;
  used_bytes: number;
  resident_bytes: number;
  allocated_bytes: number;
  cache_bytes: number;
  swap_used_bytes: number;
  swap_total_bytes: number;
}

export interface DiskInfo {
  data_path: string;
  total_bytes: number;
  used_bytes: number;
  free_bytes: number;
  fs_type: string;
  read_ops_per_sec: number;
  write_ops_per_sec: number;
  read_bytes_per_sec: number;
  write_bytes_per_sec: number;
  io_wait_percent: number;
}

export interface NetworkInfo {
  rx_bytes_per_sec: number;
  tx_bytes_per_sec: number;
  rx_packets_per_sec: number;
  tx_packets_per_sec: number;
  connections_active: number;
  connection_errors: number;
  tcp_retransmit_percent: number;
}

export interface SubsystemStatus {
  name: string;
  status: HealthStatus;
  uptime_seconds: number;
  metrics: Record<string, number>;
  last_error: string | null;
  last_error_time: number | null;
}

export interface CollectionInfo {
  name: string;
  document_count: number;
  total_size_bytes: number;
  average_document_size_bytes: number;
  index_count: number;
  created_at: number;
  last_updated_at: number;
}

export interface Document {
  id: string;
  collection: string;
  data: Record<string, unknown>;
  created_at: number;
  updated_at: number;
  version: number;
  size_bytes: number;
}

export interface QueryResult {
  documents: Document[];
  total_count: number | null;
  execution_time_ms: number;
  warning: string | null;
}

export interface CacheStats {
  hit_count: number;
  miss_count: number;
  hit_ratio: number;
  total_entries: number;
  current_size_bytes: number;
  max_size_bytes: number;
  eviction_count: number;
  ttl_expired_count: number;
  oldest_entry_age_seconds: number;
  newest_entry_age_seconds: number;
}

export interface CacheEntry {
  key: string;
  value_size_bytes: number;
  created_at: number;
  expires_at: number | null;
  last_access_at: number;
  access_count: number;
  ttl_seconds: number | null;
}

export interface QueueInfo {
  name: string;
  message_count: number;
  ready_count: number;
  reserved_count: number;
  delayed_count: number;
  buried_count: number;
  dead_letter_count: number;
  enqueue_rate_per_sec: number;
  dequeue_rate_per_sec: number;
  average_message_size_bytes: number;
  oldest_message_age_seconds: number;
  created_at: number;
  max_length: number;
  dead_letter_queue: string | null;
  visibility_timeout_seconds: number;
  retention_seconds: number;
}

export interface QueueMessage {
  id: string;
  body: string;
  state: MessageState;
  priority: number;
  enqueued_at: number;
  reserved_at: number | null;
  delayed_until: number | null;
  attempts: number;
  error_count: number;
  last_error: string | null;
  ttr_seconds: number;
}

export interface JobInfo {
  id: string;
  name: string;
  type: JobType;
  schedule: string | null;
  handler: string;
  payload: Record<string, unknown>;
  status: JobStatus;
  max_retries: number;
  retry_delay_seconds: number;
  timeout_seconds: number;
  created_at: number;
  updated_at: number;
  last_run_at: number | null;
  next_run_at: number | null;
  tags: string[];
  concurrency_policy: ConcurrencyPolicy;
}

export interface JobExecution {
  id: string;
  job_id: string;
  status: ExecutionStatus;
  started_at: number;
  finished_at: number | null;
  duration_ms: number | null;
  result: string | null;
  error: string | null;
  retry_attempt: number;
  trigger: ExecutionTrigger;
}

export interface IndexInfo {
  name: string;
  document_count: number;
  index_size_bytes: number;
  field_count: number;
  query_count: number;
  average_query_time_ms: number;
}

export interface SearchResult {
  hits: SearchHit[];
  total: number;
  execution_time_ms: number;
  max_score: number;
}

export interface SearchHit {
  score: number;
  id: string;
  fields: Record<string, unknown>;
}

export interface BucketInfo {
  name: string;
  file_count: number;
  total_size_bytes: number;
  created_at: number;
  last_modified_at: number;
  allowed_mime_types: string[];
  max_file_size_bytes: number;
  versioning_enabled: boolean;
  public: boolean;
}

export interface BlobObject {
  key: string;
  size_bytes: number;
  mime_type: string;
  etag: string;
  created_at: number;
  last_modified_at: number;
  version_id: string | null;
  metadata: Record<string, string>;
}

export interface DashboardUser {
  id: string;
  username: string;
  email: string;
  role: UserRole;
  mfa_enabled: boolean;
  created_at: number;
  last_login_at: number | null;
  enabled: boolean;
}

export interface ApiKey {
  id: string;
  name: string;
  key_prefix: string;
  role: UserRole;
  permissions: string[];
  created_at: number;
  last_used_at: number | null;
  expires_at: number | null;
  enabled: boolean;
}

export interface ConfigEntry {
  key: string;
  value: unknown;
  type: ConfigValueType;
  description: string;
  mutable: boolean;
  requires_restart: boolean;
  default_value: unknown;
}

export interface LogEntry {
  timestamp: number;
  level: LogLevel;
  subsystem: string;
  message: string;
  fields: Record<string, unknown>;
  file: string;
  line: number;
  trace_id: string | null;
  span_id: string | null;
}

export interface MetricCardData {
  title: string;
  value: string | number;
  unit?: string;
  change?: number;
  changeDirection?: 'up' | 'down';
  color?: 'accent' | 'success' | 'warning' | 'danger' | 'info';
  loading?: boolean;
}

export interface NavItem {
  id: string;
  label: string;
  path: string;
  icon: React.ReactNode;
}

export interface Column<T> {
  key: string;
  header: string;
  render?: (value: unknown, row: T) => React.ReactNode;
  width?: string;
  align?: 'left' | 'center' | 'right';
  sortable?: boolean;
}

export interface PaginationInfo {
  page: number;
  per_page: number;
  total: number;
  total_pages: number;
}
