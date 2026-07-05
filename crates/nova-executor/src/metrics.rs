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
        let hist = self.queue_wait_time_ns.lock().expect("metrics mutex poisoned");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_record_operation_increments_total() {
        let m = PipelineMetrics::new();
        m.record_operation(OperationType::Get, StatusCode::Ok);
        assert_eq!(m.operations_total.load(Ordering::Relaxed), 1);
        m.record_operation(OperationType::Create, StatusCode::Created);
        assert_eq!(m.operations_total.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_metrics_record_operation_tracks_by_type() {
        let m = PipelineMetrics::new();
        m.record_operation(OperationType::Get, StatusCode::Ok);
        m.record_operation(OperationType::Get, StatusCode::Ok);
        m.record_operation(OperationType::Create, StatusCode::Created);

        let by_type = m.operations_by_type.lock().unwrap();
        assert_eq!(*by_type.get(&OperationType::Get).unwrap(), 2);
        assert_eq!(*by_type.get(&OperationType::Create).unwrap(), 1);
    }

    #[test]
    fn test_metrics_record_operation_tracks_by_status() {
        let m = PipelineMetrics::new();
        m.record_operation(OperationType::Get, StatusCode::Ok);
        m.record_operation(OperationType::Get, StatusCode::NotFound);

        let by_status = m.operations_by_status.lock().unwrap();
        assert_eq!(*by_status.get(&StatusCode::Ok).unwrap(), 1);
        assert_eq!(*by_status.get(&StatusCode::NotFound).unwrap(), 1);
    }

    #[test]
    fn test_metrics_record_stage_tracks_latency() {
        let m = PipelineMetrics::new();
        m.record_stage(PipelineStage::Parse, 1000);
        m.record_stage(PipelineStage::Parse, 2000);
        m.record_stage(PipelineStage::Validate, 500);

        let sl = m.stage_latency.lock().unwrap();
        let (parse_total, parse_count) = sl.get(&PipelineStage::Parse).unwrap();
        assert_eq!(*parse_total, 3000);
        assert_eq!(*parse_count, 2);
        let (validate_total, validate_count) = sl.get(&PipelineStage::Validate).unwrap();
        assert_eq!(*validate_total, 500);
        assert_eq!(*validate_count, 1);
    }

    #[test]
    fn test_metrics_record_queue_wait_records_histogram() {
        let m = PipelineMetrics::new();
        m.record_queue_wait(1000);
        m.record_queue_wait(2000);
        m.record_queue_wait(3000);

        let hist = m.queue_wait_time_ns.lock().unwrap();
        assert_eq!(hist.count(), 3);
        assert_eq!(hist.avg(), 2000);
    }

    #[test]
    fn test_metrics_snapshot_returns_values() {
        let m = PipelineMetrics::new();
        m.record_operation(OperationType::Get, StatusCode::Ok);
        m.record_queue_wait(5000);
        m.rate_limit_hits.fetch_add(3, Ordering::Relaxed);
        m.parse_errors.fetch_add(1, Ordering::Relaxed);
        m.circuit_opens.fetch_add(2, Ordering::Relaxed);

        let snap = m.snapshot();
        assert_eq!(snap.operations_total, 1);
        assert_eq!(snap.rate_limit_hits, 3);
        assert_eq!(snap.parse_errors, 1);
        assert_eq!(snap.circuit_opens, 2);
        assert_eq!(snap.active_operations, 0);
        assert_eq!(snap.queue_depth, 0);
    }

    #[test]
    fn test_histogram_new_has_zero_count() {
        let h = Histogram::new();
        assert_eq!(h.count(), 0);
        assert_eq!(h.avg(), 0);
        assert_eq!(h.p50(), 0);
        assert_eq!(h.p99(), 0);
    }

    #[test]
    fn test_histogram_records_values() {
        let mut h = Histogram::new();
        h.record(500);
        h.record(1500);
        h.record(7500);

        assert_eq!(h.count(), 3);
        assert_eq!(h.avg(), (500 + 1500 + 7500) / 3);
    }

    #[test]
    fn test_histogram_percentiles() {
        let mut h = Histogram::new();
        // Record enough values for percentile calculation
        for _ in 0..100 {
            h.record(5000); // falls in 5000 bucket
        }
        // p50 should be 5000 (the upper bound of the bucket containing the 50th value)
        assert!(h.p50() >= 5000);
        assert!(h.p99() >= 5000);
    }

    #[test]
    fn test_histogram_wide_range() {
        let mut h = Histogram::new();
        h.record(100_000_000);
        assert_eq!(h.count(), 1);
        assert_eq!(h.avg(), 100_000_000);
    }

    #[test]
    fn test_metrics_snapshot_latency_values() {
        let m = PipelineMetrics::new();
        m.record_queue_wait(1000);
        m.record_queue_wait(5000);

        let snap = m.snapshot();
        assert_eq!(snap.avg_latency_ns, 3000);
        assert!(snap.p50_latency_ns >= 1000);
        assert!(snap.p99_latency_ns >= 5000);
    }

    #[test]
    fn test_metrics_retry_counters() {
        let m = PipelineMetrics::new();
        m.retry_attempts.fetch_add(5, Ordering::Relaxed);
        m.retry_successes.fetch_add(3, Ordering::Relaxed);
        m.retry_exhaustions.fetch_add(1, Ordering::Relaxed);

        assert_eq!(m.retry_attempts.load(Ordering::Relaxed), 5);
        assert_eq!(m.retry_successes.load(Ordering::Relaxed), 3);
        assert_eq!(m.retry_exhaustions.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_metrics_error_counters() {
        let m = PipelineMetrics::new();
        m.parse_errors.fetch_add(2, Ordering::Relaxed);
        m.validation_errors.fetch_add(1, Ordering::Relaxed);
        m.authorization_errors.fetch_add(3, Ordering::Relaxed);
        m.execution_errors.fetch_add(4, Ordering::Relaxed);

        assert_eq!(m.parse_errors.load(Ordering::Relaxed), 2);
        assert_eq!(m.validation_errors.load(Ordering::Relaxed), 1);
        assert_eq!(m.authorization_errors.load(Ordering::Relaxed), 3);
        assert_eq!(m.execution_errors.load(Ordering::Relaxed), 4);
    }

    #[test]
    fn test_metrics_cancellation_counters() {
        let m = PipelineMetrics::new();
        m.cancelled_operations.fetch_add(2, Ordering::Relaxed);
        m.deadline_exceeded.fetch_add(1, Ordering::Relaxed);

        assert_eq!(m.cancelled_operations.load(Ordering::Relaxed), 2);
        assert_eq!(m.deadline_exceeded.load(Ordering::Relaxed), 1);
    }
}
