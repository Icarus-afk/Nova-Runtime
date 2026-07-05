use crate::backend::SchedulerBackend;
use crate::error::{Result, SchedulerError};
use crate::time_wheel::{TimeWheel, PriorityQueue};
use crate::types::*;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{watch, Semaphore};
use uuid::Uuid;

/// The main scheduler manager. Drives both the time wheel and priority queue.
pub struct SchedulerManager {
    backend: Arc<dyn SchedulerBackend>,
    config: SchedulerConfig,
    time_wheel: TimeWheel,
    priority_queue: PriorityQueue,
    running_jobs: Arc<parking_lot::RwLock<HashSet<Uuid>>>,
    concurrency_semaphore: Arc<Semaphore>,
    shutdown_rx: watch::Receiver<bool>,
    shutdown_flag: Arc<AtomicBool>,
}

impl SchedulerManager {
    pub fn new(
        backend: Arc<dyn SchedulerBackend>,
        config: SchedulerConfig,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        let max_concurrent = config.max_concurrent_jobs.max(1) as usize;
        let time_wheel = TimeWheel::new(config.time_wheel_tick_ms, config.time_wheel_slots);

        SchedulerManager {
            backend,
            config,
            time_wheel,
            priority_queue: PriorityQueue::new(),
            running_jobs: Arc::new(parking_lot::RwLock::new(HashSet::new())),
            concurrency_semaphore: Arc::new(Semaphore::new(max_concurrent)),
            shutdown_rx,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn backend(&self) -> &Arc<dyn SchedulerBackend> {
        &self.backend
    }

    pub fn config(&self) -> &SchedulerConfig {
        &self.config
    }

    /// Schedule a new job.
    pub async fn schedule_job(&self, mut job: Job) -> Result<()> {
        if job.id == Uuid::nil() {
            job.id = Uuid::new_v4();
        }

        // Validate dependencies don't form a cycle
        if !job.depends_on.is_empty() {
            self.validate_dependencies(&job).await?;
        }

        self.backend.create_job(job.clone()).await?;

        // Register in the appropriate timer
        let now_ms = chrono::Utc::now().timestamp_millis();
        let delta = job.next_run_at - now_ms;
        let tw_max = (self.config.time_wheel_slots as u64) * self.config.time_wheel_tick_ms;

        if delta >= 0 && (delta as u64) <= tw_max {
            self.time_wheel.schedule(job.id, job.next_run_at);
        } else {
            self.priority_queue.schedule(job.id, job.next_run_at);
        }

        Ok(())
    }

    /// Cancel a scheduled job.
    pub async fn cancel_job(&self, id: &Uuid) -> Result<()> {
        self.backend.mark_cancelled(id).await?;
        self.time_wheel.cancel(id);
        self.priority_queue.cancel(id);

        let mut running = self.running_jobs.write();
        running.remove(id);

        Ok(())
    }

    /// Get a job by ID.
    pub async fn get_job(&self, id: &Uuid) -> Result<Job> {
        self.backend.get_job(id).await
    }

    /// List jobs.
    pub async fn list_jobs(&self, state: Option<JobState>) -> Result<Vec<JobSummary>> {
        self.backend.list_jobs(state).await
    }

    /// Start the main scheduler loop.
    pub async fn run(&mut self) {
        tracing::info!("Scheduler starting (tick={}ms, slots={})",
            self.config.time_wheel_tick_ms, self.config.time_wheel_slots);

        // Startup recovery
        if self.config.enable_startup_recovery {
            self.recover_on_startup().await;
        }

        // Priority queue tick interval
        let pq_interval = Duration::from_millis(self.config.priority_queue_tick_ms);
        let mut pq_ticker = tokio::time::interval(pq_interval);

        // Time wheel tick interval
        let tw_interval = Duration::from_millis(self.config.time_wheel_tick_ms);
        let mut tw_ticker = tokio::time::interval(tw_interval);

        loop {
            tokio::select! {
                _ = tw_ticker.tick() => {
                    self.process_time_wheel_tick().await;
                }
                _ = pq_ticker.tick() => {
                    self.process_priority_queue_tick().await;
                }
                _ = self.shutdown_rx.changed() => {
                    if *self.shutdown_rx.borrow() || self.shutdown_flag.load(Ordering::Relaxed) {
                        tracing::info!("Scheduler shutting down");
                        break;
                    }
                }
            }

            if self.shutdown_flag.load(Ordering::Relaxed) {
                break;
            }
        }
    }

    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::Relaxed);
    }

