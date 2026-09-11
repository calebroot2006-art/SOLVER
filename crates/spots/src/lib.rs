//! The solved-spot format. The binary and JSON representations of a solved spot and
//! a range chart, the library index, and the CLI that generates a library. This format
//! is the contract between the solver and the app; the UI never reaches past it into
//! solver internals.
//!
//! Version 1 bounded structural storage and numerical quantization. Structural
//! validation preserves untrusted source claims; codecs and capture are separate.

pub mod format;
pub mod quantize;
pub mod resource;

/// The crate's own name, so the skeleton has one thing worth asserting until the
/// real API lands.
#[must_use]
pub const fn crate_name() -> &'static str {
    "spots"
}

#[cfg(test)]
mod tests {
    use super::crate_name;

    #[test]
    fn crate_name_matches_the_package() {
        assert_eq!(crate_name(), env!("CARGO_PKG_NAME"));
    }
}
