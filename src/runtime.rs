use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Serialize)]
pub struct RuntimeSnapshot {
    pub async_pending_bytes: usize,
    pub async_batches: u64,
    pub dropped_logs: u64,
}

pub struct RuntimeStats {
    async_pending_bytes: AtomicUsize,
    async_batches: AtomicU64,
    dropped_logs: AtomicU64,
}

impl RuntimeStats {
    pub fn new() -> Self {
        Self {
            async_pending_bytes: AtomicUsize::new(0),
            async_batches: AtomicU64::new(0),
            dropped_logs: AtomicU64::new(0),
        }
    }

    pub fn record_async_push(&self, bytes: usize) {
        self.async_pending_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_async_batch(&self, bytes: usize) {
        self.async_pending_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                Some(current.saturating_sub(bytes))
            })
            .ok();
        self.async_batches.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_drop(&self) {
        self.dropped_logs.fetch_add(1, Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            async_pending_bytes: self.async_pending_bytes.load(Ordering::Relaxed),
            async_batches: self.async_batches.load(Ordering::Relaxed),
            dropped_logs: self.dropped_logs.load(Ordering::Relaxed),
        }
    }
}

pub static RUNTIME_STATS: Lazy<RuntimeStats> = Lazy::new(RuntimeStats::new);
