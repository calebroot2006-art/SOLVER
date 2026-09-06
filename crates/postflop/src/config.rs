//! Strict TOML configuration: no missing, unknown, or invalid fields.
use std::{fs, path::Path};
use serde::Deserialize;
use crate::{SolveError, Variant};

/// Solve stopping and progress settings. Phase 1 executes serially.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SolveConfig {
    /// Target in root-pot percent: 0.5 means half of one percent.
    pub target_pct_of_pot: f64,
    /// Total completed-iteration cap, including iterations already run.
    pub max_iterations: u64,
    /// Positive interval between accuracy measurements.
    pub check_every: u64,
    /// Positive wall-clock interval between progress checks at iteration boundaries.
    pub log_every_secs: u64,
    /// Zero selects the available implementation (serial); one requests serial.
    /// Values above one are rejected until parallel execution is implemented.
    pub threads: usize,
}

impl SolveConfig {
    /// Rejects invalid stop, progress, and unsupported execution settings.
    pub fn validate(&self) -> Result<(), SolveError> {
        if !self.target_pct_of_pot.is_finite() || self.target_pct_of_pot < 0.0 {
            return Err(SolveError::Config("solve.target_pct_of_pot must be finite and nonnegative".into()));
        }
        for (name, value) in [("max_iterations",self.max_iterations),("check_every",self.check_every),("log_every_secs",self.log_every_secs)] {
            if value == 0 { return Err(SolveError::Config(format!("solve.{name} must be positive"))); }
        }
        if self.threads > 1 { return Err(SolveError::Config("solve.threads must be 0 (auto/serial) or 1 in phase 1".into())); }
        Ok(())
    }
}

/// Exponents for sign-dependent regret and whole-strategy discounting.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DcfrParams {
    /// Positive-regret exponent.
    pub alpha: f64,
    /// Negative-regret exponent.
    pub beta: f64,
    /// Whole accumulated strategy exponent.
    pub gamma: f64,
}

impl DcfrParams {
    /// Rejects non-finite or negative exponents.
    pub fn validate(&self) -> Result<(), SolveError> {
        for (name,value) in [("alpha",self.alpha),("beta",self.beta),("gamma",self.gamma)] {
            if !value.is_finite() || value < 0.0 { return Err(SolveError::Config(format!("dcfr.{name} must be finite and nonnegative"))); }
        }
        Ok(())
    }
    /// Corresponding solver variant; construction validates its parameters.
    #[must_use]
    pub fn variant(&self) -> Variant { Variant::Discounted { alpha:self.alpha, beta:self.beta, gamma:self.gamma } }
}

/// The complete file, with mandatory `[solve]` and `[dcfr]` tables.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SolverConfig {
    /// Stopping, progress, and execution settings.
    pub solve: SolveConfig,
    /// Discounted CFR exponents.
    pub dcfr: DcfrParams,
}

impl SolverConfig {
    /// Parses and validates a complete configuration document.
    pub fn from_toml(text: &str) -> Result<Self, SolveError> {
        let config: Self = toml::from_str(text).map_err(|error| SolveError::Config(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }
    /// Loads UTF-8 TOML and includes the path in I/O diagnostics.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SolveError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|error| SolveError::Config(format!("{}: {error}",path.display())))?;
        Self::from_toml(&text)
    }
    /// Validates both mandatory sections.
    pub fn validate(&self) -> Result<(), SolveError> { self.solve.validate()?; self.dcfr.validate() }
}

/// Loads a complete configuration file.
pub fn load(path: impl AsRef<Path>) -> Result<SolverConfig, SolveError> { SolverConfig::load(path) }

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "[solve]\ntarget_pct_of_pot=0.5\nmax_iterations=100\ncheck_every=10\nlog_every_secs=1\nthreads=0\n[dcfr]\nalpha=1.5\nbeta=0.0\ngamma=2.0\n";
    #[test]
    fn fields_are_required_and_domains_are_checked() {
        assert!(SolverConfig::from_toml(VALID).is_ok());
        for (old,new,field) in [
            ("check_every=10","check_every=0","check_every"),
            ("max_iterations=100","max_iterations=0","max_iterations"),
            ("log_every_secs=1","log_every_secs=0","log_every_secs"),
            ("threads=0","threads=2","threads"),
            ("alpha=1.5","alpha=nan","alpha"),
            ("beta=0.0","beta=-1.0","beta"),
            ("gamma=2.0","gamma=inf","gamma"),
            ("target_pct_of_pot=0.5","target_pct_of_pot=-0.1","target_pct_of_pot"),
            ("check_every=10\n","","check_every"),
        ] {
            let error=SolverConfig::from_toml(&VALID.replace(old,new)).unwrap_err().to_string();
            assert!(error.contains(field),"{field}: {error}");
        }
        assert!(SolverConfig::from_toml(&format!("{VALID}surprise=1\n")).is_err());
    }
}