//! A swappable time source, so deadline logic can be tested without sleeping.

use std::time::Instant;

/// A source of monotonic time.
pub trait Clock {
    /// The current instant.
    fn now(&self) -> Instant;
}

/// The real clock, backed by [`Instant::now`].
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_never_goes_backwards() {
        let c = SystemClock;
        let a = c.now();
        assert!(c.now() >= a);
    }
}
