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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::OperationContextBuilder;
    use crate::OperationRequest;
    use crate::OperationType;
    use crate::OperationTarget;
    use crate::Priority;
    use std::net::SocketAddr;
    use std::time::{Duration, Instant};
    use std::sync::mpsc;

    fn test_addr() -> SocketAddr { "127.0.0.1:8080".parse().unwrap() }

    fn make_pending_op(priority: Priority) -> (PendingOperation, mpsc::Receiver<OperationResponse>) {
        let (tx, rx) = mpsc::channel();
        let ctx = OperationContextBuilder::new(test_addr())
            .priority(priority)
            .deadline(Instant::now() + Duration::from_secs(60))
            .build();
        let op = PendingOperation {
            request: OperationRequest::new(OperationType::Get, OperationTarget::System),
            context: ctx,
            completion: tx,
            submitted_at: Instant::now(),
            deadline: Instant::now() + Duration::from_secs(60),
            retry_count: 0,
            priority_age: 0,
        };
        (op, rx)
    }

    #[test]
    fn test_new_queue_is_empty() {
        let queue = OperationQueue::new(100);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_push_adds_operation() {
        let queue = OperationQueue::new(100);
        let (op, _rx) = make_pending_op(Priority::Normal);
        assert!(queue.push(op).is_ok());
        assert!(!queue.is_empty());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_pop_priority_removes_operation() {
        let queue = OperationQueue::new(100);
        let (op, _rx) = make_pending_op(Priority::Normal);
        queue.push(op).unwrap();
        let popped = queue.pop_priority();
        assert!(popped.is_some());
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_pop_priority_returns_none_when_empty() {
        let queue = OperationQueue::new(100);
        let popped = queue.pop_priority();
        assert!(popped.is_none());
    }

    #[test]
    fn test_capacity_returns_configured() {
        let queue = OperationQueue::new(500);
        assert_eq!(queue.capacity(), 500);
    }

    #[test]
    fn test_stats_returns_correct_values() {
        let queue = OperationQueue::new(200);
        let (op1, _rx) = make_pending_op(Priority::Normal);
        let (op2, _rx) = make_pending_op(Priority::High);
        queue.push(op1).unwrap();
        queue.push(op2).unwrap();

        let stats = queue.stats();
        assert_eq!(stats.depth, 2);
        assert_eq!(stats.capacity, 200);
        assert_eq!(stats.rejected, 0);
    }

    #[test]
    fn test_queue_rejects_when_full() {
        let queue = OperationQueue::new(2);
        let (op1, _rx) = make_pending_op(Priority::Normal);
        let (op2, _rx) = make_pending_op(Priority::Normal);
        let (op3, _rx) = make_pending_op(Priority::Normal);

        assert!(queue.push(op1).is_ok());
        assert!(queue.push(op2).is_ok());
        assert!(queue.push(op3).is_err());
        assert_eq!(queue.rejected(), 1);
    }

    #[test]
    fn test_priority_ordering_higher_priority_first() {
        let queue = OperationQueue::new(100);

        // Push in reverse priority order
        let (op_bg, _rx) = make_pending_op(Priority::Background);
        let (op_normal, _rx) = make_pending_op(Priority::Normal);
        let (op_high, _rx) = make_pending_op(Priority::High);
        let (op_critical, _rx) = make_pending_op(Priority::Critical);

        queue.push(op_bg).unwrap();
        queue.push(op_normal).unwrap();
        queue.push(op_high).unwrap();
        queue.push(op_critical).unwrap();

        // Should pop in priority order
        let p1 = queue.pop_priority().unwrap();
        assert_eq!(p1.context.operation_priority, Priority::Critical);

        let p2 = queue.pop_priority().unwrap();
        assert_eq!(p2.context.operation_priority, Priority::High);

        let p3 = queue.pop_priority().unwrap();
        assert_eq!(p3.context.operation_priority, Priority::Normal);

        let p4 = queue.pop_priority().unwrap();
        assert_eq!(p4.context.operation_priority, Priority::Background);
    }

    #[test]
    fn test_cancel_all_empties_queue_and_sends_responses() {
        let queue = OperationQueue::new(100);
        let (op1, rx1) = make_pending_op(Priority::Normal);
        let (op2, rx2) = make_pending_op(Priority::High);
        queue.push(op1).unwrap();
        queue.push(op2).unwrap();

        let count = queue.cancel_all("test shutdown");
        assert_eq!(count, 2);
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);

        // Both receivers should get cancellation responses
        let resp1 = rx1.recv().unwrap();
        assert!(!resp1.success);
        assert_eq!(resp1.error.as_ref().unwrap().code, ErrorCode::Cancelled);

        let resp2 = rx2.recv().unwrap();
        assert!(!resp2.success);
    }

    #[test]
    fn test_drain_removes_expired_operations() {
        let queue = OperationQueue::new(100);
        let (op_future, _rx) = make_pending_op(Priority::Normal);
        let (op_past, _rx) = {
            let (tx, rx) = mpsc::channel();
            let ctx = OperationContextBuilder::new(test_addr())
                .priority(Priority::Normal)
                .deadline(Instant::now() + Duration::from_secs(60))
                .build();
            let op = PendingOperation {
                request: OperationRequest::new(OperationType::Get, OperationTarget::System),
                context: ctx,
                completion: tx,
                submitted_at: Instant::now(),
                deadline: Instant::now() - Duration::from_secs(1), // expired
                retry_count: 0,
                priority_age: 0,
            };
            (op, rx)
        };

        queue.push(op_future).unwrap();
        queue.push(op_past).unwrap();
        assert_eq!(queue.len(), 2);

        let expired = queue.drain(Instant::now());
        assert_eq!(expired.len(), 1);
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_try_dequeue_returns_none_when_empty() {
        let queue = OperationQueue::new(100);
        assert!(queue.pop_priority().is_none());
    }

    #[test]
    fn test_clear_removes_all_operations() {
        let queue = OperationQueue::new(100);
        let (op1, _rx) = make_pending_op(Priority::Normal);
        let (op2, _rx) = make_pending_op(Priority::High);
        queue.push(op1).unwrap();
        queue.push(op2).unwrap();
        assert_eq!(queue.len(), 2);

        // cancel_all is the only "clear" method
        queue.cancel_all("clearing");
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_queue_rejects_when_priority_capacity_exceeded() {
        let queue = OperationQueue::new(1000);
        // Each priority queue has capacity: Critical=64, High=128, Normal=512, Background=320
        // Let's fill the Critical queue
        for _ in 0..64 {
            let (op, _rx) = make_pending_op(Priority::Critical);
            assert!(queue.push(op).is_ok());
        }
        // One more Critical should fail
        let (op, _rx) = make_pending_op(Priority::Critical);
        assert!(queue.push(op).is_err());
    }

    #[test]
    fn test_stats_by_priority() {
        let queue = OperationQueue::new(100);
        let (op1, _rx) = make_pending_op(Priority::Critical);
        let (op2, _rx) = make_pending_op(Priority::High);
        let (op3, _rx) = make_pending_op(Priority::Normal);
        queue.push(op1).unwrap();
        queue.push(op2).unwrap();
        queue.push(op3).unwrap();

        let stats = queue.stats();
        assert_eq!(stats.by_priority[0], 1); // Critical
        assert_eq!(stats.by_priority[1], 1); // High
        assert_eq!(stats.by_priority[2], 1); // Normal
        assert_eq!(stats.by_priority[3], 0); // Background
    }
}
