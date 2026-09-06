//! The wasm-bindgen wrapper. A browser build is a phase 13 goal, not a phase 0 one.
//! The crate exists from the start so the solver core never quietly acquires a
//! dependency that cannot cross to `wasm32-unknown-unknown`.
//!
//! Phase 0 skeleton. Phase 13 ships a browser build; until then this crate
//! only has to keep compiling.

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "wasm"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
