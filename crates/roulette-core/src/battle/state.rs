//! Points-mode battle state (TASK-024, §16.3 + §3.1/§3.2/§3.3).
//!
//! Both sides spin their own wheel; each side's bets resolve against its own
//! landed result. Player score = `chips_pool` (payout-as-chips §10.4); enemy
//! score = `enemy_chips_pool`. Player HP is *not* the combat score — it moves
//! only through intents, curse of blood, and heal effects.

use roulette_content::schema::{CardDef, CurseDef, EnemyIntentDef, WheelDef};
use serde::{Deserialize, Serialize};

use crate::bets::Bet;
use crate::board::{BoardModifiers, ModifierStack};
use crate::phys::PhysicsModifiers;
use crate::wheel::{ColorLevels, WheelConfig};

/// Which side is acting / spinning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Player,
    Enemy,
}

/// Turn phase (§16.3 `phase`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattlePhase {
    Betting,
    Spinning,
    Resolved,
}

/// A hand/pile card instance (§6.3: paint cards keep the player's `markedSlots`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardInstance {
    /// `CardDef::id` into the content bundle.
    pub def_id: String,
    /// Player-chosen slots resolved at play time (paint cards).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marked_slots: Vec<u32>,
    /// Temp cards (essence chips, clones) are exiled after play instead of
    /// discarded (§6.3 money cards).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub temp: bool,
    /// Retain Vision: stays in hand permanently.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub retained: bool,
    /// Golden Mirror clones play for 0 ⚡.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_override: Option<u8>,
}

/// A card played face-up on the felt (§3.2: locked while a temp effect runs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActiveCard {
    pub instance: CardInstance,
    /// Locked at spin: temp-effect cards cannot be removed mid-effect.
    pub locked_at_spin: bool,
}

/// Queued enemy intent with the def it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnemyIntent {
    pub action: roulette_content::schema::EnemyAction,
    pub value: u16,
    pub description: String,
    /// Pattern slot index (0..4) — the enemy loops its 4-move pattern.
    pub pattern_index: u8,
}

impl EnemyIntent {
    pub fn from_def(def: &EnemyIntentDef, pattern_index: u8) -> Self {
        Self {
            action: def.action,
            value: def.value,
            description: def.description.clone(),
            pattern_index,
        }
    }
}

/// Per-ball payout breakdown (§10.1 step 6: per-ball summation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BallPayout {
    pub landed: u32,
    /// Floor(total) per ball.
    pub damage: u16,
    pub in_prediction_sector: bool,
}

/// A winning bet's resolved contribution (pre-floor, for diagnostics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedBet {
    pub bet_type: crate::bets::BetType,
    pub amount: u16,
    pub multiplier: f32,
    pub payout: f32,
}

/// One spin's full resolution (§10.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpinOutcome {
    pub side: Side,
    /// Primary ball landing (multiball adds `balls` entries).
    pub landed: u32,
    pub balls: Vec<BallPayout>,
    pub resolved: Vec<ResolvedBet>,
    /// Winning chips added back to the bettor's pool (§10.4).
    pub total_payout: u16,
    /// True when every ball paid 0 for the bettor.
    pub all_lost: bool,
    /// Insurance consumed this spin (full stake refund §10.5).
    pub insurance_refund: u16,
    pub opponent_stunned_turns: u16,
    pub target_stunned_turns: u16,
}

/// The spin's raw input from the physics layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpinInput {
    pub side: Side,
    pub landed: u32,
    /// Extra balls (multiball / split peg / shotgun).
    #[serde(default)]
    pub extra_balls: Vec<u32>,
    /// Prediction sector (start slot index, width) if shown; drives §10.1
    /// step-4 penalties and the omniscience bonus.
    pub prediction: Option<(usize, u8)>,
    /// A played common/uncommon physics card this turn (×0.3 penalty).
    #[serde(default)]
    pub low_rarity_physics_played: bool,
}

