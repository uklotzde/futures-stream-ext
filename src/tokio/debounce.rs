// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::time::Duration;

use futures::{Stream, StreamExt as _, stream};
use tokio::{
    runtime,
    time::{self, Instant, sleep_until},
};

use crate::tokio::StreamExt as _;

const TIME_TICK: Duration = Duration::from_millis(1);

#[expect(clippy::cast_possible_truncation)]
fn periodic_stream(started_at: Instant, period: Duration) -> impl Stream<Item = usize> {
    stream::iter(0..).filter(move |&i| async move {
        // The first items is yielded immediately, all subsequent items are delayed
        // by a fixed amount between each other.
        sleep_until(started_at + period.saturating_mul(i as u32)).await;
        true
    })
}

fn run_periodic_stream_debounced(
    debounce_delay: Duration,
    item_period: Duration,
    num_items: usize,
) -> Vec<(u128, usize)> {
    let rt = runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()
        .unwrap();
    let rt_handle = rt.handle();
    rt.block_on(async move {
        let started_at = Instant::now();
        let join_handle = rt_handle.spawn(
            periodic_stream(started_at, item_period)
                .debounce(debounce_delay)
                .map(move |item| ((Instant::now() - started_at).as_millis(), item))
                .take(num_items)
                .collect::<Vec<_>>(),
        );
        rt_handle.spawn(async move {
            time::advance(TIME_TICK).await;
        });
        join_handle.await.unwrap()
    })
}

#[test]
fn debounce() {
    let debounce_delay = TIME_TICK.saturating_mul(10);
    let item_period = TIME_TICK.saturating_mul(17);
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
        &run_periodic_stream_debounced(debounce_delay, item_period, expected.len()),
        &expected
    );
}

/// Debounces a finite stream with items that arrive at the given
/// points in time (milliseconds since start).
///
/// Returns the emitted items with their emission time.
async fn debounce_schedule(
    debounce_delay: Duration,
    schedule: impl IntoIterator<Item = (u64, usize)>,
) -> Vec<(u128, usize)> {
    let started_at = Instant::now();
    stream::iter(schedule)
        .then(move |(arrival_millis, item)| async move {
            sleep_until(started_at + Duration::from_millis(arrival_millis)).await;
            item
        })
        .debounce(debounce_delay)
        .map(move |item| ((Instant::now() - started_at).as_millis(), item))
        .collect()
        .await
}

#[tokio::test(start_paused = true)]
async fn should_finish_on_empty_input_stream() {
    let collected = debounce_schedule(TIME_TICK.saturating_mul(10), []).await;
    assert_eq!(Vec::<(u128, usize)>::new(), collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_single_item_after_delay() {
    let collected = debounce_schedule(TIME_TICK.saturating_mul(10), [(5, 0)]).await;
    assert_eq!(vec![(15, 0)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_only_emit_the_last_item_of_a_burst() {
    // All items arrive within the delay window of their predecessor.
    // The delay restarts with each item.
    let collected = debounce_schedule(
        TIME_TICK.saturating_mul(10),
        [(0, 0), (5, 1), (10, 2), (19, 3)],
    )
    .await;
    assert_eq!(vec![(29, 3)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_only_emit_the_last_item_of_each_burst() {
    let collected = debounce_schedule(
        TIME_TICK.saturating_mul(10),
        [(0, 0), (5, 1), (30, 2), (35, 3), (39, 4), (100, 5)],
    )
    .await;
    assert_eq!(vec![(15, 1), (49, 4), (110, 5)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_all_items_that_arrive_slower_than_the_delay() {
    let collected =
        debounce_schedule(TIME_TICK.saturating_mul(10), [(0, 0), (11, 1), (30, 2)]).await;
    assert_eq!(vec![(10, 0), (21, 1), (40, 2)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_discard_items_that_are_ready_at_the_same_time() {
    let collected = debounce_schedule(
        TIME_TICK.saturating_mul(10),
        [(0, 0), (0, 1), (0, 2), (20, 3), (20, 4)],
    )
    .await;
    assert_eq!(vec![(10, 2), (30, 4)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_the_pending_item_after_the_input_stream_has_finished() {
    // The input stream finishes immediately after the last item
    // while it is still pending.
    let collected = debounce_schedule(TIME_TICK.saturating_mul(10), [(0, 0), (3, 1)]).await;
    assert_eq!(vec![(13, 1)], collected);
}

#[tokio::test(start_paused = true)]
async fn should_emit_items_immediately_with_zero_delay() {
    let collected = debounce_schedule(Duration::ZERO, [(0, 0), (5, 1), (5, 2), (7, 3)]).await;
    // Items that are ready at the same time are still debounced.
    assert_eq!(vec![(0, 0), (5, 2), (7, 3)], collected);
}
