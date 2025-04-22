// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_core::Stream;
use pin_project_lite::pin_project;
use tokio::time::Interval;

use crate::{IntervalEdge, ThrottleIntervalConfig, Throttler};

#[derive(Debug, Clone, Copy)]
enum State {
    Idle,
    Pending,
}

pin_project! {
    #[derive(Debug)]
    pub struct IntervalThrottler<T> {
        #[pin]
        interval: Option<Interval>,
        edge: IntervalEdge,
        state: State,
        _marker: PhantomData<T>,
    }
}

fn throttle_interval(period: Duration) -> Option<tokio::time::Interval> {
    if period.is_zero() {
        return None;
    }
    let mut interval = tokio::time::interval(period);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    Some(interval)
}

impl<T> IntervalThrottler<T> {
    #[must_use]
    #[expect(clippy::needless_pass_by_value)]
    pub(crate) fn new(config: ThrottleIntervalConfig) -> Self {
        let ThrottleIntervalConfig { period, edge } = config;
        let interval = throttle_interval(period);
        Self {
            interval,
            edge,
            state: State::Idle,
            _marker: PhantomData,
        }
    }
}

impl<T> Stream for IntervalThrottler<T> {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        match this.state {
            State::Idle => Poll::Pending,
            State::Pending => this
                .interval
                .as_pin_mut()
                .as_mut()
                .map_or(Poll::Ready(Some(())), |interval| {
                    interval.poll_tick(cx).map(|_| Some(()))
                }),
        }
    }
}

impl<T> Throttler<T> for IntervalThrottler<T> {
    fn throttle_pending(self: Pin<&mut Self>, _cx: &mut Context<'_>) {
        let this = self.project();
        match this.state {
            State::Idle => {
                *this.state = State::Pending;
                let Some(mut interval) = this.interval.as_pin_mut() else {
                    return;
                };
                match this.edge {
                    IntervalEdge::Leading => {
                        interval.reset_immediately();
                    }
                    IntervalEdge::Trailing => {
                        interval.reset();
                    }
                }
            }
            State::Pending => (),
        }
    }

