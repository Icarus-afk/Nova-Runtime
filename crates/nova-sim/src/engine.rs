use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use nova_event::EventBus;
use rand::rngs::StdRng;
use rand::SeedableRng;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::atomic::AtomicU32;

pub struct SimClock {
    started: Instant,
    virtual_elapsed: Duration,
    speed: f64,
}

impl SimClock {
    pub fn new() -> Self {
        Self { started: Instant::now(), virtual_elapsed: Duration::ZERO, speed: 1.0 }
    }
    pub fn tick(&mut self, real_dt: Duration) {
        self.virtual_elapsed += real_dt.mul_f64(self.speed);
    }
    pub fn elapsed(&self) -> Duration { self.virtual_elapsed }
    pub fn datetime(&self) -> DateTime<Utc> {
        Utc::now() - self.started.elapsed() + self.virtual_elapsed
    }
    pub fn speed(&self) -> f64 { self.speed }
    pub fn set_speed(&mut self, s: f64) { self.speed = s; }
    pub fn cycle_speed(&mut self) {
        let next = match (self.speed * 100.0).round() as u64 {
            25 => 50,
            50 => 100,
            100 => 200,
            200 => 400,
            _ => 25,
        };
        self.speed = next as f64 / 100.0;
    }
}

pub struct SimRng {
    rng: StdRng,
}

impl SimRng {
    pub fn new(seed: u64) -> Self {
        Self { rng: StdRng::seed_from_u64(seed) }
    }
    pub fn f64(&mut self) -> f64 {
        rand::Rng::r#gen(&mut self.rng)
    }
    pub fn range(&mut self, lo: u64, hi: u64) -> u64 {
        rand::Rng::gen_range(&mut self.rng, lo..hi)
    }
    pub fn bool(&mut self, p: f64) -> bool {
        self.f64() < p
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        let idx = self.range(0, items.len() as u64) as usize;
        &items[idx]
    }
    pub fn gen_bytes(&mut self, buf: &mut [u8]) {
        rand::Rng::fill(&mut self.rng, buf);
    }
}

pub struct SimMetrics {
    pub requests_total: AtomicU64,
    pub requests_active: AtomicU32,
    pub auth_success: AtomicU64,
    pub auth_failure: AtomicU64,
    pub sql_queries: AtomicU64,
    pub sql_slow: AtomicU64,
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub cache_evictions: AtomicU64,
    pub cache_invalidations: AtomicU64,
    pub queue_published: AtomicU64,
    pub queue_consumed: AtomicU64,
    pub queue_depth: AtomicU32,
    pub scheduler_jobs_fired: AtomicU64,
    pub scheduler_jobs_active: AtomicU32,
    pub search_indexed: AtomicU64,
    pub search_queries: AtomicU64,
    pub blob_uploads: AtomicU64,
    pub blob_downloads: AtomicU64,
    pub storage_reads: AtomicU64,
    pub storage_writes: AtomicU64,
    pub storage_retries: AtomicU64,
    pub events_published: AtomicU64,
    pub events_delivered: AtomicU64,
    pub workers_active: AtomicU32,
    pub workers_idle: AtomicU32,
    pub http_2xx: AtomicU64,
    pub http_4xx: AtomicU64,
    pub http_5xx: AtomicU64,
    pub memory_used_mb: AtomicU64,
    pub memory_total_mb: AtomicU64,
    pub cpu_percent: AtomicU32,
    pub uptime_secs: AtomicU64,
    pub load_level: AtomicU32,
}

impl SimMetrics {
    pub fn new() -> Self {
        Self {
            requests_total: AtomicU64::new(0),
            requests_active: AtomicU32::new(0),
            auth_success: AtomicU64::new(0),
            auth_failure: AtomicU64::new(0),
            sql_queries: AtomicU64::new(0),
            sql_slow: AtomicU64::new(0),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            cache_evictions: AtomicU64::new(0),
            cache_invalidations: AtomicU64::new(0),
            queue_published: AtomicU64::new(0),
            queue_consumed: AtomicU64::new(0),
            queue_depth: AtomicU32::new(0),
            scheduler_jobs_fired: AtomicU64::new(0),
            scheduler_jobs_active: AtomicU32::new(0),
            search_indexed: AtomicU64::new(0),
            search_queries: AtomicU64::new(0),
            blob_uploads: AtomicU64::new(0),
            blob_downloads: AtomicU64::new(0),
            storage_reads: AtomicU64::new(0),
            storage_writes: AtomicU64::new(0),
            storage_retries: AtomicU64::new(0),
            events_published: AtomicU64::new(0),
            events_delivered: AtomicU64::new(0),
            workers_active: AtomicU32::new(16),
            workers_idle: AtomicU32::new(16),
            http_2xx: AtomicU64::new(0),
            http_4xx: AtomicU64::new(0),
            http_5xx: AtomicU64::new(0),
            memory_used_mb: AtomicU64::new(256),
            memory_total_mb: AtomicU64::new(2048),
            cpu_percent: AtomicU32::new(15),
            uptime_secs: AtomicU64::new(0),
            load_level: AtomicU32::new(50),
        }
    }
}

#[derive(Clone, Debug)]
pub enum LogLevel { Debug, Info, Warn, Error }

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub subsystem: String,
    pub message: String,
    pub request_id: Option<String>,
    pub duration_ms: Option<u64>,
}

