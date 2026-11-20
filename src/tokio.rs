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
