// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{num::NonZeroUsize, pin::pin, time::Duration};

use futures_util::{Stream, StreamExt as _, stream};
use tokio::time::{Instant, sleep};

use super::test_util::{
    DefaultResetSleep, TIME_TICK, alternating_delay_stream, collect_with_millis,
};
use crate::{IntervalEdge, Sleep, StreamExt as _, ThrottleIntervalConfig, tokio::StreamExt as _};

/// Throttles an alternating delay stream.
///
/// Runs both with [`tokio::time::Sleep`] and [`DefaultResetSleep`]. Their
/// results only match if the throttled stream is polled at each deadline,
/// i.e. if the consumer is never late. Otherwise the fixed schedule of
/// [`tokio::time::Sleep`] and the drifting schedule of [`DefaultResetSleep`]
/// diverge.
async fn throttle_alternating_delay_stream(
    config: ThrottleIntervalConfig,
    odd_item_delay: Duration,
    even_item_delay: Duration,
    num_items: usize,
) -> Vec<(u128, usize)> {
    let collected = throttle_alternating_delay_stream_with_sleep::<tokio::time::Sleep>(
        config,
        odd_item_delay,
        even_item_delay,
        num_items,
    )
    .await;
    assert_eq!(
        collected,
        throttle_alternating_delay_stream_with_sleep::<DefaultResetSleep>(
            config,
            odd_item_delay,
            even_item_delay,
            num_items,
        )
        .await
    );
    collected
}

async fn throttle_alternating_delay_stream_with_sleep<S: Sleep>(
    config: ThrottleIntervalConfig,
    odd_item_delay: Duration,
    even_item_delay: Duration,
    num_items: usize,
) -> Vec<(u128, usize)> {
    let started_at = Instant::now();
    let throttled = alternating_delay_stream(started_at, odd_item_delay, even_item_delay)
        .throttle_interval_with_sleep::<S>(config, NonZeroUsize::MIN)
        .take(num_items);
    collect_with_millis(started_at, throttled).await
}

#[tokio::test(start_paused = true)]
async fn should_pass_through_an_input_stream_that_is_always_ready_with_an_empty_period() {
    let odd_item_delay = Duration::ZERO;
    let even_item_delay = Duration::ZERO;
    let period = Duration::ZERO;
    let expected_items = [
        (0, 0),
        (0, 1),
        (0, 2),
        (0, 3),
        (0, 4),
        (0, 5),
        (0, 6),
        (0, 7),
        (0, 8),
        (0, 9),
    ];
    for edge in [IntervalEdge::Leading, IntervalEdge::Trailing] {
        let config = ThrottleIntervalConfig { period, edge };
        let collected_items = throttle_alternating_delay_stream(
            config,
            odd_item_delay,
            even_item_delay,
            expected_items.len(),
        )
        .await;
        assert_eq!(expected_items, collected_items.as_slice());
    }
}

#[tokio::test(start_paused = true)]
async fn should_pass_through_the_input_stream_if_the_period_is_shorter_than_the_arrival_rate() {
    let odd_item_delay = TIME_TICK.saturating_mul(10);
    let even_item_delay = TIME_TICK.saturating_mul(20);
    let period = TIME_TICK.saturating_mul(9);
    let expected_leading = [
        (0, 0),
        (10, 1),
        (30, 2),
        (40, 3),
        (60, 4),
        (70, 5),
        (90, 6),
        (100, 7),
        (120, 8),
        (130, 9),
    ];
    let expected_trailing = [
        (9, 0),
        (18, 1),
        (39, 2),
        (48, 3),
        (69, 4),
        (78, 5),
        (99, 6),
        (108, 7),
        (129, 8),
        (138, 9),
    ];
    for (edge, expected_items) in [
        (IntervalEdge::Leading, expected_leading.as_slice()),
        (IntervalEdge::Trailing, expected_trailing.as_slice()),
    ] {
        let config = ThrottleIntervalConfig { period, edge };
        let collected_items = throttle_alternating_delay_stream(
            config,
            odd_item_delay,
            even_item_delay,
            expected_items.len(),
        )
        .await;
        assert_eq!(expected_items, collected_items.as_slice());
    }
}

