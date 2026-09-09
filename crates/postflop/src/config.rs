//! Strict TOML configuration: no missing, unknown, or invalid fields.
use crate::{SolveError, Variant};
use serde::Deserialize;
use std::{fs, path::Path};

/// Solve stopping, progress, and execution settings.
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
    /// Worker threads: zero asks for one per available core, one asks for
    /// serial execution, and any larger value asks for a pool of that size.
    /// The request is accepted whatever the solver can currently honour. Zero
    /// is resolved once through the platform's reported parallelism, so the
    /// memory estimate charges for exactly the workspaces the solver allocates.
    /// Until step 4 of `docs/phase-4/PLAN.md` wires the parallel traversal the
    /// walk itself stays serial on the first workspace.
    pub threads: usize,
}

impl SolveConfig {
    /// Rejects invalid stop and progress settings. Every thread count is valid.
    pub fn validate(&self) -> Result<(), SolveError> {
        if !self.target_pct_of_pot.is_finite() || self.target_pct_of_pot < 0.0 {
            return Err(SolveError::Config(
                "solve.target_pct_of_pot must be finite and nonnegative".into(),
            ));
        }
        for (name, value) in [
            ("max_iterations", self.max_iterations),
            ("check_every", self.check_every),
            ("log_every_secs", self.log_every_secs),
        ] {
            if value == 0 {
                return Err(SolveError::Config(format!("solve.{name} must be positive")));
            }
        }
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
        for (name, value) in [
            ("alpha", self.alpha),
            ("beta", self.beta),
            ("gamma", self.gamma),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(SolveError::Config(format!(
                    "dcfr.{name} must be finite and nonnegative"
                )));
            }
        }
        Ok(())
    }
    /// Corresponding solver variant; construction validates its parameters.
    #[must_use]
    pub fn variant(&self) -> Variant {
        Variant::Discounted {
            alpha: self.alpha,
            beta: self.beta,
            gamma: self.gamma,
        }
    }
}

/// How regrets and strategy sums are stored between iterations.
///
/// Arithmetic inside a node update stays in f64 whatever this says; the choice
/// is about the width of the accumulators, which is what decides whether a flop
/// tree fits in memory (`docs/phase-4/PLAN.md`, memory table).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Precision {
    /// Full f64 accumulators. The baseline every other width is measured against.
    #[default]
    F64,
    /// f32 accumulators. Arrives in step 7 of `docs/phase-4/PLAN.md`.
    F32,
    /// 16-bit accumulators with a per-node scale. Step 10 of the same plan.
    I16,
}

impl Precision {
    /// The spelling used in a configuration file.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::F64 => "f64",
            Self::F32 => "f32",
            Self::I16 => "i16",
        }
    }
}

/// Default working-set limit in MiB: decision 4 of `docs/phase-4/PLAN.md` puts
/// it at 12 GiB, leaving 4 GiB of a 16 GB machine for the desktop app and the
/// operating system.
const DEFAULT_MEMORY_LIMIT_MIB: usize = 12 * 1024;
/// Hard ceiling in MiB, also decision 4: 16 GiB, the shipped target machine.
///
/// This is the only place the ceiling is written down. The configuration file,
/// `RiverGame::new` and `PostflopGame::new` all refuse a larger limit against
/// this constant or against [`MEMORY_LIMIT_CEILING_BYTES`], which is derived
/// from it, so the three cannot drift apart.
pub const MEMORY_LIMIT_CEILING_MIB: usize = 16 * 1024;
/// [`MEMORY_LIMIT_CEILING_MIB`] as a byte count, in `u128` so a `usize` limit
/// from a 32-bit target can be compared against it without wrapping.
pub const MEMORY_LIMIT_CEILING_BYTES: u128 = (MEMORY_LIMIT_CEILING_MIB as u128) * 1024 * 1024;

fn default_memory_limit_mib() -> usize {
    DEFAULT_MEMORY_LIMIT_MIB
}

/// The complete file, with mandatory `[solve]` and `[dcfr]` tables.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct SolverConfig {
    /// Storage width for regrets and strategy sums. Optional; f64 when absent,
    /// so a file written before this field existed still parses.
    #[serde(default)]
    pub precision: Precision,
    /// Everything one game, its solver, its snapshots and its query workspaces
    /// may hold at once, in MiB. Optional; 12 GiB when absent, and at most the
    /// 16 GiB ceiling. Whole MiB is granularity enough for a memory limit and
    /// keeps the file free of nine-digit byte counts.
    #[serde(default = "default_memory_limit_mib")]
    pub memory_limit_mib: usize,
    /// Stopping, progress, and execution settings.
    pub solve: SolveConfig,
    /// Discounted CFR exponents.
    pub dcfr: DcfrParams,
}

impl SolverConfig {
    /// Parses and validates a complete configuration document.
    pub fn from_toml(text: &str) -> Result<Self, SolveError> {
        let config: Self =
            toml::from_str(text).map_err(|error| SolveError::Config(error.to_string()))?;
        config.validate()?;
        Ok(config)
    }
    /// Loads UTF-8 TOML and includes the path in I/O diagnostics.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, SolveError> {
        let path = path.as_ref();
        let text = fs::read_to_string(path)
            .map_err(|error| SolveError::Config(format!("{}: {error}", path.display())))?;
        Self::from_toml(&text)
    }
    /// The configured working-set limit in bytes.
    ///
    /// `validate` has already refused a zero or an over-ceiling value, so this
    /// only converts; it still reports an overflow rather than wrapping.
    pub fn memory_limit_bytes(&self) -> Result<usize, SolveError> {
        self.memory_limit_mib
            .checked_mul(1024 * 1024)
            .ok_or_else(|| SolveError::Config("memory_limit_mib overflows a byte count".into()))
    }
    /// Validates the storage width, the memory limit and both mandatory sections.
    pub fn validate(&self) -> Result<(), SolveError> {
        let unimplemented = |step: &str| {
            Err(SolveError::Config(format!(
                "precision \"{}\" is not implemented: {step} of docs/phase-4/PLAN.md adds it. Use \"f64\".",
                self.precision.as_str()
            )))
        };
        match self.precision {
            Precision::F64 => {}
            Precision::F32 => return unimplemented("step 7"),
            Precision::I16 => return unimplemented("step 10"),
        }
        if self.memory_limit_mib == 0 || self.memory_limit_mib > MEMORY_LIMIT_CEILING_MIB {
            return Err(SolveError::Config(format!(
                "memory_limit_mib must be between 1 and {MEMORY_LIMIT_CEILING_MIB}, not {}",
                self.memory_limit_mib
            )));
        }
        self.solve.validate()?;
        self.dcfr.validate()
    }
}

