//! Monotonic clocks for device-flow polling deadlines.
//!
//! GitHub device authorization expiry is measured as elapsed time from the
//! authorization response. Wall-clock timestamps can move backwards when the
//! host clock is adjusted, so the polling loop depends on this narrow
//! [`MonotonicClock`] port over [`Instant`].

use std::time::Instant;

/// Clock abstraction for monotonic elapsed-time measurements.
pub trait MonotonicClock: Send + Sync + std::fmt::Debug {
    /// Returns the current monotonic instant.
    fn now(&self) -> Instant;
}

/// Monotonic clock backed by [`Instant::now`].
#[derive(Clone, Copy, Debug, Default)]
pub struct StdMonotonicClock;

impl MonotonicClock for StdMonotonicClock {
    fn now(&self) -> Instant { Instant::now() }
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Deterministic monotonic clocks for device-flow tests.

    use std::{collections::VecDeque, sync::Mutex, time::Instant};

    use super::MonotonicClock;

    /// Deterministic monotonic clock for tests.
    #[derive(Debug)]
    pub(crate) struct FixedMonotonicClock {
        instants: Mutex<VecDeque<Instant>>,
    }

    impl FixedMonotonicClock {
        /// Creates a fixed clock that returns the supplied instants in order.
        pub(crate) fn from_instants(instants: impl IntoIterator<Item = Instant>) -> Self {
            Self { instants: Mutex::new(instants.into_iter().collect()) }
        }
    }

    impl MonotonicClock for FixedMonotonicClock {
        fn now(&self) -> Instant {
            match self.instants.lock() {
                Ok(mut instants) => next_instant(&mut instants),
                Err(poisoned) => next_instant(&mut poisoned.into_inner()),
            }
        }
    }

    fn next_instant(instants: &mut VecDeque<Instant>) -> Instant {
        let Some(instant) = instants.pop_front() else {
            panic!("fixed monotonic clock has no instants remaining");
        };
        instant
    }
}