pub struct LogBuffer {
    entries: Vec<LogEntry>,
    capacity: usize,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self { entries: Vec::with_capacity(capacity), capacity }
    }
    pub fn push(&mut self, entry: LogEntry) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LogEntry> + ExactSizeIterator {
        self.entries.iter()
    }
    pub fn len(&self) -> usize { self.entries.len() }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SimStateFlag {
    Running,
    Paused,
    Maintenance,
}

pub struct SimConfig {
    pub seed: u64,
    pub tick_rate_ms: u64,
    pub log_capacity: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self { seed: 42, tick_rate_ms: 200, log_capacity: 500 }
    }
}

pub struct TickContext<'a> {
    pub clock: &'a SimClock,
    pub rng: &'a mut SimRng,
    pub logs: &'a mut LogBuffer,
    pub metrics: &'a SimMetrics,
    pub event_bus: &'a EventBus,
    pub load: u32,
    pub failure_injected: bool,
    pub verbose: bool,
}

pub trait Subsystem {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &mut TickContext);
    fn tick(&mut self, ctx: &mut TickContext);
    fn handle_event(&mut self, _ctx: &mut TickContext, _event: &nova_event::Event) {}
    fn shutdown(&mut self, _ctx: &mut TickContext) {}
}

pub struct SimEngine {
    pub clock: SimClock,
    pub rng: SimRng,
    pub event_bus: Arc<EventBus>,
    pub metrics: SimMetrics,
    pub logs: LogBuffer,
    pub config: SimConfig,
    pub state: SimStateFlag,
    pub load: u32,
    pub subsystems: Vec<Box<dyn Subsystem + Send>>,
    last_tick: Instant,
    pub verbose: bool,
    pub failure_injected: bool,
}

impl SimEngine {
    pub fn new(config: SimConfig) -> Self {
        let bus = Arc::new(EventBus::new(4, nova_event::OverflowPolicy::DropOldest, 65536, 10000));
        Self {
            clock: SimClock::new(),
            rng: SimRng::new(config.seed),
            event_bus: bus,
            metrics: SimMetrics::new(),
            logs: LogBuffer::new(config.log_capacity),
            config,
            state: SimStateFlag::Running,
            load: 50,
            subsystems: Vec::new(),
            last_tick: Instant::now(),
            verbose: false,
            failure_injected: false,
        }
    }

    pub fn register(&mut self, mut sub: Box<dyn Subsystem + Send>) {
        {
            let mut ctx = TickContext {
                clock: &self.clock,
                rng: &mut self.rng,
                logs: &mut self.logs,
                metrics: &self.metrics,
                event_bus: &self.event_bus,
                load: self.load,
                failure_injected: self.failure_injected,
                verbose: self.verbose,
            };
            sub.init(&mut ctx);
        }
        self.subsystems.push(sub);
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick);
        self.last_tick = now;

        if self.state == SimStateFlag::Paused || self.state == SimStateFlag::Maintenance {
            return;
        }
        self.clock.tick(dt);
        let secs = self.clock.elapsed().as_secs();
        self.metrics.uptime_secs.store(secs, Ordering::Relaxed);
        self.metrics.load_level.store(self.load, Ordering::Relaxed);
        let mem_delta = self.rng.range(0, 3) as i64 - 1;
        if mem_delta >= 0 {
            self.metrics.memory_used_mb.fetch_add(mem_delta as u64, Ordering::Relaxed);
        } else {
            self.metrics.memory_used_mb.fetch_sub((-mem_delta) as u64, Ordering::Relaxed);
        }

        let cpu_base = self.load as u32 / 2 + 10;
        let cpu_vary = self.rng.range(0, 15) as u32;
        self.metrics.cpu_percent.store((cpu_base + cpu_vary).min(99), Ordering::Relaxed);

        let rng = &mut self.rng;
        let logs = &mut self.logs;
        let metrics = &self.metrics;
        let event_bus = &self.event_bus;
        for sub in self.subsystems.iter_mut() {
            let mut ctx = TickContext {
                clock: &self.clock,
                rng,
                logs,
                metrics,
                event_bus,
                load: self.load,
                failure_injected: self.failure_injected,
                verbose: self.verbose,
            };
            sub.tick(&mut ctx);
        }
    }

    pub fn log(&mut self, level: LogLevel, subsystem: &str, message: String) {
        self.logs.push(LogEntry {
            timestamp: self.clock.datetime(),
            level,
            subsystem: subsystem.to_string(),
            message,
            request_id: None,
            duration_ms: None,
        });
    }
    pub fn log_detail(&mut self, level: LogLevel, subsystem: &str, message: String, request_id: Option<String>, duration_ms: Option<u64>) {
        self.logs.push(LogEntry {
            timestamp: self.clock.datetime(),
            level,
            subsystem: subsystem.to_string(),
            message,
            request_id,
            duration_ms,
        });
    }

    pub fn inject_failure(&mut self) {
        self.failure_injected = true;
        self.log(LogLevel::Warn, "system", "Failure injection activated — subsystems will experience errors".into());
    }
    pub fn clear_failure(&mut self) {
        self.failure_injected = false;
        self.log(LogLevel::Info, "system", "Failure injection deactivated".into());
    }
}
