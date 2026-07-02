use crate::types::*;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use dashmap::DashMap;
use parking_lot::RwLock;

#[derive(Debug)]
struct AtomicF64 {
    inner: AtomicU64,
}

impl AtomicF64 {
    fn new(val: f64) -> Self {
        Self { inner: AtomicU64::new(val.to_bits()) }
    }

    fn load(&self, order: Ordering) -> f64 {
        f64::from_bits(self.inner.load(order))
    }

    fn store(&self, val: f64, order: Ordering) {
        self.inner.store(val.to_bits(), order);
    }

    fn fetch_add(&self, val: f64, order: Ordering) -> f64 {
        let mut old = self.load(Ordering::Relaxed);
        loop {
            let new = old + val;
            match self.inner.compare_exchange(old.to_bits(), new.to_bits(), order, Ordering::Relaxed) {
                Ok(_) => return old,
                Err(bits) => old = f64::from_bits(bits),
            }
        }
    }

    fn compare_exchange(&self, current: f64, new: f64, success: Ordering, failure: Ordering) -> Result<f64, f64> {
        match self.inner.compare_exchange(current.to_bits(), new.to_bits(), success, failure) {
            Ok(_) => Ok(current),
            Err(bits) => Err(f64::from_bits(bits)),
        }
    }
}

unsafe impl Send for AtomicF64 {}
unsafe impl Sync for AtomicF64 {}

pub struct TokenBucket {
    tokens: AtomicF64,
    capacity: f64,
    refill_rate: f64,
    last_refill: RwLock<Instant>,
}

impl TokenBucket {
    pub fn new(rate_per_sec: f64, burst: f64) -> Self {
        Self {
            tokens: AtomicF64::new(burst),
            capacity: burst,
            refill_rate: rate_per_sec,
            last_refill: RwLock::new(Instant::now()),
        }
    }

    fn refill(&self) {
        let now = Instant::now();
        let mut last = self.last_refill.write();
        let elapsed = now.duration_since(*last).as_secs_f64();
        if elapsed > 0.0 {
            let added = elapsed * self.refill_rate;
            *last = now;
            let current = self.tokens.fetch_add(added, Ordering::Relaxed);
            let new_val = current + added;
            if new_val > self.capacity {
                self.tokens.store(self.capacity, Ordering::Release);
            }
        }
    }

    pub fn try_consume(&self, tokens: f64) -> bool {
        self.refill();
        loop {
            let current = self.tokens.load(Ordering::Acquire);
            if current < tokens {
                return false;
            }
            match self.tokens.compare_exchange(current, current - tokens, Ordering::Release, Ordering::Relaxed) {
                Ok(_) => return true,
                Err(_) => continue,
            }
        }
    }

    pub fn available(&self) -> f64 {
        self.refill();
        self.tokens.load(Ordering::Acquire)
    }
}

