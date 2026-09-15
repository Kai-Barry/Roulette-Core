//! Bet vocabulary and win-check (TASK-019, §4.2 + §10.1 step 1).
//!
//! Win detection runs against the *effective* color after all conversions, with
//! the extra-green ladder (§4.4) and mirror slots (§10.1) folded in; payouts are
//! data-driven from the wheel's `PayoutTable`, scaled by color levels (§4.5).

use roulette_content::schema::{SlotColor, WheelDef};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::board::BoardModifiers;
use crate::wheel::{green_ladder_slots, ColorLevels, WheelConfig, PRIMES};

/// Base payouts for bet families absent from the per-wheel table (§4.2).
pub const BASE_DOZEN: f32 = 3.0;
pub const BASE_COLUMN: f32 = 3.0;
/// Special-color payout defaults when the wheel table omits them (§10.1 `|| 4.0`).
pub const DEFAULT_GOLD: f32 = 4.0;
pub const DEFAULT_PURPLE: f32 = 4.0;
pub const DEFAULT_CYAN: f32 = 4.0;
pub const DEFAULT_CRIMSON: f32 = 6.0;

/// The full §4.2 bet vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BetType {
    Red,
    Black,
    Green,
    Number(u32),
    Odd,
    Even,
    /// 1st/2nd/3rd dozen (1..=3).
    Dozen(u8),
    /// Column 1..=3, `((n−1) mod 3) + 1`.
    Column(u8),
    Gold,
    Purple,
    Cyan,
    Crimson,
}

/// A placed bet: type + chip amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bet {
    pub bet_type: BetType,
    pub amount: u16,
}

impl BetType {
    /// §10.1 win detection against the effective color/number.
    pub fn is_win_on(
        &self,
        wheel: &WheelConfig,
        landed: u32,
        levels: &ColorLevels,
        board: Option<&BoardModifiers>,
    ) -> bool {
        let color = wheel.effective_color(landed, levels.get(SlotColor::Green), board);
        match *self {
            BetType::Red => color == SlotColor::Red,
            BetType::Black => color == SlotColor::Black,
            BetType::Green => Self::green_win(wheel, landed, color, levels, board),
            BetType::Gold => color == SlotColor::Gold,
            BetType::Purple => color == SlotColor::Purple,
            BetType::Cyan => color == SlotColor::Cyan,
            BetType::Crimson => color == SlotColor::Crimson,
            BetType::Number(n) => {
                landed == n || board.is_some_and(|b| b.mirror_slots.get(&landed) == Some(&n))
            }
            // Green numbers (base + ladder) are excluded from parity (§10.1);
            // 0 is neither odd nor even.
            BetType::Odd => {
                !Self::is_green_number(wheel, levels, landed) && !landed.is_multiple_of(2)
            }
            BetType::Even => {
                !Self::is_green_number(wheel, levels, landed)
                    && landed != 0
                    && landed.is_multiple_of(2)
            }
            // Dozen/column cover 1..=36 only; 0/37+ are house numbers.
            BetType::Dozen(d) => (1..=36).contains(&landed) && landed.div_ceil(12) == d as u32,
            BetType::Column(c) => (1..=36).contains(&landed) && ((landed - 1) % 3) + 1 == c as u32,
        }
    }

    /// Green bet win rule (§10.1): base greens ∪ ladder ∪ effective green ∪
    /// Emerald-Forest primes.
    fn green_win(
        wheel: &WheelConfig,
        landed: u32,
        color: SlotColor,
        levels: &ColorLevels,
        board: Option<&BoardModifiers>,
    ) -> bool {
        if color == SlotColor::Green || wheel.green_numbers.contains(&landed) {
            return true;
        }
        if green_ladder_slots(levels.get(SlotColor::Green)).contains(&landed) {
            return true;
        }
        board.is_some_and(|b| b.emerald_forest_active && PRIMES.contains(&landed))
    }

    fn is_green_number(wheel: &WheelConfig, levels: &ColorLevels, landed: u32) -> bool {
        wheel.green_numbers.contains(&landed)
            || green_ladder_slots(levels.get(SlotColor::Green)).contains(&landed)
    }

