use crate::types::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub struct PipelineMetrics {
    pub operations_total: AtomicU64,
    pub operations_by_type: Mutex<HashMap<OperationType, u64>>,
    pub operations_by_status: Mutex<HashMap<StatusCode, u64>>,

    pub queue_depth: AtomicU64,
    pub queue_rejected: AtomicU64,
    pub queue_wait_time_ns: Mutex<Histogram>,

    pub rate_limit_hits: AtomicU64,
    pub rate_limit_waived: AtomicU64,

    pub circuit_opens: AtomicU64,
    pub circuit_half_opens: AtomicU64,
    pub circuit_closes: AtomicU64,
    pub circuit_rejected: AtomicU64,

    pub retry_attempts: AtomicU64,
    pub retry_successes: AtomicU64,
    pub retry_exhaustions: AtomicU64,

    pub cancelled_operations: AtomicU64,
    pub deadline_exceeded: AtomicU64,

    pub parse_errors: AtomicU64,
    pub validation_errors: AtomicU64,
    pub authorization_errors: AtomicU64,
    pub execution_errors: AtomicU64,

    pub active_operations: AtomicU64,
    pub current_queue_depth: AtomicU64,
    pub current_rate_limit_remaining: AtomicU64,

    pub total_duration_ns: AtomicU64,
    pub total_ops_completed: AtomicU64,
    pub stage_latency: Mutex<HashMap<PipelineStage, (u64, u64)>>,
}

#[derive(Debug, Clone)]
pub struct Histogram {
    buckets: Vec<(u64, u64)>,
    total: u64,
    sum: u64,
}

impl Histogram {
    pub fn new() -> Self {
        let bounds = vec![
            1_000,
            5_000,
            10_000,
            50_000,
            100_000,
            500_000,
            1_000_000,
            5_000_000,
            10_000_000,
            50_000_000,
            100_000_000,
            500_000_000,
            1_000_000_000,
            5_000_000_000,
            10_000_000_000,
            60_000_000_000,
            u64::MAX,
        ];
        Histogram {
            buckets: bounds.into_iter().map(|b| (b, 0)).collect(),
            total: 0,
            sum: 0,
        }
    }

    pub fn record(&mut self, value_ns: u64) {
        self.total += 1;
        self.sum += value_ns;
        for (upper, count) in &mut self.buckets {
            if value_ns <= *upper {
                *count += 1;
                break;
            }
        }
    }

    pub fn p50(&self) -> u64 {
        self.percentile(50.0)
    }

    pub fn p99(&self) -> u64 {
        self.percentile(99.0)
    }

    pub fn avg(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            self.sum / self.total
        }
    }

    pub fn count(&self) -> u64 {
        self.total
    }

    fn percentile(&self, pct: f64) -> u64 {
        if self.total == 0 {
            return 0;
        }
        let target = ((self.total as f64) * pct / 100.0).ceil() as u64;
        let mut cumulative = 0;
        for (upper, count) in &self.buckets {
            cumulative += count;
            if cumulative >= target {
                return *upper;
            }
        }
        u64::MAX
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MetricsSnapshot {
    pub operations_total: u64,
    pub active_operations: u64,
    pub queue_depth: u64,
    pub queue_rejected: u64,
    pub rate_limit_hits: u64,
    pub circuit_opens: u64,
    pub circuit_rejected: u64,
    pub retry_attempts: u64,
    pub cancelled: u64,
    pub deadline_exceeded: u64,
    pub parse_errors: u64,
    pub validation_errors: u64,
    pub authorization_errors: u64,
    pub execution_errors: u64,
    pub avg_latency_ns: u64,
    pub p50_latency_ns: u64,
    pub p99_latency_ns: u64,
}

impl PipelineMetrics {
    pub fn new() -> Self {
        PipelineMetrics {
            operations_total: AtomicU64::new(0),
            operations_by_type: Mutex::new(HashMap::new()),
            operations_by_status: Mutex::new(HashMap::new()),
            queue_depth: AtomicU64::new(0),
            queue_rejected: AtomicU64::new(0),
            queue_wait_time_ns: Mutex::new(Histogram::new()),
            rate_limit_hits: AtomicU64::new(0),
            rate_limit_waived: AtomicU64::new(0),
            circuit_opens: AtomicU64::new(0),
            circuit_half_opens: AtomicU64::new(0),
            circuit_closes: AtomicU64::new(0),
            circuit_rejected: AtomicU64::new(0),
            retry_attempts: AtomicU64::new(0),
            retry_successes: AtomicU64::new(0),
            retry_exhaustions: AtomicU64::new(0),
            cancelled_operations: AtomicU64::new(0),
            deadline_exceeded: AtomicU64::new(0),
            parse_errors: AtomicU64::new(0),
            validation_errors: AtomicU64::new(0),
            authorization_errors: AtomicU64::new(0),
            execution_errors: AtomicU64::new(0),
            active_operations: AtomicU64::new(0),
            current_queue_depth: AtomicU64::new(0),
            current_rate_limit_remaining: AtomicU64::new(0),
            total_duration_ns: AtomicU64::new(0),
            total_ops_completed: AtomicU64::new(0),
            stage_latency: Mutex::new(HashMap::new()),
        }
    }

    pub fn record_operation(&self, op_type: OperationType, status: StatusCode) {
        self.operations_total.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut by_type) = self.operations_by_type.lock() {
            *by_type.entry(op_type).or_insert(0) += 1;
        }
        if let Ok(mut by_status) = self.operations_by_status.lock() {
            *by_status.entry(status).or_insert(0) += 1;
        }
    }

    pub fn record_stage(&self, stage: PipelineStage, duration_ns: u64) {
        if let Ok(mut sl) = self.stage_latency.lock() {
            let entry = sl.entry(stage).or_insert((0, 0));
            entry.0 += duration_ns;
            entry.1 += 1;
        }
    }

    pub fn record_queue_wait(&self, wait_ns: u64) {
        if let Ok(mut hist) = self.queue_wait_time_ns.lock() {
            hist.record(wait_ns);
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let hist = self.queue_wait_time_ns.lock().unwrap();
        MetricsSnapshot {
            operations_total: self.operations_total.load(Ordering::Relaxed),
            active_operations: self.active_operations.load(Ordering::Relaxed),
            queue_depth: self.queue_depth.load(Ordering::Relaxed),
            queue_rejected: self.queue_rejected.load(Ordering::Relaxed),
            rate_limit_hits: self.rate_limit_hits.load(Ordering::Relaxed),
            circuit_opens: self.circuit_opens.load(Ordering::Relaxed),
            circuit_rejected: self.circuit_rejected.load(Ordering::Relaxed),
            retry_attempts: self.retry_attempts.load(Ordering::Relaxed),
            cancelled: self.cancelled_operations.load(Ordering::Relaxed),
            deadline_exceeded: self.deadline_exceeded.load(Ordering::Relaxed),
            parse_errors: self.parse_errors.load(Ordering::Relaxed),
            validation_errors: self.validation_errors.load(Ordering::Relaxed),
            authorization_errors: self.authorization_errors.load(Ordering::Relaxed),
            execution_errors: self.execution_errors.load(Ordering::Relaxed),
            avg_latency_ns: hist.avg(),
            p50_latency_ns: hist.p50(),
            p99_latency_ns: hist.p99(),
        }
    }
}

impl Default for PipelineMetrics {
    fn default() -> Self {
        Self::new()
    }
}
