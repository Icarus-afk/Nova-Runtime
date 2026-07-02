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
