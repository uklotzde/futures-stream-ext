// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

//! Runtime-agnostic tests that do not require any optional features.

use std::{
    num::NonZeroUsize,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_util::{Stream, StreamExt as _, stream};

use crate::{Sleep, StreamExt as _, Throttler, next_period_delay};

/// A [`Sleep`] that is ready immediately.
#[derive(Debug)]
struct NoSleep;

impl Future for NoSleep {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(())
    }
}

impl Sleep for NoSleep {
    fn sleep(_duration: Duration) -> Self {
        Self
    }
}

/// A [`Throttler`] that is always ready.
#[derive(Debug)]
struct NoThrottler;

impl Stream for NoThrottler {
    type Item = ();

    fn poll_next(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(Some(()))
    }
}

impl<T> Throttler<T> for NoThrottler {
    fn on_first_item(self: Pin<&mut Self>, _cx: &mut Context<'_>) {}

    fn on_tick(self: Pin<&mut Self>, _cx: &mut Context<'_>, _yielded_item: Option<&T>) {}
}

#[tokio::test]
async fn debounce_with_sleep_should_only_emit_last_of_ready_items() {
    let actual_output = stream::iter([1, 2, 3])
        .debounce_with_sleep::<NoSleep>(Duration::ZERO, NonZeroUsize::MAX)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(vec![3], actual_output);
}

#[tokio::test]
async fn debounce_with_sleep_should_limit_polling_of_ready_items() {
    // The input stream is always ready and never finishes.
    let actual_output = stream::iter(0..)
        .debounce_with_sleep::<NoSleep>(Duration::ZERO, NonZeroUsize::new(3).unwrap())
        .take(2)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(vec![2, 5], actual_output);
}

#[tokio::test]
async fn throttle_should_pass_through_all_items_if_always_ready() {
    let actual_output = stream::iter([1, 2, 3])
        .throttle(NoThrottler, NonZeroUsize::MIN)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(vec![1, 2, 3], actual_output);
}

#[test]
fn next_period_delay_should_stay_on_schedule() {
    let period = Duration::from_millis(10);
    // Polled immediately or late, but within the next period.
    for (elapsed_millis, expected_millis) in [(0, 10), (1, 9), (9, 1)] {
        assert_eq!(
            Duration::from_millis(expected_millis),
            next_period_delay(Duration::from_millis(elapsed_millis), period)
        );
    }
}

#[test]
fn next_period_delay_should_skip_missed_periods() {
    let period = Duration::from_millis(10);
    for (elapsed_millis, expected_millis) in [(10, 10), (15, 5), (25, 5), (30, 10)] {
        assert_eq!(
            Duration::from_millis(expected_millis),
            next_period_delay(Duration::from_millis(elapsed_millis), period)
        );
    }
}