#[tokio::test(start_paused = true)]
async fn should_throttle_on_the_leading_edge() {
    // ms:   0 | 20 | 30 | 50 | 60 | 80 | 90 | 110 | 120 | 140 | 150 | 170 | 180 | 200 | 210 | ...
    // item: 0 |  1 |  2 |  3 |  4 |  5 |  6 |   7 |   8 |   9 |  10 |  11 |  12 |  13 |  14 | ...
    let odd_item_delay = TIME_TICK.saturating_mul(20);
    let even_item_delay = TIME_TICK.saturating_mul(10);
    let config = ThrottleIntervalConfig {
        period: TIME_TICK.saturating_mul(19),
        edge: IntervalEdge::Leading,
    };
    // ms:   0 | 19 | 20 | 39 | 58 | 77 | 96 | 115 | 134 | 153 | 172 | 191 | 210 | ...
    // item: 0 |  - |  1 |  2 |  3 |  4 |  6 |   7 |   8 |  10 |  11 |  12 |  14 | ...
    let expected_items = &[
        (0, 0),
        (20, 1),
        (39, 2),
        (58, 3),
        (77, 4),
        (96, 6),
        (115, 7),
        (134, 8),
        (153, 10),
        (172, 11),
        (191, 12),
        (210, 14),
    ];
    let collected_items = throttle_alternating_delay_stream(
        config,
        odd_item_delay,
        even_item_delay,
        expected_items.len(),
    )
    .await;
    assert_eq!(expected_items, collected_items.as_slice());
}

#[tokio::test(start_paused = true)]
async fn should_throttle_on_the_trailing_edge() {
    // ms:   0 | 10 | 30 | 40 | 60 | 70 | 90 | 100 | 120 | 130 | 150 | 160 | 180 | 190 | 210 | 220 | ...
    // item: 0 |  1 |  2 |  3 |  4 |  5 |  6 |   7 |   8 |   9 |  10 |  11 |  12 |  13 |  14 |  15 | ...
    let odd_item_delay = TIME_TICK.saturating_mul(10);
    let even_item_delay = TIME_TICK.saturating_mul(20);
    let config = ThrottleIntervalConfig {
        period: TIME_TICK.saturating_mul(19),
        edge: IntervalEdge::Trailing,
    };
    // ms:   0 | 19 | 38 | 57 | 76 | 95 | 114 | 133 | 152 | 171 | 190 | 209 | 210 | 229 | ...
    // item: * |  1 |  2 |  3 |  5 |  6 |   7 |   9 |  10 |  11 |  13 |   - |   * |  15 | ...
    let expected_items = &[
        (19, 1),
        (38, 2),
        (57, 3),
        (76, 5),
        (95, 6),
        (114, 7),
        (133, 9),
        (152, 10),
        (171, 11),
        (190, 13),
        (229, 15),
    ];
    let collected_items = throttle_alternating_delay_stream(
        config,
        odd_item_delay,
        even_item_delay,
        expected_items.len(),
    )
    .await;
    assert_eq!(expected_items, collected_items.as_slice());
}

#[tokio::test(start_paused = true)]
async fn should_throttle_on_the_trailing_edge_with_long_pauses() {
    // ms:   0 | 10 | 110 | 120 | 220 | 230 | 330 | 340 | ...
    // item: 0 |  1 |   2 |   3 |   4 |   5 |   6 |   7 | ...
    let odd_item_delay = TIME_TICK.saturating_mul(10);
    let even_item_delay = TIME_TICK.saturating_mul(100);
    let config = ThrottleIntervalConfig {
        period: TIME_TICK.saturating_mul(19),
        edge: IntervalEdge::Trailing,
    };
    let expected_items = &[(19, 1), (129, 3), (239, 5), (349, 7)];
    let collected_items = throttle_alternating_delay_stream(
        config,
        odd_item_delay,
        even_item_delay,
        expected_items.len(),
    )
    .await;
    assert_eq!(expected_items, collected_items.as_slice());
}

