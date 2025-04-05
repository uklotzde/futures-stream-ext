// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::StreamExt as _;

#[tokio::test]
async fn dedup_stream() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 2, 3, 3]);
    let expected_output = vec![1, 2, 1, 2, 3];
    let actual_output = super::dedup_stream(input).collect::<Vec<_>>().await;
    assert_eq!(expected_output, actual_output);
}

#[tokio::test]
async fn dedup_ok_result_stream() {
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
    let actual_output = super::dedup_ok_result_stream(input)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
}

#[tokio::test]
async fn dedup_err_result_stream() {
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
    let actual_output = super::dedup_err_result_stream(input)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
}