    fn throttle_ready(self: Pin<&mut Self>, _cx: &mut Context<'_>, next_item: Option<&T>) {
        let this = self.project();
        match this.state {
            State::Idle => unreachable!(),
            State::Pending => {
                if next_item.is_none() {
                    *this.state = State::Idle;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, time::Duration};

    use futures::{Stream, StreamExt as _};
    use tokio::time::Instant;

    use crate::{IntervalEdge, StreamExt as _, ThrottleIntervalConfig};

    const TIME_TICK: Duration = Duration::from_millis(1);

    #[expect(clippy::cast_possible_truncation)]
    fn alternating_delay_stream(
        started_at: Instant,
        first_delay: Duration,
        second_delay: Duration,
    ) -> impl Stream<Item = usize> {
        futures::stream::iter(0..).filter(move |&i| async move {
            // The first items is yielded immediately, all subsequent items are delayed
            // by a fixed amount between each other.
            tokio::time::sleep_until(
                started_at
                    + first_delay.saturating_mul((i + 1) as u32 / 2)
                    + second_delay.saturating_mul(i as u32 / 2),
            )
            .await;
            true
        })
    }

    fn run_alternating_delay_stream(
        config: ThrottleIntervalConfig,
        first_delay: Duration,
        second_delay: Duration,
        num_items: usize,
    ) -> Vec<(u128, usize)> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()
            .unwrap();
        rt.block_on(async move {
            let started_at = tokio::time::Instant::now();
            let handle = tokio::spawn(
                alternating_delay_stream(started_at, first_delay, second_delay)
                    .throttle_interval(config, NonZeroUsize::MIN)
                    .map(move |item| ((Instant::now() - started_at).as_millis(), item))
                    .take(num_items)
                    .collect::<Vec<_>>(),
            );
            tokio::spawn(async move {
                tokio::time::advance(TIME_TICK).await;
            });
            handle.await.unwrap()
        })
    }

    #[test]
    fn should_pass_through_an_input_stream_that_is_always_ready_with_an_empty_period() {
        let first_delay = Duration::ZERO;
        let second_delay = Duration::ZERO;
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
        for config in [
            ThrottleIntervalConfig {
                period,
                edge: IntervalEdge::Leading,
            },
            ThrottleIntervalConfig {
                period,
                edge: IntervalEdge::Trailing,
            },
        ] {
            assert_eq!(
                &run_alternating_delay_stream(
                    config,
                    first_delay,
                    second_delay,
                    expected_items.len(),
                ),
                &expected_items
            );
        }
    }

    #[test]
    fn should_pass_through_the_input_stream_if_the_period_is_shorter_than_the_arrival_rate() {
        let first_delay = TIME_TICK.saturating_mul(10);
        let second_delay = TIME_TICK.saturating_mul(20);
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
        assert_eq!(
            &run_alternating_delay_stream(
                ThrottleIntervalConfig {
                    period,
                    edge: IntervalEdge::Leading,
                },
                first_delay,
                second_delay,
                expected_leading.len(),
            ),
            &expected_leading
        );
        assert_eq!(
            &run_alternating_delay_stream(
                ThrottleIntervalConfig {
                    period,
                    edge: IntervalEdge::Trailing,
                },
                first_delay,
                second_delay,
                expected_trailing.len(),
            ),
            &expected_trailing
        );
    }

    #[test]
    fn leading_edge_sequence() {
        // ms:   0 | 20 | 30 | 50 | 60 | 80 | 90 | 110 | 120 | 140 | 150 | 170 | 180 | 200 | 210 | ...
        // item: 0 |  1 |  2 |  3 |  4 |  5 |  6 |   7 |   8 |   9 |  10 |  11 |  12 |  13 |  14 | ...
        let first_delay = TIME_TICK.saturating_mul(20);
        let second_delay = TIME_TICK.saturating_mul(10);
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
        let collected_items =
            run_alternating_delay_stream(config, first_delay, second_delay, expected_items.len());
        assert_eq!(expected_items, collected_items.as_slice());
    }

    #[test]
    fn trailing_edge_sequence() {
        // ms:   0 | 10 | 30 | 40 | 60 | 70 | 90 | 100 | 120 | 130 | 150 | 160 | 180 | 190 | 210 | 220 | ...
        // item: 0 |  1 |  2 |  3 |  4 |  5 |  6 |   7 |   8 |   9 |  10 |  11 |  12 |  13 |  14 |  15 | ...
        let first_delay = TIME_TICK.saturating_mul(10);
        let second_delay = TIME_TICK.saturating_mul(20);
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
        let collected_items =
            run_alternating_delay_stream(config, first_delay, second_delay, expected_items.len());
        assert_eq!(expected_items, collected_items.as_slice());
    }

    #[test]
    fn trailing_edge_sequence_long_second_delay() {
        // ms:   0 | 10 | 110 | 120 | 220 | 230 | 330 | 340 | ...
        // item: 0 |  1 |   2 |   3 |   4 |   5 |   6 |   7 | ...
        let first_delay = TIME_TICK.saturating_mul(10);
        let second_delay = TIME_TICK.saturating_mul(100);
        let config = ThrottleIntervalConfig {
            period: TIME_TICK.saturating_mul(19),
            edge: IntervalEdge::Trailing,
        };
        let expected_items = &[(19, 1), (129, 3), (239, 5), (349, 7)];
        assert_eq!(
            &run_alternating_delay_stream(config, first_delay, second_delay, expected_items.len()),
            &expected_items
        );
    }

    #[tokio::test]
    async fn should_finish_on_empty_input_stream() {
        for period in [Duration::ZERO, TIME_TICK, TIME_TICK.saturating_mul(2)] {
            for edge in [IntervalEdge::Leading, IntervalEdge::Trailing] {
                let config = ThrottleIntervalConfig { period, edge };
                assert_eq!(
                    Vec::<()>::new(),
                    futures::stream::empty::<()>()
                        .throttle_interval(config, NonZeroUsize::MIN)
                        .collect::<Vec<_>>()
                        .await
                );
            }
        }
    }

    #[tokio::test]
    async fn should_finish_after_non_empty_input_stream_has_completed() {
        for period in [Duration::ZERO, TIME_TICK, TIME_TICK.saturating_mul(2)] {
            for edge in [IntervalEdge::Leading, IntervalEdge::Trailing] {
                let config = ThrottleIntervalConfig { period, edge };
                assert_eq!(
                    &[()],
                    futures::stream::once(async {})
                        .throttle_interval(config, NonZeroUsize::MIN)
                        .collect::<Vec<_>>()
                        .await
                        .as_slice()
                );
            }
        }
    }
}
