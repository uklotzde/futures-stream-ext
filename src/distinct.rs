// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::{
    pin::Pin,
    task::{Context, Poll, ready},
};

use futures_util::Stream;
use pin_project_lite::pin_project;

/// Stateful predicate for filtering stream items synchronously.
trait FilterState<T> {
    /// Updates the state and returns `true` if `next_item` should be passed through.
    fn filter_next(&mut self, next_item: &T) -> bool;
}

pin_project! {
    /// Filters stream items synchronously with a stateful predicate.
    ///
    /// Unlike [`StreamExt::filter()`](futures_util::StreamExt::filter) it
    /// neither buffers the item nor creates a future for each item.
    #[derive(Debug)]
    struct FilterStatefulSync<St, State> {
        #[pin]
        stream: St,
        state: State,
    }
}

impl<St, State> Stream for FilterStatefulSync<St, State>
where
    St: Stream,
    State: FilterState<St::Item>,
{
    type Item = St::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        while let Some(next_item) = ready!(this.stream.as_mut().poll_next(cx)) {
            if this.state.filter_next(&next_item) {
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

/// Implements [`Stream`] for a public wrapper of [`FilterStatefulSync`].
macro_rules! delegate_stream_to_inner {
    () => {
        type Item = St::Item;

        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.project().inner.poll_next(cx)
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            self.inner.size_hint()
        }
    };
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

impl<T: Clone + PartialEq> FilterState<T> for Option<T> {
    fn filter_next(&mut self, next_item: &T) -> bool {
        filter_distinct_until_changed(self, next_item)
    }
}

pin_project! {
    /// Result of [`StreamExt::distinct_until_changed()`](crate::StreamExt::distinct_until_changed).
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct DistinctUntilChanged<St: Stream> {
        #[pin]
        inner: FilterStatefulSync<St, Option<St::Item>>,
    }
}

impl<St: Stream> DistinctUntilChanged<St> {
    pub(crate) const fn new(stream: St) -> Self {
        Self {
            inner: FilterStatefulSync {
                stream,
                state: None,
            },
        }
    }
}

impl<St> Stream for DistinctUntilChanged<St>
where
    St: Stream,
    St::Item: Clone + PartialEq,
{
    delegate_stream_to_inner!();
}

#[derive(Debug)]
struct ByKeyState<K, F> {
    last_key: K,
    key_fn: F,
}

impl<T, K, F> FilterState<T> for ByKeyState<K, F>
where
    K: PartialEq,
    F: FnMut(&T) -> K,
{
    fn filter_next(&mut self, next_item: &T) -> bool {
        let next_key = (self.key_fn)(next_item);
        if self.last_key == next_key {
            return false;
        }
        self.last_key = next_key;
        true
    }
}

pin_project! {
    /// Result of [`StreamExt::distinct_until_changed_by_key()`](crate::StreamExt::distinct_until_changed_by_key).
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct DistinctUntilChangedByKey<St, K, F> {
        #[pin]
        inner: FilterStatefulSync<St, ByKeyState<K, F>>,
    }
}

impl<St, K, F> DistinctUntilChangedByKey<St, K, F> {
    pub(crate) const fn new(stream: St, initial_key: K, key_fn: F) -> Self {
        Self {
            inner: FilterStatefulSync {
                stream,
                state: ByKeyState {
                    last_key: initial_key,
                    key_fn,
                },
            },
        }
    }
}

impl<St, K, F> Stream for DistinctUntilChangedByKey<St, K, F>
where
    St: Stream,
    K: PartialEq,
    F: FnMut(&St::Item) -> K,
{
    delegate_stream_to_inner!();
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

#[derive(Debug)]
struct OkResultState<T>(DistinctState<T>);

impl<T, E> FilterState<Result<T, E>> for OkResultState<T>
where
    T: Clone + PartialEq,
{
    fn filter_next(&mut self, next_result: &Result<T, E>) -> bool {
        filter_distinct_until_changed_ok_result(&mut self.0, next_result)
    }
}

pin_project! {
    /// Result of [`StreamExt::distinct_until_changed_ok_result()`](crate::StreamExt::distinct_until_changed_ok_result).
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct DistinctUntilChangedOkResult<St, T> {
        #[pin]
        inner: FilterStatefulSync<St, OkResultState<T>>,
    }
}

impl<St, T> DistinctUntilChangedOkResult<St, T> {
    pub(crate) const fn new(stream: St) -> Self {
        Self {
            inner: FilterStatefulSync {
                stream,
                state: OkResultState(DistinctState::new()),
            },
        }
    }
}

impl<St, T, E> Stream for DistinctUntilChangedOkResult<St, T>
where
    St: Stream<Item = Result<T, E>>,
    T: Clone + PartialEq,
{
    delegate_stream_to_inner!();
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

#[derive(Debug)]
struct ErrResultState<E>(DistinctState<E>);

impl<T, E> FilterState<Result<T, E>> for ErrResultState<E>
where
    E: Clone + PartialEq,
{
    fn filter_next(&mut self, next_result: &Result<T, E>) -> bool {
        filter_distinct_until_changed_err_result(&mut self.0, next_result)
    }
}

pin_project! {
    /// Result of [`StreamExt::distinct_until_changed_err_result()`](crate::StreamExt::distinct_until_changed_err_result).
    #[derive(Debug)]
    #[must_use = "streams do nothing unless polled or .awaited"]
    pub struct DistinctUntilChangedErrResult<St, E> {
        #[pin]
        inner: FilterStatefulSync<St, ErrResultState<E>>,
    }
}

impl<St, E> DistinctUntilChangedErrResult<St, E> {
    pub(crate) const fn new(stream: St) -> Self {
        Self {
            inner: FilterStatefulSync {
                stream,
                state: ErrResultState(DistinctState::new()),
            },
        }
    }
}

impl<St, T, E> Stream for DistinctUntilChangedErrResult<St, E>
where
    St: Stream<Item = Result<T, E>>,
    E: Clone + PartialEq,
{
    delegate_stream_to_inner!();
}

#[cfg(test)]
mod tests;
