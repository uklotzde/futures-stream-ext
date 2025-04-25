// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_core::Stream;

use crate::{StreamExt, Throttle, ThrottleIntervalConfig};

mod throttle;
pub use self::throttle::IntervalThrottler;

impl<S: Stream> StreamExt for S {
    type IntervalThrottler = IntervalThrottler<S::Item>;

    fn throttle_interval(
        self,
        config: ThrottleIntervalConfig,
        poll_next_max_ready_count: std::num::NonZeroUsize,
    ) -> Throttle<Self, Self::IntervalThrottler>
    where
        Self: Sized,
    {
        let throttler = IntervalThrottler::new(config);
        self.throttle(throttler, poll_next_max_ready_count)
    }
}
