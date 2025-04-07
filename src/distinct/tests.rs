// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::StreamExt as _;

#[tokio::test]
async fn distinct_until_changed() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 2, 3, 3]);
    let expected_output = vec![1, 2, 1, 2, 3];
    let actual_output = super::distinct_until_changed(input.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
}

#[tokio::test]
async fn distinct_until_changed_map() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 4, 5, 2, 3, 3, 7]);
    let expected_output = vec![1, 2, 1, 2, 4, 2, 7];
    let actual_output = super::distinct_until_changed_map(input.clone(), |item| item / 2)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
}

#[tokio::test]
async fn distinct_until_changed_ok_result() {
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
    let actual_output = super::distinct_until_changed_ok_result(input.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
}

#[tokio::test]
async fn distinct_until_changed_err_result() {
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
    let actual_output = super::distinct_until_changed_err_result(input.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
}
