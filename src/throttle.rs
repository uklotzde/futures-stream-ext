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
    /// A new item has been received from the input stream while idle.
    ///
    /// The throttler is idle initially and after `on_tick` has been
    /// called with `None`. This method is not called again until then.
    ///
    /// After invocation of this method `on_tick` will be called
    /// with `Some` when the current interval has elapsed. The current
    /// item that will be yielded may still change until `on_tick`
    /// is called.
    ///
    /// The `cx` argument is only provided for consistency and can safely
    /// be ignored in most cases. The throttler will be polled immediately
    /// after this method returns.
    fn on_first_item(self: Pin<&mut Self>, cx: &mut Context<'_>);

    /// The current interval has elapsed.
    ///
    /// Called after each tick before the throttler is polled again. The
    /// throttler should start the next interval only if `yielded_item` is `Some`.
    ///
    /// Provides the pending item of the throttled input stream that is
    /// about to be yielded or `None` if no item has been received from the
    /// input stream during the last interval.
    fn on_tick(self: Pin<&mut Self>, cx: &mut Context<'_>, yielded_item: Option<&T>);
}

pin_project! {
    /// Throttled stream
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct Throttled<St: Stream, Th: Throttler<<St as Stream>::Item>> {
        #[pin]
        stream: Fuse<St>,
        #[pin]
        throttler: Th,
        poll_next_max_ready_count: NonZeroUsize,
        pending_item: Option<St::Item>,
        // Set until `on_first_item` has been called and
        // again after `on_tick` has been called with `None`.
        throttler_idle: bool,
    }
}

impl<St, Th> Throttled<St, Th>
where
    St: Stream,
    Th: Throttler<<St as Stream>::Item>,
{
    pub fn new(stream: St, throttler: Th, poll_next_max_ready_count: NonZeroUsize) -> Self {
        Self {
            stream: stream.fuse(),
            throttler,
            poll_next_max_ready_count,
            pending_item: None,
            throttler_idle: true,
        }
    }
}

impl<St, Th> Stream for Throttled<St, Th>
where
    St: Stream,
    Th: Throttler<<St as Stream>::Item>,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        // We want to receive the most recent item that is ready.
        let (latest_item, stream_maybe_ready) =
            poll_latest_ready(this.stream.as_mut(), cx, *this.poll_next_max_ready_count);
        if let Some(latest_item) = latest_item {
            if *this.throttler_idle {
                *this.throttler_idle = false;
                this.throttler.as_mut().on_first_item(cx);
            }
            *this.pending_item = Some(latest_item);
        }

        if this.stream.is_terminated() {
            if this.pending_item.is_none() {
                // Stream has finished.
                return Poll::Ready(None);
            }
            // Yield the last pending item when the throttler is ready.
            ready!(this.throttler.as_mut().poll_next(cx));
            return Poll::Ready(this.pending_item.take());
        }

        if this.throttler.as_mut().poll_next(cx).is_ready() {
            let yielded_item = this.pending_item.take();
            this.throttler.as_mut().on_tick(cx, yielded_item.as_ref());
            if yielded_item.is_none() {
                *this.throttler_idle = true;
            } else {
                return Poll::Ready(yielded_item);
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
    /// No items are pending.
    Idle,
    /// Items are pending and the next tick is ready immediately.
    TickReady,
    /// Items are pending and the next tick is ready when `sleep` has elapsed.
    Sleeping,
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
        state: IntervalThrottlerState,
    }
}

impl<S: Sleep> IntervalThrottler<S> {
    #[must_use]
    pub const fn new(config: ThrottleIntervalConfig) -> Self {
        Self {
            config,
            sleep: None,
            state: IntervalThrottlerState::Idle,
        }
    }
}

/// Starts the first or the next interval.
fn start_interval<S: Sleep>(
    sleep: Pin<&mut Option<S>>,
    period: Duration,
) -> IntervalThrottlerState {
    if period.is_zero() {
        // No throttling.
        return IntervalThrottlerState::TickReady;
    }
    reset_or_sleep(sleep, period);
    IntervalThrottlerState::Sleeping
}

impl<S: Sleep> Stream for IntervalThrottler<S> {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let IntervalThrottlerProjection { sleep, state, .. } = self.project();
        match state {
            IntervalThrottlerState::Idle => return Poll::Pending,
            // The first tick or no throttling.
            IntervalThrottlerState::TickReady => (),
            IntervalThrottlerState::Sleeping => {
                let sleep = sleep
                    .as_pin_mut()
                    .expect("sleep must exist while waiting for it");
                ready!(sleep.poll(cx));
            }
        }
        // The next interval is started by `on_tick()`.
        Poll::Ready(Some(()))
    }
}

impl<T, S: Sleep> Throttler<T> for IntervalThrottler<S> {
    fn on_first_item(self: Pin<&mut Self>, _cx: &mut Context<'_>) {
        let IntervalThrottlerProjection {
            config,
            sleep,
            state,
        } = self.project();
        debug_assert!(matches!(state, IntervalThrottlerState::Idle));
        *state = match config.edge {
            // The first tick is ready immediately.
            IntervalEdge::Leading => IntervalThrottlerState::TickReady,
            IntervalEdge::Trailing => start_interval(sleep, config.period),
        };
    }

    fn on_tick(self: Pin<&mut Self>, _cx: &mut Context<'_>, yielded_item: Option<&T>) {
        let IntervalThrottlerProjection {
            config,
            sleep,
            state,
        } = self.project();
        if yielded_item.is_none() {
            // Leave the elapsed sleep unarmed until items are pending again.
            *state = IntervalThrottlerState::Idle;
            return;
        }
        match state {
            IntervalThrottlerState::Idle => panic!("on_tick called while idle"),
            IntervalThrottlerState::TickReady => *state = start_interval(sleep, config.period),
            IntervalThrottlerState::Sleeping => {
                // Continue on a fixed schedule.
                sleep
                    .as_pin_mut()
                    .expect("sleep must exist while waiting for it")
                    .reset_next_period(config.period);
            }
        }
    }
}