/// Fight result (TASK-029).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleOutcome {
    InProgress,
    PlayerVictory,
    PlayerDefeat,
    SuddenDeath,
}

/// Points-mode battle state machine (§16.3 `BattleState`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleState {
    pub round: u32,
    pub max_rounds: u32,
    pub is_sudden_death: bool,
    pub phase: BattlePhase,
    /// Whose wheel is being spun / bets resolved (§3.2 turn flow).
    pub active_wheel_owner: Side,
    /// Side whose turn it is to act (player first, §3.1).
    pub turn: Side,

    // --- scores & pools (§3.3: score == pool in points mode) ---
    pub chips_pool: u16,
    pub enemy_chips_pool: u16,
    /// Player HP — moves only via intents/curse blood/heals (§3.3).
    pub player_hp: u16,
    pub player_max_hp: u16,
    pub enemy_hp: u16,
    pub player_damage_taken: u16,

    // --- piles (§3.1: exactly 2 cards at start; no free auto-draw) ---
    pub hand: Vec<CardInstance>,
    pub draw_pile: Vec<CardInstance>,
    pub discard_pile: Vec<CardInstance>,
    pub active_played: Vec<ActiveCard>,
    pub draws_this_turn: u8,
    pub free_cards_next_turn: u8,
    pub free_cards_active: u8,

    // --- bets ---
    pub bets: Vec<Bet>,
    pub enemy_bets: Vec<Bet>,
    /// REBET backup (§3.4: backed up on clear).
    pub rebet_backup: Vec<Bet>,

    // --- wheels (both sides own one, §4) ---
    pub player_wheel: WheelConfig,
    pub enemy_wheel: WheelConfig,
    pub player_levels: ColorLevels,
    pub enemy_levels: ColorLevels,

    // --- modifiers (PAT-002) ---
    pub player_stack: ModifierStack,
    /// Battle-owned board state (streak counters etc.) folded into snapshots.
    pub player_board: BoardModifiers,
    pub enemy_board: BoardModifiers,
    pub physics: PhysicsModifiers,
    /// Run-baseline physics the per-spin arms reset to after each resolve
    /// (cheats are spin-scoped; run-level mods and curses overlay here).
    #[serde(default = "crate::phys::modifiers::PhysicsModifiers::default")]
    pub physics_baseline: PhysicsModifiers,
    /// Capital Venture bank: ⚡ granted on round wins, drained by the run
    /// layer at battle end (§6.3 money_venture).
    #[serde(default)]
    pub venture_bank: u16,
    /// CYAN refill backup (§10.2: pool refilled to turn-start value).
    pub turn_start_pool: u16,

    // --- combat state ---
    pub curses: Vec<CurseDef>,
    pub enemy_intent: Option<EnemyIntent>,
    /// Index into the enemy's 4-move looping intent pattern (§7.1).
    pub enemy_intent_index: u8,
    /// Probability the enemy plays the EV-optimal line (§7.4, 0.25–1.0).
    pub enemy_difficulty: f32,
    pub enemy_stun_turns: u16,
    pub player_stun_turns: u16,
    /// Total damage dealt this turn (STUN_STRIKE ≥ 5 trigger).
    pub damage_this_turn: u16,
    pub last_spin_result: Option<SpinOutcome>,
    pub combat_mode: CombatMode,
}

/// Combat scoring mode (§3.3: points is default; damage is legacy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CombatMode {
    Points,
    Damage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionError {
    NotEnoughChips,
    NotYourTurn,
    HandFull,
    NoBetToRemove,
    NoCardAt(usize),
    EmptyDrawPileAndDiscard,
    /// Lead curse: single bet slot cap exceeded.
    BetCapExceeded(u16),
    InvalidBet(String),
}
impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActionError::NotEnoughChips => write!(f, "not enough chips"),
            ActionError::NotYourTurn => write!(f, "not your turn"),
            ActionError::HandFull => write!(f, "hand is full"),
            ActionError::NoBetToRemove => write!(f, "no bet to remove"),
            ActionError::NoCardAt(i) => write!(f, "no card at hand index {i}"),
            ActionError::EmptyDrawPileAndDiscard => write!(f, "draw pile and discard both empty"),
            ActionError::BetCapExceeded(cap) => write!(f, "bet exceeds lead cap of {cap}"),
            ActionError::InvalidBet(why) => write!(f, "invalid bet: {why}"),
        }
    }
}

