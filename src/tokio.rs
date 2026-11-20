// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions that depend on the Tokio runtime.

use std::{num::NonZeroUsize, pin::Pin, time::Duration};

use futures_util::Stream;

use crate::{Debounced, IntervalThrottlerConfig, StreamExt as _, Throttled};

/// [`IntervalThrottler`](crate::IntervalThrottler) using [`tokio::time::Sleep`].
pub type TokioIntervalThrottler = crate::IntervalThrottler<tokio::time::Sleep>;

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
        let delay =
            crate::next_period_delay(now.saturating_duration_since(self.deadline()), period);
        tokio::time::Sleep::reset(self, now + delay);
    }
}

/// Extension trait for [`Stream`] using the Tokio runtime.
///
/// See also: [`crate::StreamExt`]
pub trait StreamExt: Stream {
    /// Debounces an input stream using [`tokio::time::Sleep`].
    ///
    /// See also: [`debounce_with_sleep()`](crate::StreamExt::debounce_with_sleep)
    fn debounce(
        self,
        delay: Duration,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Debounced<Self, tokio::time::Sleep>
    where
        Self: Sized,
    {
        self.debounce_with_sleep(delay, poll_next_max_ready_count)
    }

    /// Throttles an input stream by using a fixed interval with [`tokio::time::Sleep`].
    ///
    /// See also: [`throttle_interval_with_sleep()`](crate::StreamExt::throttle_interval_with_sleep)
    fn throttle_interval(
        self,
        config: IntervalThrottlerConfig,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, TokioIntervalThrottler>
    where
        Self: Sized,
    {
        self.throttle_interval_with_sleep(config, poll_next_max_ready_count)
    }
}

impl<St: Stream> StreamExt for St {}

#[cfg(test)]
mod tests;
