// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::{StreamExt as _, stream::FusedStream};

use crate::StreamExt as _;

#[tokio::test]
async fn should_filter_out_adjacent_duplicates() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 2, 3, 3]);
    let expected = vec![1, 2, 1, 2, 3];
    let actual = input.distinct_until_changed().collect::<Vec<_>>().await;
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn should_filter_out_adjacent_items_that_are_equal_by_comparison() {
    let input = futures_util::stream::iter([1, 2, 3, 5, 6, 6, 8, 1, 2]);
    let expected = vec![1, 3, 5, 8, 1];
    // Compares with the last passed through item, not the last input item.
    let actual = input
        .distinct_until_changed_by(|last, next: &i32| last.abs_diff(*next) <= 1)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn should_filter_out_adjacent_items_with_the_same_key() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 4, 5, 2, 3, 3, 7]);
    let expected = vec![1, 2, 1, 2, 4, 2, 7];
    let actual = input
        .distinct_until_changed_by_key(None, |item| Some(item / 2))
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn should_filter_out_adjacent_duplicate_ok_items() {
    let input = futures_util::stream::iter([
        Ok(1),
        Ok(1),
        Ok(2),
        Err(1),
        Ok(2),
        Err(2),
        Err(2),
        Ok(3),
        Ok(3),
    ]);
    let expected = vec![Ok(1), Ok(2), Err(1), Ok(2), Err(2), Err(2), Ok(3)];
    let actual = input.distinct_until_changed_ok().collect::<Vec<_>>().await;
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn should_filter_out_adjacent_duplicate_err_items() {
    let input = futures_util::stream::iter([
        Err(1),
        Err(1),
        Err(2),
        Ok(1),
        Err(2),
        Ok(2),
        Ok(2),
        Err(3),
        Err(3),
    ]);
    let expected = vec![Err(1), Err(2), Ok(1), Err(2), Ok(2), Ok(2), Err(3)];
    let actual = input.distinct_until_changed_err().collect::<Vec<_>>().await;
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn should_be_terminated_after_a_fused_input_stream_has_finished() {
    async fn assert_terminated_after_finished(mut stream: impl FusedStream + Unpin) {
        assert!(!stream.is_terminated());
        while stream.next().await.is_some() {}
        assert!(stream.is_terminated());
    }
    let items = || futures_util::stream::iter([1, 1, 2]).fuse();
    let results = || futures_util::stream::iter([Ok::<_, ()>(1), Err(()), Ok(1)]).fuse();
    assert_terminated_after_finished(items().distinct_until_changed()).await;
    assert_terminated_after_finished(items().distinct_until_changed_by(|lhs, rhs| lhs == rhs))
        .await;
    assert_terminated_after_finished(
        items().distinct_until_changed_by_key(None, |item| Some(*item)),
    )
    .await;
    assert_terminated_after_finished(results().distinct_until_changed_ok()).await;
    assert_terminated_after_finished(results().distinct_until_changed_err()).await;
}
