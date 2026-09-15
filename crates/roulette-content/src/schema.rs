//! Typed content schema for Roulette.OS (blueprint §16.2, TASK-006..008).
//!
//! Content is data: cards, wheels, upgrades, enemies, curses, events and forge
//! ops are declared in RON and validated at load. Gameplay mechanics consume
//! [`EffectKind`] values through the dispatch module in `roulette-core`
//! (content is data, mechanics are code — PLAN §3).

use serde::{Deserialize, Serialize};

/// Schema version of the content bundle; bump on breaking schema changes.
pub const SCHEMA_VERSION: u32 = 1;

/// The seven slot colors actually used by game rules (blueprint §16.2: 14
/// defined, 7 used).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SlotColor {
    Red,
    Black,
    Green,
    Gold,
    Purple,
    Cyan,
    Crimson,
}

/// Enemy intent vocabulary (blueprint §16.2 `EnemyActionType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnemyAction {
    Attack,
    Shield,
    PhysicsDebuff,
    StealChips,
    BlockRed,
    Buff,
}

/// A queued enemy intent: action + numeric value + flavor text (§7.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyIntentDef {
    pub action: EnemyAction,
    pub value: u16,
    pub description: String,
}

/// Card rarity (blueprint §16.2 `CardRarity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardRarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
}

/// Card type — one of the seven §6.1 categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardType {
    Physics,
    Board,
    Payout,
    Utility,
    Chaos,
    Paint,
    Money,
}

/// Wheel rarity used by the loadout store / shop (§16.2 `WheelRarity`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WheelRarity {
    Common,
    Uncommon,
    Rare,
    Legendary,
}

/// Selects the slots a slot-targeting effect applies to (TASK-007).
///
/// `RandomSlots`/`PlayerChoice` defer selection to play time; the count is the
/// number of slots picked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum NumberSet {
    All,
    Below(u32),
    Above(u32),
    /// Inclusive range `[lo, hi]` of wheel numbers.
    Range(u32, u32),
    List(Vec<u32>),
    Odd,
    Even,
    Prime,
    SingleDigit,
    /// Numbers 19–36.
    High,
    /// Numbers 1–18.
    Low,
    RandomSlots(u8),
    PlayerChoice(u8),
}

/// Duration of a temporary modifier (TASK-008).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DurationKind {
    /// Until the fight ends.
    Fight,
    /// A number of subsequent spins.
    Spins(u8),
    /// Only the next winning bet.
    NextWin,
}

/// Which bet family a payout multiplier targets (TASK-008 `PayoutMult`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum PayoutTarget {
    Red,
    Black,
    Green,
    Gold,
    Purple,
    Cyan,
    Crimson,
    /// Any single-number bet.
    SingleNumber,
    Number(u32),
    Odd,
    Even,
    Prime,
    Low,
    High,
    /// Dozen band 1..=3.
    Dozen(u8),
    /// Column 1..=3.
    Column(u8),
}

/// Zone-mark vocabulary (§6.2 `boardModifiers` zone lists).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ZoneKind {
    Lucky,
    Cursed,
    ChipMine,
    LifeFountain,
    DangerZone,
    GoldFoil,
    CopperPlate,
    Mirror,
}

/// Scope of a slot color conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ConvertScope {
    /// Until the end of the current round (paint cards).
    Round,
    /// Only the next spin.
    Spin,
    /// Until the fight ends.
    Fight,
}

/// Where temporary essence-chip cards are inserted (money cards §6.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ChipDestination {
    Hand,
    DrawPile,
    DiscardPile,
}

/// Hand/pile surgery operations (TASK-008 `HandOp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum HandOp {
    /// Golden Mirror: clone a random hand card into two 0-cost copies.
    CopyRandomHandCard,
    /// Copy Paste: clone the last played non-utility card.
    CopyLastPlayed,
    /// Recycle Bin: move one card from discard to hand.
    RecycleFromDiscard,
    /// Retain Vision: one chosen hand card is retained permanently.
    Retain,
    /// Essence Chip: the played card is exiled after resolving.
    ExileSelf,
    /// Deck Shuffle: shuffle the discard pile back into the draw pile.
    Reshuffle,
    /// Identity Shift: discard a hand card, draw one for 0 cost.
    DiscardFromHandDrawOne,
}

/// Stun trigger condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum StunCondition {
    OnPlay,
    /// Stun when `>=` this much damage was dealt this turn (Stun Strike).
    OnDamageGE(u16),
}

