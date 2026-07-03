use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{Datelike, Timelike};
use uuid::Uuid;

/// How a job is triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScheduleType {
    /// Run once at a specific time.
    OneTime,
    /// Run repeatedly at a fixed interval.
    Interval,
    /// Run according to a cron expression.
    Cron,
}

impl Default for ScheduleType {
    fn default() -> Self {
        ScheduleType::OneTime
    }
}

/// Current state of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum JobState {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Skipped,
}

impl Default for JobState {
    fn default() -> Self {
        JobState::Pending
    }
}

/// A scheduled job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub name: String,
    pub schedule_type: ScheduleType,
    pub interval_secs: Option<u64>,
    pub cron_expression: Option<String>,
    pub scheduled_at: i64,
    pub next_run_at: i64,
    pub last_run_at: Option<i64>,
    pub state: JobState,
    pub max_retries: u32,
    pub retry_count: u32,
    pub retry_delay_secs: u32,
    pub timeout_secs: u32,
    pub payload: Vec<u8>,
    pub tags: HashMap<String, String>,
    pub depends_on: Vec<Uuid>,
    pub prevent_overlap: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Job {
    pub fn new(name: &str, scheduled_at: i64, payload: Vec<u8>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Job {
            id: Uuid::new_v4(),
            name: name.to_string(),
            schedule_type: ScheduleType::OneTime,
            interval_secs: None,
            cron_expression: None,
            scheduled_at,
            next_run_at: scheduled_at,
            last_run_at: None,
            state: JobState::Pending,
            max_retries: 3,
            retry_count: 0,
            retry_delay_secs: 10,
            timeout_secs: 300,
            payload,
            tags: HashMap::new(),
            depends_on: Vec::new(),
            prevent_overlap: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn is_ready(&self, now_ms: i64) -> bool {
        self.state == JobState::Pending && self.next_run_at <= now_ms
    }

    pub fn is_recurring(&self) -> bool {
        matches!(self.schedule_type, ScheduleType::Interval | ScheduleType::Cron)
    }
}

/// Cron schedule parsed into next-fire timestamps.
#[derive(Debug, Clone)]
pub struct CronSchedule {
    pub expression: String,
    pub minute: Vec<u8>,
    pub hour: Vec<u8>,
    pub day_of_month: Vec<i8>,
    pub month: Vec<u8>,
    pub day_of_week: Vec<u8>,
}

impl CronSchedule {
    pub fn parse(expr: &str) -> Result<Self, String> {
        let parts: Vec<&str> = expr.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(format!("cron expression must have 5 fields, got {}", parts.len()));
        }

        let minute = Self::parse_field(parts[0], 0, 59)?;
        let hour = Self::parse_field(parts[1], 0, 23)?;
        let day_of_month = Self::parse_field_i8(parts[2], 1, 31)?;
        let month = Self::parse_field(parts[3], 1, 12)?;
        let day_of_week = Self::parse_field(parts[4], 0, 6)?;

        Ok(CronSchedule {
            expression: expr.to_string(),
            minute,
            hour,
            day_of_month,
            month,
            day_of_week,
        })
    }

    fn parse_field(field: &str, min: u8, max: u8) -> Result<Vec<u8>, String> {
        if field == "*" {
            return Ok((min..=max).collect());
        }

        let mut values = Vec::new();

        // Handle comma-separated values
        for part in field.split(',') {
            if part.contains('/') {
                // Step values: e.g. */5, 1-10/2
                let parts: Vec<&str> = part.split('/').collect();
                if parts.len() != 2 {
                    return Err(format!("Invalid step expression: {}", part));
                }
                let range = if parts[0] == "*" {
                    min..=max
                } else if parts[0].contains('-') {
                    let range_parts: Vec<&str> = parts[0].split('-').collect();
                    if range_parts.len() != 2 {
                        return Err(format!("Invalid range: {}", parts[0]));
                    }
                    let lo: u8 = range_parts[0].parse().map_err(|_| format!("Invalid number: {}", range_parts[0]))?;
                    let hi: u8 = range_parts[1].parse().map_err(|_| format!("Invalid number: {}", range_parts[1]))?;
                    lo..=hi
                } else {
                    let val: u8 = parts[0].parse().map_err(|_| format!("Invalid number: {}", parts[0]))?;
                    val..=max
                };
                let step: u8 = parts[1].parse().map_err(|_| format!("Invalid step: {}", parts[1]))?;
                if step == 0 {
                    return Err("Step cannot be zero".to_string());
                }
                for v in range.step_by(step as usize) {
                    if v >= min && v <= max {
                        values.push(v);
                    }
                }
            } else if part.contains('-') {
                let range_parts: Vec<&str> = part.split('-').collect();
                if range_parts.len() != 2 {
                    return Err(format!("Invalid range: {}", part));
                }
                let lo: u8 = range_parts[0].parse().map_err(|_| format!("Invalid number: {}", range_parts[0]))?;
                let hi: u8 = range_parts[1].parse().map_err(|_| format!("Invalid number: {}", range_parts[1]))?;
                for v in lo..=hi {
                    if v >= min && v <= max {
                        values.push(v);
                    }
                }
            } else {
                let val: u8 = part.parse().map_err(|_| format!("Invalid number: {}", part))?;
                if val < min || val > max {
                    return Err(format!("Value {} out of range ({}-{})", val, min, max));
                }
                values.push(val);
            }
        }

        if values.is_empty() {
            return Err(format!("No valid values in field: {}", field));
        }

        values.sort();
        values.dedup();
        Ok(values)
    }

    fn parse_field_i8(field: &str, min: i8, max: i8) -> Result<Vec<i8>, String> {
        if field == "*" {
            return Ok((min..=max).collect());
        }
        let mut values = Vec::new();
        for part in field.split(',') {
            if part == "*" {
                return Ok((min..=max).collect());
            }
            let val: i8 = part.parse().map_err(|_| format!("Invalid number: {}", part))?;
            if val < min || val > max {
                return Err(format!("Value {} out of range ({}-{})", val, min, max));
            }
            values.push(val);
        }
        if values.is_empty() {
            return Err(format!("No valid values in field: {}", field));
        }
        values.sort();
        values.dedup();
        Ok(values)
    }

    /// Compute the next scheduled time after `after_ms`.
    pub fn next_after(&self, after_ms: i64) -> Option<i64> {
        let after = chrono::DateTime::from_timestamp_millis(after_ms)?;
        let mut candidate = after.checked_add_signed(chrono::TimeDelta::seconds(60))?;

        for _ in 0..(365 * 24 * 60) {
            // Search up to 1 year ahead
            let m = candidate.month() as u8;
            let d = candidate.day() as i8;
            let w = candidate.weekday().num_days_from_sunday() as u8;
            let h = candidate.hour() as u8;
            let min = candidate.minute() as u8;

            if self.month.contains(&m)
                && self.day_of_month.contains(&d)
                && self.day_of_week.contains(&w)
                && self.hour.contains(&h)
                && self.minute.contains(&min)
            {
                return Some(candidate.timestamp_millis());
            }

            candidate = candidate.checked_add_signed(chrono::TimeDelta::seconds(60))?;
        }

        None
    }
}

/// Configuration for the scheduler subsystem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub time_wheel_tick_ms: u64,
    pub time_wheel_slots: usize,
    pub priority_queue_tick_ms: u64,
    pub max_jobs_per_queue: usize,
    pub max_concurrent_jobs: u32,
    pub default_job_timeout_secs: u32,
    pub default_max_retries: u32,
    pub default_retry_delay_secs: u32,
    pub enable_startup_recovery: bool,
    pub enable_catch_up: bool,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        SchedulerConfig {
            time_wheel_tick_ms: 100,
            time_wheel_slots: 360,
            priority_queue_tick_ms: 1000,
            max_jobs_per_queue: 10000,
            max_concurrent_jobs: 64,
            default_job_timeout_secs: 300,
            default_max_retries: 3,
            default_retry_delay_secs: 10,
            enable_startup_recovery: true,
            enable_catch_up: true,
        }
    }
}