pub struct RateLimiter {
    global: TokenBucket,
    per_user: DashMap<u128, TokenBucket>,
    per_ip: DashMap<IpAddr, TokenBucket>,
    per_operation: DashMap<OperationType, TokenBucket>,
    critical: TokenBucket,
    max_tracked_users: usize,
    max_tracked_ips: usize,
    config: RwLock<RateLimitConfig>,
    hits: AtomicU64,
    waived: AtomicU64,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub global_per_sec: f64,
    pub global_burst: f64,
    pub user_per_sec: f64,
    pub user_burst: f64,
    pub ip_per_sec: f64,
    pub ip_burst: f64,
    pub critical_per_sec: f64,
    pub critical_burst: f64,
    pub max_tracked_users: usize,
    pub max_tracked_ips: usize,
    pub operation_limits: HashMap<OperationType, f64>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut op_limits = HashMap::new();
        op_limits.insert(OperationType::Get, 5000.0);
        op_limits.insert(OperationType::List, 500.0);
        op_limits.insert(OperationType::Create, 1000.0);
        op_limits.insert(OperationType::Update, 1000.0);
        op_limits.insert(OperationType::Delete, 500.0);
        op_limits.insert(OperationType::Search, 200.0);
        op_limits.insert(OperationType::Enqueue, 2000.0);
        op_limits.insert(OperationType::Dequeue, 2000.0);
        op_limits.insert(OperationType::BlobPut, 100.0);
        op_limits.insert(OperationType::BlobGet, 500.0);
        Self {
            global_per_sec: 10000.0,
            global_burst: 20000.0,
            user_per_sec: 100.0,
            user_burst: 200.0,
            ip_per_sec: 1000.0,
            ip_burst: 2000.0,
            critical_per_sec: 50000.0,
            critical_burst: 100000.0,
            max_tracked_users: 10000,
            max_tracked_ips: 10000,
            operation_limits: op_limits,
        }
    }
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            global: TokenBucket::new(config.global_per_sec, config.global_burst),
            per_user: DashMap::new(),
            per_ip: DashMap::new(),
            per_operation: {
                let map = DashMap::new();
                for (op, rate) in &config.operation_limits {
                    let burst = (rate * 2.0).max(100.0);
                    map.insert(*op, TokenBucket::new(*rate, burst));
                }
                map
            },
            critical: TokenBucket::new(config.critical_per_sec, config.critical_burst),
            max_tracked_users: config.max_tracked_users,
            max_tracked_ips: config.max_tracked_ips,
            config: RwLock::new(config),
            hits: AtomicU64::new(0),
            waived: AtomicU64::new(0),
        }
    }

    pub fn check(&self, ctx: &OperationContext, req: &OperationRequest) -> Result<(), ()> {
        if !self.global.try_consume(1.0) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Err(());
        }
        if let Some(ref session) = ctx.user_session {
            let bucket = self.get_or_create_user_bucket(session.user_id);
            if !bucket.try_consume(1.0) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Err(());
            }
        }
        let ip = ctx.source_addr.ip();
        let ip_bucket = self.get_or_create_ip_bucket(ip);
        if !ip_bucket.try_consume(1.0) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Err(());
        }
        let op_bucket = self.get_or_create_operation_bucket(req.operation_type);
        if !op_bucket.try_consume(1.0) {
            self.hits.fetch_add(1, Ordering::Relaxed);
            return Err(());
        }
        if req.options.priority == Priority::Critical {
            if !self.critical.try_consume(1.0) {
                self.waived.fetch_add(1, Ordering::Relaxed);
            }
        }
        Ok(())
    }

    fn get_or_create_user_bucket(&self, user_id: u128) -> dashmap::mapref::one::RefMut<'_, u128, TokenBucket> {
        if !self.per_user.contains_key(&user_id) && self.per_user.len() >= self.max_tracked_users {
            if let Some(entry) = self.per_user.iter().next() {
                let key = *entry.key();
                drop(entry);
                self.per_user.remove(&key);
            }
        }
        self.per_user.entry(user_id).or_insert_with(|| {
            let cfg = self.config.read();
            TokenBucket::new(cfg.user_per_sec, cfg.user_burst)
        })
    }

    fn get_or_create_ip_bucket(&self, ip: IpAddr) -> dashmap::mapref::one::RefMut<'_, IpAddr, TokenBucket> {
        if !self.per_ip.contains_key(&ip) && self.per_ip.len() >= self.max_tracked_ips {
            if let Some(entry) = self.per_ip.iter().next() {
                let key = *entry.key();
                drop(entry);
                self.per_ip.remove(&key);
            }
        }
        self.per_ip.entry(ip).or_insert_with(|| {
            let cfg = self.config.read();
            TokenBucket::new(cfg.ip_per_sec, cfg.ip_burst)
        })
    }

    fn get_or_create_operation_bucket(&self, op: OperationType) -> dashmap::mapref::one::RefMut<'_, OperationType, TokenBucket> {
        self.per_operation.entry(op).or_insert_with(|| {
            let cfg = self.config.read();
            let rate = cfg.operation_limits.get(&op).copied().unwrap_or(1000.0);
            let burst = (rate * 2.0).max(100.0);
            TokenBucket::new(rate, burst)
        })
    }

    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    pub fn waived(&self) -> u64 {
        self.waived.load(Ordering::Relaxed)
    }

    pub fn update_config(&mut self, config: RateLimitConfig) {
        *self.config.write() = config.clone();
        self.global = TokenBucket::new(config.global_per_sec, config.global_burst);
        self.critical = TokenBucket::new(config.critical_per_sec, config.critical_burst);
    }

    pub fn reset_key(&self, user_id: u128) {
        self.per_user.remove(&user_id);
    }

    pub fn reset_all(&self) {
        self.per_user.clear();
        self.per_ip.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::OperationContextBuilder;
    use crate::OperationRequest;
    use crate::OperationType;
    use crate::OperationTarget;
    use crate::UserSession;
    use std::collections::HashMap;
    use std::net::SocketAddr;

    fn test_addr() -> SocketAddr {
        "127.0.0.1:8080".parse().unwrap()
    }

    #[test]
    fn test_token_bucket_consumes_within_capacity() {
        let bucket = TokenBucket::new(100.0, 10.0);
        for _ in 0..10 {
            assert!(bucket.try_consume(1.0));
        }
    }

    #[test]
    fn test_token_bucket_rejects_when_exhausted() {
        let bucket = TokenBucket::new(100.0, 5.0);
        for _ in 0..5 {
            assert!(bucket.try_consume(1.0));
        }
        assert!(!bucket.try_consume(1.0));
    }

    #[test]
    fn test_token_bucket_available_returns_remaining() {
        let bucket = TokenBucket::new(0.0, 10.0);
        assert_eq!(bucket.available(), 10.0);
        assert!(bucket.try_consume(3.0));
        assert_eq!(bucket.try_consume(7.0), true);
        assert_eq!(bucket.try_consume(1.0), false);
    }

    #[test]
    fn test_rate_limiter_allows_within_global_limit() {
        let config = RateLimitConfig {
            global_per_sec: 100.0,
            global_burst: 100.0,
            user_per_sec: 100.0,
            user_burst: 100.0,
            ip_per_sec: 100.0,
            ip_burst: 100.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);
        let ctx = OperationContextBuilder::new(test_addr())
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        for _ in 0..50 {
            assert!(rl.check(&ctx, &req).is_ok(), "expected OK within limit");
        }
    }

    #[test]
    fn test_rate_limiter_rejects_above_global_limit() {
        let config = RateLimitConfig {
            global_per_sec: 1000.0,
            global_burst: 5.0,
            user_per_sec: 1000.0,
            user_burst: 1000.0,
            ip_per_sec: 1000.0,
            ip_burst: 1000.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);
        let ctx = OperationContextBuilder::new(test_addr())
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        for _ in 0..5 {
            assert!(rl.check(&ctx, &req).is_ok());
        }
        // 6th should fail (burst exhausted)
        assert!(rl.check(&ctx, &req).is_err());
    }

    #[test]
    fn test_rate_limiter_per_user() {
        let config = RateLimitConfig {
            global_per_sec: 10000.0,
            global_burst: 10000.0,
            user_per_sec: 1000.0,
            user_burst: 3.0,
            ip_per_sec: 10000.0,
            ip_burst: 10000.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);

        let session = UserSession {
            user_id: 42,
            username: "test".into(),
            roles: vec![],
            permissions: vec![],
            session_id: 1,
            metadata: HashMap::new(),
        };
        let ctx = OperationContextBuilder::new(test_addr())
            .user_session(session)
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        for _ in 0..3 {
            assert!(rl.check(&ctx, &req).is_ok());
        }
        assert!(rl.check(&ctx, &req).is_err());
    }

    #[test]
    fn test_rate_limiter_hits_counter() {
        let config = RateLimitConfig {
            global_per_sec: 1000.0,
            global_burst: 2.0,
            user_per_sec: 1000.0,
            user_burst: 1000.0,
            ip_per_sec: 1000.0,
            ip_burst: 1000.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);
        let ctx = OperationContextBuilder::new(test_addr())
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        assert!(rl.check(&ctx, &req).is_ok());
        assert!(rl.check(&ctx, &req).is_ok());
        let _ = rl.check(&ctx, &req); // 3rd should fail
        assert!(rl.hits() >= 1);
    }

    #[test]
    fn test_rate_limiter_reset_key() {
        let config = RateLimitConfig {
            global_per_sec: 10000.0,
            global_burst: 10000.0,
            user_per_sec: 1000.0,
            user_burst: 1.0,
            ip_per_sec: 10000.0,
            ip_burst: 10000.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);

        let session = UserSession {
            user_id: 99,
            username: "test".into(),
            roles: vec![],
            permissions: vec![],
            session_id: 1,
            metadata: HashMap::new(),
        };
        let ctx = OperationContextBuilder::new(test_addr())
            .user_session(session)
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        assert!(rl.check(&ctx, &req).is_ok());
        assert!(rl.check(&ctx, &req).is_err());

        rl.reset_key(99);
        // After reset, should be allowed again
        assert!(rl.check(&ctx, &req).is_ok());
    }

    #[test]
    fn test_rate_limiter_reset_all() {
        let config = RateLimitConfig {
            global_per_sec: 10000.0,
            global_burst: 10000.0,
            user_per_sec: 1000.0,
            user_burst: 1.0,
            ip_per_sec: 1000.0,
            ip_burst: 1.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);

        let session = UserSession {
            user_id: 50,
            username: "test".into(),
            roles: vec![],
            permissions: vec![],
            session_id: 1,
            metadata: HashMap::new(),
        };
        let ctx = OperationContextBuilder::new(test_addr())
            .user_session(session)
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        assert!(rl.check(&ctx, &req).is_ok());
        assert!(rl.check(&ctx, &req).is_err());

        rl.reset_all();
        // After reset, should be allowed again
        assert!(rl.check(&ctx, &req).is_ok());
    }

    #[test]
    fn test_rate_limiter_different_keys_independent() {
        let config = RateLimitConfig {
            global_per_sec: 10000.0,
            global_burst: 10000.0,
            user_per_sec: 1000.0,
            user_burst: 2.0,
            ip_per_sec: 10000.0,
            ip_burst: 10000.0,
            ..Default::default()
        };
        let rl = RateLimiter::new(config);

        let session1 = UserSession {
            user_id: 1,
            username: "alice".into(),
            roles: vec![],
            permissions: vec![],
            session_id: 1,
            metadata: HashMap::new(),
        };
        let session2 = UserSession {
            user_id: 2,
            username: "bob".into(),
            roles: vec![],
            permissions: vec![],
            session_id: 2,
            metadata: HashMap::new(),
        };

        let ctx1 = OperationContextBuilder::new(test_addr())
            .user_session(session1)
            .operation_type(OperationType::Get)
            .build();
        let ctx2 = OperationContextBuilder::new(test_addr())
            .user_session(session2)
            .operation_type(OperationType::Get)
            .build();
        let req = OperationRequest::new(OperationType::Get, OperationTarget::System);

        assert!(rl.check(&ctx1, &req).is_ok());
        assert!(rl.check(&ctx1, &req).is_ok());
        assert!(rl.check(&ctx1, &req).is_err()); // alice exhausted

        assert!(rl.check(&ctx2, &req).is_ok()); // bob still has tokens
        assert!(rl.check(&ctx2, &req).is_ok());
        assert!(rl.check(&ctx2, &req).is_err()); // bob exhausted
    }
}
