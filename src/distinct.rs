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
    struct FilterStatefulSync<St, State, F> {
        #[pin]
        stream: St,
        state: State,
        filter_update_state_fn: F,
    }
}

impl<St, State, F> Stream for FilterStatefulSync<St, State, F>
where
    St: Stream,
    F: FnMut(&mut State, &St::Item) -> bool,
{
    type Item = St::Item;

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

const fn filter_stateful_sync<St, State, F>(
    stream: St,
    initial_state: State,
    filter_update_state_fn: F,
) -> impl Stream<Item = St::Item>
where
    St: Stream,
    F: FnMut(&mut State, &St::Item) -> bool,
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

/// State for filtering distinct values that are interrupted by other items.
///
/// Used by [`filter_distinct_until_changed_ok_result()`] and
/// [`filter_distinct_until_changed_err_result()`]. Keeps the last value
/// after an interruption to reuse its allocation.
#[derive(Debug, Clone)]
pub struct DistinctState<T> {
    last_value: Option<T>,
    interrupted: bool,
}

impl<T> DistinctState<T> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_value: None,
            interrupted: false,
        }
    }
}

impl<T> Default for DistinctState<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Like [`filter_distinct_until_changed()`], but passes through and
/// interrupts the sequence if `next_value` is `None`.
fn filter_distinct_until_changed_some<T>(
    state: &mut DistinctState<T>,
    next_value: Option<&T>,
) -> bool
where
    T: Clone + PartialEq,
{
    let Some(next_value) = next_value else {
        state.interrupted = true;
        return true;
    };
    if !state.interrupted {
        return filter_distinct_until_changed(&mut state.last_value, next_value);
    }
    state.interrupted = false;
    match &mut state.last_value {
        // Reuse the existing allocation.
        Some(last_value) => last_value.clone_from(next_value),
        None => state.last_value = Some(next_value.clone()),
    }
    true
}

/// Filters out subsequent/adjacent stream items.
///
/// See also: [`filter_distinct_until_changed()`].
pub fn distinct_until_changed<St>(stream: St) -> impl Stream<Item = St::Item>
where
    St: Stream,
    St::Item: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, filter_distinct_until_changed)
}

/// Filters out subsequent/adjacent stream items with the same key.
///
/// Compares the keys of stream items that are extracted by `key_fn`.
/// The key of each stream item is extracted once and the items are
/// passed through unmodified.
/// The last key is stored until a different key occurs.
///
/// See also: [`distinct_until_changed()`]
pub fn distinct_until_changed_by_key<St, K, F>(
    stream: St,
    initial_key: K,
    mut key_fn: F,
) -> impl Stream<Item = St::Item>
where
    St: Stream,
    F: FnMut(&St::Item) -> K,
    K: PartialEq,
{
    filter_stateful_sync(stream, initial_key, move |last_key, next_item| {
        let next_key = key_fn(next_item);
        if *last_key == next_key {
            return false;
        }
        *last_key = next_key;
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
    state: &mut DistinctState<T>,
    next_result: &Result<T, E>,
) -> bool
where
    T: Clone + PartialEq,
{
    filter_distinct_until_changed_some(state, next_result.as_ref().ok())
}

/// Filters out subsequent/adjacent `Ok` items in a result stream.
///
/// See also: [`filter_distinct_until_changed_ok_result()`]
pub fn distinct_until_changed_ok_result<St, T, E>(stream: St) -> impl Stream<Item = St::Item>
where
    St: Stream<Item = Result<T, E>>,
    T: Clone + PartialEq,
{
    filter_stateful_sync(
        stream,
        DistinctState::new(),
        filter_distinct_until_changed_ok_result,
    )
}

/// Filters out subsequent/adjacent `Err` sequence items.
///
/// Each distinct `Err` sequence item will be cloned once.
/// All other `Err` sequence items are discarded.
/// All `Ok` sequence items are passed through.
///
/// See also: [`filter_distinct_until_changed()`]
pub fn filter_distinct_until_changed_err_result<T, E>(
    state: &mut DistinctState<E>,
    next_result: &Result<T, E>,
) -> bool
where
    E: Clone + PartialEq,
{
    filter_distinct_until_changed_some(state, next_result.as_ref().err())
}

/// Filters out subsequent/adjacent `Err` items in a result stream.
///
/// See also: [`filter_distinct_until_changed_err_result()`]
pub fn distinct_until_changed_err_result<St, T, E>(stream: St) -> impl Stream<Item = St::Item>
where
    St: Stream<Item = Result<T, E>>,
    E: Clone + PartialEq,
{
    filter_stateful_sync(
        stream,
        DistinctState::new(),
        filter_distinct_until_changed_err_result,
    )
}

#[cfg(test)]
mod tests;
