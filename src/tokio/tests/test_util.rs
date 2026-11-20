// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Shared test utilities with a paused Tokio clock.

use std::{
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt as _, stream};
use tokio::time::{Instant, sleep_until};

use crate::Sleep;

pub(super) const TIME_TICK: Duration = Duration::from_millis(1);

/// A [`Sleep`] that only implements [`Sleep::sleep()`].
///
/// Exercises the default implementations of [`Sleep::reset()`] and
/// [`Sleep::reset_next_period()`].
#[derive(Debug)]
pub(super) struct DefaultResetSleep(Pin<Box<tokio::time::Sleep>>);

impl Future for DefaultResetSleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.0.as_mut().poll(cx)
    }
}

impl Sleep for DefaultResetSleep {
    fn sleep(duration: Duration) -> Self {
        Self(Box::pin(tokio::time::sleep(duration)))
    }
}

/// Infinite stream of items `0, 1, 2, ...` with alternating delays.
///
/// The first item is ready immediately. The delay before odd items is
/// `odd_item_delay` and the delay before even items is `even_item_delay`.
#[expect(clippy::cast_possible_truncation)]
pub(super) fn alternating_delay_stream(
    started_at: Instant,
    odd_item_delay: Duration,
    even_item_delay: Duration,
) -> impl Stream<Item = usize> {
    stream::iter(0..).then(move |i| async move {
        sleep_until(
            started_at
                + odd_item_delay.saturating_mul((i + 1) as u32 / 2)
                + even_item_delay.saturating_mul(i as u32 / 2),
        )
        .await;
        i
    })
}

/// Finite stream of items that arrive at the given points in time
/// (milliseconds since `started_at`).
pub(super) fn scheduled_stream(
    started_at: Instant,
    schedule: impl IntoIterator<Item = (u64, usize)>,
) -> impl Stream<Item = usize> {
    stream::iter(schedule).then(move |(arrival_millis, item)| async move {
        sleep_until(started_at + Duration::from_millis(arrival_millis)).await;
        item
    })
}

/// Collects all items with their emission time (milliseconds since `started_at`).
pub(super) async fn collect_with_millis<St: Stream>(
    started_at: Instant,
    stream: St,
) -> Vec<(u128, St::Item)> {
    stream
        .map(move |item| (started_at.elapsed().as_millis(), item))
        .collect()
        .await
}