/// The typed effect DSL (PAT-001): every §6.2/§6.4 effect in wave-1 content.
///
/// Adding a genuinely new mechanic = one new variant + one dispatch arm
/// (GUD-002). Fields of [`EffectKind::Physics`] follow blueprint §16.3
/// `PhysicsModifiers`; `None` means "leave unchanged".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum EffectKind {
    /// Arms physics cheat values for the next spin (§5.3).
    Physics {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        friction: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ball_mass: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        spin_speed: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        wheel_tilt: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target_zone_bias: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bias_red_only: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bias_black_only: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        nudge_distance: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bounce_randomness: Option<f32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        multiball: Option<u8>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        split_peg: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shotgun: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        prediction_size: Option<u8>,
    },
    /// Bet-family payout multiplier for a duration (payout cards §6.3).
    PayoutMult { target: PayoutTarget, value: f32, duration: DurationKind },
    /// Exact-number payout override, e.g. number 7 → 200× (LUCKY_SEVEN).
    CustomNumberMult { numbers: Vec<u32>, mult: f32 },
    /// Recolor wheel slots (board converts + paint cards).
    ConvertSlots {
        to: SlotColor,
        /// Restrict to slots currently of this color; `None` = any eligible.
        from: Option<SlotColor>,
        numbers: NumberSet,
        scope: ConvertScope,
    },
    /// Monochrome-style color swap (Red ↔ Black).
    SwapRedBlack { scope: ConvertScope },
    /// Emerald Rift / Jade Path style extra green slot.
    AddGreenSlot { number: u32 },
    /// Marks zones on the felt (lucky/cursed/chip mine/... §10.4).
    ZoneMark {
        kind: ZoneKind,
        /// Explicit slot numbers, when the effect names them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        slots: Vec<u32>,
        /// Count of slots to pick at play time when `slots` is empty.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<u8>,
        /// Optional numeric payload (e.g. chip-mine chips, danger damage).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<u16>,
    },
    /// Global damage multiplier for N spins (LUCKY_INDEX style).
    GlobalMult { mult: f32, spins: u8 },
    /// GREEN_RIPPLE: +mult per active green slot, evaluated at spin.
    GreenRipple { mult_per_slot: f32 },
    /// Draw `count` cards immediately.
    Draw { count: u8 },
    /// Discard the entire hand.
    DiscardHand,
    /// Discard N random hand cards.
    DiscardRandom { count: u8 },
    /// Discard the whole hand, then draw the same number.
    DiscardHandThenDrawEqual,
    /// Immediately grant chips to the chips pool.
    GrantChips { amount: u16 },
    /// Reduce player HP (events only).
    LossHp { amount: u16 },
    /// Stun the enemy.
    StunEnemy { turns: u8, condition: StunCondition },
    /// One-shot ×2 arm on the next winning number bet.
    DoubleNextPayout,
    /// Insurance Policy: refund all bet chips on a lost spin.
    Insurance,
    /// Lucky Charm: reroll a 0-damage spin once.
    LuckyCharm,
    /// Emerald Forest: prime slots count as green; green payout ×2.
    EmeraldForest,
    /// Golden Heist: +amount chips whenever a gold slot lands.
    GoldenHeist { amount: u16 },
    /// Risk Capital: gain chips now, drain per spin for the fight.
    RiskCapital { gain: u16, drain_per_spin: u16 },
    /// Capital Venture: +amount shop chips at round end if won.
    CapitalVenture { amount: u16 },
    /// Double Down: double all bets; enemy damage taken doubles on a miss.
    DoubleDown,
    /// Tax Refund: +amount chips per essence-chip card in discard.
    TaxRefund { per_card: u16 },
    /// Hand/pile surgery.
    HandOp { kind: HandOp },
    /// Insert N temporary essence-chip cards.
    TempEssenceChips { count: u8, destination: ChipDestination },
    /// Adrenaline-style +actions/+essence burst.
    ActionSurge { chips: u16 },
    /// Grant a card by id (events).
    GrantCard { card_id: String },
}

/// Per-bet-family payout table (§4.2). Special colors are optional: wheels
/// without them simply omit the field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayoutTable {
    pub red: f32,
    pub black: f32,
    pub green: f32,
    pub number: f32,
    pub odd: f32,
    pub even: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gold: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purple: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cyan: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crimson: Option<f32>,
}

/// Static card definition (§16.2 `Card`, minus instance fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CardDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// ⚡ Essence cost, 0..=5 (TASK-006).
    pub cost: u8,
    pub card_type: CardType,
    pub rarity: CardRarity,
    pub effects: Vec<EffectKind>,
}

/// Static wheel definition (§16.2 `WheelConfig`). `numbers` is the physical
/// slot order; colors cover every slot explicitly (red/black included) so the
/// runtime config is fully data-defined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WheelDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Physical slot order (§4.6: the European sequence for classic wheels).
    pub numbers: Vec<u32>,
    pub green_numbers: Vec<u32>,
    pub red_numbers: Vec<u32>,
    pub black_numbers: Vec<u32>,
    /// Special overrides (gold/purple/cyan/crimson/extra green), keyed by number.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub slot_colors: std::collections::BTreeMap<u32, SlotColor>,
    pub payout_multipliers: PayoutTable,
    /// Purchased board-upgrade ids (§4.7).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub upgrades: Vec<String>,
    pub rarity: WheelRarity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub points_cost: Option<u8>,
}

