// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Shared test utilities with a paused Tokio clock.

use std::time::Duration;

use futures::{Stream, StreamExt as _, stream};
use tokio::time::{Instant, sleep_until};

pub(super) const TIME_TICK: Duration = Duration::from_millis(1);

/// Infinite stream of items `0, 1, 2, ...` with alternating delays.
///
/// The first item is ready immediately. The delay before odd items is
/// `first_delay` and the delay before even items is `second_delay`.
#[expect(clippy::cast_possible_truncation)]
pub(super) fn alternating_delay_stream(
    started_at: Instant,
    first_delay: Duration,
    second_delay: Duration,
) -> impl Stream<Item = usize> {
    stream::iter(0..).then(move |i| async move {
        sleep_until(
            started_at
                + first_delay.saturating_mul((i + 1) as u32 / 2)
                + second_delay.saturating_mul(i as u32 / 2),
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
pub(super) async fn collect_with_millis<S: Stream>(
    started_at: Instant,
    stream: S,
) -> Vec<(u128, S::Item)> {
    stream
        .map(move |item| ((Instant::now() - started_at).as_millis(), item))
        .collect()
        .await
}
