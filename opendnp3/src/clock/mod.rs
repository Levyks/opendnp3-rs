//! Time abstraction for the runtime-agnostic, `no_std`-compatible core.
//!
//! Core protocol objects never read a wall or OS clock directly: drivers pass
//! monotonic millisecond timestamps ([`MonotonicTime`]) into `on_timeout(now)`
//! and state-machine methods, and objects report deadlines via `next_wakeup()`.

use core::future::Future;
use core::ops::{Add, Sub};
use core::time::Duration;

/// A `no_std`-compatible monotonic timestamp in milliseconds.
///
/// Maps directly to `embassy_time::Instant::as_millis()`,
/// `std::time::Instant::elapsed().as_millis()`, and FFI `u64` timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Hash)]
pub struct MonotonicTime(pub u64);

impl MonotonicTime {
    pub const ZERO: Self = Self(0);

    pub const fn from_millis(ms: u64) -> Self {
        Self(ms)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        let ms = u64::try_from(duration.as_millis()).ok()?;
        self.0.checked_add(ms).map(Self)
    }

    pub fn saturating_add(self, duration: Duration) -> Self {
        let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(ms))
    }

    pub fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }

    #[cfg(feature = "std")]
    pub fn from_elapsed(origin: std::time::Instant) -> Self {
        Self(origin.elapsed().as_millis() as u64)
    }
}

impl Add<Duration> for MonotonicTime {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        self.saturating_add(rhs)
    }
}

impl Sub<MonotonicTime> for MonotonicTime {
    type Output = Duration;

    fn sub(self, rhs: MonotonicTime) -> Self::Output {
        self.saturating_duration_since(rhs)
    }
}

/// A source of monotonic time able to produce a future that resolves at a deadline.
pub trait Clock {
    type Sleep: Future<Output = ()>;

    /// The current monotonic time.
    fn now(&self) -> MonotonicTime;

    /// A future that resolves at or after `deadline`.
    fn sleep_until(&self, deadline: MonotonicTime) -> Self::Sleep;
}

/// A manually-driven clock for deterministic tests (`no_std` compatible).
#[derive(Debug, Clone, Default)]
pub struct ManualClock {
    now: MonotonicTime,
}

impl ManualClock {
    pub const fn new() -> Self {
        Self {
            now: MonotonicTime::ZERO,
        }
    }

    pub const fn now(&self) -> MonotonicTime {
        self.now
    }

    /// Advance the clock.
    pub fn advance(&mut self, duration: Duration) {
        self.now = self.now.saturating_add(duration);
    }

    /// Whether `deadline` has been reached.
    pub const fn expired(&self, deadline: MonotonicTime) -> bool {
        self.now.0 >= deadline.0
    }
}
