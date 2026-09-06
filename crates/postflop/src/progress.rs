//! Structured progress and timestamped diagnostic logging.
use crate::Exploitability;
use chrono::{SecondsFormat, Utc};
use std::time::Duration;

/// An accuracy measurement taken between complete iterations.
#[derive(Clone, Debug)]
pub struct Progress {
    /// ISO-8601 UTC wall-clock time.
    pub timestamp: String,
    /// Fully completed alternating iterations.
    pub iterations: u64,
    /// Measured chips and percent, never an inferred residual.
    pub exploitability: Exploitability,
    /// Elapsed wall time for this driver invocation.
    pub elapsed: Duration,
}

impl Progress {
    pub(crate) fn record(
        iterations: u64,
        exploitability: Exploitability,
        elapsed: Duration,
    ) -> Self {
        let progress = Self {
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            iterations,
            exploitability,
            elapsed,
        };
        log::info!(
            "{} iteration={} nash_conv_chips={:.12} pct_of_pot={:.12} elapsed_secs={:.3}",
            progress.timestamp,
            iterations,
            exploitability.nash_conv,
            exploitability.pct_of_pot,
            elapsed.as_secs_f64()
        );
        progress
    }
}
