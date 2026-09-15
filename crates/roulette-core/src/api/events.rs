//! `EngineEvent` stream (TASK-042, PAT-004): serde-serializable, float-free,
//! so a future web frontend can replay/render deterministically (REQ-007/008).

use crate::battle::state::Side;
use crate::bets::BetType;
use crate::run::map::NodeType;
use crate::run::state::{Difficulty, GameState};
use roulette_content::schema::{EnemyTier, SlotColor};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum EngineEvent {
    RunStarted {
        seed: String,
        difficulty: Difficulty,
    },
    NodePicked {
        id: String,
        node_type: NodeType,
    },
    BattleStarted {
        tier: EnemyTier,
        round_limit: u32,
        enemy_hp: u16,
    },
    CardPlayed {
        card_id: String,
        cost: u16,
    },
    DrawBought {
        cost: u16,
        card_id: String,
    },
    BetPlaced {
        bet: BetType,
        amount: u16,
    },
    BetRemoved {
        bet: BetType,
        amount: u16,
    },
    BetsCleared,
    Rebet,
    Sacrificed,
    PredictionShown {
        start_slot: usize,
        size: u8,
    },
    SpinStarted {
        side: Side,
    },
    BallLanded {
        side: Side,
        number: u32,
    },
    SpinResolved {
        side: Side,
        pts: u16,
        insurance_refund: u16,
        all_lost: bool,
    },
    IntentExecuted,
    RoundEnded {
        round: u32,
        player_pts: u16,
        enemy_pts: u16,
        outcome: RoundOutcome,
    },
    BattleEnded {
        result: BattleResultEvent,
    },
    NodeCompleted,
    NodeFailed,
    RewardCards {
        ids: Vec<String>,
    },
    ChipsChanged {
        side: Side,
        delta: i32,
    },
    Purchased {
        item: usize,
        price: u16,
    },
    CardGained(String),
    WheelGained(String),
    Healed(u16),
    ForgeApplied {
        op: usize,
    },
    ForgeRerolled,
    EventChosen {
        choice: String,
        hp_lost: u16,
        chips_gained: u16,
        cards: Vec<String>,
    },
    ColorLevelBought {
        color: SlotColor,
        level: u8,
    },
    /// §4.8 customizer: a slot's color cycled to `color`.
    SlotColorCycled {
        slot: usize,
        color: SlotColor,
    },
    /// §4.8 customizer: a slot carrying `number` was added.
    SlotAdded {
        number: u32,
    },
    /// §4.8 customizer: the slot at `slot` was removed.
    SlotRemoved {
        slot: usize,
    },
    /// §4.8 customizer: the slot's number was set.
    SlotNumberSet {
        slot: usize,
        number: u32,
    },
    /// §4.8 customizer: the draft wheel was committed to the run.
    WheelSaved,
    /// §4.8 customizer: the draft wheel was discarded.
    WheelCancelled,
    GameOver,
    RunVictory,
    Undone,
}

/// Serializable copy of [`crate::battle::state::BattleOutcome`] (events must
/// stay float-free and serde-friendly).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundOutcome {
    InProgress,
    PlayerVictory,
    PlayerDefeat,
    SuddenDeath,
}

impl From<crate::battle::state::BattleOutcome> for RoundOutcome {
    fn from(o: crate::battle::state::BattleOutcome) -> Self {
        match o {
            crate::battle::state::BattleOutcome::InProgress => RoundOutcome::InProgress,
            crate::battle::state::BattleOutcome::PlayerVictory => RoundOutcome::PlayerVictory,
            crate::battle::state::BattleOutcome::PlayerDefeat => RoundOutcome::PlayerDefeat,
            crate::battle::state::BattleOutcome::SuddenDeath => RoundOutcome::SuddenDeath,
        }
    }
}

/// Game state re-export for CLI screens.
pub type ViewGameState = GameState;

/// Serializable mirror of [`crate::run::state::BattleResult`] for the event
/// log (the run-layer enum is not serde-derived).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum BattleResultEvent {
    Victory { rewards: Vec<String> },
    Defeat,
    GameOver,
}

impl From<crate::run::state::BattleResult> for BattleResultEvent {
    fn from(r: crate::run::state::BattleResult) -> Self {
        match r {
            crate::run::state::BattleResult::Victory { rewards } => {
                BattleResultEvent::Victory { rewards }
            }
            crate::run::state::BattleResult::Defeat => BattleResultEvent::Defeat,
            crate::run::state::BattleResult::GameOver => BattleResultEvent::GameOver,
        }
    }
}
