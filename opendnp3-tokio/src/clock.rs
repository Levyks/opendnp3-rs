//! Tokio implementation of [`opendnp3::clock::Clock`].

use std::time::Instant;

use opendnp3::clock::Clock;

/// [`Clock`] implementation backed by `tokio::time`.
#[derive(Debug, Clone, Copy, Default)]
pub struct TokioClock;

impl Clock for TokioClock {
    type Sleep = tokio::time::Sleep;

    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep_until(&self, deadline: Instant) -> Self::Sleep {
        tokio::time::sleep_until(tokio::time::Instant::from_std(deadline))
    }
}
