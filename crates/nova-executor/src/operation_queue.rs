use crate::types::*;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::Instant;
use std::fmt;

pub type CompletionSender = mpsc::Sender<OperationResponse>;

pub struct PendingOperation {
    pub request: OperationRequest,
    pub context: OperationContext,
    pub completion: CompletionSender,
    pub submitted_at: Instant,
    pub deadline: Instant,
    pub retry_count: u8,
    pub priority_age: u32,
}

impl fmt::Debug for PendingOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PendingOperation")
            .field("request.operation_type", &self.request.operation_type)
            .field("retry_count", &self.retry_count)
            .field("priority_age", &self.priority_age)
            .finish()
    }
}

struct QueueInner {
    queues: [VecDeque<PendingOperation>; 4],
    depth: usize,
}

pub struct OperationQueue {
    inner: Mutex<QueueInner>,
    capacity: usize,
    capacity_by_priority: [usize; 4],
    current_depth: AtomicUsize,
    rejected_count: AtomicU64,
    wait_time_total: AtomicU64,
    wait_time_count: AtomicU64,
}

pub struct QueueStats {
    pub depth: usize,
    pub capacity: usize,
    pub rejected: u64,
    pub by_priority: [usize; 4],
    pub avg_wait_ms: u64,
    pub wait_count: u64,
}

impl OperationQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Mutex::new(QueueInner {
                queues: [
                    VecDeque::with_capacity(64),
                    VecDeque::with_capacity(128),
                    VecDeque::with_capacity(512),
                    VecDeque::with_capacity(320),
                ],
                depth: 0,
            }),
            capacity,
            capacity_by_priority: [64, 128, 512, 320],
            current_depth: AtomicUsize::new(0),
            rejected_count: AtomicU64::new(0),
            wait_time_total: AtomicU64::new(0),
            wait_time_count: AtomicU64::new(0),
        }
    }

    pub fn push(&self, op: PendingOperation) -> Result<(), OperationResponse> {
        let mut inner = self.inner.lock();

        if inner.depth >= self.capacity {
            self.rejected_count.fetch_add(1, Ordering::Relaxed);
            return Err(OperationResponse::error(
                ErrorCode::ServiceUnavailable,
                "Queue capacity exceeded",
            ));
        }

        let priority_idx = op.context.operation_priority as usize;
        if inner.queues[priority_idx].len() >= self.capacity_by_priority[priority_idx] {
            self.rejected_count.fetch_add(1, Ordering::Relaxed);
            return Err(OperationResponse::error(
                ErrorCode::ServiceUnavailable,
                "Priority queue capacity exceeded",
            ));
        }

        inner.queues[priority_idx].push_back(op);
        inner.depth += 1;
        self.current_depth.store(inner.depth, Ordering::Release);
        Ok(())
    }

    pub fn pop_priority(&self) -> Option<PendingOperation> {
        let mut inner = self.inner.lock();

        if inner.depth == 0 {
            return None;
        }

        let now = Instant::now();

        // Aging: promote operations from lower priorities if they've waited too long
        for level in (1..4).rev() {
            let queue = &mut inner.queues[level];
            if queue.is_empty() {
                continue;
            }

            let mut to_promote: Vec<PendingOperation> = Vec::new();
            let mut i = queue.len();
            while i > 0 {
                i -= 1;
                let wait = now - queue[i].submitted_at;
                let current_priority = queue[i].context.operation_priority;
                if wait >= current_priority.max_wait() && queue[i].priority_age < 3 {
                    to_promote.push(queue.remove(i).unwrap());
                }
            }

            for mut op in to_promote {
                op.priority_age += 1;
                if let Some(new_priority) = op.context.operation_priority.age_up() {
                    op.context.operation_priority = new_priority;
                    let new_idx = new_priority as usize;
                    inner.queues[new_idx].push_back(op);
                } else {
                    // Already at highest priority, keep in current queue
                    inner.queues[level].push_back(op);
                }
            }
        }

        // Pop from the highest non-empty priority queue
        for level in 0..4 {
            if let Some(op) = inner.queues[level].pop_front() {
                inner.depth -= 1;
                self.current_depth.store(inner.depth, Ordering::Release);

                let wait_ns = now.duration_since(op.submitted_at).as_nanos() as u64;
                self.wait_time_total.fetch_add(wait_ns, Ordering::Relaxed);
                self.wait_time_count.fetch_add(1, Ordering::Relaxed);

                return Some(op);
            }
        }

        None
    }

    pub fn cancel_all(&self, reason: &str) -> usize {
        let mut inner = self.inner.lock();
        let mut count = 0;

        for level in 0..4 {
            while let Some(op) = inner.queues[level].pop_front() {
                let response = OperationResponse::error(ErrorCode::Cancelled, reason);
                let _ = op.completion.send(response);
                count += 1;
            }
        }

        inner.depth = 0;
        self.current_depth.store(0, Ordering::Release);
        count
    }

    pub fn drain(&self, deadline: Instant) -> Vec<PendingOperation> {
        let mut inner = self.inner.lock();
        let mut expired = Vec::new();

        for level in 0..4 {
            let queue = &mut inner.queues[level];
            let mut i = queue.len();
            while i > 0 {
                i -= 1;
                if queue[i].deadline <= deadline {
                    let op = queue.remove(i).unwrap();
                    expired.push(op);
                }
            }
        }

        inner.depth -= expired.len();
        self.current_depth.store(inner.depth, Ordering::Release);
        expired
    }

    pub fn stats(&self) -> QueueStats {
        let inner = self.inner.lock();

        let by_priority = [
            inner.queues[0].len(),
            inner.queues[1].len(),
            inner.queues[2].len(),
            inner.queues[3].len(),
        ];

        let wait_count = self.wait_time_count.load(Ordering::Relaxed);
        let avg_wait_ms = if wait_count > 0 {
            self.wait_time_total.load(Ordering::Relaxed) / wait_count / 1_000_000
        } else {
            0
        };

        QueueStats {
            depth: inner.depth,
            capacity: self.capacity,
            rejected: self.rejected_count.load(Ordering::Relaxed),
            by_priority,
            avg_wait_ms,
            wait_count,
        }
    }

    pub fn len(&self) -> usize {
        self.current_depth.load(Ordering::Acquire)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn rejected(&self) -> u64 {
        self.rejected_count.load(Ordering::Relaxed)
    }
}

impl fmt::Debug for OperationQueue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stats = self.stats();
        f.debug_struct("OperationQueue")
            .field("depth", &stats.depth)
            .field("capacity", &stats.capacity)
            .field("rejected", &stats.rejected)
            .field("by_priority", &stats.by_priority)
            .finish()
    }
}
