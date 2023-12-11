// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions of the `futures::Stream` trait.

// Repetitions of module/type names occur frequently when using many
// modules for keeping the size of the source files handy. Often
// types have the same name as their parent module.
#![allow(clippy::module_name_repetitions)]
// Repeating the type name in `Default::default()` expressions is not needed
// as long as the context is obvious.
#![allow(clippy::default_trait_access)]

use futures_core::Stream;

mod throttle;
#[cfg(feature = "tokio")]
pub use self::throttle::IntervalThrottler;
pub use self::throttle::{IntervalThrottlerConfig, Throttle, Throttler};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalEdge {
    Leading,
    Trailing,
}

pub trait StreamExt {
    fn throttle<T>(
        self,
        throttler: T,
        poll_next_max_ready_count: std::num::NonZeroUsize,
    ) -> Throttle<Self, T>
    where
        Self: Stream + Sized,
        T: Throttler<Self::Item>,
    {
        Throttle::new(self, throttler, poll_next_max_ready_count)
    }

    #[cfg(feature = "tokio")]
    fn throttle_interval(
        self,
        config: IntervalThrottlerConfig,
        poll_next_max_ready_count: std::num::NonZeroUsize,
    ) -> Throttle<Self, IntervalThrottler<Self::Item>>
    where
        Self: Stream + Sized,
    {
        let throttler = IntervalThrottler::new(config);
        self.throttle(throttler, poll_next_max_ready_count)
    }
}

impl<S: Stream> StreamExt for S {}
