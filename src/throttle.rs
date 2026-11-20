// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};

use futures_util::{
    Stream, StreamExt as _,
    stream::{Fuse, FusedStream as _},
};
use pin_project_lite::pin_project;

use crate::{IntervalEdge, Sleep, pending_and_wake_if, poll_latest_ready, reset_or_sleep};

/// Callbacks for throttling a stream
pub trait Throttler<T>: Stream<Item = ()> {
    /// A new item has been received from the input stream.
    ///
    /// After invocation of this method `throttle_ready` will be called
    /// with `Some` when the current interval has elapsed. The current
    /// item that will be yielded may still change until `throttle_ready`
    /// is called.
    ///
    /// The `cx` argument is only provided for consistency and can safely
    /// be ignored in most cases. The throttler will be polled immediately
    /// after this method returns.
    fn throttle_pending(self: Pin<&mut Self>, cx: &mut Context<'_>);

    /// The current interval has elapsed.
    ///
    /// Provides the pending item of the throttled input stream that is ready
    /// or `None` if no item has been received from the input stream during
    /// the last interval.
    fn throttle_ready(self: Pin<&mut Self>, cx: &mut Context<'_>, next_item: Option<&T>);
}

pin_project! {
    /// Throttled stream
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct Throttled<S: Stream, T: Throttler<<S as Stream>::Item>> {
        #[pin]
        stream: Fuse<S>,
        #[pin]
        throttler: T,
        poll_next_max_ready_count: NonZeroUsize,
        pending: Option<S::Item>,
    }
}

impl<S, T> Throttled<S, T>
where
    S: Stream,
    T: Throttler<<S as Stream>::Item>,
{
    pub fn new(stream: S, throttler: T, poll_next_max_ready_count: NonZeroUsize) -> Self {
        Self {
            stream: stream.fuse(),
            throttler,
            poll_next_max_ready_count,
            pending: None,
        }
    }
}

impl<S, T> Stream for Throttled<S, T>
where
    S: Stream,
    T: Throttler<<S as Stream>::Item>,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // We want to receive the most recent item that is ready.
        let (latest_item, stream_maybe_ready) =
            poll_latest_ready(this.stream.as_mut(), cx, *this.poll_next_max_ready_count);
        if let Some(latest_item) = latest_item {
            if this.pending.is_none() {
                this.throttler.as_mut().throttle_pending(cx);
            }
            *this.pending = Some(latest_item);
        }

        if this.stream.is_terminated() {
            if this.pending.is_none() {
                // Stream has finished.
                return Poll::Ready(None);
            }
            // Yield the last pending item when the throttler is ready.
            ready!(this.throttler.as_mut().poll_next(cx));
            return Poll::Ready(this.pending.take());
        }

        if this.throttler.as_mut().poll_next(cx).is_ready() {
            let next_item = this.pending.take();
            this.throttler
                .as_mut()
                .throttle_ready(cx, next_item.as_ref());
            if next_item.is_some() {
                return Poll::Ready(next_item);
            }
        }
        pending_and_wake_if(cx, stream_maybe_ready)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThrottleIntervalConfig {
    /// Throttling period
    ///
    /// The minimum interval between subsequent items that limits the
    /// maximum frequency of the output stream.
    pub period: Duration,

    /// Interval edge gate
    ///
    /// Controls whether the pending item of the stream is yielded
    /// immediately or after the interval has elapsed.
    pub edge: IntervalEdge,
}

#[derive(Debug, Clone, Copy)]
enum IntervalThrottlerState {
    Idle,
    Pending,
}

pin_project! {
    /// Throttles a stream by using a fixed interval.
    ///
    /// The interval is implemented by the given [`Sleep`] type. While items are
    /// pending, intervals follow a fixed schedule with missed intervals skipped
    /// (see [`Sleep::reset_next_period()`]).
    #[derive(Debug)]
    #[project = IntervalThrottlerProjection]
    pub struct IntervalThrottler<S> {
        config: ThrottleIntervalConfig,
        // Created on demand and reused afterwards.
        #[pin]
        sleep: Option<S>,
        // Wait for `sleep` before the next tick.
        // Always `false` if the period is zero.
        sleep_armed: bool,
        state: IntervalThrottlerState,
    }
}

impl<S: Sleep> IntervalThrottler<S> {
    #[must_use]
    pub const fn new(config: ThrottleIntervalConfig) -> Self {
        Self {
            config,
            sleep: None,
            sleep_armed: false,
            state: IntervalThrottlerState::Idle,
        }
    }
}

/// Starts the next interval.
fn restart_interval<S: Sleep>(
    sleep: Pin<&mut Option<S>>,
    sleep_armed: &mut bool,
    period: Duration,
) {
    if period.is_zero() {
        // No throttling.
        *sleep_armed = false;
        return;
    }
    reset_or_sleep(sleep, period);
    *sleep_armed = true;
}

impl<S: Sleep> Stream for IntervalThrottler<S> {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let IntervalThrottlerProjection {
            config,
            sleep,
            sleep_armed,
            state,
        } = self.project();
        match state {
            IntervalThrottlerState::Idle => Poll::Pending,
            IntervalThrottlerState::Pending => {
                if *sleep_armed {
                    let mut sleep = sleep.as_pin_mut().expect("sleep must exist while armed");
                    ready!(sleep.as_mut().poll(cx));
                    // Continue on a fixed schedule.
                    sleep.reset_next_period(config.period);
                } else {
                    // The first tick or no throttling.
                    restart_interval(sleep, sleep_armed, config.period);
                }
                Poll::Ready(Some(()))
            }
        }
    }
}

impl<T, S: Sleep> Throttler<T> for IntervalThrottler<S> {
    fn throttle_pending(self: Pin<&mut Self>, _cx: &mut Context<'_>) {
        let IntervalThrottlerProjection {
            config,
            sleep,
            sleep_armed,
            state,
        } = self.project();
        if matches!(state, IntervalThrottlerState::Pending) {
            return;
        }
        *state = IntervalThrottlerState::Pending;
        match config.edge {
            // The first tick is ready immediately.
            IntervalEdge::Leading => *sleep_armed = false,
            IntervalEdge::Trailing => restart_interval(sleep, sleep_armed, config.period),
        }
    }

    fn throttle_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>, next_item: Option<&T>) {
        let state = self.project().state;
        assert!(matches!(state, IntervalThrottlerState::Pending));
        if next_item.is_none() {
            *state = IntervalThrottlerState::Idle;
        }
    }
}
