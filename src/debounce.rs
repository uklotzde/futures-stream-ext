// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};

use futures_util::{
    Stream, StreamExt as _,
    stream::{Fuse, FusedStream as _},
};
use pin_project_lite::pin_project;

use crate::Sleep;

pin_project! {
    #[derive(Debug)]
    #[project = DelayedProjected]
    struct Delayed<T, S: Sleep> {
        output: Option<T>,

        #[pin]
        sleep: S,
    }
}

impl<T, S: Sleep> Delayed<T, S> {
    pub(crate) fn new(output: T, delay: Duration) -> Self {
        Self {
            output: Some(output),
            sleep: S::sleep(delay),
        }
    }
}

impl<T, S: Sleep> Future for Delayed<T, S> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let DelayedProjected { output, sleep } = self.project();
        ready!(sleep.poll(cx));
        let output = output
            .take()
            .expect("future must not be polled again after ready");
        Poll::Ready(output)
    }
}

pin_project! {
    /// Result of [`StreamExt::debounce()`](crate::StreamExt::debounce).
    #[derive(Debug)]
    #[project = DebouncedProjected]
    pub struct Debounced<St: Stream, S: Sleep> {
        #[pin]
        stream: Fuse<St>,

        delay: Duration,

        #[pin]
        pending: Option<Delayed<St::Item, S>>,
    }
}

impl<St: Stream, S: Sleep> Debounced<St, S> {
    pub(crate) fn new(stream: St, delay: Duration) -> Self {
        Self {
            stream: stream.fuse(),
            delay,
            pending: None,
        }
    }
}

impl<St, S> Stream for Debounced<St, S>
where
    St: Stream,
    S: Sleep,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let DebouncedProjected {
            delay,
            mut stream,
            mut pending,
        } = self.project();

        // Continue polling the stream while ready. The fused stream
        // is never polled again after it has finished.
        let mut last_item = None;
        while let Poll::Ready(Some(next_item)) = stream.as_mut().poll_next(cx) {
            last_item = Some(next_item);
        }

        // Replace pending with delayed last item from the stream.
        if let Some(last_item) = last_item {
            let next_pending = Delayed::new(last_item, *delay);
            // The currently pending future is canceled and dropped by overwriting it.
            pending.set(Some(next_pending));
        }

        let Some(poll_pending) = pending.as_mut().as_pin_mut() else {
            // No pending item.
            return if stream.is_terminated() {
                // Stream has finished.
                Poll::Ready(None)
            } else {
                // New stream items may arrive later.
                Poll::Pending
            };
        };

        let item = ready!(poll_pending.poll(cx));
        // The future must not be polled again after it became ready.
        pending.set(None);
        Poll::Ready(Some(item))
    }
}
