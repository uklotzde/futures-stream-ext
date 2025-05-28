// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};

use futures_util::stream::Stream;
use pin_project_lite::pin_project;

use crate::IntervalEdge;

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

/// Internal state.
#[derive(Debug, Clone, Copy)]
enum State {
    Streaming,
    Finishing,
    Finished,
}

pin_project! {
    /// Throttled stream
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct Throttled<S: Stream, T: Throttler<<S as Stream>::Item>> {
        #[pin]
        stream: S,
        #[pin]
        throttler: T,
        poll_next_max_ready_count: NonZeroUsize,
        state: State,
        pending: Option<S::Item>,
    }
}

impl<S, T> Throttled<S, T>
where
    S: Stream,
    T: Throttler<<S as Stream>::Item>,
{
    pub const fn new(stream: S, throttler: T, poll_next_max_ready_count: NonZeroUsize) -> Self {
        Self {
            stream,
            throttler,
            poll_next_max_ready_count,
            state: State::Streaming,
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

        if matches!(this.state, State::Streaming) {
            // Poll the inner stream while it yields items. We want to receive
            // the most recent item that is ready.
            let mut ready_count = 0;
            loop {
                match this.stream.as_mut().poll_next(cx) {
                    Poll::Ready(Some(item)) => {
                        if this.pending.is_none() {
                            this.throttler.as_mut().throttle_pending(cx);
                        }
                        *this.pending = Some(item);
                        debug_assert!(ready_count < this.poll_next_max_ready_count.get());
                        ready_count += 1;
                        if ready_count >= this.poll_next_max_ready_count.get() {
                            // Stop polling the inner stream to prevent endless loops
                            // for streams that are always ready.
                            // Wake ourselves up to ensure that polling the stream continues after
                            // after polling the throttler.
                            cx.waker().wake_by_ref();
                            break;
                        }
                    }
                    Poll::Ready(None) => {
                        *this.state = State::Finishing;
                        break;
                    }
                    Poll::Pending => {
                        break;
                    }
                }
            }
        }

        // Poll the throttler.
        match this.state {
            State::Streaming => {
                ready!(this.throttler.as_mut().poll_next(cx));
                let next_item = this.pending.take();
                this.throttler
                    .as_mut()
                    .throttle_ready(cx, next_item.as_ref());
                if next_item.is_some() {
                    Poll::Ready(next_item)
                } else {
                    Poll::Pending
                }
            }
            State::Finishing => {
                if this.pending.is_some() {
                    ready!(this.throttler.as_mut().poll_next(cx));
                    let last_item = this.pending.take();
                    // Wake ourselves up for the final state transition from `Finishing`
                    // to `Finished` that becomes ready immediately.
                    cx.waker().wake_by_ref();
                    Poll::Ready(last_item)
                } else {
                    // The final state transition.
                    *this.state = State::Finished;
                    Poll::Ready(None)
                }
            }
            State::Finished => panic!("stream polled after completion"),
        }
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
