use crate::error::{Result, SchedulerError};
use crate::types::*;
use async_trait::async_trait;
use nova_core::StorageEngine;
use std::sync::Arc;

/// Abstract scheduler backend trait.
#[async_trait]
pub trait SchedulerBackend: Send + Sync {
    /// Create a new job.
    async fn create_job(&self, job: Job) -> Result<()>;

    /// Get a job by ID.
    async fn get_job(&self, id: &uuid::Uuid) -> Result<Job>;

    /// Update an existing job.
    async fn update_job(&self, job: Job) -> Result<()>;

    /// Delete a job.
    async fn delete_job(&self, id: &uuid::Uuid) -> Result<()>;

    /// List jobs, optionally filtered by state.
    async fn list_jobs(&self, state: Option<JobState>) -> Result<Vec<JobSummary>>;

    /// Find jobs that are ready to run (pending, next_run_at <= now).
    async fn find_ready_jobs(&self, now_ms: i64, limit: u32) -> Result<Vec<Job>>;

    /// Mark a job as running.
    async fn mark_running(&self, id: &uuid::Uuid) -> Result<()>;

    /// Mark a job as completed.
    async fn mark_completed(&self, id: &uuid::Uuid) -> Result<()>;

    /// Mark a job as failed.
    async fn mark_failed(&self, id: &uuid::Uuid) -> Result<()>;

    /// Mark a job as cancelled.
    async fn mark_cancelled(&self, id: &uuid::Uuid) -> Result<()>;

    /// Reschedule a recurring job for its next run.
    async fn reschedule(&self, job: &Job, next_run_at: i64) -> Result<()>;

    /// Recover pending jobs on startup.
    async fn recover_pending_jobs(&self, now_ms: i64) -> Result<Vec<Job>>;

    /// Get scheduler statistics.
    async fn stats(&self) -> Result<SchedulerStats>;
}

/// StorageEngine-backed scheduler backend.
pub struct StorageSchedulerBackend {
    store: Arc<dyn StorageEngine>,
}

impl StorageSchedulerBackend {
    pub fn new(store: Arc<dyn StorageEngine>) -> Self {
        Self { store }
    }

    fn job_key(id: &uuid::Uuid) -> nova_core::Key {
        nova_core::Key::from(format!("sched:job:{}", id).into_bytes())
    }

    fn job_pending_key(id: &uuid::Uuid, next_run_at: i64) -> nova_core::Key {
        nova_core::Key::from(format!("sched:pending:{:020}:{}", next_run_at, id).into_bytes())
    }

    fn job_index_key() -> nova_core::Key {
        nova_core::Key::from("sched:index")
    }
}

#[async_trait]
impl SchedulerBackend for StorageSchedulerBackend {
    async fn create_job(&self, job: Job) -> Result<()> {
        let key = Self::job_key(&job.id);
        let data = serde_json::to_vec(&job)
            .map_err(|e| SchedulerError::Internal(e.to_string()))?;

        if self.store.get(&key)?.is_some() {
            return Err(SchedulerError::JobAlreadyExists(job.id.to_string()));
        }

        self.store.set(&key, nova_core::Value::new(data))?;

        // Add to pending index if pending
        if job.state == JobState::Pending {
            let pending_key = Self::job_pending_key(&job.id, job.next_run_at);
            self.store.set(&pending_key, nova_core::Value::new(vec![]))?;
        }

        Ok(())
    }

    async fn get_job(&self, id: &uuid::Uuid) -> Result<Job> {
        let key = Self::job_key(id);
        let data = self.store.get(&key)?
            .ok_or_else(|| SchedulerError::JobNotFound(id.to_string()))?;
        serde_json::from_slice(data.as_bytes())
            .map_err(|e| SchedulerError::Internal(e.to_string()))
    }

    async fn update_job(&self, job: Job) -> Result<()> {
        let key = Self::job_key(&job.id);
        if self.store.get(&key)?.is_none() {
            return Err(SchedulerError::JobNotFound(job.id.to_string()));
        }
        let data = serde_json::to_vec(&job)
            .map_err(|e| SchedulerError::Internal(e.to_string()))?;
        self.store.set(&key, nova_core::Value::new(data))?;
        Ok(())
    }

    async fn delete_job(&self, id: &uuid::Uuid) -> Result<()> {
        let key = Self::job_key(id);
        if self.store.get(&key)?.is_none() {
            return Err(SchedulerError::JobNotFound(id.to_string()));
        }
        self.store.delete(&key)?;

        // Clean up pending index
        let start = nova_core::Key::from(format!("sched:pending:").into_bytes());
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        for (k, _) in &entries {
            let kstr = String::from_utf8_lossy(k.as_bytes());
            if kstr.contains(&id.to_string()) {
                let _ = self.store.delete(&k.clone());
            }
        }

        Ok(())
    }

