// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions of the [`Stream`] trait.

// Repetitions of module/type names occur frequently when using many
// modules for keeping the size of the source files handy. Often
// types have the same name as their parent module.
#![allow(clippy::module_name_repetitions)]
// Repeating the type name in `Default::default()` expressions is not needed
// as long as the context is obvious.
#![allow(clippy::default_trait_access)]

use std::num::NonZeroUsize;

use futures_core::Stream;

mod throttle;
#[cfg(feature = "tokio")]
pub use self::throttle::IntervalThrottler;
pub use self::throttle::{IntervalThrottlerConfig, Throttle, Throttler};

/// Interval edge trigger variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalEdge {
    Leading,
    Trailing,
}

/// Extension trait for [`Stream`]
pub trait StreamExt: Stream {
    /// Throttle an input stream
    ///
    /// The throttler defines the throttling strategy.
    ///
    /// The `poll_next_max_ready_count` argument controls how many _Ready_ items
    /// are polled at once from the input stream during an invocation of
    /// [`Stream::poll_next()`] before polling the throttler. This limit
    /// ensures that input streams which are always ready are not polled forever.
    /// A value of [`NonZeroUsize::MIN`] will poll the input stream only once and
    /// could be used as a save default. Using a greater value to skip multiple
    /// items at once will reduce the number of calls to the throttler and
    /// improves the performance and efficiency.
    fn throttle<T>(self, throttler: T, poll_next_max_ready_count: NonZeroUsize) -> Throttle<Self, T>
    where
        Self: Stream + Sized,
        T: Throttler<Self::Item>,
    {
        Throttle::new(self, throttler, poll_next_max_ready_count)
    }

    /// Throttle an input stream by using a fixed interval
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
