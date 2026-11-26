//! Tokio implementation of [`opendnp3::clock::Clock`].

use std::time::Instant;

use opendnp3::clock::{Clock, MonotonicTime};

/// [`Clock`] implementation backed by `tokio::time`, anchored to a monotonic origin.
#[derive(Debug, Clone, Copy)]
pub struct TokioClock {
    origin: Instant,
}

impl TokioClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl Default for TokioClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for TokioClock {
    type Sleep = tokio::time::Sleep;

    fn now(&self) -> MonotonicTime {
        MonotonicTime::from_elapsed(self.origin)
    }

    fn sleep_until(&self, deadline: MonotonicTime) -> Self::Sleep {
        let now = self.now();
        let dur = deadline.saturating_duration_since(now);
        tokio::time::sleep(dur)
    }
}
