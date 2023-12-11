// SPDX-FileCopyrightText: The futures-stream-ext authors
// SPDX-License-Identifier: MPL-2.0

use std::time::Duration;

use crate::IntervalEdge;

#[cfg(feature = "tokio")]
mod tokio;
#[cfg(feature = "tokio")]
pub use self::tokio::IntervalThrottler;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalThrottlerConfig {
    /// The minimum interval between subsequent items.
    pub period: Duration,

    /// Controls whether the pending item of the stream is yielded
    /// immediately or after the interval has elapsed.
    pub edge: IntervalEdge,
}
