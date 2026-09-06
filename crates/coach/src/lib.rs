//! The coach. EV-loss grading into four tiers with a mixed-strategy mode, the fact
//! builder that turns a decision into stable fact IDs, the validated template library,
//! the language-model client that may only select approved facts and templates, and the
//! leak tracker behind the drill queue.
//!
//! Phase 0 skeleton. Phase 9 builds the grader, the fact builder, the
//! templates, and the model client.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "coach"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
