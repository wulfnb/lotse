//! A timed-out queued command must never execute later against the browser.
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Deadline {
    expires: Instant,
    cancelled: Arc<AtomicBool>,
}
impl Deadline {
    pub fn new(timeout: Duration) -> Self {
        Self {
            expires: Instant::now() + timeout,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn expired(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.expires
    }
    pub fn remaining(&self) -> Duration {
        self.expires.saturating_duration_since(Instant::now())
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
