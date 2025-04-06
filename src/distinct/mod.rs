// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_core::Stream;

use crate::filter_stateful_sync;

/// Filters out subsequent/adjacent stream items.
///
/// Each distinct stream item will be cloned once.
/// All other stream items are discarded.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed<S>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream,
    S::Item: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, |last_item, next_item| {
        if let Some(last_item) = last_item {
            if last_item == next_item {
                // Discard the next item.
                return false;
            }
        }
        *last_item = Some(next_item.clone());
        true
    })
}

/// Filters out subsequent/adjacent `Ok` items in a result stream.
///
/// Each distinct `Ok` stream item will be cloned once.
/// All other `Ok` stream items are discarded.
/// All `Err` stream items are passed through.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed_ok_result<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    T: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, |last_ok, next_result| {
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
    })
}

/// Filters out subsequent/adjacent `Err` items in a result stream.
///
/// Each distinct `Err` stream item will be cloned once.
/// All other `Err` stream items are discarded.
/// All `Ok` stream items are passed through.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed_err_result<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    E: Clone + PartialEq,
{
    filter_stateful_sync(stream, None, |last_err, next_result| {
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
    })
}

#[cfg(test)]
mod tests;
