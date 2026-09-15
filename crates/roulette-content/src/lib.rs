//! `roulette_content` — typed schema + loader + validator for all game content.
//!
//! Content ships as RON files under `content/`; the engine consumes definitions
//! from this crate and never hard-codes cards, wheels or enemies (REQ-001).

pub mod schema;
pub mod validate;

pub use validate::{validate_noop, ContentError};
