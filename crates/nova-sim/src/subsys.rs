use crate::engine::*;
use std::sync::atomic::Ordering;
use std::time::Instant as StdInstant;
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::Method;

macro_rules! log {
    ($ctx:expr, $lvl:expr, $sub:expr, $msg:expr) => {
        $ctx.logs.push(LogEntry {
            timestamp: $ctx.clock.datetime(),
            level: $lvl,
            subsystem: $sub.to_string(),
            message: $msg,
            request_id: None,
            duration_ms: None,
        });
    };
}
macro_rules! log_detail {
    ($ctx:expr, $lvl:expr, $sub:expr, $msg:expr, $rid:expr, $dur:expr) => {
        $ctx.logs.push(LogEntry {
            timestamp: $ctx.clock.datetime(),
            level: $lvl,
            subsystem: $sub.to_string(),
            message: $msg,
            request_id: $rid,
            duration_ms: $dur,
        });
    };
}

struct EndpointDef {
    path: &'static str,
    method: Method,
    body: Option<&'static str>,
}

impl EndpointDef {
    fn get(path: &'static str) -> Self { Self { path, method: Method::GET, body: None } }
    fn post(path: &'static str, body: Option<&'static str>) -> Self { Self { path, method: Method::POST, body } }
}

pub struct HttpSubsystem {
    client: Client,
    target: String,
    counter: u64,
    endpoints: Vec<EndpointDef>,
    server_down: bool,
    probe_tick: u64,
    consecutive_errors: u64,
}

impl HttpSubsystem {
    pub fn new(target: &str) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .expect("http client");
        let endpoints = vec![
            EndpointDef::get("/health"),
            EndpointDef::get("/ready"),
            EndpointDef::get("/api/v1/queues"),
            EndpointDef::get("/api/v1/queues"),
            EndpointDef::get("/api/v1/blobs"),
            EndpointDef::get("/api/v1/blobs/stats"),
            EndpointDef::get("/api/v1/cache/stats"),
            EndpointDef::get("/api/v1/cache/keys"),
            EndpointDef::get("/api/v1/scheduler/stats"),
            EndpointDef::get("/api/v1/scheduler/jobs"),
            EndpointDef::get("/api/v1/search/indexes"),
            EndpointDef::get("/api/v1/sql/tables"),
            EndpointDef::post("/api/v1/auth/login", Some(r#"{"username":"admin","password":"admin123"}"#)),
            EndpointDef::post("/api/v1/sql/query", Some(r#"{"query":"SELECT * FROM iot_weather_sensors"}"#)),
        ];
        Self { client, target: target.to_string(), counter: 0, endpoints, server_down: false, probe_tick: 0, consecutive_errors: 0 }
    }

    fn build_request(&self, ep: &EndpointDef) -> RequestBuilder {
        let url = format!("{}{}", self.target, ep.path);
        let rb = self.client.request(ep.method.clone(), &url);
        match ep.body {
            Some(body) => rb.header("Content-Type", "application/json").body(body.to_string()),
            None => rb,
        }
    }
}

impl Subsystem for HttpSubsystem {
    fn name(&self) -> &'static str { "http" }
    fn init(&mut self, ctx: &mut TickContext) {
        match self.client.get(&format!("{}/health", self.target)).send() {
            Ok(_) => {
                self.server_down = false;
                log!(ctx, LogLevel::Info, "http", format!("Connected to {}", self.target));
            }
            Err(_) => {
                self.server_down = true;
                log!(ctx, LogLevel::Warn, "http", format!("{} unreachable — requests suspended", self.target));
            }
        }
    }
    fn tick(&mut self, ctx: &mut TickContext) {
        self.probe_tick += 1;

        if self.server_down && self.probe_tick % 10 != 0 {
            return;
        }

        let count = if self.server_down { 1 } else { ctx.rng.range(1, (ctx.load as u64 / 8 + 2).min(8)) };
        ctx.metrics.requests_total.fetch_add(count, Ordering::Relaxed);

        for _ in 0..count {
            self.counter += 1;
            let req_id = format!("req-{:06x}", self.counter);
            let ep = if self.server_down {
                &self.endpoints[0] // /health
            } else {
                ctx.rng.pick(&self.endpoints)
            };
            let start = StdInstant::now();
            let method_str = ep.method.as_str();
            let path = ep.path;

            let result = match self.build_request(ep).send() {
                Ok(resp) => {
                    if self.server_down {
                        self.server_down = false;
                        self.consecutive_errors = 0;
                        log!(ctx, LogLevel::Info, "http", format!("{} is back up", self.target));
                    }
                    let status = resp.status();
                    let dur_ms = start.elapsed().as_secs_f64() * 1000.0;
                    (status.as_u16(), dur_ms as u64)
                }
                Err(e) => {
                    let dur_ms = start.elapsed().as_secs_f64() * 1000.0;
                    self.consecutive_errors += 1;
                    if self.consecutive_errors >= 5 && !self.server_down {
                        self.server_down = true;
                        log!(ctx, LogLevel::Error, "http", format!("{} unreachable — requests suspended", self.target));
                    }
                    ctx.metrics.http_5xx.fetch_add(1, Ordering::Relaxed);
                    let kind = if e.is_timeout() { "timeout" } else if e.is_connect() { "refused" } else { "unreachable" };
                    log_detail!(ctx, LogLevel::Error, "http",
                        format!("{method_str} {path} → {kind} ({dur_ms:.0}ms)"), Some(req_id), Some(dur_ms as u64));
                    continue;
                }
            };

            let (status, dur_ms) = result;
            if status >= 500 {
                ctx.metrics.http_5xx.fetch_add(1, Ordering::Relaxed);
                log_detail!(ctx, LogLevel::Error, "http",
                    format!("{method_str} {path} → {status} ({dur_ms}ms)"), Some(req_id), Some(dur_ms));
            } else if status >= 400 {
                ctx.metrics.http_4xx.fetch_add(1, Ordering::Relaxed);
                log_detail!(ctx, LogLevel::Warn, "http",
                    format!("{method_str} {path} → {status} ({dur_ms}ms)"), Some(req_id), Some(dur_ms));
            } else {
                ctx.metrics.http_2xx.fetch_add(1, Ordering::Relaxed);
                if ctx.verbose {
                    log_detail!(ctx, LogLevel::Info, "http",
                        format!("{method_str} {path} → {status} ({dur_ms}ms)"), Some(req_id), Some(dur_ms));
                }
            }
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct AuthSubsystem {
    users: &'static [&'static str],
}

impl AuthSubsystem {
    pub fn new() -> Self { Self { users: &["admin", "alice", "bob", "charlie", "dave", "eve"] } }
}

impl Subsystem for AuthSubsystem {
    fn name(&self) -> &'static str { "auth" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let count = ctx.rng.range(0, ctx.load as u64 / 20 + 1);
        for _ in 0..count {
            let user = ctx.rng.pick(self.users);
            let ok = !(ctx.failure_injected && ctx.rng.bool(0.12)) && ctx.rng.bool(0.92);
            if ok {
                ctx.metrics.auth_success.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Info, "auth", format!("User '{user}' authenticated"));

                let event = nova_event::EventBuilder::new("auth.user.login")
                    .map(|b| b.source(nova_event::Subsystem::System, "auth", "sim", "nova-sim")
                        .build(serde_json::to_vec(&serde_json::json!({"user": user})).unwrap_or_default()));
                if let Ok(e) = event { let _ = ctx.event_bus.publish(e); }
            } else {
                ctx.metrics.auth_failure.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Warn, "auth", format!("Authentication failed for user '{user}'"));
            }
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct SqlSubsystem {
    queries: &'static [&'static str],
}

impl SqlSubsystem {
    pub fn new() -> Self {
        Self { queries: &[
            "SELECT * FROM users WHERE id = ?", "SELECT * FROM items LIMIT 50",
            "INSERT INTO orders (user_id, total) VALUES (?, ?)",
            "UPDATE users SET last_login = NOW() WHERE id = ?",
            "DELETE FROM sessions WHERE expires_at < NOW()",
            "SELECT COUNT(*) FROM items WHERE category = ?",
            "SELECT u.*, o.total FROM users u JOIN orders o ON u.id = o.user_id",
            "INSERT INTO audit_log (action, user_id) VALUES (?, ?)",
        ]}
    }
}

impl Subsystem for SqlSubsystem {
    fn name(&self) -> &'static str { "sql" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let count = ctx.rng.range(0, ctx.load as u64 / 8 + 1);
        for _ in 0..count {
            ctx.metrics.sql_queries.fetch_add(1, Ordering::Relaxed);
            let query = ctx.rng.pick(self.queries);
            let slow = ctx.rng.bool(0.02) || (ctx.failure_injected && ctx.rng.bool(0.08));
            let dur = if slow { ctx.rng.range(500, 3000) } else { ctx.rng.range(1, 80) };
            let is_error = ctx.failure_injected && ctx.rng.bool(0.05);

            if is_error {
                ctx.metrics.sql_slow.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Error, "sql", format!("Query failed: {query} ({dur}ms) — connection timeout"));
            } else if slow {
                ctx.metrics.sql_slow.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Warn, "sql", format!("Slow query ({dur}ms): {query}"));
            } else if ctx.verbose {
                log!(ctx, LogLevel::Info, "sql", format!("{query} ({dur}ms)"));
            }

            let ev_type = if query.starts_with("INSERT") || query.starts_with("UPDATE") || query.starts_with("DELETE") { "write" } else { "read" };
            let event = nova_event::EventBuilder::new(&format!("sql.{ev_type}.executed"))
                .map(|b| b.build(serde_json::to_vec(&serde_json::json!({"query": query, "duration_ms": dur})).unwrap_or_default()));
            if let Ok(e) = event { let _ = ctx.event_bus.publish(e); }
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct CacheSubsystem {
    store: std::collections::HashMap<String, Vec<u8>>,
    max_entries: usize,
}

impl CacheSubsystem {
    pub fn new() -> Self { Self { store: std::collections::HashMap::new(), max_entries: 1000 } }
    fn maybe_evict(&mut self, ctx: &mut TickContext) {
        while self.store.len() > self.max_entries {
            let key = {
                let keys: Vec<String> = self.store.keys().cloned().collect();
                ctx.rng.pick(&keys).clone()
            };
            self.store.remove(&key);
            ctx.metrics.cache_evictions.fetch_add(1, Ordering::Relaxed);
            log!(ctx, LogLevel::Warn, "cache", format!("Evicted key '{key}' — capacity reached"));
        }
    }
}

impl Subsystem for CacheSubsystem {
    fn name(&self) -> &'static str { "cache" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let ops = ctx.rng.range(0, ctx.load as u64 / 5 + 1);
        for _ in 0..ops {
            let hit = ctx.rng.bool(0.85);
            if hit {
                ctx.metrics.cache_hits.fetch_add(1, Ordering::Relaxed);
                if ctx.verbose { log!(ctx, LogLevel::Info, "cache", "Cache HIT".into()); }
            } else {
                ctx.metrics.cache_misses.fetch_add(1, Ordering::Relaxed);
                let key = format!("key-{:04x}", ctx.rng.range(0, 9999));
                self.store.insert(key.clone(), vec![0u8; 64]);
                log!(ctx, LogLevel::Info, "cache", format!("Cache MISS — populated key '{key}' from origin"));
                self.maybe_evict(ctx);
            }
        }
    }
    fn handle_event(&mut self, ctx: &mut TickContext, event: &nova_event::Event) {
        let topic = &event.metadata.event_type.canonical;
        if topic.starts_with("cache.invalidate") {
            let key = topic.strip_prefix("cache.invalidate.").unwrap_or("");
            if !key.is_empty() && !key.starts_with("pattern.") {
                self.store.remove(key);
                ctx.metrics.cache_invalidations.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Info, "cache", format!("Invalidated key '{key}' via event bus"));
            } else if let Some(pattern) = key.strip_prefix("pattern.") {
                let pattern = pattern.trim_end_matches('*').trim_end_matches(':');
                self.store.retain(|k, _| !k.starts_with(pattern));
                ctx.metrics.cache_invalidations.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Info, "cache", format!("Invalidated pattern '{pattern}*' via event bus"));
            }
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct QueueSubsystem {
    messages: Vec<String>,
    max_depth: u32,
}

impl QueueSubsystem {
    pub fn new() -> Self { Self { messages: Vec::new(), max_depth: 500 } }
}

impl Subsystem for QueueSubsystem {
    fn name(&self) -> &'static str { "queue" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let produce = ctx.rng.range(0, ctx.load as u64 / 12 + 1) as u32;
        for _ in 0..produce {
            if self.messages.len() < self.max_depth as usize {
                let msg = format!("job-{:06x}", ctx.rng.range(0, 0xFFFFFF));
                self.messages.push(msg.clone());
                ctx.metrics.queue_published.fetch_add(1, Ordering::Relaxed);
                if ctx.verbose { log!(ctx, LogLevel::Info, "queue", format!("Published '{msg}' to queue 'default'")); }
            }
        }
        let consume = ctx.rng.range(0, ctx.load as u64 / 15 + 1) as usize;
        for _ in 0..consume.min(self.messages.len()) {
            let msg = self.messages.remove(0);
            ctx.metrics.queue_consumed.fetch_add(1, Ordering::Relaxed);
            let ok = !(ctx.failure_injected && ctx.rng.bool(0.10));
            if ok {
                log!(ctx, LogLevel::Info, "queue", format!("Consumed and acked '{msg}'"));
            } else {
                log!(ctx, LogLevel::Error, "queue", format!("Consumer failed processing '{msg}' — requeueing"));
                self.messages.push(msg);
            }
        }
        ctx.metrics.queue_depth.store(self.messages.len() as u32, Ordering::Relaxed);
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct SchedulerSubsystem {
    counter: u64,
}

impl SchedulerSubsystem {
    pub fn new() -> Self { Self { counter: 0 } }
}

impl Subsystem for SchedulerSubsystem {
    fn name(&self) -> &'static str { "sched" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let secs = ctx.clock.elapsed().as_secs();
        let jobs_fire = if secs % 30 == 0 { 3 } else if secs % 10 == 0 { 1 } else { 0 };
        for _ in 0..jobs_fire {
            self.counter += 1;
            ctx.metrics.scheduler_jobs_fired.fetch_add(1, Ordering::Relaxed);
            ctx.metrics.scheduler_jobs_active.fetch_add(1, Ordering::Relaxed);
            let job = format!("cron-{:04x}", self.counter);
            log!(ctx, LogLevel::Info, "sched", format!("Fired scheduled job '{job}'"));

            let event = nova_event::EventBuilder::new("scheduler.job.fired")
                .map(|b| b.build(serde_json::to_vec(&serde_json::json!({"job": job})).unwrap_or_default()));
            if let Ok(e) = event { let _ = ctx.event_bus.publish(e); }
        }
        let completed = ctx.rng.range(0, ctx.metrics.scheduler_jobs_active.load(Ordering::Relaxed) as u64 + 1) as u32;
        ctx.metrics.scheduler_jobs_active.fetch_sub(completed, Ordering::Relaxed);
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct SearchSubsystem {
    index_size: usize,
}

impl SearchSubsystem {
    pub fn new() -> Self { Self { index_size: 0 } }
}

impl Subsystem for SearchSubsystem {
    fn name(&self) -> &'static str { "search" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let indexing = ctx.rng.range(0, ctx.load as u64 / 30 + 1);
        for _ in 0..indexing {
            self.index_size += 1;
            ctx.metrics.search_indexed.fetch_add(1, Ordering::Relaxed);
            if ctx.verbose { log!(ctx, LogLevel::Info, "search", "Indexed document in 'main' index".into()); }
        }
        let queries = ctx.rng.range(0, ctx.load as u64 / 15 + 1);
        for _ in 0..queries {
            ctx.metrics.search_queries.fetch_add(1, Ordering::Relaxed);
            let dur = ctx.rng.range(2, 60);
            let results = if self.index_size > 0 { ctx.rng.range(0, self.index_size as u64) } else { 0 };
            if ctx.verbose { log!(ctx, LogLevel::Info, "search", format!("Search query returned {results} results ({dur}ms)")); }
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct BlobSubsystem;

impl BlobSubsystem {
    pub fn new() -> Self { Self }
}

impl Subsystem for BlobSubsystem {
    fn name(&self) -> &'static str { "blob" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let upload = ctx.rng.range(0, ctx.load as u64 / 25 + 1);
        for _ in 0..upload {
            ctx.metrics.blob_uploads.fetch_add(1, Ordering::Relaxed);
            let size = ctx.rng.range(1, 50);
            log!(ctx, LogLevel::Info, "blob", format!("Uploaded blob ({size} KB)"));
        }
        let download = ctx.rng.range(0, ctx.load as u64 / 20 + 1);
        for _ in 0..download {
            ctx.metrics.blob_downloads.fetch_add(1, Ordering::Relaxed);
            if ctx.verbose { log!(ctx, LogLevel::Info, "blob", "Downloaded blob (1.2 MB)".into()); }
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct StorageSubsystem;

impl StorageSubsystem {
    pub fn new() -> Self { Self }
}

impl Subsystem for StorageSubsystem {
    fn name(&self) -> &'static str { "storage" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let reads = ctx.rng.range(0, ctx.load as u64 / 5 + 2);
        let writes = ctx.rng.range(0, ctx.load as u64 / 8 + 1);
        ctx.metrics.storage_reads.fetch_add(reads, Ordering::Relaxed);
        ctx.metrics.storage_writes.fetch_add(writes, Ordering::Relaxed);

        for _ in 0..reads {
            if ctx.failure_injected && ctx.rng.bool(0.08) {
                ctx.metrics.storage_retries.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Warn, "storage", "Read retry #2 — I/O timeout on page 0x3A1F".into());
            }
        }
        for _ in 0..writes {
            if ctx.failure_injected && ctx.rng.bool(0.06) {
                ctx.metrics.storage_retries.fetch_add(1, Ordering::Relaxed);
                log!(ctx, LogLevel::Warn, "storage", "Write retry #1 — checksum mismatch, replaying WAL".into());
            }
        }
        if ctx.verbose && reads + writes > 0 {
            log!(ctx, LogLevel::Info, "storage", format!("{reads} reads, {writes} writes completed"));
        }
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct WorkerSubsystem;

impl WorkerSubsystem {
    pub fn new() -> Self { Self }
}

impl Subsystem for WorkerSubsystem {
    fn name(&self) -> &'static str { "worker" }
    fn init(&mut self, ctx: &mut TickContext) {
        ctx.metrics.workers_idle.store(16, Ordering::Relaxed);
    }
    fn tick(&mut self, ctx: &mut TickContext) {
        let total: u32 = 16;
        let queue_depth = ctx.metrics.queue_depth.load(Ordering::Relaxed);
        let active_jobs = ctx.metrics.scheduler_jobs_active.load(Ordering::Relaxed);
        let needed = (queue_depth / 10 + active_jobs).min(total);
        let active = ctx.rng.range(needed as u64 / 2, needed as u64 + 1) as u32;
        ctx.metrics.workers_active.store(active, Ordering::Relaxed);
        ctx.metrics.workers_idle.store(total - active, Ordering::Relaxed);
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct EventBusSubsystem;

impl EventBusSubsystem {
    pub fn new() -> Self { Self }
}

impl Subsystem for EventBusSubsystem {
    fn name(&self) -> &'static str { "events" }
    fn init(&mut self, _ctx: &mut TickContext) {}
    fn tick(&mut self, ctx: &mut TickContext) {
        let count = ctx.rng.range(0, ctx.load as u64 / 10 + 1);
        for _ in 0..count {
            let topics = &["system.heartbeat", "metrics.snapshot", "health.check.passed", "internal.gc.run"];
            let topic = ctx.rng.pick(topics);
            ctx.metrics.events_published.fetch_add(1, Ordering::Relaxed);
            if ctx.verbose { log!(ctx, LogLevel::Debug, "events", format!("Published '{topic}'")); }

            let event = nova_event::EventBuilder::new(topic)
                .map(|b| b.source(nova_event::Subsystem::System, "sim", "nova-sim", "0")
                    .build(vec![]));
            if let Ok(e) = event { let _ = ctx.event_bus.publish(e); }
        }
        ctx.metrics.events_delivered.store(
            ctx.metrics.events_published.load(Ordering::Relaxed),
            Ordering::Relaxed,
        );
    }
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}
