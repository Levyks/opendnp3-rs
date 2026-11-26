//! Time abstraction for the runtime-agnostic core.
//!
//! Core protocol objects never read a clock directly: drivers pass the
//! current time into `tick(now)` / `on_timeout(now)` methods, and objects
//! report when they next need attention via `next_wakeup()`-style methods.
//! This trait exists so that real (async) drivers can convert those
//! deadlines into sleeps on their runtime.

use std::future::Future;
use std::time::{Duration, Instant};

/// A source of time able to produce a future that resolves at a deadline.
///
/// Implemented for tokio in the `opendnp3-tokio` crate; consumers of other
/// runtimes implement this for their timer facilities.
pub trait Clock {
    type Sleep: Future<Output = ()>;

    /// The current time.
    fn now(&self) -> Instant;

    /// A future that resolves at or after `deadline`.
    fn sleep_until(&self, deadline: Instant) -> Self::Sleep;
}

/// A manually-driven clock for deterministic tests.
///
/// Equivalent to the C++ test suite's `MockExecutor` time: tests advance the
/// clock explicitly and observe timer-driven behavior without real sleeps.
#[derive(Debug, Clone)]
pub struct ManualClock {
    now: Instant,
}

impl ManualClock {
    pub fn new() -> Self {
        Self {
            now: Instant::now(),
        }
    }

    pub fn now(&self) -> Instant {
        self.now
    }

    /// Advance the clock.
    pub fn advance(&mut self, duration: Duration) {
        self.now += duration;
    }

    /// Whether `deadline` has been reached.
    pub fn expired(&self, deadline: Instant) -> bool {
        self.now >= deadline
    }
}

impl Default for ManualClock {
    fn default() -> Self {
        Self::new()
    }
}