/// Loads a complete configuration file.
pub fn load(path: impl AsRef<Path>) -> Result<SolverConfig, SolveError> {
    SolverConfig::load(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "[solve]\ntarget_pct_of_pot=0.5\nmax_iterations=100\ncheck_every=10\nlog_every_secs=1\nthreads=0\n[dcfr]\nalpha=1.5\nbeta=0.0\ngamma=2.0\n";
    #[test]
    fn fields_are_required_and_domains_are_checked() {
        assert!(SolverConfig::from_toml(VALID).is_ok());
        for (old, new, field) in [
            ("check_every=10", "check_every=0", "check_every"),
            ("max_iterations=100", "max_iterations=0", "max_iterations"),
            ("log_every_secs=1", "log_every_secs=0", "log_every_secs"),
            ("alpha=1.5", "alpha=nan", "alpha"),
            ("beta=0.0", "beta=-1.0", "beta"),
            ("gamma=2.0", "gamma=inf", "gamma"),
            (
                "target_pct_of_pot=0.5",
                "target_pct_of_pot=-0.1",
                "target_pct_of_pot",
            ),
            ("check_every=10\n", "", "check_every"),
        ] {
            let error = SolverConfig::from_toml(&VALID.replace(old, new))
                .unwrap_err()
                .to_string();
            assert!(error.contains(field), "{field}: {error}");
        }
        assert!(SolverConfig::from_toml(&format!("{VALID}surprise=1\n")).is_err());
    }

    #[test]
    fn any_thread_count_is_accepted_and_absent_precision_means_f64() {
        for threads in ["threads=0", "threads=1", "threads=2", "threads=64"] {
            let config = SolverConfig::from_toml(&VALID.replace("threads=0", threads))
                .unwrap_or_else(|error| panic!("{threads}: {error}"));
            assert_eq!(config.precision, Precision::F64);
        }
    }

    #[test]
    fn the_shipped_configuration_file_still_loads() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/solver.toml");
        let config = SolverConfig::load(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        assert_eq!(config.precision, Precision::F64);
        // Decision 4's default, and the shipped file says it rather than
        // leaving the number compiled into the solver.
        assert_eq!(config.memory_limit_mib, 12 * 1024);
        assert_eq!(
            config.memory_limit_bytes().unwrap(),
            12 * 1024 * 1024 * 1024
        );
    }

    #[test]
    fn the_memory_limit_defaults_to_twelve_gibibytes_and_stops_at_sixteen() {
        // A file written before the key existed still parses, at the default.
        let absent = SolverConfig::from_toml(VALID).unwrap();
        assert_eq!(absent.memory_limit_mib, 12 * 1024);
        assert_eq!(
            absent.memory_limit_bytes().unwrap(),
            12 * 1024 * 1024 * 1024
        );

        // A bare key belongs to the table above it, so it leads the file.
        let with = |mib: usize| {
            SolverConfig::from_toml(&format!(
                "memory_limit_mib={mib}
{VALID}"
            ))
        };
        assert_eq!(with(4096).unwrap().memory_limit_bytes().unwrap(), 1 << 32);
        assert_eq!(
            with(16 * 1024).unwrap().memory_limit_mib,
            MEMORY_LIMIT_CEILING_MIB
        );
        // One ceiling in two units. Both game constructors compare a byte limit
        // against the derived form, so this is the number they enforce.
        assert_eq!(MEMORY_LIMIT_CEILING_BYTES, 16 * 1024 * 1024 * 1024);
        assert_eq!(
            u128::try_from(
                with(MEMORY_LIMIT_CEILING_MIB)
                    .unwrap()
                    .memory_limit_bytes()
                    .unwrap()
            )
            .unwrap(),
            MEMORY_LIMIT_CEILING_BYTES
        );
        for refused in [0, 16 * 1024 + 1, 1_000_000] {
            let error = with(refused).unwrap_err().to_string();
            assert!(error.contains("memory_limit_mib"), "{refused}: {error}");
            assert!(error.contains("16384"), "{refused}: {error}");
        }
    }

    #[test]
    fn unimplemented_precisions_name_the_plan_step_that_adds_them() {
        // A bare key belongs to the table above it, so precision leads the file.
        let with =
            |value: &str| SolverConfig::from_toml(&format!("precision=\"{value}\"\n{VALID}"));
        assert_eq!(with("f64").unwrap().precision, Precision::F64);

        let f32_error = with("f32").unwrap_err().to_string();
        assert_eq!(
            f32_error,
            "invalid config: precision \"f32\" is not implemented: \
             step 7 of docs/phase-4/PLAN.md adds it. Use \"f64\"."
        );
        let i16_error = with("i16").unwrap_err().to_string();
        assert!(i16_error.contains("step 10"), "{i16_error}");
        assert!(with("f16").unwrap_err().to_string().contains("precision"));
    }
}
