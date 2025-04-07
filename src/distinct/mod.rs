// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_core::Stream;

use crate::filter_stateful_sync;

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
    if let Some(last_item) = last_item {
        if last_item == next_item {
            // Discard the next item.
            return false;
        }
    }
    *last_item = Some(next_item.clone());
    true
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
    if let Ok(next_ok) = &next_result {
        if let Some(last_ok) = &last_ok {
            if last_ok == next_ok {
                // Discard the next item.
                return false;
            }
        }
        *last_ok = Some(next_ok.clone());
    } else {
        *last_ok = None;
    }
    true
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
    if let Err(next_err) = &next_result {
        if let Some(last_err) = &last_err {
            if last_err == next_err {
                // Discard the next item.
                return false;
            }
        }
        *last_err = Some(next_err.clone());
    } else {
        *last_err = None;
    }
    true
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
