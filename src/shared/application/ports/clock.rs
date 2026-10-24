pub(crate) trait Clock {
    fn now_ms(&self) -> u64;
}

#[cfg(test)]
pub(crate) struct FixedClock {
    now_ms: u64,
}

#[cfg(test)]
impl FixedClock {
    pub(crate) fn new(now_ms: u64) -> Self {
        FixedClock { now_ms }
    }
}

#[cfg(test)]
impl Clock for FixedClock {
    fn now_ms(&self) -> u64 {
        self.now_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_report_the_wall_clock_milliseconds_it_was_fixed_at() {
        let clock = FixedClock::new(1_700_000_123_456);

        let now = clock.now_ms();

        assert_eq!(now, 1_700_000_123_456);
    }
}
