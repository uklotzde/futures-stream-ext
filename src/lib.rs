// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions of the [`Stream`] trait and utilities for
//! transforming or shaping streams.

use std::{num::NonZeroUsize, pin::Pin, time::Duration};

use futures_util::{Stream, StreamExt as _};

mod distinct;
pub use self::distinct::{
    distinct_until_changed, distinct_until_changed_err_result, distinct_until_changed_map,
    distinct_until_changed_ok_result, filter_distinct_until_changed,
    filter_distinct_until_changed_err_result, filter_distinct_until_changed_ok_result,
};

mod debounce;
pub use self::debounce::Debounced;

mod throttle;
pub use self::throttle::{IntervalThrottler, ThrottleIntervalConfig, Throttled, Throttler};

#[cfg(feature = "tokio")]
#[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
pub mod tokio;

/// Interval edge trigger variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntervalEdge {
    Leading,
    Trailing,
}

pub trait Sleep: Future<Output = ()> + Sized {
    /// Suspends the task for the given duration.
    fn sleep(duration: Duration) -> Self;

    /// Restarts the sleep to elapse after the given duration from now.
    ///
    /// Might be invoked both before and after the sleep has elapsed.
    ///
    /// The default implementation replaces `self` with a new sleep.
    /// Override it if the sleep could be reset more efficiently,
    /// e.g. by reusing an already registered timer.
    fn reset(mut self: Pin<&mut Self>, duration: Duration) {
        self.set(Self::sleep(duration));
    }
}

/// Resets the sleep or creates a new one if none exists yet.
fn reset_or_sleep<S: Sleep>(mut sleep: Pin<&mut Option<S>>, duration: Duration) {
    if let Some(sleep) = sleep.as_mut().as_pin_mut() {
        sleep.reset(duration);
    } else {
        sleep.set(Some(S::sleep(duration)));
    }
}

/// Extension trait for [`Stream`].
///
/// Runtime-agnostic. Extensions that depend on the Tokio runtime are provided
/// by `tokio::StreamExt` (requires the `tokio` feature).
pub trait StreamExt: Stream {
    /// Debounces an input stream.
    ///
    /// Delays items until no more new items have arrived during the delay window.
    ///
    /// Filters out items that arrive in quick succession. Only the last item from
    /// each sequence is emitted. All preceding items are discarded.
    ///
    /// The delay is implemented by the given [`Sleep`] type.
    fn debounce_with_sleep<S: Sleep>(self, delay: Duration) -> Debounced<Self, S>
    where
        Self: Sized,
    {
        Debounced::new(self, delay)
    }

    /// Throttles an input stream.
    ///
    /// The throttler defines the throttling strategy.
    ///
    /// The `poll_next_max_ready_count` argument controls how many _Ready_ items
    /// are polled subsequently in a row from the input stream during an invocation
    /// of [`Stream::poll_next()`] before polling the throttler. This limit
    /// ensures that input streams which are always ready are not polled forever.
    ///
    /// A value of [`NonZeroUsize::MIN`] will poll the input stream only once and
    /// could be used as a save default. Using a greater value to skip multiple
    /// items at once will reduce the number of calls to the throttler and
    /// improves the performance and efficiency.
    fn throttle<T>(
        self,
        throttler: T,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, T>
    where
        Self: Sized,
        T: Throttler<Self::Item>,
    {
        Throttled::new(self, throttler, poll_next_max_ready_count)
    }

    /// Throttles an input stream by using a fixed interval.
    ///
    /// The interval is implemented by the given [`Sleep`] type.
    ///
    /// See also: [`throttle()`](Self::throttle)
    fn throttle_interval_with_sleep<S: Sleep>(
        self,
        config: ThrottleIntervalConfig,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, IntervalThrottler<S>>
    where
        Self: Sized,
    {
        self.throttle(IntervalThrottler::new(config), poll_next_max_ready_count)
    }
}

impl<S: Stream> StreamExt for S {}

fn filter_stateful_sync<S, T, F>(
    stream: S,
    initial_state: T,
    mut filter_update_state_fn: F,
) -> impl Stream<Item = S::Item>
where
    S: Stream,
    F: FnMut(&mut T, &S::Item) -> bool,
{
    let mut state = initial_state;
    stream
        .filter(move |next_item| std::future::ready(filter_update_state_fn(&mut state, next_item)))
}

#[cfg(test)]
mod tests;
