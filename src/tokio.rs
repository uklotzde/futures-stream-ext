// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions that depend on the Tokio runtime.

use std::{num::NonZeroUsize, time::Duration};

use futures_util::Stream;

use crate::{Debounced, StreamExt as _, ThrottleIntervalConfig, Throttled};

#[cfg(test)]
mod debounce;

mod throttle;
pub use self::throttle::IntervalThrottler;

impl crate::Sleep for tokio::time::Sleep {
    fn sleep(duration: Duration) -> Self {
        tokio::time::sleep(duration)
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

    /// Throttles an input stream by using a fixed interval.
    ///
    /// See also: [`throttle()`](crate::StreamExt::throttle)
    fn throttle_interval(
        self,
        config: ThrottleIntervalConfig,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, IntervalThrottler<Self::Item>>
    where
        Self: Sized,
    {
        self.throttle(IntervalThrottler::new(config), poll_next_max_ready_count)
    }
}

impl<S: Stream> StreamExt for S {}
