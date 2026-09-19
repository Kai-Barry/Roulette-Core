//! # Roulette Core Engine
//!
//! `roulette_core` is a fast, deterministic, modular, headless engine and Monte Carlo simulator
//! for *Roulette of the Damned*—a dark 3D roguelike deckbuilder where combat is driven by customized game devices.
//!
//! ## Modular Subsystems
//! - [`bet`]: Device-agnostic betting rules, bet types, and payout multipliers.
//! - [`cards`]: Playable modifier cards, card effects, starter decks, and device modification hooks.
//! - [`combat`]: Modular combat state machine orchestrator and turn execution pipeline.
//! - [`device`]: Abstract [`device::GameDevice`] trait with concrete implementations ([`device::EuropeanWheel`], [`device::AmericanWheel`], [`device::DiceDevice`]).
//! - [`mode`]: Abstract [`mode::CombatMode`] trait defining victory/defeat rules for 1v1, Horde Mode, Boss Phases, etc.
//! - [`rng`]: Pure deterministic Mulberry32 PRNG with string seed support and Fisher-Yates shuffle.
//! - [`target`]: Abstract [`target::Target`] trait for enemy AI, bosses, shields, and multi-target encounters.
//! - [`wheel`]: Backward-compatibility aliases re-exporting [`device`] and [`bet`].

pub mod bet;
pub mod cards;
pub mod combat;
pub mod device;
pub mod mode;
pub mod rng;
pub mod target;
pub mod wheel;
