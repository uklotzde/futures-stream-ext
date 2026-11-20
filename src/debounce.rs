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

use crate::{Sleep, reset_or_sleep};

pin_project! {
    /// Result of [`StreamExt::debounce_with_sleep()`](crate::StreamExt::debounce_with_sleep).
    #[derive(Debug)]
    #[project = DebouncedProjected]
    pub struct Debounced<St: Stream, S> {
        #[pin]
        stream: Fuse<St>,

        delay: Duration,

        pending_item: Option<St::Item>,

        // Created on demand and reused afterwards.
        // Always `Some` while `pending_item` is `Some`.
        #[pin]
        sleep: Option<S>,
    }
}

impl<St: Stream, S: Sleep> Debounced<St, S> {
    pub(crate) fn new(stream: St, delay: Duration) -> Self {
        Self {
            stream: stream.fuse(),
            delay,
            pending_item: None,
            sleep: None,
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
            mut stream,
            delay,
            pending_item,
            mut sleep,
        } = self.project();

        // Continue polling the stream while ready. The fused stream
        // is never polled again after it has finished.
        let mut last_item = None;
        while let Poll::Ready(Some(next_item)) = stream.as_mut().poll_next(cx) {
            last_item = Some(next_item);
        }

        // Replace the pending item and restart the delay.
        if let Some(last_item) = last_item {
            *pending_item = Some(last_item);
            reset_or_sleep(sleep.as_mut(), *delay);
        }

        if pending_item.is_none() {
            return if stream.is_terminated() {
                // Stream has finished.
                Poll::Ready(None)
            } else {
                // New stream items may arrive later.
                Poll::Pending
            };
        }

        let sleep = sleep
            .as_pin_mut()
            .expect("sleep must exist while an item is pending");
        ready!(sleep.poll(cx));
        Poll::Ready(pending_item.take())
    }
}