/// Statistics for the scheduler.
#[derive(Debug, Clone, Default)]
pub struct SchedulerStats {
    pub jobs_pending: u64,
    pub jobs_running: u64,
    pub jobs_completed: u64,
    pub jobs_failed: u64,
    pub jobs_cancelled: u64,
    pub jobs_skipped: u64,
    pub total_scheduled: u64,
    pub total_executed: u64,
    pub total_failures: u64,
    pub time_wheel_ticks: u64,
}

/// Summary of a job for listing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSummary {
    pub id: Uuid,
    pub name: String,
    pub schedule_type: ScheduleType,
    pub state: JobState,
    pub next_run_at: i64,
    pub last_run_at: Option<i64>,
    pub retry_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_new() {
        let now = chrono::Utc::now().timestamp_millis();
        let job = Job::new("test-job", now, b"payload".to_vec());
        assert_eq!(job.name, "test-job");
        assert_eq!(job.state, JobState::Pending);
        assert_eq!(job.schedule_type, ScheduleType::OneTime);
        assert_eq!(job.payload, b"payload");
    }

    #[test]
    fn test_job_is_ready() {
        let now = chrono::Utc::now().timestamp_millis();
        let job = Job::new("ready-job", now - 1000, vec![]);
        assert!(job.is_ready(now));

        let future_job = Job::new("future-job", now + 10000, vec![]);
        assert!(!future_job.is_ready(now));
    }

    #[test]
    fn test_job_is_recurring() {
        let now = chrono::Utc::now().timestamp_millis();
        let mut job = Job::new("test", now, vec![]);
        assert!(!job.is_recurring());

        job.schedule_type = ScheduleType::Interval;
        assert!(job.is_recurring());

        job.schedule_type = ScheduleType::Cron;
        assert!(job.is_recurring());
    }

    #[test]
    fn test_cron_every_minute() {
        let cron = CronSchedule::parse("* * * * *").unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let next = cron.next_after(now).unwrap();
        assert!(next > now);
        assert!(next - now <= 60000);
    }

    #[test]
    fn test_cron_every_5_minutes() {
        let cron = CronSchedule::parse("*/5 * * * *").unwrap();
        assert!(!cron.minute.is_empty());
        assert_eq!(cron.minute.len(), 12);
        assert_eq!(cron.minute[0], 0);
        assert_eq!(cron.minute[1], 5);
    }

    #[test]
    fn test_cron_once_per_hour() {
        let cron = CronSchedule::parse("0 * * * *").unwrap();
        assert_eq!(cron.minute, vec![0]);
        assert_eq!(cron.hour.len(), 24);
    }

    #[test]
    fn test_cron_daily_at_midnight() {
        let cron = CronSchedule::parse("0 0 * * *").unwrap();
        assert_eq!(cron.minute, vec![0]);
        assert_eq!(cron.hour, vec![0]);
    }

    #[test]
    fn test_cron_range() {
        let cron = CronSchedule::parse("0 9-17 * * 1-5").unwrap();
        assert_eq!(cron.minute, vec![0]);
        assert_eq!(cron.hour.len(), 9); // 9,10,11,12,13,14,15,16,17
        assert_eq!(cron.day_of_week.len(), 5); // Mon-Fri
    }

    #[test]
    fn test_cron_invalid_expression() {
        assert!(CronSchedule::parse("invalid").is_err());
        assert!(CronSchedule::parse("* * * * * *").is_err());
    }

    #[test]
    fn test_cron_field_out_of_range() {
        assert!(CronSchedule::parse("60 * * * *").is_err());
        assert!(CronSchedule::parse("* 24 * * *").is_err());
        assert!(CronSchedule::parse("* * 32 * *").is_err());
        assert!(CronSchedule::parse("* * * 13 *").is_err());
        assert!(CronSchedule::parse("* * * * 7").is_err());
    }

    #[test]
    fn test_scheduler_config_defaults() {
        let c = SchedulerConfig::default();
        assert_eq!(c.time_wheel_tick_ms, 100);
        assert_eq!(c.time_wheel_slots, 360);
        assert_eq!(c.priority_queue_tick_ms, 1000);
        assert_eq!(c.max_jobs_per_queue, 10000);
        assert_eq!(c.max_concurrent_jobs, 64);
        assert_eq!(c.default_job_timeout_secs, 300);
        assert_eq!(c.default_max_retries, 3);
        assert!(c.enable_startup_recovery);
        assert!(c.enable_catch_up);
    }

    #[test]
    fn test_schedule_type_default() {
        assert_eq!(ScheduleType::default(), ScheduleType::OneTime);
    }

    #[test]
    fn test_job_state_default() {
        assert_eq!(JobState::default(), JobState::Pending);
    }

    #[test]
    fn test_job_depends_on_empty_by_default() {
        let now = chrono::Utc::now().timestamp_millis();
        let job = Job::new("test", now, vec![]);
        assert!(job.depends_on.is_empty());
    }

    #[test]
    fn test_job_tags_empty_by_default() {
        let now = chrono::Utc::now().timestamp_millis();
        let job = Job::new("test", now, vec![]);
        assert!(job.tags.is_empty());
    }
}
