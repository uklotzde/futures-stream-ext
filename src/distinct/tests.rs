// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::StreamExt as _;

use crate::StreamExt as _;

#[tokio::test]
async fn should_filter_out_adjacent_duplicates() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 2, 3, 3]);
    let expected_output = vec![1, 2, 1, 2, 3];
    let actual_output = input.distinct_until_changed().collect::<Vec<_>>().await;
    assert_eq!(expected_output, actual_output);
}

#[tokio::test]
async fn should_filter_out_adjacent_items_with_the_same_key() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 4, 5, 2, 3, 3, 7]);
    let expected_output = vec![1, 2, 1, 2, 4, 2, 7];
    let actual_output = input
        .distinct_until_changed_by_key(None, |item| Some(item / 2))
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
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
    let expected_output = vec![Ok(1), Ok(2), Err(1), Ok(2), Err(2), Err(2), Ok(3)];
    let actual_output = input.distinct_until_changed_ok().collect::<Vec<_>>().await;
    assert_eq!(expected_output, actual_output);
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
    let expected_output = vec![Err(1), Err(2), Ok(1), Err(2), Ok(2), Ok(2), Err(3)];
    let actual_output = input.distinct_until_changed_err().collect::<Vec<_>>().await;
    assert_eq!(expected_output, actual_output);
}
