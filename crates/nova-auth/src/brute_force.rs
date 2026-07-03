use dashmap::DashMap;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Tracks and detects brute-force login attempts per identifier (username, IP).
pub struct BruteForceDetector {
    /// Map from identifier -> VecDeque of attempt timestamps
    attempts: DashMap<String, VecDeque<Instant>>,
    max_attempts: u32,
    window_secs: u64,
    lockout_duration_secs: u64,
    /// Map from identifier -> locked_until
    locked: DashMap<String, Instant>,
}

impl BruteForceDetector {
    pub fn new(max_attempts: u32, window_secs: u64, lockout_duration_secs: u64) -> Self {
        BruteForceDetector {
            attempts: DashMap::new(),
            max_attempts,
            window_secs,
            lockout_duration_secs,
            locked: DashMap::new(),
        }
    }

    /// Record a failed attempt.
    pub fn record_failure(&self, identifier: &str) {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        // Add attempt
        self.attempts.entry(identifier.to_string())
            .or_insert_with(VecDeque::new)
            .push_back(now);

        // Prune old attempts outside the window
        if let Some(mut attempts) = self.attempts.get_mut(identifier) {
            while let Some(&t) = attempts.front() {
                if now.duration_since(t) > window {
                    attempts.pop_front();
                } else {
                    break;
                }
            }

            // Check if max attempts exceeded
            if attempts.len() >= self.max_attempts as usize {
                let lock_until = now + Duration::from_secs(self.lockout_duration_secs);
                self.locked.insert(identifier.to_string(), lock_until);
            }
        }
    }

    /// Record a successful attempt (clears failure history).
    pub fn record_success(&self, identifier: &str) {
        self.attempts.remove(identifier);
        self.locked.remove(identifier);
    }

    /// Check if the identifier is currently locked out.
    pub fn is_locked(&self, identifier: &str) -> bool {
        if let Some(lock_until) = self.locked.get(identifier) {
            if Instant::now() < *lock_until {
                return true;
            }
            // Lock expired, clean up
            drop(lock_until);
            self.locked.remove(identifier);
            self.attempts.remove(identifier);
        }
        false
    }

    /// Get remaining lockout time in milliseconds (0 if not locked).
    pub fn remaining_lockout_ms(&self, identifier: &str) -> u64 {
        self.locked.get(identifier).map_or(0, |lock_until| {
            let remaining = lock_until.saturating_duration_since(Instant::now());
            remaining.as_millis() as u64
        })
    }

    /// Get the number of recent failed attempts.
    pub fn failure_count(&self, identifier: &str) -> usize {
        self.attempts.get(identifier).map_or(0, |a| a.len())
    }

    /// Clean up stale entries.
    pub fn cleanup(&self) {
        let now = Instant::now();
        let window = Duration::from_secs(self.window_secs);

        self.attempts.retain(|_, attempts| {
            while let Some(&t) = attempts.front() {
                if now.duration_since(t) > window {
                    attempts.pop_front();
                } else {
                    break;
                }
            }
            !attempts.is_empty()
        });

        self.locked.retain(|_, lock_until| now < *lock_until);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brute_force_new() {
        let detector = BruteForceDetector::new(5, 60, 300);
        assert!(!detector.is_locked("testuser"));
        assert_eq!(detector.failure_count("testuser"), 0);
    }

    #[test]
    fn test_brute_force_lockout() {
        let detector = BruteForceDetector::new(3, 60, 300);
        let username = "victim";

        assert!(!detector.is_locked(username));
        detector.record_failure(username);
        detector.record_failure(username);
        detector.record_failure(username);

        assert!(detector.is_locked(username));
    }

    #[test]
    fn test_brute_force_success_clears() {
        let detector = BruteForceDetector::new(3, 60, 300);
        let username = "user";

        detector.record_failure(username);
        detector.record_failure(username);
        detector.record_success(username);

        assert!(!detector.is_locked(username));
        assert_eq!(detector.failure_count(username), 0);
    }

    #[test]
    fn test_brute_force_remaining_lockout() {
        let detector = BruteForceDetector::new(2, 60, 3600);
        let username = "locked_user";

        detector.record_failure(username);
        detector.record_failure(username);

        assert!(detector.is_locked(username));
        assert!(detector.remaining_lockout_ms(username) > 0);
    }

    #[test]
    fn test_brute_force_not_locked_below_threshold() {
        let detector = BruteForceDetector::new(5, 60, 300);
        let username = "safe";

        detector.record_failure(username);
        detector.record_failure(username);

        assert!(!detector.is_locked(username));
        assert_eq!(detector.failure_count(username), 2);
    }

    #[test]
    fn test_brute_force_cleanup() {
        let detector = BruteForceDetector::new(3, 0, 0); // zero window — expires immediately
        let username = "expired";

        detector.record_failure(username);
        std::thread::sleep(Duration::from_millis(10));
        detector.cleanup();

        // After cleanup, the entry should be removed since the window is 0
        assert_eq!(detector.failure_count(username), 0);
    }
}
