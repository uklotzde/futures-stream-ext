// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{
    Stream, StreamExt as _,
    stream::{Fuse, FusedStream as _},
};
use pin_project_lite::pin_project;

use crate::{Sleep, pending_and_wake_if, poll_latest_ready, reset_or_sleep};

pin_project! {
    /// Result of [`StreamExt::debounce_with_sleep()`](crate::StreamExt::debounce_with_sleep).
    #[derive(Debug)]
    #[project = DebouncedProjected]
    pub struct Debounced<St: Stream, S> {
        #[pin]
        stream: Fuse<St>,

        delay: Duration,

        poll_next_max_ready_count: NonZeroUsize,

        pending_item: Option<St::Item>,

        // Created on demand and reused afterwards.
        // Always `Some` while `pending_item` is `Some`.
        #[pin]
        sleep: Option<S>,
    }
}

impl<St: Stream, S: Sleep> Debounced<St, S> {
    pub(crate) fn new(
        stream: St,
        delay: Duration,
        poll_next_max_ready_count: NonZeroUsize,
    ) -> Self {
        Self {
            stream: stream.fuse(),
            delay,
            poll_next_max_ready_count,
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
            poll_next_max_ready_count,
            pending_item,
            mut sleep,
        } = self.project();

        let (latest_item, stream_maybe_ready) =
            poll_latest_ready(stream.as_mut(), cx, *poll_next_max_ready_count);

        // Replace the pending item and restart the delay.
        if let Some(latest_item) = latest_item {
            *pending_item = Some(latest_item);
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
        if sleep.poll(cx).is_ready() {
            return Poll::Ready(pending_item.take());
        }
        pending_and_wake_if(cx, stream_maybe_ready)
    }
}
