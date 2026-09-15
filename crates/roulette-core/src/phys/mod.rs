//! Wheel/ball physics simulation (blueprint §5) — the game's soul.
//!
//! Fixed-step (120 Hz) deterministic simulation used identically for live spins and
//! prediction dry-runs. The dry-run draws from an **independent RNG substream**
//! (DEC-001, `docs/DECISIONS.md`): same spin parameters, independent launch jitter, so
//! prediction is a probabilistic band — never certain knowledge.
//!
//! Constants live in [`constants`] verbatim from §5.1/§5.2 and must not be restated
//! elsewhere.

pub mod constants;
pub mod events;
pub mod layout;
pub mod modifiers;
pub mod simulator;

pub use events::SimEvent;
pub use layout::WheelLayout;
pub use modifiers::PhysicsModifiers;
pub use simulator::{Prediction, Simulator};

use crate::Rng;

/// Fixed integration step in seconds (§5.1: PHYSICS_STEP 1/120).
pub const PHYSICS_STEP: f64 = constants::PHYSICS_STEP;

/// Headless step cap (§10.6: max 60 s sim = 7200 steps).
pub const MAX_STEPS: usize = constants::MAX_STEPS;

/// RNG substream label for live spins (GUD-004 / DEC-001).
pub const SPIN_STREAM: &str = "physics";

/// RNG substream label for prediction dry-runs (GUD-004 / DEC-001).
pub const PREDICTION_STREAM: &str = "prediction";

/// Convenience: child RNG for live spins from a master stream.
pub fn spin_rng(master: &Rng) -> Rng {
    master.derive(SPIN_STREAM)
}

/// Convenience: child RNG for prediction dry-runs from a master stream (DEC-001:
/// independent of [`spin_rng`], so prediction never reveals the exact outcome).
pub fn prediction_rng(master: &Rng) -> Rng {
    master.derive(PREDICTION_STREAM)
}
