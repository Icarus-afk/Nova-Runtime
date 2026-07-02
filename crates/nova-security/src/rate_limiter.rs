
use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub struct TokenBucket {
    tokens_per_second: f64,
    burst_size: f64,
    current_tokens: f64,
    last_refill_at: u64,
    last_request_at: u64,
}

impl TokenBucket {
    pub fn new(tokens_per_second: f64, burst_size: f64) -> Self {
        TokenBucket {
            tokens_per_second,
            burst_size,
            current_tokens: burst_size,
            last_refill_at: 0,
            last_request_at: 0,
        }
    }

    pub fn try_consume(&mut self, count: f64, now_ms: u64) -> std::result::Result<(), u64> {
        if self.last_refill_at == 0 {
            self.last_refill_at = now_ms;
            self.current_tokens = self.burst_size;
        }

        let elapsed_ms = now_ms.saturating_sub(self.last_refill_at);
        let refill = elapsed_ms as f64 * self.tokens_per_second / 1000.0;
        self.current_tokens = (self.current_tokens + refill).min(self.burst_size);
        self.last_refill_at = now_ms;

        if self.current_tokens >= count {
            self.current_tokens -= count;
            self.last_request_at = now_ms;
            Ok(())
        } else {
            let needed = count - self.current_tokens;
            let retry_after_ms = (needed / self.tokens_per_second * 1000.0).ceil() as u64;
            Err(retry_after_ms)
        }
    }

    pub fn reset(&mut self) {
        self.current_tokens = self.burst_size;
        self.last_refill_at = 0;
        self.last_request_at = 0;
    }
}

#[derive(Debug, Clone)]
pub struct EndpointRateLimit {
    pub path_pattern: String,
    pub tokens_per_second: f64,
    pub burst_size: f64,
    pub cost_per_request: f64,
}

pub struct RateLimiter {
    buckets: DashMap<(String, String), TokenBucket>,
    global_bucket: Mutex<Option<TokenBucket>>,
    endpoint_limits: Vec<EndpointRateLimit>,
    cleanup_interval_ms: u64,
    last_cleanup: AtomicU64,
}

impl RateLimiter {
    pub fn new(
        global_rate: Option<(f64, f64)>,
        endpoint_limits: Vec<EndpointRateLimit>,
    ) -> Self {
        let global_bucket = global_rate.map(|(tps, burst)| TokenBucket::new(tps, burst));

        RateLimiter {
            buckets: DashMap::new(),
            global_bucket: Mutex::new(global_bucket),
            endpoint_limits,
            cleanup_interval_ms: 60_000,
            last_cleanup: AtomicU64::new(0),
        }
    }

    pub fn check(
        &self,
        ip: &str,
        endpoint: &str,
        cost: f64,
        now_ms: u64,
    ) -> std::result::Result<(), u64> {
        {
            let mut global = self.global_bucket.lock();
            if let Some(ref mut bucket) = *global {
                if let Err(retry) = bucket.try_consume(1.0, now_ms) {
                    return Err(retry);
                }
            }
        }

        let key = (ip.to_string(), endpoint.to_string());
        let mut bucket = self.buckets.entry(key).or_insert_with(|| {
            let limit = self
                .endpoint_limits
                .iter()
                .find(|l| endpoint.starts_with(&l.path_pattern))
                .cloned()
                .unwrap_or(EndpointRateLimit {
                    path_pattern: String::new(),
                    tokens_per_second: 100.0,
                    burst_size: 200.0,
                    cost_per_request: 1.0,
                });
            TokenBucket::new(limit.tokens_per_second, limit.burst_size)
        });

        let result = bucket.try_consume(cost, now_ms);

        if now_ms.saturating_sub(self.last_cleanup.load(Ordering::Relaxed))
            > self.cleanup_interval_ms
        {
            self.last_cleanup.store(now_ms, Ordering::Relaxed);
            self.cleanup_stale(now_ms);
        }

        result.map_err(|retry| retry)
    }

