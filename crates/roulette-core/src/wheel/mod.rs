//! Runtime wheel (TASK-018): declarative `WheelConfig` built from a `WheelDef`,
//! with effective-color resolution (§10.1 step 1), §4.5 color-level payout
//! scaling, the §4.4 extra-green ladder, §4.8 customizer ops and §4.7 board-upgrade
//! application.
//!
//! Resolution order for a slot's effective color — later layers win:
//! 1. base: `green_numbers` → green, `red_numbers` → red, else black (§4.1),
//! 2. permanent special `slot_colors` (template init + customizer + purchased
//!    converts) — overrides red/black,
//! 3. temporary paint (round/spin scope, from the [`crate::board::ModifierStack`]),
//! 4. fight-scope converts — highest precedence (TASK-023: convert > paint > base).

use std::collections::{BTreeMap, BTreeSet};

use roulette_content::schema::{
    BoardUpgradeDef, BoardUpgradeEffect, PayoutTable, SlotColor, WheelDef, WheelRarity,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::board::BoardModifiers;

/// The §4.1 canonical red-number set (European order complement).
pub const CANONICAL_RED: [u32; 18] =
    [1, 3, 5, 7, 9, 12, 14, 16, 18, 19, 21, 23, 25, 27, 30, 32, 34, 36];

/// Primes ≤ 36 (§10.3 zone list; used by Emerald Forest and prime bets).
pub const PRIMES: [u32; 11] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31];

/// Canonical red/black membership for numbers with no explicit color (§4.1).
pub fn canonical_color(number: u32) -> SlotColor {
    if CANONICAL_RED.contains(&number) {
        SlotColor::Red
    } else {
        SlotColor::Black
    }
}

/// §4.5 color levels (1..=10, Forge "Planet" upgrades / board `ColorLevel`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ColorLevels {
    levels: BTreeMap<SlotColor, u8>,
}

impl ColorLevels {
    pub fn get(&self, color: SlotColor) -> u8 {
        *self.levels.get(&color).unwrap_or(&0)
    }

    /// Raises a color's level by `n`, capped at [`MAX_COLOR_LEVEL`].
    pub fn level_up(&mut self, color: SlotColor, n: u8) {
        let next = self.get(color).saturating_add(n).min(MAX_COLOR_LEVEL);
        self.levels.insert(color, next);
    }

    /// §4.5 payout scaling step per level for a color.
    pub fn payout_step(color: SlotColor) -> f32 {
        match color {
            SlotColor::Red | SlotColor::Black => 0.2,
            SlotColor::Green => 2.0,
            SlotColor::Gold | SlotColor::Purple | SlotColor::Cyan | SlotColor::Crimson => 0.5,
        }
    }
}

/// Max color level (§4.5).
pub const MAX_COLOR_LEVEL: u8 = 10;

/// §4.4 extra green slots granted by green color levels (cumulative: each rung
/// adds to the previous).
pub fn green_ladder_slots(green_level: u8) -> &'static [u32] {
    const NONE: &[u32] = &[];
    const L1: &[u32] = &[32];
    const L2: &[u32] = &[32, 11, 22];
    const L4: &[u32] = &[32, 11, 22, 5, 17, 29];
    match green_level {
        0 => NONE,
        1 => L1,
        2 | 3 => L2,
        _ => L4,
    }
}

/// Wheel-bought physics upgrade accumulators (§4.7 `light_ball` / `heavy_friction`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WheelPhysicsMods {
    pub ball_mass_mult: f32,
    pub friction_mult: f32,
}

impl Default for WheelPhysicsMods {
    fn default() -> Self {
        Self { ball_mass_mult: 1.0, friction_mult: 1.0 }
    }
}

/// Declarative runtime wheel — the mutable per-run wheel (blueprint §16.2
/// `WheelConfig`). Cloneable and serde-able so the customizer can deep-clone it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WheelConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Physical slot order (§4.6 canonical sequences).
    pub numbers: Vec<u32>,
    pub green_numbers: Vec<u32>,
    /// Explicit red/black assignments — may be non-canonical (§4.6 red_heavy
    /// forced-red slots; customizer edits).
    pub red_numbers: Vec<u32>,
    pub black_numbers: Vec<u32>,
    /// Permanent special colors, keyed by number (template init, customizer
    /// edits, purchased converts).
    pub slot_colors: BTreeMap<u32, SlotColor>,
    pub payout_multipliers: PayoutTable,
    /// Purchased board-upgrade ids (§4.7).
    pub upgrades: Vec<String>,
    pub rarity: WheelRarity,
    pub points_cost: Option<u8>,
    /// Lucky numbers: number → HP healed on landing (§4.7 `lucky_number`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub lucky_numbers: BTreeMap<u32, u16>,
    /// Accumulated wheel-bought physics multipliers.
    #[serde(default)]
    pub physics_mods: WheelPhysicsMods,
    /// Unlocked color abilities (§4.5: Fever Ignition, Glacier Core, Synapse Link).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub unlocked_abilities: BTreeSet<SlotColor>,
}

