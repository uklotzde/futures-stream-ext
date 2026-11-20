// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    pin::Pin,
    task::{Context, Poll, ready},
};

use futures_util::Stream;
use pin_project_lite::pin_project;

pin_project! {
    /// Filters stream items synchronously with a stateful predicate.
    ///
    /// Unlike [`StreamExt::filter()`](futures_util::StreamExt::filter) it
    /// neither buffers the item nor creates a future for each item.
    #[must_use = "streams do nothing unless polled or .awaited"]
    struct FilterStatefulSync<S, T, F> {
        #[pin]
        stream: S,
        state: T,
        filter_update_state_fn: F,
    }
}

impl<S, T, F> Stream for FilterStatefulSync<S, T, F>
where
    S: Stream,
    F: FnMut(&mut T, &S::Item) -> bool,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        while let Some(next_item) = ready!(this.stream.as_mut().poll_next(cx)) {
            if (this.filter_update_state_fn)(this.state, &next_item) {
                return Poll::Ready(Some(next_item));
            }
        }
        Poll::Ready(None)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.stream.size_hint();
        (0, upper)
    }
}

const fn filter_stateful_sync<S, T, F>(
    stream: S,
    initial_state: T,
    filter_update_state_fn: F,
) -> impl Stream<Item = S::Item>
where
    S: Stream,
    F: FnMut(&mut T, &S::Item) -> bool,
{
    FilterStatefulSync {
        stream,
        state: initial_state,
        filter_update_state_fn,
    }
}

/// Filtering of subsequent/adjacent sequence items.
///
/// Each distinct sequence item will be cloned once.
/// All other sequence items are discarded.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn filter_distinct_until_changed<T>(last_item: &mut Option<T>, next_item: &T) -> bool
where
    T: Clone + PartialEq,
{
    match last_item {
        // Discard the next item.
        Some(last_item) if last_item == next_item => false,
        Some(last_item) => {
            // Reuse the existing allocation.
            last_item.clone_from(next_item);
            true
        }
        None => {
            *last_item = Some(next_item.clone());
            true
        }
    }
}

/// Like [`filter_distinct_until_changed()`], but resets `last_item`
/// and passes through if `next_item` is `None`.
fn filter_distinct_until_changed_some<T>(last_item: &mut Option<T>, next_item: Option<&T>) -> bool
where
    T: Clone + PartialEq,
{
    let Some(next_item) = next_item else {
        *last_item = None;
        return true;
    };
    filter_distinct_until_changed(last_item, next_item)
}

/// Filters out subsequent/adjacent stream items.
///
/// See also: [`filter_distinct_until_changed()`].
pub fn distinct_until_changed<S>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream,
    S::Item: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, filter_distinct_until_changed)
}

/// Filters out subsequent/adjacent stream items (mapped).
///
/// Operates on mapped values of stream items.
/// Each stream item will be mapped once.
/// The last value is stored until a different value occurs.
///
/// See also: [`distinct_until_changed()`]
pub fn distinct_until_changed_map<S, T, F>(
    stream: S,
    initial_value: T,
    mut map_fn: F,
) -> impl Stream<Item = S::Item>
where
    S: Stream,
    F: FnMut(&S::Item) -> T,
    T: PartialEq,
{
    filter_stateful_sync(stream, initial_value, move |last_value, next_item| {
        let next_value = map_fn(next_item);
        if *last_value == next_value {
            return false;
        }
        *last_value = next_value;
        true
    })
}

/// Filters out subsequent/adjacent `Ok` sequence items.
///
/// Each distinct `Ok` sequence item will be cloned once.
/// All other `Ok` sequence items are discarded.
/// All `Err` sequence items are passed through.
///
/// See also: [`filter_distinct_until_changed()`]
pub fn filter_distinct_until_changed_ok_result<T, E>(
    last_ok: &mut Option<T>,
    next_result: &Result<T, E>,
) -> bool
where
    T: Clone + PartialEq,
{
    filter_distinct_until_changed_some(last_ok, next_result.as_ref().ok())
}

/// Filters out subsequent/adjacent `Ok` items in a result stream.
///
/// See also: [`filter_distinct_until_changed_ok_result()`]
pub fn distinct_until_changed_ok_result<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    T: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, filter_distinct_until_changed_ok_result)
}

/// Filters out subsequent/adjacent `Err` sequence items.
///
/// Each distinct `Err` sequence item will be cloned once.
/// All other `Err` sequence items are discarded.
/// All `Ok` sequence items are passed through.
///
/// See also: [`filter_distinct_until_changed()`]
pub fn filter_distinct_until_changed_err_result<T, E>(
    last_err: &mut Option<E>,
    next_result: &Result<T, E>,
) -> bool
where
    E: Clone + PartialEq,
{
    filter_distinct_until_changed_some(last_err, next_result.as_ref().err())
}

/// Filters out subsequent/adjacent `Err` items in a result stream.
///
/// See also: [`filter_distinct_until_changed_err_result()`]
pub fn distinct_until_changed_err_result<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    E: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, filter_distinct_until_changed_err_result)
}

#[cfg(test)]
mod tests;
