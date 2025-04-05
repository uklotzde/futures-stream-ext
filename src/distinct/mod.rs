// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_core::Stream;
use futures_util::StreamExt as _;

/// Filters out subsequent/adjacent stream items.
///
/// Each distinct stream item will be cloned once.
/// All other stream items are discarded.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed_stream<S>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream,
    S::Item: Clone + PartialEq,
{
    let mut last_item = None;
    stream.filter(move |next_item| {
        if let Some(last_item) = &last_item {
            if last_item == next_item {
                // Discard the next item.
                return std::future::ready(false);
            }
        }
        last_item = Some(next_item.clone());
        std::future::ready(true)
    })
}

/// Filters out subsequent/adjacent `Ok` items in a result stream.
///
/// Each distinct `Ok` stream item will be cloned once.
/// All other `Ok` stream items are discarded.
/// All `Err` stream items are passed through.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed_ok_result_stream<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    T: Clone + PartialEq,
{
    let mut last_ok = None;
    stream.filter(move |next_result| {
        if let Ok(next_ok) = &next_result {
            if let Some(last_ok) = &last_ok {
                if last_ok == next_ok {
                    // Discard the next item.
                    return std::future::ready(false);
                }
            }
            last_ok = Some(next_ok.clone());
        } else {
            last_ok = None;
        }
        std::future::ready(true)
    })
}

/// Filters out subsequent/adjacent `Err` items in a result stream.
///
/// Each distinct `Err` stream item will be cloned once.
/// All other `Err` stream items are discarded.
/// All `Ok` stream items are passed through.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed_err_result_stream<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    E: Clone + PartialEq,
{
    let mut last_err = None;
    stream.filter(move |next_result| {
        if let Err(next_err) = &next_result {
            if let Some(last_err) = &last_err {
                if last_err == next_err {
                    // Discard the next item.
                    return std::future::ready(false);
                }
            }
            last_err = Some(next_err.clone());
        } else {
            last_err = None;
        }
        std::future::ready(true)
    })
}

/// Filters out subsequent/adjacent stream items according to a _memo_ mapping.
///
/// For each stream item a _memo_ value is created and stored temporarily
/// until the next stream item arrives. The filtering is performed by
/// [`PartialEq`] on subsequent _memo_ values.
///
/// Filtering could be skipped and reset for an item by mapping it to
/// `distinct_memo` value. This special value is considered unequal
/// to any other _memo_ value, even to itself.
///
/// A typical choice for the _memo_ type is [`Option`], using
/// `None` for both the `initial_memo` and `distinct_memo` value.
///
/// The argument `initial_memo` should equal `distinct_memo` to ensure that
/// the first stream item is not filtered out. Separate arguments are needed
/// to avoid a [`Clone`] trait bound on the _memo_ type.
///
/// See also: <https://rxmarbles.com/#distinctUntilChanged>
pub fn distinct_until_changed_stream_memo<S, M, F>(
    stream: S,
    initial_memo: M,
    distinct_memo: M,
    mut map_memo_fn: F,
) -> impl Stream<Item = S::Item>
where
    S: Stream,
    M: PartialEq,
    F: FnMut(&S::Item) -> M,
{
    let mut last_memo = initial_memo;
    stream.filter(move |next_item| {
        let next_memo = map_memo_fn(next_item);
        if last_memo != distinct_memo && last_memo == next_memo {
            // Discard the next item.
            return std::future::ready(false);
        }
        last_memo = next_memo;
        std::future::ready(true)
    })
}

#[cfg(test)]
mod tests;
