//! `roulette_core` — headless deterministic engine for *Roulette.OS* (Roulette of the Damned).
//!
//! Content is data (see `roulette-content`), mechanics are code; the seam between
//! them is the typed effect DSL in `cards::effects`. See `ARCHITECTURE.md` and
//! `docs/ROULETTE_OS_GAME_BLUEPRINT.md` for the normative specifications.

pub mod deterministic_floats;
pub mod phys;
pub mod rng;

pub use rng::Rng;