/// Wheel mutation errors (customizer §4.8 + upgrades §4.7).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum WheelError {
    #[error("wheel must keep at least 2 slots (§4.8)")]
    TooFewSlots,
    #[error("number {0} is already on the wheel; slot numbers must be unique")]
    DuplicateNumber(u32),
    #[error("number {0} is not on the wheel")]
    UnknownNumber(u32),
    #[error("slot index {0} out of bounds (wheel has {1} slots)")]
    SlotIndex(usize, usize),
    #[error("color {0:?} is already at max level {MAX_COLOR_LEVEL}")]
    MaxLevel(SlotColor),
}

impl WheelConfig {
    /// Builds the runtime config from a content `WheelDef`.
    pub fn from_def(def: &WheelDef) -> Self {
        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            description: def.description.clone(),
            numbers: def.numbers.clone(),
            green_numbers: def.green_numbers.clone(),
            red_numbers: def.red_numbers.clone(),
            black_numbers: def.black_numbers.clone(),
            slot_colors: def.slot_colors.clone(),
            payout_multipliers: def.payout_multipliers.clone(),
            upgrades: def.upgrades.clone(),
            rarity: def.rarity,
            points_cost: def.points_cost,
            lucky_numbers: BTreeMap::new(),
            physics_mods: WheelPhysicsMods::default(),
            unlocked_abilities: BTreeSet::new(),
        }
    }

    /// Number of physical slots.
    pub fn len(&self) -> usize {
        self.numbers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.numbers.is_empty()
    }

    /// The special color permanently attached to a number, if any.
    pub fn special_color(&self, number: u32) -> Option<SlotColor> {
        self.slot_colors.get(&number).copied()
    }

    /// Effective color of a slot after all conversions (§10.1 step 1).
    ///
    /// Precedence (later wins): base list → permanent specials → temporary
    /// paint (round/spin scope) → fight-scope converts. `green_level` grants the
    /// §4.4 ladder greens on plain red/black slots (specials stay special).
    pub fn effective_color(
        &self,
        number: u32,
        green_level: u8,
        board: Option<&BoardModifiers>,
    ) -> SlotColor {
        // Base layer: explicit lists, then canonical red/black fallback.
        let base = if self.green_numbers.contains(&number) {
            SlotColor::Green
        } else if let Some(special) = self.special_color(number) {
            special
        } else if self.red_numbers.contains(&number) {
            SlotColor::Red
        } else if self.black_numbers.contains(&number) {
            SlotColor::Black
        } else {
            canonical_color(number)
        };
        // Ladder greens only claim plain red/black slots (§4.4).
        let base = if base != SlotColor::Green
            && !matches!(
                base,
                SlotColor::Gold | SlotColor::Purple | SlotColor::Cyan | SlotColor::Crimson
            )
            && green_ladder_slots(green_level).contains(&number)
        {
            SlotColor::Green
        } else {
            base
        };
        // Temporary layers, weakest to strongest: paint then convert.
        let Some(board) = board else {
            return base;
        };
        let painted = board.color_of_converted(number, false);
        let converted = board.color_of_converted(number, true);
        let final_color = converted.or(painted).unwrap_or(base);
        // Monochrome swap (§6.3): red ↔ black after every other layer; green
        // and specials are unaffected.
        match (board.swap_red_black, final_color) {
            (true, SlotColor::Red) => SlotColor::Black,
            (true, SlotColor::Black) => SlotColor::Red,
            _ => final_color,
        }
    }

    /// Applies the §4.4 ladder for a green level-up: extra green numbers join
    /// `green_numbers` (idempotent).
    pub fn apply_green_ladder(&mut self, green_level: u8) {
        for n in green_ladder_slots(green_level) {
            if !self.green_numbers.contains(n) {
                self.green_numbers.push(*n);
                self.green_numbers.sort_unstable();
            }
        }
    }

    // ------------------------------------------------------------------
    // §4.8 customizer ops (deep-clone first; save/cancel is clone-or-drop)
    // ------------------------------------------------------------------

    /// Cycles a slot's color through red → black → green → gold → purple →
    /// cyan → crimson → red. Slot indices index `numbers` (§4.8 cell-grid).
    pub fn cycle_slot_color(&mut self, slot_index: usize) -> Result<SlotColor, WheelError> {
        let number = self
            .numbers
            .get(slot_index)
            .copied()
            .ok_or(WheelError::SlotIndex(slot_index, self.len()))?;
        let current = self.effective_color(number, 0, None);
        let next = match current {
            SlotColor::Red => SlotColor::Black,
            SlotColor::Black => SlotColor::Green,
            SlotColor::Green => SlotColor::Gold,
            SlotColor::Gold => SlotColor::Purple,
            SlotColor::Purple => SlotColor::Cyan,
            SlotColor::Cyan => SlotColor::Crimson,
            SlotColor::Crimson => SlotColor::Red,
        };
        self.set_slot_color(number, next);
        Ok(next)
    }

    /// Sets a number's color, keeping the color lists / `slot_colors` consistent.
    pub fn set_slot_color(&mut self, number: u32, to: SlotColor) {
        self.clear_color_marks(number);
        match to {
            SlotColor::Green => {
                if !self.green_numbers.contains(&number) {
                    self.green_numbers.push(number);
                    self.green_numbers.sort_unstable();
                }
            }
            SlotColor::Red => self.red_numbers.push(number),
            SlotColor::Black => self.black_numbers.push(number),
            special => {
                self.slot_colors.insert(number, special);
            }
        }
    }

    /// Removes a number from every color list / special map.
    fn clear_color_marks(&mut self, number: u32) {
        self.green_numbers.retain(|&n| n != number);
        self.red_numbers.retain(|&n| n != number);
        self.black_numbers.retain(|&n| n != number);
        self.slot_colors.remove(&number);
    }

    /// Adds a slot carrying `number` (uniqueness enforced, §4.8).
    pub fn add_slot(&mut self, number: u32) -> Result<(), WheelError> {
        if self.numbers.contains(&number) {
            return Err(WheelError::DuplicateNumber(number));
        }
        self.numbers.push(number);
        Ok(())
    }

    /// Removes a slot; the wheel must keep ≥ 2 slots (§4.8).
    pub fn remove_slot(&mut self, slot_index: usize) -> Result<(), WheelError> {
        if self.len() <= 2 {
            return Err(WheelError::TooFewSlots);
        }
        if slot_index >= self.len() {
            return Err(WheelError::SlotIndex(slot_index, self.len()));
        }
        let number = self.numbers.remove(slot_index);
        self.clear_color_marks(number);
        Ok(())
    }

    /// Renumbers a slot; uniqueness enforced (§4.8).
    pub fn set_slot_number(
        &mut self,
        slot_index: usize,
        new_number: u32,
    ) -> Result<(), WheelError> {
        let old = self
            .numbers
            .get(slot_index)
            .copied()
            .ok_or(WheelError::SlotIndex(slot_index, self.len()))?;
        if new_number != old && self.numbers.contains(&new_number) {
            return Err(WheelError::DuplicateNumber(new_number));
        }
        // Re-key color bookkeeping.
        if let Some(color) = self.slot_colors.remove(&old) {
            self.slot_colors.insert(new_number, color);
        }
        for list in [&mut self.green_numbers, &mut self.red_numbers, &mut self.black_numbers] {
            if let Some(pos) = list.iter().position(|&n| n == old) {
                list[pos] = new_number;
                list.sort_unstable();
            }
        }
        self.numbers[slot_index] = new_number;
        Ok(())
    }

    // ------------------------------------------------------------------
    // §4.7 board upgrades
    // ------------------------------------------------------------------

    /// Applies a purchased board upgrade, recording its id (§4.7).
    pub fn apply_upgrade(
        &mut self,
        levels: &mut ColorLevels,
        upgrade: &BoardUpgradeDef,
    ) -> Result<(), WheelError> {
        match &upgrade.effect {
            BoardUpgradeEffect::MultiplierBoost { color, amount } => match color {
                SlotColor::Red => self.payout_multipliers.red += amount,
                SlotColor::Black => self.payout_multipliers.black += amount,
                SlotColor::Green => self.payout_multipliers.green += amount,
                SlotColor::Gold => {
                    self.payout_multipliers.gold =
                        Some(self.payout_multipliers.gold.unwrap_or(4.0) + amount)
                }
                SlotColor::Purple => {
                    self.payout_multipliers.purple =
                        Some(self.payout_multipliers.purple.unwrap_or(4.0) + amount)
                }
                SlotColor::Cyan => {
                    self.payout_multipliers.cyan =
                        Some(self.payout_multipliers.cyan.unwrap_or(4.0) + amount)
                }
                SlotColor::Crimson => {
                    self.payout_multipliers.crimson =
                        Some(self.payout_multipliers.crimson.unwrap_or(6.0) + amount)
                }
            },
            BoardUpgradeEffect::AddGreenSlot { number } => {
                if self.numbers.contains(number) && !self.green_numbers.contains(number) {
                    self.green_numbers.push(*number);
                    self.green_numbers.sort_unstable();
                }
            }
            BoardUpgradeEffect::ConvertToColor { number, to } => {
                if self.numbers.contains(number) {
                    self.set_slot_color(*number, *to);
                }
            }
            BoardUpgradeEffect::LuckyNumber { number, heal } => {
                self.lucky_numbers.insert(*number, *heal);
            }
            BoardUpgradeEffect::PhysicsMod { mass_mult, friction_mult } => {
                if let Some(m) = mass_mult {
                    self.physics_mods.ball_mass_mult *= m;
                }
                if let Some(m) = friction_mult {
                    self.physics_mods.friction_mult *= m;
                }
            }
            BoardUpgradeEffect::UnlockAbility { color } => {
                self.unlocked_abilities.insert(*color);
            }
            BoardUpgradeEffect::ColorLevel { color, levels: n } => {
                if levels.get(*color) >= MAX_COLOR_LEVEL {
                    return Err(WheelError::MaxLevel(*color));
                }
                levels.level_up(*color, *n);
                if *color == SlotColor::Green {
                    self.apply_green_ladder(levels.get(SlotColor::Green));
                }
            }
        }
        self.upgrades.push(upgrade.id.clone());
        Ok(())
    }
}
