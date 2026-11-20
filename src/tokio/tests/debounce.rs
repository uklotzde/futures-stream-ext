// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{num::NonZeroUsize, time::Duration};

use futures_util::StreamExt as _;
use tokio::time::Instant;

use super::test_util::{
    DefaultResetSleep, alternating_delay_stream, collect_with_millis, ms, scheduled_stream,
};
use crate::{Sleep, StreamExt as _};

/// Debounces a periodic input stream with the given sleep.
async fn debounce_periodic_with_sleep<S: Sleep>(
    debounce_delay: Duration,
    item_period: Duration,
    num_items: usize,
) -> Vec<(u128, usize)> {
    let started_at = Instant::now();
    let debounced = alternating_delay_stream(started_at, item_period, item_period)
        .debounce_with_sleep::<S>(debounce_delay, NonZeroUsize::MIN)
        .take(num_items);
    collect_with_millis(started_at, debounced).await
}

#[tokio::test(start_paused = true)]
async fn should_debounce_a_periodic_input_stream() {
    let debounce_delay = ms(10);
    let item_period = ms(17);
    let expected = [
        (10, 0),
        (27, 1),
        (44, 2),
        (61, 3),
        (78, 4),
        (95, 5),
        (112, 6),
        (129, 7),
        (146, 8),
        (163, 9),
    ];
    assert_eq!(
        expected.as_slice(),
        debounce_periodic_with_sleep::<tokio::time::Sleep>(
            debounce_delay,
            item_period,
            expected.len()
        )
        .await
    );
    assert_eq!(
        expected.as_slice(),
        debounce_periodic_with_sleep::<DefaultResetSleep>(
            debounce_delay,
            item_period,
            expected.len()
        )
        .await
    );
}

/// Debounces a finite stream with items that arrive at the given
/// points in time (milliseconds since start).
///
/// Returns the emitted items with their emission time.
async fn debounce_schedule(
    debounce_delay: Duration,
    schedule: impl IntoIterator<Item = (u64, usize)> + Clone,
) -> Vec<(u128, usize)> {
    debounce_schedule_with_max_ready_count(debounce_delay, schedule, NonZeroUsize::MIN).await
}

/// Like [`debounce_schedule()`], but with a custom `poll_next_max_ready_count`.
///
/// Runs both with [`tokio::time::Sleep`] and [`DefaultResetSleep`], which
/// must produce the same results. Debouncing always restarts the delay from
/// now, so both sleeps behave identically regardless of when they are polled.
async fn debounce_schedule_with_max_ready_count(
    debounce_delay: Duration,
    schedule: impl IntoIterator<Item = (u64, usize)> + Clone,
    poll_next_max_ready_count: NonZeroUsize,
) -> Vec<(u128, usize)> {
    let collected = debounce_schedule_with_sleep::<tokio::time::Sleep>(
        debounce_delay,
        schedule.clone(),
        poll_next_max_ready_count,
    )
    .await;
    assert_eq!(
        collected,
        debounce_schedule_with_sleep::<DefaultResetSleep>(
            debounce_delay,
            schedule,
            poll_next_max_ready_count,
        )
        .await
    );
    collected
}

async fn debounce_schedule_with_sleep<S: Sleep>(
    debounce_delay: Duration,
    schedule: impl IntoIterator<Item = (u64, usize)>,
    poll_next_max_ready_count: NonZeroUsize,
) -> Vec<(u128, usize)> {
    let started_at = Instant::now();
    let debounced = scheduled_stream(started_at, schedule)
        .debounce_with_sleep::<S>(debounce_delay, poll_next_max_ready_count);
    collect_with_millis(started_at, debounced).await
}

#[tokio::test(start_paused = true)]
async fn should_finish_on_empty_input_stream() {
    let collected = debounce_schedule(ms(10), []).await;
    assert_eq!(Vec::<(u128, usize)>::new(), collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_single_item_after_delay() {
    let collected = debounce_schedule(ms(10), [(5, 0)]).await;
    assert_eq!(vec![(15, 0)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_only_emit_the_last_item_of_a_burst() {
    // All items arrive within the delay window of their predecessor.
    // The delay restarts with each item.
    let collected = debounce_schedule(ms(10), [(0, 0), (5, 1), (10, 2), (19, 3)]).await;
    assert_eq!(vec![(29, 3)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_only_emit_the_last_item_of_each_burst() {
    let collected = debounce_schedule(
        ms(10),
        [(0, 0), (5, 1), (30, 2), (35, 3), (39, 4), (100, 5)],
    )
    .await;
    assert_eq!(vec![(15, 1), (49, 4), (110, 5)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_all_items_that_arrive_slower_than_the_delay() {
    let collected = debounce_schedule(ms(10), [(0, 0), (11, 1), (30, 2)]).await;
    assert_eq!(vec![(10, 0), (21, 1), (40, 2)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_discard_items_that_are_ready_at_the_same_time() {
    let collected = debounce_schedule(ms(10), [(0, 0), (0, 1), (0, 2), (20, 3), (20, 4)]).await;
    assert_eq!(vec![(10, 2), (30, 4)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_the_pending_item_after_the_input_stream_has_finished() {
    // The input stream finishes immediately after the last item
    // while it is still pending.
    let collected = debounce_schedule(ms(10), [(0, 0), (3, 1)]).await;
    assert_eq!(vec![(13, 1)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_items_immediately_with_zero_delay() {
    let schedule = [(0, 0), (5, 1), (5, 2), (7, 3)];
    // Items that are ready at the same time are still debounced
    // if they are polled together.
    assert_eq!(
        vec![(0, 0), (5, 2), (7, 3)],
        debounce_schedule_with_max_ready_count(Duration::ZERO, schedule, NonZeroUsize::MAX).await
    );
    // Otherwise each of them is emitted.
    assert_eq!(
        vec![(0, 0), (5, 1), (5, 2), (7, 3)],
        debounce_schedule_with_max_ready_count(Duration::ZERO, schedule, NonZeroUsize::MIN).await
    );
}
