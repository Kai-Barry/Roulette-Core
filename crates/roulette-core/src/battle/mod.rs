//! Points-mode battle engine (Phase 4, GOAL-005).
//!
//! Modules:
//! - [`state`]: `BattleState` + piles/scores/curse state (§16.3, §3.1)
//! - [`actions`]: player turn actions (§3.2)
//! - [`resolve`]: the ordered §10.1 damage pipeline + §10.2/§10.4/§10.5 side effects
//! - [`enemy`]: EV-based enemy AI + intent execution (§7)
//! - [`round_end`]: round ticks, Curse of Blood, sudden death, outcomes

pub mod actions;
pub mod enemy;
pub mod resolve;
pub mod round_end;
pub mod state;

pub use state::{
    ActionError, ActiveCard, BallPayout, BattleOutcome, BattlePhase, BattleState, CardInstance,
    CombatMode, EnemyIntent, Side, SpinInput, SpinOutcome,
};