    /// Base payout multiplier from the wheel table, §4.5 level-scaled (§10.1
    /// step 1). Specials fall back to 4/4/4/6 when the table omits them.
    pub fn base(&self, wheel: &WheelConfig, levels: &ColorLevels) -> f32 {
        let table = &wheel.payout_multipliers;
        let scaled = |base: f32, color: SlotColor| -> f32 {
            base + ColorLevels::payout_step(color) * levels.get(color) as f32
        };
        match *self {
            BetType::Red => scaled(table.red, SlotColor::Red),
            BetType::Black => scaled(table.black, SlotColor::Black),
            BetType::Green => scaled(table.green, SlotColor::Green),
            BetType::Number(_) => table.number,
            BetType::Odd => table.odd,
            BetType::Even => table.even,
            BetType::Dozen(_) => BASE_DOZEN,
            BetType::Column(_) => BASE_COLUMN,
            BetType::Gold => scaled(table.gold.unwrap_or(DEFAULT_GOLD), SlotColor::Gold),
            BetType::Purple => scaled(table.purple.unwrap_or(DEFAULT_PURPLE), SlotColor::Purple),
            BetType::Cyan => scaled(table.cyan.unwrap_or(DEFAULT_CYAN), SlotColor::Cyan),
            BetType::Crimson => {
                scaled(table.crimson.unwrap_or(DEFAULT_CRIMSON), SlotColor::Crimson)
            }
        }
    }
}

/// Bet placement / wheel-machinery errors (§4.2, §4.8).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum BetError {
    #[error("bet amount must be positive")]
    ZeroAmount,
    #[error("dozen index {0} out of range (1..=3)")]
    DozenRange(u8),
    #[error("column index {0} out of range (1..=3)")]
    ColumnRange(u8),
    #[error("number {0} is not on the wheel")]
    NotOnWheel(u32),
    #[error("number {0} is already on the wheel; slot numbers must be unique")]
    DuplicateNumber(u32),
}

impl Bet {
    pub fn new(bet_type: BetType, amount: u16) -> Result<Self, BetError> {
        if amount == 0 {
            return Err(BetError::ZeroAmount);
        }
        match bet_type {
            BetType::Dozen(d) if !(1..=3).contains(&d) => Err(BetError::DozenRange(d)),
            BetType::Column(c) if !(1..=3).contains(&c) => Err(BetError::ColumnRange(c)),
            _ => Ok(Self { bet_type, amount }),
        }
    }

    /// Builds a number bet after validating the number is on the wheel.
    pub fn number_on(wheel: &WheelConfig, number: u32, amount: u16) -> Result<Self, BetError> {
        if amount == 0 {
            return Err(BetError::ZeroAmount);
        }
        if !wheel.numbers.contains(&number) {
            return Err(BetError::NotOnWheel(number));
        }
        Ok(Self { bet_type: BetType::Number(number), amount })
    }

    /// §10.1 win detection against the effective color/number.
    pub fn is_win(
        &self,
        wheel: &WheelConfig,
        landed: u32,
        levels: &ColorLevels,
        board: Option<&BoardModifiers>,
    ) -> bool {
        self.bet_type.is_win_on(wheel, landed, levels, board)
    }

    /// Base payout multiplier (§10.1 step 1); see [`BetType::base`].
    pub fn base_multiplier(&self, wheel: &WheelConfig, levels: &ColorLevels) -> f32 {
        self.bet_type.base(wheel, levels)
    }
}

/// Validates a content wheel def against the §4.6 bet-machinery invariants.
pub fn validate_wheel_def(def: &WheelDef) -> Result<(), BetError> {
    let mut seen = std::collections::BTreeSet::new();
    for &n in &def.numbers {
        if !seen.insert(n) {
            return Err(BetError::DuplicateNumber(n));
        }
    }
    for &g in &def.green_numbers {
        if !seen.contains(&g) {
            return Err(BetError::NotOnWheel(g));
        }
    }
    for &s in def.slot_colors.keys() {
        if !seen.contains(&s) {
            return Err(BetError::NotOnWheel(s));
        }
    }
    Ok(())
}
