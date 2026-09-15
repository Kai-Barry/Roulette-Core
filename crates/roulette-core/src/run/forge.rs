//! The Forge (§9.2, TASK-038): 3 free upgrade operations, rerollable for 5⚡,
//! rarity roll bronze 50 / silver 35 / gold 15, ≥2-slot rule, ⚙ CUSTOMIZE.

use roulette_content::schema::{ForgeEffect, ForgeOpDef, ForgeRarity, SlotColor};

use crate::rng::Rng;
use crate::wheel::{WheelConfig, WheelError};

/// Free operations granted per forge node (§9.2).
pub const FREE_OPS: u8 = 3;
/// Reroll price (§9.2).
pub const REROLL_PRICE: u16 = 5;

/// Errors from applying a forge operation to a wheel.
#[derive(Debug, Clone, PartialEq)]
pub enum ForgeError {
    Wheel(WheelError),
    /// No eligible slot exists for the operation.
    NoEligibleSlot,
    /// Operation vocabulary id not found in content.
    UnknownOp(String),
}

impl std::fmt::Display for ForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForgeError::Wheel(e) => write!(f, "{e}"),
            ForgeError::NoEligibleSlot => write!(f, "no eligible slot for forge op"),
            ForgeError::UnknownOp(id) => write!(f, "unknown forge op: {id}"),
        }
    }
}

impl From<WheelError> for ForgeError {
    fn from(e: WheelError) -> Self {
        ForgeError::Wheel(e)
    }
}

/// One forge visit's offer state (cached per forge node).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ForgeOffer {
    pub ops: Vec<ForgeOpDef>,
    /// Free operations remaining at this node.
    pub free_ops_remaining: u8,
    pub rerolls_used: u8,
}

impl ForgeOffer {
    /// Rolls 3 offers; each rolls bronze 50 / silver 35 / gold 15 and picks a
    /// random op of that rarity from the vocabulary.
    pub fn generate(vocabulary: &[ForgeOpDef], rng: &mut Rng) -> Self {
        Self { ops: roll_ops(vocabulary, rng), free_ops_remaining: FREE_OPS, rerolls_used: 0 }
    }

    /// Rerolls the offer list for 5 ⚡ (§9.2).
    pub fn reroll(&mut self, vocabulary: &[ForgeOpDef], rng: &mut Rng) {
        self.ops = roll_ops(vocabulary, rng);
        self.rerolls_used += 1;
    }

    /// Price of taking an op now: free while the free budget lasts, otherwise
    /// the def's ⚡ price (8–25, §9.2).
    pub fn op_price(&self, op_index: usize) -> Option<u16> {
        self.ops.get(op_index).map(|op| {
            if self.free_ops_remaining > 0 { 0 } else { op.cost as u16 }
        })
    }
}

fn roll_ops(vocabulary: &[ForgeOpDef], rng: &mut Rng) -> Vec<ForgeOpDef> {
    let mut ops: Vec<ForgeOpDef> = Vec::new();
    for _ in 0..FREE_OPS {
        let rarity = roll_op_rarity(rng);
        let pool: Vec<&ForgeOpDef> =
            vocabulary.iter().filter(|op| op.rarity == rarity).collect();
        let fallback: Vec<&ForgeOpDef> = if pool.is_empty() {
            vocabulary.iter().collect()
        } else {
            pool
        };
        if let Some(op) = rng.pick(&fallback) {
            ops.push((*op).clone());
        }
    }
    ops
}

/// Forge rarity roll (§9.2: bronze 50% / silver 35% / gold 15%).
pub fn roll_op_rarity(rng: &mut Rng) -> ForgeRarity {
    let roll = rng.next_f64();
    if roll < 0.15 {
        ForgeRarity::Gold
    } else if roll < 0.50 {
        ForgeRarity::Silver
    } else {
        ForgeRarity::Bronze
    }
}

/// Applies a forge operation to the run wheel, enforcing the ≥2-slot rule
/// (§9.2). `levels` is the green level used for ladder slot bookkeeping.
pub fn apply_op(
    wheel: &mut WheelConfig,
    op: &ForgeOpDef,
    green_level: u8,
    rng: &mut Rng,
) -> Result<(), ForgeError> {
    match &op.effect {
        ForgeEffect::DestroyRandom { count } => {
            for _ in 0..*count {
                if wheel.len() <= 2 {
                    return Err(ForgeError::Wheel(WheelError::TooFewSlots));
                }
                let idx = rng.range_usize(0, wheel.len() - 1);
                wheel.remove_slot(idx)?;
            }
        }
        ForgeEffect::RemoveColor { color, count } => {
            for _ in 0..*count {
                let idx = wheel
                    .numbers
                    .iter()
                    .position(|&n| slot_color(wheel, n, green_level) == *color)
                    .ok_or(ForgeError::NoEligibleSlot)?;
                strip_color(wheel, idx, green_level);
            }
        }
        ForgeEffect::RemoveGreen => {
            let idx = wheel
                .green_numbers
                .first()
                .copied()
                .and_then(|n| wheel.numbers.iter().position(|&m| m == n))
                .ok_or(ForgeError::NoEligibleSlot)?;
            strip_color(wheel, idx, green_level);
        }
        ForgeEffect::AddColor { color, count } => {
            for _ in 0..*count {
                // Paint an unpainted (or differently colored) slot; greens are
                // only taken when the target color is green itself.
                let idx = (0..wheel.len())
                    .find(|&i| {
                        let n = wheel.numbers[i];
                        if *color == SlotColor::Green {
                            !wheel.green_numbers.contains(&n)
                        } else {
                            slot_color(wheel, n, green_level) != SlotColor::Green
                                && slot_color(wheel, n, green_level) != *color
                        }
                    })
                    .ok_or(ForgeError::NoEligibleSlot)?;
                let n = wheel.numbers[idx];
                wheel.set_slot_color(n, *color);
            }
        }
        ForgeEffect::UpgradeMultiplier { color, amount } => {
            let table = &mut wheel.payout_multipliers;
            let bump = |slot: &mut Option<f32>, default: f32| {
                *slot = Some(slot.unwrap_or(default) + amount);
            };
            match color {
                SlotColor::Red => table.red += amount,
                SlotColor::Black => table.black += amount,
                SlotColor::Green => table.green += amount,
                SlotColor::Gold => bump(&mut table.gold, 4.0),
                SlotColor::Purple => bump(&mut table.purple, 4.0),
                SlotColor::Cyan => bump(&mut table.cyan, 6.0),
                SlotColor::Crimson => bump(&mut table.crimson, 6.0),
            }
        }
        ForgeEffect::ConvertColor { from, to, count } => {
            for _ in 0..*count {
                let idx = wheel
                    .numbers
                    .iter()
                    .position(|&n| slot_color(wheel, n, green_level) == *from)
                    .ok_or(ForgeError::NoEligibleSlot)?;
                let n = wheel.numbers[idx];
                wheel.set_slot_color(n, *to);
            }
        }
    }
    Ok(())
}

/// Effective base color of a slot (green ladder slots count as green).
fn slot_color(wheel: &WheelConfig, number: u32, green_level: u8) -> SlotColor {
    wheel.effective_color(number, green_level, None)
}

/// Strips a slot back to its wheel-table base color (red/black; 0 stays green).
fn strip_color(wheel: &mut WheelConfig, slot_index: usize, _green_level: u8) {
    let number = wheel.numbers[slot_index];
    let base = if number == 0 { SlotColor::Green } else { SlotColor::Red };
    wheel.set_slot_color(number, base);
}