    pub fn cleanup_stale(&self, now_ms: u64) {
        let staleness_threshold = 300_000;
        self.buckets.retain(|_, bucket| {
            now_ms.saturating_sub(bucket.last_request_at) < staleness_threshold
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(secs: f64) -> u64 {
        (secs * 1000.0) as u64
    }

    #[test]
    fn test_token_bucket_consume_within_limit() {
        let mut bucket = TokenBucket::new(10.0, 10.0);
        assert!(bucket.try_consume(5.0, ms(0.0)).is_ok());
    }

    #[test]
    fn test_token_bucket_consume_exact_limit() {
        let mut bucket = TokenBucket::new(10.0, 10.0);
        assert!(bucket.try_consume(10.0, ms(0.0)).is_ok());
    }

    #[test]
    fn test_token_bucket_consume_exceeds_burst() {
        let mut bucket = TokenBucket::new(10.0, 5.0);
        let err = bucket.try_consume(10.0, ms(0.0)).unwrap_err();
        assert!(err > 0);
    }

    #[test]
    fn test_token_bucket_refill() {
        let mut bucket = TokenBucket::new(10.0, 10.0);
        bucket.try_consume(10.0, ms(0.0)).unwrap();
        assert!(bucket.try_consume(5.0, ms(500.0)).is_ok());
    }

    #[test]
    fn test_token_bucket_refill_not_enough() {
        let mut bucket = TokenBucket::new(10.0, 10.0);
        bucket.try_consume(10.0, ms(0.0)).unwrap();
        let err = bucket.try_consume(5.0, ms(100.0)).unwrap_err();
        assert!(err > 0);
    }

    #[test]
    fn test_token_bucket_reset() {
        let mut bucket = TokenBucket::new(10.0, 10.0);
        bucket.try_consume(10.0, ms(0.0)).unwrap();
        bucket.reset();
        assert!(bucket.try_consume(10.0, ms(100.0)).is_ok());
    }

    #[test]
    fn test_token_bucket_burst_capacity() {
        let mut bucket = TokenBucket::new(1.0, 5.0);
        assert!(bucket.try_consume(5.0, ms(0.0)).is_ok());
        assert!(bucket.try_consume(1.0, ms(0.0)).is_err());
    }

    #[test]
    fn test_token_bucket_does_not_exceed_burst() {
        let mut bucket = TokenBucket::new(10.0, 10.0);
        bucket.try_consume(10.0, ms(0.0)).unwrap();
        assert!(bucket.try_consume(10.0, ms(10_000.0)).is_ok());
        assert!(bucket.try_consume(1.0, ms(10_000.0)).is_err());
    }

    #[test]
    fn test_rate_limiter_per_key() {
        let limiter = RateLimiter::new(None, vec![]);
        assert!(limiter.check("127.0.0.1", "/api/test", 1.0, ms(0.0)).is_ok());
    }

    #[test]
    fn test_rate_limiter_global() {
        let limiter = RateLimiter::new(Some((10.0, 10.0)), vec![]);
        for _ in 0..10 {
            assert!(limiter.check("1.2.3.4", "/api/test", 1.0, ms(0.0)).is_ok());
        }
        let err = limiter.check("5.6.7.8", "/api/other", 1.0, ms(0.0)).unwrap_err();
        assert!(err > 0);
    }

    #[test]
    fn test_rate_limiter_endpoint_limit() {
        let endpoint = EndpointRateLimit {
            path_pattern: "/api".to_string(),
            tokens_per_second: 5.0,
            burst_size: 5.0,
            cost_per_request: 1.0,
        };
        let limiter = RateLimiter::new(None, vec![endpoint]);
        for _ in 0..5 {
            assert!(limiter.check("1.2.3.4", "/api/data", 1.0, ms(0.0)).is_ok());
        }
        let err = limiter.check("1.2.3.4", "/api/data", 1.0, ms(0.0)).unwrap_err();
        assert!(err > 0);
    }

    #[test]
    fn test_rate_limiter_different_keys_independent() {
        let limiter = RateLimiter::new(None, vec![]);
        for _ in 0..200 {
            assert!(limiter.check("host_a", "/api", 1.0, ms(0.0)).is_ok());
        }
        assert!(limiter.check("host_b", "/api", 1.0, ms(0.0)).is_ok());
    }

    #[test]
    fn test_cleanup_stale() {
        let limiter = RateLimiter::new(None, vec![]);
        assert!(limiter.check("old_host", "/api", 1.0, ms(0.0)).is_ok());
        assert_eq!(limiter.buckets.len(), 1);
        limiter.cleanup_stale(ms(300_001.0));
        assert_eq!(limiter.buckets.len(), 0);
    }
}
