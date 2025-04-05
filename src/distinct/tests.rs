// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use futures_util::StreamExt as _;

#[tokio::test]
async fn distinct_until_changed_stream() {
    let input = futures_util::stream::iter([1, 1, 2, 1, 2, 2, 2, 3, 3]);
    let expected_output = vec![1, 2, 1, 2, 3];
    let actual_output = super::distinct_until_changed_stream(input.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
    let actual_output_memo =
        super::distinct_until_changed_stream_memo(input, None, None, |next_item| Some(*next_item))
            .collect::<Vec<_>>()
            .await;
    assert_eq!(expected_output, actual_output_memo);
}

#[tokio::test]
async fn distinct_until_changed_ok_result_stream() {
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
    let actual_output = super::distinct_until_changed_ok_result_stream(input.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
    let actual_output_memo =
        super::distinct_until_changed_stream_memo(input, None, None, |next_result| {
            next_result.ok()
        })
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output_memo);
}

#[tokio::test]
async fn distinct_until_changed_err_result_stream() {
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
    let actual_output = super::distinct_until_changed_err_result_stream(input.clone())
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output);
    let actual_output_memo =
        super::distinct_until_changed_stream_memo(input, None, None, |next_result| {
            next_result.err()
        })
        .collect::<Vec<_>>()
        .await;
    assert_eq!(expected_output, actual_output_memo);
}
