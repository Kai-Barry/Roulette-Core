//! `roulette_content` — typed schema + loader + validator for all game content.
//!
//! Content ships as RON files under `content/`; the engine consumes definitions
//! from this crate and never hard-codes cards, wheels or enemies (REQ-001).

pub mod loader;
pub mod schema;
pub mod validate;

// `Content` is the schema type; the loader module adds inherent impls on it.
pub use loader::embedded;
pub use schema::{
    BoardUpgradeDef, BoardUpgradeEffect, CardDef, CardRarity, CardType, ChipDestination, Content,
    ConvertScope, CurseDef, CurseEffect, DurationKind, EffectKind, EnemyAction, EnemyDef,
    EnemyIntentDef, EnemyTier, EventChoice, EventDef, ForgeEffect, ForgeOpDef, ForgeRarity, HandOp,
    NumberSet, PayoutTable, PayoutTarget, SlotColor, StunCondition, WheelDef, WheelRarity,
    ZoneKind, SCHEMA_VERSION,
};
pub use validate::ContentError;

/// Re-export validation so callers can pre-validate constructed bundles.
pub use validate::validate;
