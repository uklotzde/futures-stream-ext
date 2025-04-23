// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    pin::Pin,
    task::{Context, Poll, ready},
    time::Duration,
};

use futures_core::Stream;
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
    #[cfg_attr(docsrs, doc(cfg(feature = "tokio")))]
    #[project = DebouncedProjected]
    pub struct Debounced<St: Stream, S: Sleep> {
        #[pin]
        stream: Option<St>,

        delay: Duration,

        #[pin]
        pending: Option<Delayed<St::Item, S>>,
    }
}

impl<St: Stream, S: Sleep> Debounced<St, S> {
    pub(crate) const fn new(stream: St, delay: Duration) -> Self {
        Self {
            stream: Some(stream),
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

        if let Some(mut poll_stream) = stream.as_mut().as_pin_mut() {
            let mut last_item = None;

            while let Poll::Ready(next_item) = poll_stream.as_mut().poll_next(cx) {
                if let Some(next_item) = next_item {
                    last_item = Some(next_item);
                    // Continue polling the stream while ready.
                    continue;
                }
                // Stream has finished and must not be polled again.
                stream.set(None);
                if last_item.is_none() && pending.is_none() {
                    // Finished if no item is pending.
                    return Poll::Ready(None);
                }
                break;
            }

            // Replace pending with delayed last item from the stream.
            if let Some(last_item) = last_item {
                let next_pending = Delayed::new(last_item, *delay);
                // The currently pending future is canceled and dropped by overwriting it.
                pending.set(Some(next_pending));
            }
        }

        let Some(poll_pending) = pending.as_mut().as_pin_mut() else {
            // No pending item.
            return Poll::Pending;
        };
        let item = ready!(poll_pending.poll(cx));
        // The future must not be polled again after it became ready.
        pending.set(None);
        Poll::Ready(Some(item))
    }
}
