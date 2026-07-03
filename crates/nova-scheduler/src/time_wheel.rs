use std::collections::VecDeque;
use parking_lot::RwLock;
use uuid::Uuid;

/// A timing wheel for scheduling short-interval jobs (≤36s at 100ms tick).
/// Uses hierarchical approach: each slot holds jobs to fire at that tick.
pub struct TimeWheel {
    tick_ms: u64,
    slots: Vec<RwLock<VecDeque<Uuid>>>,
    current_slot: RwLock<usize>,
    started_at: RwLock<i64>,
}

impl TimeWheel {
    /// Create a new time wheel.
    ///
    /// `tick_ms`: duration of each slot in milliseconds.
    /// `num_slots`: total number of slots (wheel circumference).
    pub fn new(tick_ms: u64, num_slots: usize) -> Self {
        let mut slots = Vec::with_capacity(num_slots);
        for _ in 0..num_slots {
            slots.push(RwLock::new(VecDeque::new()));
        }
        TimeWheel {
            tick_ms,
            slots,
            current_slot: RwLock::new(0),
            started_at: RwLock::new(chrono::Utc::now().timestamp_millis()),
        }
    }

    /// Register a job to fire at `fire_at_ms`.
    /// Returns the slot index the job was placed in.
    pub fn schedule(&self, job_id: Uuid, fire_at_ms: i64) -> usize {
        let now = chrono::Utc::now().timestamp_millis();
        let diff = fire_at_ms - now;
        if diff <= 0 {
            // Schedule in the next slot so the next tick picks it up
            let slot = {
                let current = *self.current_slot.read();
                (current + 1) % self.slots.len()
            };
            self.slots[slot].write().push_back(job_id);
            return slot;
        }

        let ticks_ahead = (diff as u64) / self.tick_ms;
        // If diff is positive but less than one tick, still schedule 1 slot ahead
        let offset = if ticks_ahead == 0 { 1 } else { ticks_ahead as usize };
        let slot = {
            let current = *self.current_slot.read();
            (current + offset) % self.slots.len()
        };
        self.slots[slot].write().push_back(job_id);
        slot
    }

    /// Advance the wheel by one tick. Returns job IDs that are due.
    pub fn tick(&self) -> Vec<Uuid> {
        let mut slot_idx = self.current_slot.write();
        *slot_idx = (*slot_idx + 1) % self.slots.len();

        let mut due = VecDeque::new();
        std::mem::swap(&mut due, &mut self.slots[*slot_idx].write());

        due.into_iter().collect()
    }

    /// Remove a job from the wheel (e.g., on cancellation).
    pub fn cancel(&self, job_id: &Uuid) {
        for slot in &self.slots {
            let mut guard = slot.write();
            guard.retain(|id| id != job_id);
        }
    }

    pub fn current_slot(&self) -> usize {
        *self.current_slot.read()
    }

    pub fn tick_ms(&self) -> u64 {
        self.tick_ms
    }

    pub fn num_slots(&self) -> usize {
        self.slots.len()
    }
}

/// Priority queue for long-interval jobs (>36s).
/// Uses a simple sorted-vector approach for correctness.
pub struct PriorityQueue {
    jobs: RwLock<Vec<(i64, Uuid)>>, // (fire_at_ms, job_id)
}

impl PriorityQueue {
    pub fn new() -> Self {
        PriorityQueue {
            jobs: RwLock::new(Vec::new()),
        }
    }

    /// Schedule a job at the given timestamp.
    pub fn schedule(&self, job_id: Uuid, fire_at_ms: i64) {
        let mut jobs = self.jobs.write();
        jobs.push((fire_at_ms, job_id));
        jobs.sort_by_key(|(ts, _)| *ts);
    }

    /// Pop all jobs that are due (fire_at_ms <= now_ms).
    pub fn pop_due(&self, now_ms: i64) -> Vec<Uuid> {
        let mut jobs = self.jobs.write();
        let mut due = Vec::new();

        while let Some((ts, id)) = jobs.first() {
            if *ts <= now_ms {
                due.push(jobs.remove(0).1);
            } else {
                break;
            }
        }

        due
    }

