// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions that depend on the Tokio runtime.

use std::{num::NonZeroUsize, pin::Pin, time::Duration};

use futures_util::Stream;

use crate::{Debounced, StreamExt as _, ThrottleIntervalConfig, Throttled};

#[cfg(test)]
mod debounce;

#[cfg(test)]
mod throttle;

/// [`IntervalThrottler`](crate::IntervalThrottler) using [`tokio::time::Sleep`].
pub type IntervalThrottler = crate::IntervalThrottler<tokio::time::Sleep>;

impl crate::Sleep for tokio::time::Sleep {
    fn sleep(duration: Duration) -> Self {
        tokio::time::sleep(duration)
    }

    fn reset(self: Pin<&mut Self>, duration: Duration) {
        // Reuses the registered timer.
        tokio::time::Sleep::reset(self, tokio::time::Instant::now() + duration);
    }

    fn reset_next_period(self: Pin<&mut Self>, period: Duration) {
        let now = tokio::time::Instant::now();
        let deadline = next_period_deadline(self.deadline(), now, period);
        tokio::time::Sleep::reset(self, deadline);
    }
}

/// Next deadline on a fixed schedule with missed periods skipped.
///
/// Similar to [`tokio::time::MissedTickBehavior::Skip`], but without
/// any tolerance for slightly delayed ticks.
fn next_period_deadline(
    deadline: tokio::time::Instant,
    now: tokio::time::Instant,
    period: Duration,
) -> tokio::time::Instant {
    debug_assert!(!period.is_zero());
    let next_deadline = deadline + period;
    if next_deadline > now {
        return next_deadline;
    }
    // Skip missed periods and align with the schedule.
    let behind_nanos = (now - deadline).as_nanos() % period.as_nanos();
    let behind =
        Duration::from_nanos(u64::try_from(behind_nanos).expect("remainder is less than period"));
    now + period - behind
}

/// Extension trait for [`Stream`] using the Tokio runtime.
///
/// See also: [`crate::StreamExt`]
pub trait StreamExt: Stream {
    /// Debounces an input stream using [`tokio::time::Sleep`].
    ///
    /// See also: [`debounce_with_sleep()`](crate::StreamExt::debounce_with_sleep)
    fn debounce(self, delay: Duration) -> Debounced<Self, tokio::time::Sleep>
    where
        Self: Sized,
    {
        self.debounce_with_sleep(delay)
    }

    /// Throttles an input stream by using a fixed interval with [`tokio::time::Sleep`].
    ///
    /// See also: [`throttle_interval_with_sleep()`](crate::StreamExt::throttle_interval_with_sleep)
    fn throttle_interval(
        self,
        config: ThrottleIntervalConfig,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, IntervalThrottler>
    where
        Self: Sized,
    {
        self.throttle_interval_with_sleep(config, poll_next_max_ready_count)
    }
}

impl<S: Stream> StreamExt for S {}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::next_period_deadline;

    #[test]
    fn next_period_deadline_on_schedule() {
        let deadline = Instant::now();
        let period = Duration::from_millis(10);
        // Polled immediately or late, but within the next period.
        for delay_millis in [0, 1, 9] {
            let now = deadline + Duration::from_millis(delay_millis);
            assert_eq!(
                deadline + period,
                next_period_deadline(deadline, now, period)
            );
        }
    }

    #[test]
    fn next_period_deadline_skips_missed_periods() {
        let deadline = Instant::now();
        let period = Duration::from_millis(10);
        for (delay_millis, expected_millis) in [(10, 20), (15, 20), (25, 30), (30, 40)] {
            let now = deadline + Duration::from_millis(delay_millis);
            assert_eq!(
                deadline + Duration::from_millis(expected_millis),
                next_period_deadline(deadline, now, period)
            );
        }
    }
}
