//! Structured progress and timestamped diagnostic logging.
use crate::Exploitability;
use chrono::{SecondsFormat, Utc};
use std::time::Duration;

/// One progress event from a running solve.
///
/// An event is not a measurement. The driver measures on `check_every` and at
/// the iteration cap; the wall-clock interval only decides how often it says
/// where the solve has got to. So an event carries the last measurement taken,
/// [`Self::measured_at`] says which iteration that measurement covers, and
/// [`Self::stale`] says whether it is older than [`Self::iterations`]. A
/// consumer that plots convergence keeps the fresh events; one that shows a
/// user the solve is alive can use them all.
#[derive(Clone, Debug)]
pub struct Progress {
    /// ISO-8601 UTC wall-clock time.
    pub timestamp: String,
    /// Fully completed alternating iterations.
    pub iterations: u64,
    /// Measured chips and percent, never an inferred residual. `None` before
    /// the first measurement, which is the only time no number exists at all.
    pub exploitability: Option<Exploitability>,
    /// Iteration [`Self::exploitability`] was measured at, `None` with it.
    pub measured_at: Option<u64>,
    /// Whether the measurement is older than this event's iteration count.
    pub stale: bool,
    /// Elapsed wall time for this driver invocation.
    pub elapsed: Duration,
}

impl Progress {
    pub(crate) fn record(
        iterations: u64,
        measurement: Option<(Exploitability, u64)>,
        elapsed: Duration,
    ) -> Self {
        let progress = Self {
            timestamp: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            iterations,
            exploitability: measurement.map(|(value, _)| value),
            measured_at: measurement.map(|(_, at)| at),
            stale: measurement.is_some_and(|(_, at)| at != iterations),
            elapsed,
        };
        match progress.exploitability {
            Some(exploitability) => log::info!(
                "{} iteration={} measured_at={} stale={} nash_conv_chips={:.12} pct_of_pot={:.12} elapsed_secs={:.3}",
                progress.timestamp,
                iterations,
                progress.measured_at.unwrap_or(iterations),
                progress.stale,
                exploitability.nash_conv,
                exploitability.pct_of_pot,
                elapsed.as_secs_f64()
            ),
            None => log::info!(
                "{} iteration={} measured_at=none elapsed_secs={:.3}",
                progress.timestamp,
                iterations,
                elapsed.as_secs_f64()
            ),
        }
        progress
    }
}