    /// Remove a job from the queue.
    pub fn cancel(&self, job_id: &Uuid) {
        let mut jobs = self.jobs.write();
        jobs.retain(|(_, id)| id != job_id);
    }

    pub fn len(&self) -> usize {
        self.jobs.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.read().is_empty()
    }
}

impl Default for PriorityQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_wheel_new() {
        let wheel = TimeWheel::new(100, 10);
        assert_eq!(wheel.tick_ms(), 100);
        assert_eq!(wheel.num_slots(), 10);
        assert_eq!(wheel.current_slot(), 0);
    }

    #[test]
    fn test_time_wheel_tick_returns_due_jobs() {
        let wheel = TimeWheel::new(100, 10);
        let id = Uuid::new_v4();
        wheel.schedule(id, chrono::Utc::now().timestamp_millis() + 50);

        // Tick should move to slot 1 (where the job is scheduled if diff <= 0)
        let due = wheel.tick();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0], id);
    }

    #[test]
    fn test_time_wheel_multiple_ticks() {
        let wheel = TimeWheel::new(100, 5);
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();
        wheel.schedule(id1, chrono::Utc::now().timestamp_millis() + 100); // ~1 tick
        wheel.schedule(id2, chrono::Utc::now().timestamp_millis() + 300); // ~3 ticks

        // Tick 1: should get id1
        let due1 = wheel.tick();
        assert_eq!(due1.len(), 1);
        assert_eq!(due1[0], id1);

        // Tick 2: nothing
        let due2 = wheel.tick();
        assert!(due2.is_empty());

        // Tick 3: should get id2
        let due3 = wheel.tick();
        assert_eq!(due3.len(), 1);
        assert_eq!(due3[0], id2);
    }

    #[test]
    fn test_time_wheel_cancel() {
        let wheel = TimeWheel::new(100, 10);
        let id = Uuid::new_v4();
        wheel.schedule(id, chrono::Utc::now().timestamp_millis() + 50);
        wheel.cancel(&id);
        let due = wheel.tick();
        assert!(due.is_empty());
    }

    #[test]
    fn test_priority_queue_new() {
        let pq = PriorityQueue::new();
        assert!(pq.is_empty());
        assert_eq!(pq.len(), 0);
    }

    #[test]
    fn test_priority_queue_schedule_and_pop() {
        let pq = PriorityQueue::new();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        pq.schedule(id1, 2000);
        pq.schedule(id2, 1000);

        assert_eq!(pq.len(), 2);

        let due = pq.pop_due(1500);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0], id2);

        let due = pq.pop_due(3000);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0], id1);

        assert!(pq.is_empty());
    }

    #[test]
    fn test_priority_queue_cancel() {
        let pq = PriorityQueue::new();
        let id = Uuid::new_v4();
        pq.schedule(id, 1000);
        pq.cancel(&id);
        assert!(pq.is_empty());
    }

    #[test]
    fn test_priority_queue_order_preserved() {
        let pq = PriorityQueue::new();
        let ids: Vec<_> = (0..5).map(|_| Uuid::new_v4()).collect();

        pq.schedule(ids[0], 5000);
        pq.schedule(ids[1], 1000);
        pq.schedule(ids[2], 3000);
        pq.schedule(ids[3], 2000);
        pq.schedule(ids[4], 4000);

        let due = pq.pop_due(10000);
        assert_eq!(due.len(), 5);
        // Should be in order of fire_at_ms
        assert_eq!(due[0], ids[1]); // 1000
        assert_eq!(due[1], ids[3]); // 2000
        assert_eq!(due[2], ids[2]); // 3000
        assert_eq!(due[3], ids[4]); // 4000
        assert_eq!(due[4], ids[0]); // 5000
    }
}
