// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use futures::{Stream, StreamExt as _, stream};
    use tokio::{
        runtime,
        time::{self, Instant, sleep_until},
    };

    use crate::StreamExt;

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

    // TODO: Add more tests, especially for edge cases.
}
