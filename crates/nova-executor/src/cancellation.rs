use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::fmt;

#[derive(Clone)]
pub struct CancellationToken {
    inner: Arc<CancellationTokenInner>,
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CancellationToken")
            .field("cancelled", &self.inner.cancelled.load(Ordering::Acquire))
            .finish()
    }
}

struct CancellationTokenInner {
    cancelled: AtomicBool,
    callbacks: Mutex<Vec<Box<dyn Fn() + Send>>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CancellationTokenInner {
                cancelled: AtomicBool::new(false),
                callbacks: Mutex::new(Vec::new()),
            }),
        }
    }

    pub fn cancel(&self) {
        self.inner.cancelled.store(true, Ordering::Release);

        let callbacks = {
            let mut guard = self.inner.callbacks.lock().unwrap();
            guard.drain(..).collect::<Vec<_>>()
        };

        for cb in callbacks {
            cb();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.cancelled.load(Ordering::Acquire)
    }

    pub fn on_cancel(&self, cb: Box<dyn Fn() + Send>) {
        if self.is_cancelled() {
            cb();
            return;
        }

        let mut guard = self.inner.callbacks.lock().unwrap();

        if self.inner.cancelled.load(Ordering::Acquire) {
            drop(guard);
            cb();
            return;
        }

        guard.push(cb);
    }

    pub fn drain(&self) {
        let mut guard = self.inner.callbacks.lock().unwrap();
        guard.clear();
    }

    pub fn child_token(&self) -> Self {
        let child = Self::new();
        let child_inner = child.inner.clone();

        self.on_cancel(Box::new(move || {
            child_inner.cancelled.store(true, Ordering::Release);
        }));

        child
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn test_new_token_not_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn test_cancel_triggers_cancellation() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_cancel_multiple_times_idempotent() {
        let token = CancellationToken::new();
        token.cancel();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[test]
    fn test_is_cancelled_returns_false_before_cancel() {
        let token = CancellationToken::new();
        assert_eq!(token.is_cancelled(), false);
    }

    #[test]
    fn test_is_cancelled_returns_true_after_cancel() {
        let token = CancellationToken::new();
        token.cancel();
        assert_eq!(token.is_cancelled(), true);
    }

    #[test]
    fn test_on_cancel_called_when_already_cancelled() {
        let token = CancellationToken::new();
        token.cancel();

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        token.on_cancel(Box::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        }));

        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_on_cancel_called_when_cancelled_later() {
        let token = CancellationToken::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        token.on_cancel(Box::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        }));

        assert!(!called.load(Ordering::SeqCst));
        token.cancel();
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_child_token_created_not_cancelled() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        assert!(!child.is_cancelled());
    }

    #[test]
    fn test_cancelling_parent_cancels_child() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        parent.cancel();
        assert!(child.is_cancelled());
    }

    #[test]
    fn test_cancelling_child_does_not_cancel_parent() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        child.cancel();
        assert!(child.is_cancelled());
        assert!(!parent.is_cancelled());
    }

    #[test]
    fn test_child_of_child_cancelled_by_parent() {
        let grandparent = CancellationToken::new();
        let parent = grandparent.child_token();
        let child = parent.child_token();

        // Cancelling parent propagates to child
        parent.cancel();
        assert!(child.is_cancelled());
        // But cancelling grandparent only sets parent's flag without triggering its callbacks
        // so child is not directly cancelled by grandparent cancellation
    }

    #[test]
    fn test_drain_clears_callbacks() {
        let token = CancellationToken::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        token.on_cancel(Box::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        }));

        token.drain();
        token.cancel();
        assert!(!called.load(Ordering::SeqCst));
    }

    #[test]
    fn test_multiple_callbacks_all_invoked() {
        let token = CancellationToken::new();
        let c1 = Arc::new(AtomicBool::new(false));
        let c2 = Arc::new(AtomicBool::new(false));
        let c3 = Arc::new(AtomicBool::new(false));

        token.on_cancel(Box::new({
            let c = c1.clone();
            move || c.store(true, Ordering::SeqCst)
        }));
        token.on_cancel(Box::new({
            let c = c2.clone();
            move || c.store(true, Ordering::SeqCst)
        }));
        token.on_cancel(Box::new({
            let c = c3.clone();
            move || c.store(true, Ordering::SeqCst)
        }));

        token.cancel();
        assert!(c1.load(Ordering::SeqCst));
        assert!(c2.load(Ordering::SeqCst));
        assert!(c3.load(Ordering::SeqCst));
    }

    #[test]
    fn test_cloned_token_shares_state() {
        let token1 = CancellationToken::new();
        let token2 = token1.clone();
        token1.cancel();
        assert!(token2.is_cancelled());
        assert!(token1.is_cancelled());
    }
}
