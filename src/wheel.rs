//! Roulette Wheel Legacy Module & Direct Alias re-exports.
//!
//! Provides backward-compatible aliases mapping to [`crate::device`] and [`crate::bet`].

pub use crate::bet::{Bet, BetType};
pub use crate::device::{EuropeanWheel, OutcomeSlot, OutcomeSlot as Slot, SlotColor};

/// Legacy Wheel alias pointing to EuropeanWheel for backward compatibility.
pub type Wheel = EuropeanWheel;
