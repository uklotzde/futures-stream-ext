// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Extensions of the [`Stream`] trait and utilities for
//! transforming or shaping streams.

#![cfg_attr(docsrs, feature(doc_cfg))]

use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, stream::Fuse};

mod distinct;
pub use self::distinct::{
    DistinctState, DistinctUntilChanged, DistinctUntilChangedBy, DistinctUntilChangedByKey,
    DistinctUntilChangedErr, DistinctUntilChangedOk, filter_distinct_until_changed,
    filter_distinct_until_changed_err, filter_distinct_until_changed_ok,
};

mod debounce;
pub use self::debounce::Debounced;

mod throttle;
pub use self::throttle::{IntervalThrottler, IntervalThrottlerConfig, Throttled, Throttler};

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

    /// Restarts the elapsed sleep for the next period of a fixed schedule.
    ///
    /// The next deadline should be the previous deadline plus `period`,
    /// independent of when the sleep has been polled. Missed periods
    /// should be skipped.
    ///
    /// The default implementation invokes [`reset()`](Self::reset) and
    /// starts the next period from now, i.e. the schedule may drift.
    /// Implementations that know their deadline should override it
    /// by using [`next_period_delay()`].
    fn reset_next_period(self: Pin<&mut Self>, period: Duration) {
        self.reset(period);
    }
}

/// Delay until the next deadline on a fixed schedule with missed periods skipped.
///
/// The `elapsed` argument is the time that has passed since the previous
/// deadline.
///
/// Similar to `tokio::time::MissedTickBehavior::Skip`, but without
/// any tolerance for slightly delayed ticks.
///
/// # Panics
///
/// Panics if `period` is zero.
#[must_use]
pub fn next_period_delay(elapsed: Duration, period: Duration) -> Duration {
    assert!(!period.is_zero());
    if elapsed < period {
        return period.saturating_sub(elapsed);
    }
    // Skip missed periods and align with the schedule.
    let behind_nanos = elapsed.as_nanos() % period.as_nanos();
    let behind =
        Duration::from_nanos(u64::try_from(behind_nanos).expect("remainder is less than period"));
    period.saturating_sub(behind)
}

/// Resets the sleep or creates a new one if none exists yet.
fn reset_or_sleep<S: Sleep>(mut sleep: Pin<&mut Option<S>>, duration: Duration) {
    if let Some(sleep) = sleep.as_mut().as_pin_mut() {
        sleep.reset(duration);
    } else {
        sleep.set(Some(S::sleep(duration)));
    }
}

/// Polls the latest ready item from the stream.
///
/// Continues polling the stream while ready, but at most `max_ready_count`
/// times in a row to prevent endless loops for streams that are always ready.
/// The fused stream is never polled again after it has finished.
///
/// Returns the last ready item, if any, and whether the stream might still
/// be ready.
fn poll_latest_ready<St: Stream>(
    mut stream: Pin<&mut Fuse<St>>,
    cx: &mut Context<'_>,
    max_ready_count: NonZeroUsize,
) -> (Option<St::Item>, bool) {
    let mut latest_item = None;
    let mut ready_count = 0;
    while let Poll::Ready(Some(next_item)) = stream.as_mut().poll_next(cx) {
        latest_item = Some(next_item);
        ready_count += 1;
        if ready_count >= max_ready_count.get() {
            return (latest_item, true);
        }
    }
    (latest_item, false)
}

/// Returns `Pending` and wakes the task if the stream might still be ready.
///
/// The self-wake ensures that polling the stream continues. It is not needed
/// when returning `Ready`, because the consumer will poll again anyway.
fn pending_and_wake_if<T>(cx: &mut Context<'_>, stream_maybe_ready: bool) -> Poll<T> {
    if stream_maybe_ready {
        cx.waker().wake_by_ref();
    }
    Poll::Pending
}

/// Extension trait for [`Stream`].
///
/// Runtime-agnostic. Extensions that depend on the Tokio runtime are provided
/// by `tokio::StreamExt` (requires the `tokio` feature).
pub trait StreamExt: Stream {
    /// Filters out subsequent/adjacent stream items.
    ///
    /// See also: [`filter_distinct_until_changed()`]
    fn distinct_until_changed(self) -> DistinctUntilChanged<Self>
    where
        Self: Sized,
        Self::Item: Clone + PartialEq,
    {
        DistinctUntilChanged::new(self)
    }