    async fn list_jobs(&self, state: Option<JobState>) -> Result<Vec<JobSummary>> {
        let start = nova_core::Key::from("sched:job:");
        let end = {
            let mut b = start.as_bytes().to_vec();
            b.push(0xFFu8);
            nova_core::Key::from(b)
        };
        let entries = self.store.scan(start..end)?;
        let mut summaries = Vec::new();

        for (_, value) in &entries {
            if let Ok(job) = serde_json::from_slice::<Job>(value.as_bytes()) {
                if let Some(ref filter_state) = state {
                    if &job.state != filter_state {
                        continue;
                    }
                }
                summaries.push(JobSummary {
                    id: job.id,
                    name: job.name,
                    schedule_type: job.schedule_type,
                    state: job.state,
                    next_run_at: job.next_run_at,
                    last_run_at: job.last_run_at,
                    retry_count: job.retry_count,
                });
            }
        }

        Ok(summaries)
    }

    async fn find_ready_jobs(&self, now_ms: i64, limit: u32) -> Result<Vec<Job>> {
        let start = nova_core::Key::from("sched:pending:");
        let end = nova_core::Key::from(format!("sched:pending:{:020}:", now_ms + 1).into_bytes());
        let entries = self.store.scan(start..end)?;

        let mut jobs = Vec::new();
        for (key, _) in entries.iter().take(limit as usize) {
            let key_str = String::from_utf8_lossy(key.as_bytes());
            let parts: Vec<&str> = key_str.split(':').collect();
            if parts.len() < 3 {
                continue;
            }
            let id_str = parts[parts.len() - 1];
            let id = match uuid::Uuid::parse_str(id_str) {
                Ok(id) => id,
                Err(_) => continue,
            };

            if let Ok(job) = self.get_job(&id).await {
                if job.state == JobState::Pending && job.next_run_at <= now_ms {
                    jobs.push(job);
                }
            }
        }

        Ok(jobs)
    }

    async fn mark_running(&self, id: &uuid::Uuid) -> Result<()> {
        let mut job = self.get_job(id).await?;
        job.state = JobState::Running;
        job.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_job(job).await
    }

    async fn mark_completed(&self, id: &uuid::Uuid) -> Result<()> {
        let mut job = self.get_job(id).await?;
        job.state = JobState::Completed;
        job.last_run_at = Some(chrono::Utc::now().timestamp_millis());
        job.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_job(job).await
    }

    async fn mark_failed(&self, id: &uuid::Uuid) -> Result<()> {
        let mut job = self.get_job(id).await?;
        job.state = JobState::Failed;
        job.last_run_at = Some(chrono::Utc::now().timestamp_millis());
        job.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_job(job).await
    }

    async fn mark_cancelled(&self, id: &uuid::Uuid) -> Result<()> {
        let mut job = self.get_job(id).await?;
        job.state = JobState::Cancelled;
        job.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_job(job).await
    }

    async fn reschedule(&self, job: &Job, next_run_at: i64) -> Result<()> {
        let mut updated = job.clone();
        updated.state = JobState::Pending;
        updated.next_run_at = next_run_at;
        updated.last_run_at = Some(chrono::Utc::now().timestamp_millis());
        updated.retry_count = 0;
        updated.updated_at = chrono::Utc::now().timestamp_millis();
        self.update_job(updated).await
    }

    async fn recover_pending_jobs(&self, now_ms: i64) -> Result<Vec<Job>> {
        let ready = self.find_ready_jobs(now_ms, 10000).await?;

        // Also find running jobs (they may have been left running after a crash)
        let all_jobs = self.list_jobs(None).await?;
        let mut recovered = ready;

        for summary in &all_jobs {
            if summary.state == JobState::Running {
                if let Ok(job) = self.get_job(&summary.id).await {
                    recovered.push(job);
                }
            }
        }

        Ok(recovered)
    }

    async fn stats(&self) -> Result<SchedulerStats> {
        let all = self.list_jobs(None).await?;
        let mut stats = SchedulerStats::default();

        for s in &all {
            stats.total_scheduled += 1;
            match s.state {
                JobState::Pending => stats.jobs_pending += 1,
                JobState::Running => stats.jobs_running += 1,
                JobState::Completed => stats.jobs_completed += 1,
                JobState::Failed => {
                    stats.jobs_failed += 1;
                    stats.total_failures += 1;
                }
                JobState::Cancelled => stats.jobs_cancelled += 1,
                JobState::Skipped => stats.jobs_skipped += 1,
            }
        }

        Ok(stats)
    }
}
