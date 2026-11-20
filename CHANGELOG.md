<!-- SPDX-FileCopyrightText: The futures-stream-ext authors -->
<!-- SPDX-License-Identifier: MPL-2.0 -->

# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2026-11-20

This release contains many breaking changes.
See [Migrating from 0.5](#migrating-from-05) for a summary.

### Added

- `StreamExt` can be used without the `tokio` feature.
  It provides the runtime-agnostic `debounce_with_sleep()` and `throttle_interval_with_sleep()`, which are generic over the `Sleep` implementation.
- `IntervalThrottler` is runtime-agnostic and generic over the `Sleep` implementation.
  It can be constructed with `IntervalThrottler::new()` and used with `StreamExt::throttle()`.
- `Sleep::reset()` restarts a sleep.
  The default implementation replaces the sleep with a new one.
  The implementation for `tokio::time::Sleep` reuses the registered timer.
- `Sleep::reset_next_period()` restarts an elapsed sleep on a fixed schedule.
  The default implementation falls back to `reset()`, so existing `Sleep` implementations keep working unchanged.
- `next_period_delay()` calculates the delay until the next deadline on a fixed schedule, with missed periods skipped.
  `Sleep` implementations that know their deadline can use it to override `reset_next_period()`.
- `DistinctState` holds the state of `filter_distinct_until_changed_ok()` and `filter_distinct_until_changed_err()`.
- The stream types `DistinctUntilChanged`, `DistinctUntilChangedByKey`, `DistinctUntilChangedOk` and `DistinctUntilChangedErr`.
  They implement `FusedStream` if the wrapped stream does.

### Changed

- **Breaking:** `debounce()` and `throttle_interval()` moved to the new `tokio::StreamExt` trait.
  The associated types `StreamExt::Sleep` and `StreamExt::IntervalThrottler` were removed.
- **Breaking:** `debounce()` and `debounce_with_sleep()` require a `poll_next_max_ready_count` argument, like `throttle()`.
- **Breaking:** The free functions `distinct_until_changed()`, `distinct_until_changed_map()`, `distinct_until_changed_ok_result()` and `distinct_until_changed_err_result()` were replaced by `StreamExt` methods.
  The methods return named stream types instead of `impl Stream`.
- **Breaking:** `distinct_until_changed_map()` was renamed to `distinct_until_changed_by_key()`.
  It compares the keys of items and passes the items through unmodified.
- **Breaking:** The `_result` suffix was dropped from `distinct_until_changed_ok()`, `distinct_until_changed_err()`, `filter_distinct_until_changed_ok()` and `filter_distinct_until_changed_err()`.
- **Breaking:** `filter_distinct_until_changed_ok()` and `filter_distinct_until_changed_err()` take a `DistinctState` instead of an `Option`.
- **Breaking:** The `Throttler` callbacks were renamed: `throttle_pending()` to `on_first_item()` and `throttle_ready()` to `on_tick()`.
- **Breaking:** `Throttled` calls `Throttler::on_first_item()` only while the throttler is idle, i.e. initially and after `on_tick()` was called with `None`.
  Throttlers should start the next interval in `on_tick()` only if an item is yielded.
- **Breaking:** `ThrottleIntervalConfig` was renamed to `IntervalThrottlerConfig`.
- **Breaking:** `tokio::IntervalThrottler` was replaced by the type alias `tokio::TokioIntervalThrottler` for `IntervalThrottler<tokio::time::Sleep>`.
  It no longer has a type parameter.
- **Breaking:** `Throttled::new()` is no longer public.
  Use `StreamExt::throttle()` instead.
- **Breaking:** The minimum supported Rust version is 1.89.
- Throttle intervals follow a fixed schedule and skip missed periods, instead of drifting by the scheduling latency.
- `Debounced` and `IntervalThrottler` reuse their sleep instead of creating a new one for each item or interval.
- `IntervalThrottler` starts the next interval only while items are pending.
- The distinct filters neither buffer items nor create a future for each item, and they reuse the allocation of the last value.

### Removed

- The dependency on `futures-core`.

### Fixed

- `Debounced` no longer polls an input stream that is always ready forever.
- `Throttled` no longer wakes itself up spuriously when it yields an item.

### Migrating from 0.5

| 0.5                                                           | 0.6                                                            |
| ------------------------------------------------------------- | -------------------------------------------------------------- |
| `StreamExt::debounce(delay)`                                  | `tokio::StreamExt::debounce(delay, poll_next_max_ready_count)` |
| `StreamExt::throttle_interval()`                              | `tokio::StreamExt::throttle_interval()`                        |
| `distinct_until_changed(stream)`                              | `stream.distinct_until_changed()`                              |
| `distinct_until_changed_map(stream, initial, f)`              | `stream.distinct_until_changed_by_key(initial, f)`             |
| `distinct_until_changed_ok_result(stream)`                    | `stream.distinct_until_changed_ok()`                           |
| `distinct_until_changed_err_result(stream)`                   | `stream.distinct_until_changed_err()`                          |
| `filter_distinct_until_changed_ok_result(&mut Option<T>, …)`  | `filter_distinct_until_changed_ok(&mut DistinctState<T>, …)`   |
| `filter_distinct_until_changed_err_result(&mut Option<E>, …)` | `filter_distinct_until_changed_err(&mut DistinctState<E>, …)`  |
| `Throttler::throttle_pending()`                               | `Throttler::on_first_item()`                                   |
| `Throttler::throttle_ready()`                                 | `Throttler::on_tick()`                                         |
| `ThrottleIntervalConfig`                                      | `IntervalThrottlerConfig`                                      |
| `tokio::IntervalThrottler<T>`                                 | `tokio::TokioIntervalThrottler`                                |
| `Throttled::new(stream, throttler, n)`                        | `stream.throttle(throttler, n)`                                |

A value of `NonZeroUsize::MIN` for `poll_next_max_ready_count` is a safe default.

## [0.5.0] and earlier

Changes before 0.6 are not recorded in this file.
See the [commit history](https://github.com/uklotzde/futures-stream-ext/commits/v0.5.0).

[Unreleased]: https://github.com/uklotzde/futures-stream-ext/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/uklotzde/futures-stream-ext/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/uklotzde/futures-stream-ext/releases/tag/v0.5.0