    /// Filters out subsequent/adjacent stream items that are equal according to `eq_fn`.
    ///
    /// `eq_fn` is invoked with the last passed through item and the next item.
    /// The next item is discarded if `eq_fn` returns `true`.
    /// Each passed through item is cloned once and stored until it is replaced
    /// by the next item that is not equal.
    ///
    /// See also: [`distinct_until_changed()`](Self::distinct_until_changed)
    fn distinct_until_changed_by<F>(self, eq_fn: F) -> DistinctUntilChangedBy<Self, F>
    where
        Self: Sized,
        Self::Item: Clone,
        F: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        DistinctUntilChangedBy::new(self, eq_fn)
    }

    /// Filters out subsequent/adjacent stream items with the same key.
    ///
    /// Compares the keys of stream items that are extracted by `key_fn`.
    /// The key of each stream item is extracted once and the items are
    /// passed through unmodified.
    /// The last key is stored until a different key occurs.
    ///
    /// See also: [`distinct_until_changed()`](Self::distinct_until_changed)
    fn distinct_until_changed_by_key<K, F>(
        self,
        initial_key: K,
        key_fn: F,
    ) -> DistinctUntilChangedByKey<Self, K, F>
    where
        Self: Sized,
        K: PartialEq,
        F: FnMut(&Self::Item) -> K,
    {
        DistinctUntilChangedByKey::new(self, initial_key, key_fn)
    }

    /// Filters out subsequent/adjacent `Ok` items in a result stream.
    ///
    /// See also: [`filter_distinct_until_changed_ok()`]
    fn distinct_until_changed_ok<T, E>(self) -> DistinctUntilChangedOk<Self, T>
    where
        Self: Stream<Item = Result<T, E>> + Sized,
        T: Clone + PartialEq,
    {
        DistinctUntilChangedOk::new(self)
    }

    /// Filters out subsequent/adjacent `Err` items in a result stream.
    ///
    /// See also: [`filter_distinct_until_changed_err()`]
    fn distinct_until_changed_err<T, E>(self) -> DistinctUntilChangedErr<Self, E>
    where
        Self: Stream<Item = Result<T, E>> + Sized,
        E: Clone + PartialEq,
    {
        DistinctUntilChangedErr::new(self)
    }

    /// Debounces an input stream.
    ///
    /// Delays items until no more new items have arrived during the delay window.
    ///
    /// Filters out items that arrive in quick succession. Only the last item from
    /// each sequence is emitted. All preceding items are discarded.
    ///
    /// The delay is implemented by the given [`Sleep`] type.
    ///
    /// The `poll_next_max_ready_count` argument controls how many _Ready_ items
    /// are polled consecutively from the input stream during an invocation
    /// of [`Stream::poll_next()`] before polling the delay. This limit ensures
    /// that input streams which are always ready are not polled forever.
    /// See also: [`throttle()`](Self::throttle)
    fn debounce_with_sleep<S: Sleep>(
        self,
        delay: Duration,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Debounced<Self, S>
    where
        Self: Sized,
    {
        Debounced::new(self, delay, poll_next_max_ready_count)
    }

    /// Throttles an input stream.
    ///
    /// The throttler defines the throttling strategy.
    ///
    /// The `poll_next_max_ready_count` argument controls how many _Ready_ items
    /// are polled consecutively from the input stream during an invocation
    /// of [`Stream::poll_next()`] before polling the throttler. This limit
    /// ensures that input streams which are always ready are not polled forever.
    ///
    /// A value of [`NonZeroUsize::MIN`] will poll the input stream only once and
    /// could be used as a safe default. Using a greater value to skip multiple
    /// items at once will reduce the number of calls to the throttler and
    /// improves the performance and efficiency.
    fn throttle<Th>(
        self,
        throttler: Th,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, Th>
    where
        Self: Sized,
        Th: Throttler<Self::Item>,
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
        config: IntervalThrottlerConfig,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Throttled<Self, IntervalThrottler<S>>
    where
        Self: Sized,
    {
        self.throttle(IntervalThrottler::new(config), poll_next_max_ready_count)
    }
}

impl<St: Stream> StreamExt for St {}

#[cfg(test)]
mod tests;
