//! Board modifiers (TASK-020) and the modifier stack (TASK-021, PAT-002).
//!
//! [`ModifierStack`] is the source of truth for every *timed* card/board effect:
//! entries carry a typed kind, a duration and (for paint cards) the player's
//! `marked_slots`. [`BoardModifiers`] is the derived snapshot the bet/damage
//! pipeline consumes — no loose booleans; everything on it is either folded from
//! the stack or explicit battle state (streak counters).

use std::collections::{BTreeMap, BTreeSet};

use roulette_content::schema::{PayoutTarget, SlotColor, ZoneKind};
use serde::{Deserialize, Serialize};

/// Which payout-table column a [`ModifierKind::PayoutMult`] targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultTarget {
    Red,
    Black,
    Green,
    Gold,
    Purple,
    Cyan,
    Crimson,
    SingleNumber,
    Number(u32),
    Odd,
    Even,
    Prime,
    Low,
    High,
    Dozen(u8),
    Column(u8),
}

impl From<PayoutTarget> for MultTarget {
    fn from(t: PayoutTarget) -> Self {
        match t {
            PayoutTarget::Red => Self::Red,
            PayoutTarget::Black => Self::Black,
            PayoutTarget::Green => Self::Green,
            PayoutTarget::Gold => Self::Gold,
            PayoutTarget::Purple => Self::Purple,
            PayoutTarget::Cyan => Self::Cyan,
            PayoutTarget::Crimson => Self::Crimson,
            PayoutTarget::SingleNumber => Self::SingleNumber,
            PayoutTarget::Number(n) => Self::Number(n),
            PayoutTarget::Odd => Self::Odd,
            PayoutTarget::Even => Self::Even,
            PayoutTarget::Prime => Self::Prime,
            PayoutTarget::Low => Self::Low,
            PayoutTarget::High => Self::High,
            PayoutTarget::Dozen(d) => Self::Dozen(d),
            PayoutTarget::Column(c) => Self::Column(c),
        }
    }
}

/// Scope of a timed entry, mirroring `DurationKind` / `ConvertScope` from the DSL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierScope {
    /// Until the fight ends.
    Fight,
    /// Expires after `n` spins (tick_at_round_end).
    Spins(u8),
    /// Until the end of the current round (paint cards).
    Round,
    /// Only the next spin.
    Spin,
    /// Only the next winning bet (one-shot).
    NextWin,
}

/// A resolved, typed modifier entry. Card DSL `NumberSet`s (including
/// `RandomSlots`/`PlayerChoice`) are resolved to explicit numbers at play time,
/// so the stack holds only concrete data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModifierEntry {
    /// Source card / upgrade id.
    pub source: String,
    /// Paint-card player-chosen slots (§6.3 `markedSlots`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marked_slots: Vec<u32>,
    pub kind: ModifierKind,
    pub scope: ModifierScope,
}

/// Typed timed-effect vocabulary (PAT-002). One variant per stackable mechanic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModifierKind {
    PayoutMult {
        target: MultTarget,
        mult: f32,
    },
    CustomNumberMult {
        numbers: Vec<u32>,
        mult: f32,
    },
    ConvertSlots {
        to: SlotColor,
        numbers: Vec<u32>,
    },
    SwapRedBlack,
    ZoneMark {
        kind: ZoneKind,
        slots: Vec<u32>,
        value: Option<u16>,
    },
    GlobalMult {
        mult: f32,
    },
    DoubleNextPayout,
    /// Refund all bet chips on the next lost spin.
    Insurance,
    /// Primes count as green; green payout ×2 (§10.1).
    EmeraldForest,
    /// +chips whenever a gold slot lands.
    GoldenHeist {
        amount: u16,
    },
    /// Chips pool drains this much per spin (Risk Capital).
    RiskCapitalDrain {
        per_spin: u16,
    },
    /// Reroll one 0-damage spin.
    LuckyCharm,
}