/// Board-upgrade effect vocabulary (§4.7, `BoardUpgrade.effectType`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum BoardUpgradeEffect {
    MultiplierBoost {
        color: SlotColor,
        amount: f32,
    },
    AddGreenSlot {
        number: u32,
    },
    ConvertToColor {
        number: u32,
        to: SlotColor,
    },
    /// Sinner's Seven: number heals this much HP on landing.
    LuckyNumber {
        number: u32,
        heal: u16,
    },
    PhysicsMod {
        mass_mult: Option<f32>,
        friction_mult: Option<f32>,
    },
    UnlockAbility {
        color: SlotColor,
    },
    /// "Planet" upgrade: +levels to a color's payout ladder (§4.5).
    ColorLevel {
        color: SlotColor,
        levels: u8,
    },
}

/// Static board upgrade definition (§4.7, 18 items).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardUpgradeDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Store cost in PTS.
    pub cost: u8,
    pub effect: BoardUpgradeEffect,
}

/// Curse effect vocabulary (§8: curses corrupt tools, not numbers).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum CurseEffect {
    /// Faraday: magnetic cards/cheats disabled.
    DisableMagnetism,
    /// Fog: no prediction band.
    DisablePrediction,
    /// Rust: friction ×mult.
    FrictionMultiplier(f32),
    /// Greed: chips pool starts at this value instead of 30.
    ChipsPoolOverride(u16),
    /// Avarice: every draw costs a flat amount.
    FlatDrawCost(u16),
    /// Fragile: healing disabled.
    DisableHealing,
    /// Eclipse: green bets deal 0.
    VoidGreenPayouts,
    /// Curse of Blood: −amount HP at the start of every round.
    HpLossPerRound(u16),
    /// Lead: single slot bets capped at this many chips.
    BetCap(u16),
    /// Choked: hand limit reduced to this.
    HandLimitOverride(u8),
}

/// Static curse definition (§8, 10 items).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurseDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub effect: CurseEffect,
}

/// Enemy tier (§7.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum EnemyTier {
    Normal,
    Elite,
    Boss,
}

/// Static enemy definition (§16.2 `Enemy`, §7.1 roster).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnemyDef {
    pub id: String,
    pub name: String,
    pub max_hp: u16,
    pub tier: EnemyTier,
    /// Wheel template id this enemy plays (§7.1).
    pub wheel_id: String,
    /// 4-move looping intent pattern (§7.1).
    pub intent_pattern: [EnemyIntentDef; 4],
    /// Probability of playing the EV-optimal line (§7.4, 0.25–1.0).
    pub difficulty: f32,
    pub sprite_name: String,
}

/// One choice on an event screen (§9.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventChoice {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub effects: Vec<EffectKind>,
}

/// Static event definition (§9.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventDef {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub flavor: String,
    pub choices: Vec<EventChoice>,
}

/// Forge operation types (§9.2, verbatim six).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ForgeEffect {
    DestroyRandom { count: u8 },
    RemoveColor { color: SlotColor, count: u8 },
    RemoveGreen,
    AddColor { color: SlotColor, count: u8 },
    UpgradeMultiplier { color: SlotColor, amount: f32 },
    ConvertColor { from: SlotColor, to: SlotColor, count: u8 },
}

/// Forge offer rarity (§9.2: bronze 50% / silver 35% / gold 15%).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum ForgeRarity {
    Bronze,
    Silver,
    Gold,
}

/// Static forge operation definition (§9.2). Offers are generated per forge
/// node; defs here are the operation vocabulary + price bands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForgeOpDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rarity: ForgeRarity,
    /// ⚡ cost of the operation (8–25 per §9.2).
    pub cost: u8,
    pub effect: ForgeEffect,
}

/// Root content bundle (TASK-009). Loaded from a directory of RON files or
/// from the embedded defaults.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub schema_version: u32,
    #[serde(default)]
    pub cards: Vec<CardDef>,
    #[serde(default)]
    pub wheels: Vec<WheelDef>,
    #[serde(default)]
    pub board_upgrades: Vec<BoardUpgradeDef>,
    #[serde(default)]
    pub curses: Vec<CurseDef>,
    #[serde(default)]
    pub enemies: Vec<EnemyDef>,
    #[serde(default)]
    pub events: Vec<EventDef>,
    #[serde(default)]
    pub forge_ops: Vec<ForgeOpDef>,
}
