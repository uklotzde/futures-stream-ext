// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_core::Stream;
use futures_util::StreamExt as _;

/// Deduplicates subsequent stream items by mapping them to a _memo_ value.
///
/// For each stream item a _memo_ value is created and stored temporarily
/// until the next stream item arrives. The deduplication is performed by
/// [`PartialEq`] on subsequent _memo_ values.
///
/// The deduplication could be skipped and reset for any item by returning
/// the special `ignore_memo` value.
pub fn dedup_stream_memo<S, M, F>(
    stream: S,
    ignore_memo: M,
    mut map_memo_fn: F,
) -> impl Stream<Item = S::Item>
where
    S: Stream,
    M: Clone + PartialEq,
    F: FnMut(&S::Item) -> M,
{
    let mut last_memo = ignore_memo.clone();
    stream.filter(move |next_item| {
        let next_memo = map_memo_fn(next_item);
        if last_memo != ignore_memo && last_memo == next_memo {
            // Discard the next item.
            return std::future::ready(false);
        }
        last_memo = next_memo;
        std::future::ready(true)
    })
}

/// Deduplicates subsequent stream items.
///
/// Deduplication is implemented by cloning and comparing subsequent
/// stream items.
pub fn dedup_stream<S>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream,
    S::Item: Clone + PartialEq,
{
    dedup_stream_memo(stream, None, |next_item| Some(next_item.clone()))
}

/// Deduplicates subsequent `Ok` items in a result stream.
///
/// Each `Ok` stream item needs to be cloned for deduplication.
/// Instances of type `T` should therefore be cheaply cloneable.
pub fn dedup_ok_result_stream<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    T: Clone + PartialEq,
{
    dedup_stream_memo(stream, None, |next_result| {
        let Ok(next_ok) = next_result else {
            // `Err` values are not deduplicated and instead reset deduplication.
            return None;
        };
        Some(next_ok.clone())
    })
}

/// Deduplicates subsequent `Err` items in a result stream.
///
/// Each `Err` stream item needs to be cloned for deduplication.
/// Instances of type `E` should therefore be cheaply cloneable.
pub fn dedup_err_result_stream<S, T, E>(stream: S) -> impl Stream<Item = S::Item>
where
    S: Stream<Item = Result<T, E>>,
    E: Clone + PartialEq,
{
    dedup_stream_memo(stream, None, |next_result| {
        let Err(next_err) = next_result else {
            // `Err` values are not deduplicated and instead reset deduplication.
            return None;
        };
        Some(next_err.clone())
    })
}

#[cfg(test)]
mod tests;