/// Derived per-spin board state consumed by the bet/damage pipeline (§16.3).
///
/// Timed fields are folded in by [`ModifierStack::snapshot`]; streak counters and
/// one-shot consumption flags are battle-owned state (§10.3) mutated in place.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BoardModifiers {
    /// Exact-number payout overrides (e.g. LUCKY_SEVEN 7 → 200).
    pub custom_number_multipliers: BTreeMap<u32, f32>,
    /// Mirror slots: landed number → mirrored target number (§10.1 mirror wins).
    pub mirror_slots: BTreeMap<u32, u32>,
    /// Fight-scope slot converts (e.g. Blood Baptism). Highest precedence.
    pub converts: BTreeMap<SlotColor, BTreeSet<u32>>,
    /// Round/spin-scope paint recolors. Yields to `converts`.
    pub paints: BTreeMap<SlotColor, BTreeSet<u32>>,
    /// Red↔black monochrome swap (Monochrome card): active when set.
    pub swap_red_black: bool,
    /// Zone-marked slots by kind (§10.4 zone triggers).
    pub zone_marks: BTreeMap<ZoneKind, BTreeSet<u32>>,
    /// Zone payloads (chip-mine chips, fountain heal, mirror target), keyed
    /// by (kind, slot).
    pub zone_values: BTreeMap<(ZoneKind, u32), u16>,
    /// LUCKY_INDEX-style global damage multiplier (folded product, base 1.0).
    pub global_multiplier: f32,
    /// Card-armed payout boosts folded by target (GREEN_GREED ×50 etc.).
    pub payout_multipliers: BTreeMap<MultTarget, f32>,
    // --- battle-owned streak state (§10.3) ---
    pub red_streak_count: u8,
    pub black_streak_count: u8,
    pub red_streak_active: bool,
    pub black_streak_active: bool,
    // --- battle-owned flags/one-shots ---
    pub insurance_active: bool,
    pub risk_capital_active: bool,
    pub risk_capital_drain: u16,
    pub golden_heist_active: bool,
    pub golden_heist_amount: u16,
    pub emerald_forest_active: bool,
    pub double_next_payout: bool,
    /// LUCKY_CHARM rerolls currently available.
    pub lucky_charms: u8,
    /// TURBO_SPIN ×1.5 (consumed by the pipeline).
    pub turbo_active: bool,
    /// Omniscience ×3 when the ball lands in the prediction sector.
    pub omniscience_active: bool,
    /// GREEN_RIPPLE: +5 × green slots on the green multiplier.
    pub green_ripple_active: bool,
    /// HEAVY_NUDGE armed: all-zero spin → +15 chips (§10.4).
    pub heavy_nudge_armed: bool,
    /// STUN_STRIKE armed: ≥5 damage this turn → +2 stun (§10.4).
    pub stun_strike_armed: bool,
    /// BLOCK_RED intent: red bets voided on the next spin (§7.2).
    pub block_red_active: bool,
}

impl BoardModifiers {
    /// Convert-layer color for a number; `fight` selects converts vs paints.
    pub fn color_of_converted(&self, number: u32, fight: bool) -> Option<SlotColor> {
        let layer = if fight { &self.converts } else { &self.paints };
        layer.iter().find(|(_, slots)| slots.contains(&number)).map(|(color, _)| *color)
    }

    /// Slots marked with a zone kind.
    pub fn zone(&self, kind: ZoneKind) -> Option<&BTreeSet<u32>> {
        self.zone_marks.get(&kind)
    }

    /// Zone payload for a slot (0 when the mark carries no value).
    pub fn zone_value(&self, kind: ZoneKind, slot: u32) -> u16 {
        *self.zone_values.get(&(kind, slot)).unwrap_or(&0)
    }

    /// Folded card-armed payout multiplier for a target (§10.1 step 3 zone
    /// multipliers from the card arms HIGH/LOW/dozen/column/prime).
    pub fn payout_multiplier_for(&self, target: &MultTarget) -> f32 {
        self.payout_multipliers.get(target).copied().unwrap_or(1.0)
    }
}

/// Expiry outcomes for a ticked entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expired {
    Yes,
    No,
}

/// PAT-002 modifier stack: every timed effect is an entry with a scope; expiry
/// ticks at round end; one-shots are consumed by the battle layer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModifierStack {
    entries: Vec<ModifierEntry>,
}

