//! Exploitability. The best-response calculator over the public-state tree, and
//! the metric built from it: `nash_conv` in chips per hand, its half, and that half
//! as a percentage of the fixed root pot.
//!
//! Phase 0 skeleton. Phase 3 takes the calculator over from postflop and runs it on the real tree.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "bestresponse"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