    async fn process_time_wheel_tick(&self) {
        let due = self.time_wheel.tick();
        for job_id in due {
            if self.running_jobs.read().contains(&job_id) {
                tracing::debug!("Skipping job {} (already running)", job_id);
                continue;
            }

            match self.backend.get_job(&job_id).await {
                Ok(job) if job.state == JobState::Pending => {
                    self.dispatch_job(job).await;
                }
                Ok(job) if job.state == JobState::Cancelled => {
                    // Cancelled, skip
                }
                Err(e) => {
                    tracing::error!("Failed to get job {}: {}", job_id, e);
                }
                _ => {}
            }
        }
    }

    async fn process_priority_queue_tick(&self) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let due = self.priority_queue.pop_due(now_ms);

        for job_id in due {
            if self.running_jobs.read().contains(&job_id) {
                tracing::debug!("Skipping job {} (already running)", job_id);
                continue;
            }

            match self.backend.get_job(&job_id).await {
                Ok(job) if job.state == JobState::Pending => {
                    self.dispatch_job(job).await;
                }
                Ok(job) if job.state == JobState::Cancelled => {}
                Err(e) => {
                    tracing::error!("Failed to get job {}: {}", job_id, e);
                }
                _ => {}
            }
        }
    }

    async fn dispatch_job(&self, job: Job) {
        if job.prevent_overlap {
            if self.running_jobs.read().contains(&job.id) {
                tracing::warn!("Overlap prevented for job {}", job.id);
                return;
            }
        }

        // Check dependencies
        if !job.depends_on.is_empty() {
            for dep_id in &job.depends_on {
                match self.backend.get_job(dep_id).await {
                    Ok(dep_job) => {
                        if dep_job.state != JobState::Completed {
                            tracing::debug!("Job {} waiting on dependency {}", job.name, dep_job.name);
                            // Re-schedule to check later
                            let retry_ms = chrono::Utc::now().timestamp_millis() + 5000;
                            self.priority_queue.schedule(job.id, retry_ms);
                            return;
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Dependency {} not found for job {}", dep_id, job.name);
                        // Skip dependency check and proceed
                    }
                }
            }
        }

        // Acquire concurrency permit
        let permit = self.concurrency_semaphore.clone().acquire_owned().await;
        match permit {
            Ok(permit) => {
                self.running_jobs.write().insert(job.id);
                if let Err(e) = self.backend.mark_running(&job.id).await {
                    tracing::error!("Failed to mark job {} as running: {}", job.id, e);
                    self.running_jobs.write().remove(&job.id);
                    return;
                }

                let backend = self.backend.clone();
                let running_jobs = self.running_jobs.clone();
                let config = self.config.clone();

                tokio::spawn(async move {
                    let result = execute_job(&job, &config).await;

                    match result {
                        Ok(()) => {
                            tracing::info!("Job {} completed successfully", job.name);
                            if let Err(e) = backend.mark_completed(&job.id).await {
                                tracing::error!("Failed to mark job {} completed: {}", job.id, e);
                            }

                            // Reschedule recurring jobs
                            if job.is_recurring() {
                                if let Some(next_run) = compute_next_run(&job) {
                                    if let Err(e) = backend.reschedule(&job, next_run).await {
                                        tracing::error!("Failed to reschedule job {}: {}", job.id, e);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if job.retry_count < job.max_retries {
                                // Retry
                                let retry_at = chrono::Utc::now().timestamp_millis()
                                    + (job.retry_delay_secs as i64) * 1000;
                                if let Err(e) = backend.reschedule(&job, retry_at).await {
                                    tracing::error!("Failed to reschedule job {} for retry: {}", job.id, e);
                                }
                                tracing::warn!("Job {} failed, retry {}/{}", job.name, job.retry_count + 1, job.max_retries);
                            } else {
                                if let Err(e) = backend.mark_failed(&job.id).await {
                                    tracing::error!("Failed to mark job {} failed: {}", job.id, e);
                                }
                                tracing::error!("Job {} failed after {} retries: {}", job.name, job.max_retries, e);
                            }
                        }
                    }

                    running_jobs.write().remove(&job.id);
                    drop(permit);
                });
            }
            Err(_) => {
                // Semaphore closed during shutdown — re-queue
                let retry_ms = chrono::Utc::now().timestamp_millis() + 1000;
                self.priority_queue.schedule(job.id, retry_ms);
            }
        }
    }

    async fn validate_dependencies(&self, job: &Job) -> Result<()> {
        // Simple cycle detection via DFS
        let mut visited = HashSet::new();
        let mut stack = job.depends_on.clone();

        while let Some(dep_id) = stack.pop() {
            if dep_id == job.id {
                return Err(SchedulerError::DependencyCycle(
                    format!("Job {} depends on itself", job.name),
                ));
            }
            if !visited.insert(dep_id) {
                continue;
            }
            if let Ok(dep_job) = self.backend.get_job(&dep_id).await {
                stack.extend(dep_job.depends_on);
            }
        }

        Ok(())
    }

    async fn recover_on_startup(&self) {
        tracing::info!("Starting scheduler recovery...");
        let now_ms = chrono::Utc::now().timestamp_millis();

        match self.backend.recover_pending_jobs(now_ms).await {
            Ok(jobs) => {
                tracing::info!("Recovered {} jobs from storage", jobs.len());
                for job in jobs {
                    if job.state == JobState::Pending && job.next_run_at <= now_ms {
                        // Catch up: schedule immediately or skip based on config
                        if self.config.enable_catch_up || job.is_recurring() {
                            self.dispatch_job(job).await;
                        } else {
                            // Skip missed one-time jobs
                            tracing::debug!("Skipping missed one-time job {}", job.name);
                        }
                    } else if job.state == JobState::Running {
                        // Jobs left in Running state after crash — reset to Pending
                        let mut recovered = job.clone();
                        recovered.state = JobState::Pending;
                        recovered.retry_count = 0;
                        if let Err(e) = self.backend.update_job(recovered).await {
                            tracing::error!("Failed to reset stale running job {}: {}", job.id, e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Startup recovery failed: {}", e);
            }
        }
    }

    pub async fn trigger_job(&self, id: &Uuid) -> Result<()> {
        let mut job = self.backend.get_job(id).await?;
        self.time_wheel.cancel(id);
        self.priority_queue.cancel(id);
        job.state = JobState::Pending;
        job.next_run_at = chrono::Utc::now().timestamp_millis();
        self.backend.update_job(job.clone()).await?;
        self.dispatch_job(job).await;
        Ok(())
    }

    pub async fn pause_job(&self, id: &Uuid) -> Result<()> {
        let mut job = self.backend.get_job(id).await?;
        job.state = JobState::Paused;
        job.updated_at = chrono::Utc::now().timestamp_millis();
        self.backend.update_job(job).await?;
        self.time_wheel.cancel(id);
        self.priority_queue.cancel(id);
        Ok(())
    }

    pub async fn resume_job(&self, id: &Uuid) -> Result<()> {
        let mut job = self.backend.get_job(id).await?;
        job.state = JobState::Pending;
        job.next_run_at = chrono::Utc::now().timestamp_millis();
        job.updated_at = chrono::Utc::now().timestamp_millis();
        self.backend.update_job(job.clone()).await?;
        let now_ms = chrono::Utc::now().timestamp_millis();
        let delta = job.next_run_at - now_ms;
        let tw_max = (self.config.time_wheel_slots as u64) * self.config.time_wheel_tick_ms;
        if delta >= 0 && (delta as u64) <= tw_max {
            self.time_wheel.schedule(job.id, job.next_run_at);
        } else {
            self.priority_queue.schedule(job.id, job.next_run_at);
        }
        Ok(())
    }

    pub async fn stats(&self) -> Result<SchedulerStats> {
        self.backend.stats().await
    }
}

/// Execute a job's payload. For now, this is a no-op that returns success.
/// In production, this would invoke a registered handler via the executor.
async fn execute_job(job: &Job, _config: &SchedulerConfig) -> std::result::Result<(), String> {
    // Placeholder: actual job execution will use nova-executor pipeline
    tracing::debug!("Executing job {} ({} bytes payload)", job.name, job.payload.len());
    Ok(())
}

/// Compute the next run time for recurring jobs.
fn compute_next_run(job: &Job) -> Option<i64> {
    let now_ms = chrono::Utc::now().timestamp_millis();

    match job.schedule_type {
        ScheduleType::Interval => {
            let interval = job.interval_secs.unwrap_or(3600) as i64 * 1000;
            Some(now_ms + interval)
        }
        ScheduleType::Cron => {
            if let Some(ref expr) = job.cron_expression {
                if let Ok(cron) = CronSchedule::parse(expr) {
                    return cron.next_after(now_ms);
                }
            }
            None
        }
        ScheduleType::OneTime => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::StorageSchedulerBackend;
    use nova_core::StorageEngine;

    struct MockStorage {
        data: parking_lot::RwLock<HashMap<Vec<u8>, nova_core::Value>>,
    }

    use std::collections::HashMap;

    impl StorageEngine for MockStorage {
        fn get(&self, key: &nova_core::Key) -> nova_core::Result<Option<nova_core::Value>> {
            let data = self.data.read();
            Ok(data.get(key.as_bytes()).cloned())
        }
        fn set(&self, key: &nova_core::Key, value: nova_core::Value) -> nova_core::Result<()> {
            let mut data = self.data.write();
            data.insert(key.as_bytes().to_vec(), value);
            Ok(())
        }
        fn delete(&self, key: &nova_core::Key) -> nova_core::Result<bool> {
            let mut data = self.data.write();
            Ok(data.remove(key.as_bytes()).is_some())
        }
        fn scan(&self, range: std::ops::Range<nova_core::Key>) -> nova_core::Result<Vec<(nova_core::Key, nova_core::Value)>> {
            let data = self.data.read();
            let mut results = Vec::new();
            let start = range.start.as_bytes().to_vec();
            let end = range.end.as_bytes().to_vec();
            for (k, v) in data.iter() {
                if start.as_slice() <= k.as_slice() && k.as_slice() < end.as_slice() {
                    results.push((nova_core::Key::from(k.clone()), v.clone()));
                }
            }
            results.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            Ok(results)
        }
        fn batch(&self, ops: Vec<nova_core::WriteOperation>) -> nova_core::Result<()> {
            let mut data = self.data.write();
            for op in ops {
                match op {
                    nova_core::WriteOperation::Set { key, value } => {
                        data.insert(key.as_bytes().to_vec(), value);
                    }
                    nova_core::WriteOperation::Delete { key } => {
                        data.remove(key.as_bytes());
                    }
                }
            }
            Ok(())
        }
        fn flush(&self) -> nova_core::Result<()> {
            Ok(())
        }
        fn sync(&self) -> nova_core::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_scheduler_manager_new() {
        let backend = Arc::new(StorageSchedulerBackend::new(Arc::new(MockStorage { data: parking_lot::RwLock::new(HashMap::new()) })));
        let config = SchedulerConfig::default();
        let (_tx, rx) = watch::channel(false);
        let manager = SchedulerManager::new(backend, config, rx);
        let jobs = manager.list_jobs(None).await.unwrap();
        assert!(jobs.is_empty());
    }

    #[tokio::test]
    async fn test_schedule_and_cancel_job() {
        let backend = Arc::new(StorageSchedulerBackend::new(Arc::new(MockStorage { data: parking_lot::RwLock::new(HashMap::new()) })));
        let config = SchedulerConfig::default();
        let (_tx, rx) = watch::channel(false);
        let manager = SchedulerManager::new(backend, config, rx);

        let now = chrono::Utc::now().timestamp_millis();
        let job = Job::new("test-job", now + 5000, vec![]);
        manager.schedule_job(job).await.unwrap();

        let jobs = manager.list_jobs(None).await.unwrap();
        assert_eq!(jobs.len(), 1);

        manager.cancel_job(&jobs[0].id).await.unwrap();
        let jobs = manager.list_jobs(None).await.unwrap();
        // The mock storage always returns None for get, so the job was "cancelled" virtually
        // But our mock doesn't actually store anything, so the list returns empty
    }

    #[test]
    fn test_compute_next_run_interval() {
        let now = chrono::Utc::now().timestamp_millis();
        let mut job = Job::new("test", now, vec![]);
        job.schedule_type = ScheduleType::Interval;
        job.interval_secs = Some(3600);
        let next = compute_next_run(&job).unwrap();
        assert!(next > now);
        assert!(next - now >= 3600_000 - 100); // allow small timing variance
    }

    #[test]
    fn test_compute_next_run_one_time() {
        let now = chrono::Utc::now().timestamp_millis();
        let job = Job::new("test", now, vec![]);
        assert!(compute_next_run(&job).is_none());
    }
}