impl ModifierStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[ModifierEntry] {
        &self.entries
    }

    /// Pushes a resolved modifier entry.
    pub fn push(&mut self, entry: ModifierEntry) {
        self.entries.push(entry);
    }

    /// Removes every entry whose scope has ended by the end of a round:
    /// `Spins(n)` counts down (removing at 0), `Round`-scoped entries clear.
    pub fn tick_at_round_end(&mut self) {
        self.entries.retain_mut(|entry| match entry.scope {
            ModifierScope::Round => false,
            ModifierScope::Spins(n) => {
                if n <= 1 {
                    false
                } else {
                    entry.scope = ModifierScope::Spins(n - 1);
                    true
                }
            }
            _ => true,
        });
    }

    /// Removes spin-scoped entries (`Spin`/`NextWin`); called at spin
    /// resolution after one-shots have been consumed (Phase 4).
    pub fn clear_spin_scoped(&mut self) {
        self.entries
            .retain(|entry| !matches!(entry.scope, ModifierScope::Spin | ModifierScope::NextWin));
    }

    /// Consumes one-shot entries (`NextWin`), returning their kinds.
    pub fn take_next_win_oneshots(&mut self) -> Vec<ModifierKind> {
        let mut taken = Vec::new();
        self.entries.retain(|entry| {
            if entry.scope == ModifierScope::NextWin {
                taken.push(entry.kind.clone());
                false
            } else {
                true
            }
        });
        taken
    }

    /// Folds the stack into a fresh [`BoardModifiers`] snapshot for the
    /// pipeline. Converts are applied after paints so convert > paint (TASK-023).
    pub fn snapshot(&self) -> BoardModifiers {
        let mut board = BoardModifiers { global_multiplier: 1.0, ..BoardModifiers::default() };
        self.fold_into(&mut board);
        board
    }

    /// Folds the stack's entries on top of an existing board (the battle
    /// pipeline starts from the battle-owned board and adds card arms).
    pub fn fold_into(&self, board: &mut BoardModifiers) {
        // Paint (Round/Spin) first, converts (Fight) second — later layer wins.
        for entry in &self.entries {
            if let ModifierKind::ConvertSlots { to, numbers } = &entry.kind {
                for &n in numbers {
                    board.paints.entry(*to).or_default().insert(n);
                }
            }
        }
        for entry in &self.entries {
            if matches!(entry.scope, ModifierScope::Fight) {
                if let ModifierKind::ConvertSlots { to, numbers } = &entry.kind {
                    for &n in numbers {
                        for slots in board.paints.values_mut() {
                            slots.remove(&n);
                        }
                        board.converts.entry(*to).or_default().insert(n);
                    }
                }
            }
        }
        for entry in &self.entries {
            match &entry.kind {
                ModifierKind::PayoutMult { target, mult } => {
                    *board.payout_multipliers.entry(*target).or_insert(1.0) *= mult;
                }
                ModifierKind::CustomNumberMult { numbers, mult } => {
                    for &n in numbers {
                        *board.custom_number_multipliers.entry(n).or_insert(1.0) *= mult;
                    }
                }
                ModifierKind::ConvertSlots { .. } => {}
                ModifierKind::SwapRedBlack => board.swap_red_black = true,
                ModifierKind::ZoneMark { kind, slots, value } => {
                    let set = board.zone_marks.entry(*kind).or_default();
                    for &slot in slots {
                        set.insert(slot);
                        if let Some(v) = value {
                            board.zone_values.insert((*kind, slot), *v);
                            // Mirror zones drive number-bet mirror wins (§10.1).
                            if *kind == ZoneKind::Mirror {
                                board.mirror_slots.insert(slot, *v as u32);
                            }
                        }
                    }
                }
                ModifierKind::GlobalMult { mult } => board.global_multiplier *= mult,
                ModifierKind::DoubleNextPayout => board.double_next_payout = true,
                ModifierKind::Insurance => board.insurance_active = true,
                ModifierKind::EmeraldForest => board.emerald_forest_active = true,
                ModifierKind::GoldenHeist { amount } => {
                    board.golden_heist_active = true;
                    board.golden_heist_amount = board.golden_heist_amount.max(*amount);
                }
                ModifierKind::RiskCapitalDrain { per_spin } => {
                    board.risk_capital_active = true;
                    board.risk_capital_drain += per_spin;
                }
                ModifierKind::LuckyCharm => board.lucky_charms += 1,
            }
        }
    }

    /// Total payout multiplier targeting `target` from all live entries
    /// (multiplicative fold; generic single-number mults apply to `Number(n)`).
    pub fn payout_mult(&self, target: &MultTarget) -> f32 {
        let mut mult = 1.0;
        for entry in &self.entries {
            let ModifierKind::PayoutMult { target: t, mult: m } = &entry.kind else {
                continue;
            };
            let applies = match (t, target) {
                (a, b) if a == b => true,
                (MultTarget::SingleNumber, MultTarget::Number(_)) => true,
                (MultTarget::Number(n), MultTarget::Number(b)) => n == b,
                _ => false,
            };
            if applies {
                mult *= m;
            }
        }
        mult
    }
}