impl BattleState {
    /// Sets up a fight (§3.1): pools 30 (15 under Greed), exactly 2 cards drawn,
    /// round limit 3/5/8 by tier, player acts first.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        player_wheel: WheelConfig,
        enemy_wheel: WheelConfig,
        player_hp: u16,
        player_max_hp: u16,
        enemy_hp: u16,
        max_rounds: u32,
        curses: Vec<CurseDef>,
        combat_mode: CombatMode,
    ) -> Self {
        let start_pool = curses
            .iter()
            .find_map(|c| match c.effect {
                roulette_content::schema::CurseEffect::ChipsPoolOverride(v) => Some(v),
                _ => None,
            })
            .unwrap_or(30);
        Self {
            round: 1,
            max_rounds,
            is_sudden_death: false,
            phase: BattlePhase::Betting,
            active_wheel_owner: Side::Player,
            turn: Side::Player,
            chips_pool: start_pool,
            enemy_chips_pool: 30,
            player_hp,
            player_max_hp,
            enemy_hp,
            player_damage_taken: 0,
            hand: Vec::new(),
            draw_pile: Vec::new(),
            discard_pile: Vec::new(),
            active_played: Vec::new(),
            draws_this_turn: 0,
            free_cards_next_turn: 0,
            free_cards_active: 0,
            bets: Vec::new(),
            enemy_bets: Vec::new(),
            rebet_backup: Vec::new(),
            player_wheel,
            enemy_wheel,
            player_levels: ColorLevels::default(),
            enemy_levels: ColorLevels::default(),
            player_stack: ModifierStack::new(),
            player_board: BoardModifiers::default(),
            enemy_board: BoardModifiers::default(),
            physics: PhysicsModifiers::default(),
            physics_baseline: PhysicsModifiers::default(),
            venture_bank: 0,
            turn_start_pool: start_pool,
            curses,
            enemy_intent: None,
            enemy_intent_index: 0,
            enemy_difficulty: 0.5,
            enemy_stun_turns: 0,
            player_stun_turns: 0,
            damage_this_turn: 0,
            last_spin_result: None,
            combat_mode,
        }
    }

    /// Seeds the piles from card defs and deals the starting hand of exactly 2
    /// (§3.1). Shuffles with the run RNG's derived "battle" substream.
    pub fn deal_from_defs(&mut self, defs: &[CardDef], rng: &mut crate::rng::Rng) {
        self.draw_pile = defs
            .iter()
            .map(|d| CardInstance {
                def_id: d.id.clone(),
                marked_slots: Vec::new(),
                temp: false,
                retained: false,
                cost_override: None,
            })
            .collect();
        rng.shuffle(&mut self.draw_pile);
        self.discard_pile.clear();
        self.hand.clear();
        self.draw_free(2);
    }

    // ------------------------------------------------------------------
    // Curse state (§8): curses corrupt tools, not numbers.
    // ------------------------------------------------------------------

    fn curse_effect(&self) -> impl Iterator<Item = &roulette_content::schema::CurseEffect> {
        self.curses.iter().map(|c| &c.effect)
    }

    /// Chips pool start override (Greed: 15).
    pub fn pool_override(&self) -> Option<u16> {
        self.curse_effect().find_map(|e| match e {
            roulette_content::schema::CurseEffect::ChipsPoolOverride(v) => Some(*v),
            _ => None,
        })
    }

    /// Draw cost for the next draw this turn (§3.1 ladder: 1st free, then
    /// 3/5/7/9/11…; Avarice flat 3).
    pub fn draw_cost(&self) -> u16 {
        if let Some(roulette_content::schema::CurseEffect::FlatDrawCost(v)) = self
            .curse_effect()
            .find(|e| matches!(e, roulette_content::schema::CurseEffect::FlatDrawCost(_)))
        {
            return *v;
        }
        match self.draws_this_turn {
            0 => 0,
            n => 2 * u16::from(n) + 1,
        }
    }

    pub fn hand_limit(&self) -> usize {
        self.curse_effect()
            .find_map(|e| match e {
                roulette_content::schema::CurseEffect::HandLimitOverride(v) => Some(*v as usize),
                _ => None,
            })
            .unwrap_or(8)
    }

    /// Single bet slot cap (Lead: 5; else unlimited).
    pub fn bet_cap(&self) -> Option<u16> {
        self.curse_effect().find_map(|e| match e {
            roulette_content::schema::CurseEffect::BetCap(v) => Some(*v),
            _ => None,
        })
    }

    pub fn healing_allowed(&self) -> bool {
        !self
            .curse_effect()
            .any(|e| matches!(e, roulette_content::schema::CurseEffect::DisableHealing))
    }

    pub fn green_voided(&self) -> bool {
        self.curses
            .iter()
            .any(|c| matches!(c.effect, roulette_content::schema::CurseEffect::VoidGreenPayouts))
    }

    /// Magnetism allowed (Faraday forces targetZoneBias 0).
    pub fn magnetism_allowed(&self) -> bool {
        !self
            .curse_effect()
            .any(|e| matches!(e, roulette_content::schema::CurseEffect::DisableMagnetism))
    }

    /// Prediction band allowed (Fog: none).
    pub fn prediction_allowed(&self) -> bool {
        !self
            .curse_effect()
            .any(|e| matches!(e, roulette_content::schema::CurseEffect::DisablePrediction))
    }

    /// Friction multiplier from the Rust curse (×2).
    pub fn friction_multiplier(&self) -> f32 {
        self.curse_effect()
            .find_map(|e| match e {
                roulette_content::schema::CurseEffect::FrictionMultiplier(m) => Some(*m),
                _ => None,
            })
            .unwrap_or(1.0)
    }

    /// Curse of Blood: −HP per round start.
    pub fn hp_loss_per_round(&self) -> u16 {
        self.curse_effect()
            .find_map(|e| match e {
                roulette_content::schema::CurseEffect::HpLossPerRound(v) => Some(*v),
                _ => None,
            })
            .unwrap_or(0)
    }

    // ------------------------------------------------------------------
    // Accessors used by the resolve pipeline.
    // ------------------------------------------------------------------

    pub fn wheel(&self, side: Side) -> &WheelConfig {
        match side {
            Side::Player => &self.player_wheel,
            Side::Enemy => &self.enemy_wheel,
        }
    }

    pub fn wheel_mut(&mut self, side: Side) -> &mut WheelConfig {
        match side {
            Side::Player => &mut self.player_wheel,
            Side::Enemy => &mut self.enemy_wheel,
        }
    }

    pub fn pool(&self, side: Side) -> u16 {
        match side {
            Side::Player => self.chips_pool,
            Side::Enemy => self.enemy_chips_pool,
        }
    }

    pub fn pool_mut(&mut self, side: Side) -> &mut u16 {
        match side {
            Side::Player => &mut self.chips_pool,
            Side::Enemy => &mut self.enemy_chips_pool,
        }
    }

    pub fn score(&self, side: Side) -> u16 {
        self.pool(side)
    }

    /// Enemy build context: its wheel comes straight from the template.
    pub fn enemy_wheel_from_def(def: &WheelDef) -> WheelConfig {
        WheelConfig::from_def(def)
    }
}