/// Configs with zero and non-zero periods for both edges.
fn finish_test_configs() -> impl Iterator<Item = ThrottleIntervalConfig> {
    [Duration::ZERO, TIME_TICK, TIME_TICK.saturating_mul(2)]
        .into_iter()
        .flat_map(|period| {
            [IntervalEdge::Leading, IntervalEdge::Trailing]
                .map(|edge| ThrottleIntervalConfig { period, edge })
        })
}

/// Throttles the input stream and collects all items.
async fn collect_throttled_with_sleep<S: Sleep, St: Stream>(
    stream: St,
    config: ThrottleIntervalConfig,
) -> Vec<St::Item> {
    stream
        .throttle_interval_with_sleep::<S>(config, NonZeroUsize::MIN)
        .collect()
        .await
}

#[tokio::test]
async fn should_finish_on_empty_input_stream() {
    for config in finish_test_configs() {
        assert_eq!(
            Vec::<()>::new(),
            collect_throttled_with_sleep::<tokio::time::Sleep, _>(stream::empty::<()>(), config)
                .await
        );
        assert_eq!(
            Vec::<()>::new(),
            collect_throttled_with_sleep::<DefaultResetSleep, _>(stream::empty::<()>(), config)
                .await
        );
    }
}

#[tokio::test]
async fn should_finish_after_non_empty_input_stream_has_completed() {
    for config in finish_test_configs() {
        assert_eq!(
            vec![()],
            collect_throttled_with_sleep::<tokio::time::Sleep, _>(stream::once(async {}), config)
                .await
        );
        assert_eq!(
            vec![()],
            collect_throttled_with_sleep::<DefaultResetSleep, _>(stream::once(async {}), config)
                .await
        );
    }
}

async fn drain_always_ready_input_stream_with_sleep<S: Sleep>() -> Vec<(u128, usize)> {
    let config = ThrottleIntervalConfig {
        period: TIME_TICK.saturating_mul(10),
        edge: IntervalEdge::Trailing,
    };
    let started_at = Instant::now();
    let throttled = stream::iter(0..5).throttle_interval_with_sleep::<S>(config, NonZeroUsize::MIN);
    collect_with_millis(started_at, throttled).await
}

#[tokio::test(start_paused = true)]
async fn should_drain_an_always_ready_input_stream_between_ticks() {
    // All items are ready immediately and only the last one is yielded
    // after the first interval has elapsed.
    let expected_items = vec![(10, 4)];
    assert_eq!(
        expected_items,
        drain_always_ready_input_stream_with_sleep::<tokio::time::Sleep>().await
    );
    assert_eq!(
        expected_items,
        drain_always_ready_input_stream_with_sleep::<DefaultResetSleep>().await
    );
}

/// Only for [`tokio::time::Sleep`], which keeps a fixed schedule.
/// [`DefaultResetSleep`] is allowed to drift when polled late.
#[tokio::test(start_paused = true)]
async fn should_keep_the_fixed_schedule_and_skip_missed_periods_if_polled_late() {
    // A new item arrives every millisecond.
    let started_at = Instant::now();
    let config = ThrottleIntervalConfig {
        period: TIME_TICK.saturating_mul(10),
        edge: IntervalEdge::Trailing,
    };
    let mut throttled = pin!(
        alternating_delay_stream(started_at, TIME_TICK, TIME_TICK)
            .throttle_interval(config, NonZeroUsize::MIN)
    );
    let mut tick_millis = Vec::new();
    while tick_millis.len() < 4 {
        throttled.next().await.unwrap();
        tick_millis.push(started_at.elapsed().as_millis());
        if tick_millis.len() == 1 {
            // Miss the deadline at 20 ms by 15 ms.
            sleep(TIME_TICK.saturating_mul(25)).await;
        }
    }
    // The late tick at 35 ms is followed by the next deadline on the
    // original schedule at 40 ms, not 10 ms after the late poll at 45 ms.
    assert_eq!(vec![10, 35, 40, 50], tick_millis);
}
