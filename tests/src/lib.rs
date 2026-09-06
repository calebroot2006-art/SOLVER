//! Small poker games and independent scalar references for the CFR core.
//!
//! This crate is a test harness, not a production poker engine. Physical cards
//! remain distinct in Leduc; public chance masks implement card removal.

pub mod history_oracle;
pub mod kuhn;
pub mod leduc;
pub mod nan_game;
mod toy;

pub use toy::{Rules, ToyGame};
