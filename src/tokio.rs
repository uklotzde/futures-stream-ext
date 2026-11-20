// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{num::NonZeroUsize, time::Duration};

use futures_util::Stream;
use tokio::time::Sleep;

use crate::{StreamExt, ThrottleIntervalConfig, Throttled};

#[cfg(test)]
mod debounce;

mod throttle;
pub use self::throttle::IntervalThrottler;

impl crate::Sleep for tokio::time::Sleep {
    fn sleep(duration: Duration) -> Self {
        tokio::time::sleep(duration)
    }
}

impl<S: Stream> StreamExt for S {
    type Sleep = Sleep;
    type IntervalThrottler = IntervalThrottler<S::Item>;

    fn throttle_interval(
        self,
        config: ThrottleIntervalConfig,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, Self::IntervalThrottler>
    where
        Self: Sized,
    {
        let throttler = IntervalThrottler::new(config);
        self.throttle(throttler, poll_next_max_ready_count)
    }
}
