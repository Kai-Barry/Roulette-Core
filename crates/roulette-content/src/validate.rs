//! Content validator (TASK-009, REQ-010): fails fast with file + id + reason.

use crate::schema::{
    Content, CurseEffect, EffectKind, ForgeEffect, NumberSet, PayoutTable, SlotColor, WheelDef,
};
use std::collections::HashSet;

/// Validation error taxonomy: every failure names the file, the offending id
/// and the reason (REQ-010).
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("{file}: schema version {found} is not supported (expected {expected})")]
    SchemaVersion { file: String, found: u32, expected: u32 },
    #[error("{file}: RON parse error: {source}")]
    Parse {
        file: String,
        #[source]
        source: ron::error::SpannedError,
    },
    #[error("{file}: IO error: {source}")]
    Io {
        file: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{file}: duplicate id `{id}` in {collection}")]
    DuplicateId { file: String, collection: &'static str, id: String },
    #[error("{file}: `{id}` has an empty/blank {field}")]
    EmptyField { file: String, id: String, field: &'static str },
    #[error("{file}: card `{id}` cost {cost} outside 0..=5")]
    CardCost { file: String, id: String, cost: u8 },
    #[error("{file}: card `{id}` has no effects")]
    NoEffects { file: String, id: String },
    #[error("{file}: wheel `{id}` must have at least 2 slots (has {slots})")]
    WheelTooSmall { file: String, id: String, slots: usize },
    #[error("{file}: wheel `{id}` has duplicate slot number {number}")]
    DuplicateSlotNumber { file: String, id: String, number: u32 },
    #[error("{file}: wheel `{id}` slot {number} is uncolored (must be red, black or green)")]
    SlotUncolored { file: String, id: String, number: u32 },
    #[error("{file}: wheel `{id}` slot {number} is colored inconsistently")]
    SlotDoubleColored { file: String, id: String, number: u32 },
    #[error("{file}: wheel `{id}` payout for `{field}` must be positive (got {value})")]
    PayoutNotPositive { file: String, id: String, field: &'static str, value: f32 },
    #[error("non-finite payout `{field}` {value} in `{file}` for `{id}`")]
    PayoutNotFinite { file: String, id: String, field: &'static str, value: f32 },
    #[error("{file}: wheel `{id}` references unknown upgrade `{upgrade}`")]
    UnknownUpgrade { file: String, id: String, upgrade: String },
    #[error("{file}: enemy `{id}` references unknown wheel `{wheel_id}`")]
    UnknownWheel { file: String, id: String, wheel_id: String },
    #[error("{file}: enemy `{id}` difficulty {difficulty} outside 0.0..=1.0")]
    DifficultyOutOfRange { file: String, id: String, difficulty: f32 },
    #[error("{file}: enemy `{id}` max_hp must be positive")]
    ZeroMaxHp { file: String, id: String },
    #[error("{file}: event `{id}` choice `{choice}` grants unknown card `{card_id}`")]
    UnknownCardRef { file: String, id: String, choice: String, card_id: String },
    #[error("{file}: event `{id}` has no choices")]
    EventNoChoices { file: String, id: String },
    #[error("{file}: `{id}` has a non-positive effect value")]
    UpgradeValueNonPositive { file: String, id: String },
    #[error("{file}: curse `{id}` effect value out of range")]
    CurseValueOutOfRange { file: String, id: String },
    #[error("{file}: forge op `{id}` cost {cost} outside 0..=25")]
    ForgeCost { file: String, id: String, cost: u8 },
    #[error("{file}: `{id}` effect uses an empty number list")]
    EmptyNumberList { file: String, id: String },
}

fn check_ids(
    file: &str,
    collection: &'static str,
    ids: impl IntoIterator<Item = String>,
) -> Result<(), ContentError> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            return Err(ContentError::DuplicateId { file: file.to_string(), collection, id });
        }
    }
    Ok(())
}

fn check_payout_table(file: &str, id: &str, p: &PayoutTable) -> Result<(), ContentError> {
    let fields: [(&'static str, f32); 10] = [
        ("red", p.red),
        ("black", p.black),
        ("green", p.green),
        ("number", p.number),
        ("odd", p.odd),
        ("even", p.even),
        ("gold", p.gold.unwrap_or(1.0)),
        ("purple", p.purple.unwrap_or(1.0)),
        ("cyan", p.cyan.unwrap_or(1.0)),
        ("crimson", p.crimson.unwrap_or(1.0)),
    ];
    for (field, value) in fields {
        if !value.is_finite() {
            return Err(ContentError::PayoutNotFinite {
                file: file.to_string(),
                id: id.to_string(),
                field,
                value,
            });
        }
        if value <= 0.0 {
            return Err(ContentError::PayoutNotPositive {
                file: file.to_string(),
                id: id.to_string(),
                field,
                value,
            });
        }
    }
    Ok(())
}

fn validate_wheel(
    file: &str,
    upgrades: &HashSet<String>,
    w: &WheelDef,
) -> Result<(), ContentError> {
    let id = &w.id;
    if w.numbers.len() < 2 {
        return Err(ContentError::WheelTooSmall {
            file: file.to_string(),
            id: id.clone(),
            slots: w.numbers.len(),
        });
    }
    let mut seen = HashSet::new();
    for &n in &w.numbers {
        if !seen.insert(n) {
            return Err(ContentError::DuplicateSlotNumber {
                file: file.to_string(),
                id: id.clone(),
                number: n,
            });
        }
    }
    let mut green: HashSet<u32> = w.green_numbers.iter().copied().collect();
    let red: HashSet<u32> = w.red_numbers.iter().copied().collect();
    let black: HashSet<u32> = w.black_numbers.iter().copied().collect();
    // Overrides may land on any base color (e.g. slot 7 gold on a red slot);
    // they may additionally grant green to numbers outside green_numbers.
    for (&n, c) in &w.slot_colors {
        match c {
            SlotColor::Green => {
                green.insert(n);
            }
            SlotColor::Gold | SlotColor::Purple | SlotColor::Cyan | SlotColor::Crimson => {}
            SlotColor::Red | SlotColor::Black => {
                return Err(ContentError::SlotDoubleColored {
                    file: file.to_string(),
                    id: id.clone(),
                    number: n,
                })
            }
        }
    }
    for &n in &w.numbers {
        let hits = [green.contains(&n), red.contains(&n), black.contains(&n)];
        if hits.iter().filter(|h| **h).count() > 1 {
            return Err(ContentError::SlotDoubleColored {
                file: file.to_string(),
                id: id.clone(),
                number: n,
            });
        }
        if !hits.iter().any(|h| *h) {
            return Err(ContentError::SlotUncolored {
                file: file.to_string(),
                id: id.clone(),
                number: n,
            });
        }
    }
    check_payout_table(file, id, &w.payout_multipliers)?;
    for upgrade in &w.upgrades {
        if !upgrades.contains(upgrade) {
            return Err(ContentError::UnknownUpgrade {
                file: file.to_string(),
                id: id.clone(),
                upgrade: upgrade.clone(),
            });
        }
    }
    Ok(())
}

fn validate_effect(file: &str, id: &str, effect: &EffectKind) -> Result<(), ContentError> {
    match effect {
        EffectKind::ConvertSlots { numbers: NumberSet::List(list), .. } => {
            if list.is_empty() {
                return Err(ContentError::EmptyNumberList {
                    file: file.to_string(),
                    id: id.to_string(),
                });
            }
        }
        EffectKind::CustomNumberMult { numbers, .. } if numbers.is_empty() => {
            return Err(ContentError::EmptyNumberList {
                file: file.to_string(),
                id: id.to_string(),
            });
        }
        _ => {}
    }
    Ok(())
}

fn validate_card(file: &str, card: &crate::schema::CardDef) -> Result<(), ContentError> {
    if card.id.trim().is_empty() || card.name.trim().is_empty() {
        return Err(ContentError::EmptyField {
            file: file.to_string(),
            id: card.id.clone(),
            field: if card.id.trim().is_empty() { "id" } else { "name" },
        });
    }
    if card.cost > 5 {
        return Err(ContentError::CardCost {
            file: file.to_string(),
            id: card.id.clone(),
            cost: card.cost,
        });
    }
    if card.effects.is_empty() {
        return Err(ContentError::NoEffects { file: file.to_string(), id: card.id.clone() });
    }
    for effect in &card.effects {
        validate_effect(file, &card.id, effect)?;
    }
    Ok(())
}

fn validate_board_upgrade(
    file: &str,
    u: &crate::schema::BoardUpgradeDef,
) -> Result<(), ContentError> {
    use crate::schema::BoardUpgradeEffect::*;
    let positive = match &u.effect {
        MultiplierBoost { amount, .. } => *amount > 0.0,
        LuckyNumber { heal, .. } => *heal > 0,
        PhysicsMod { mass_mult, friction_mult } => {
            mass_mult.is_none_or(|m| m > 0.0) && friction_mult.is_none_or(|f| f > 0.0)
        }
        ColorLevel { levels, .. } => *levels > 0,
        _ => true,
    };
    if !positive {
        return Err(ContentError::UpgradeValueNonPositive {
            file: file.to_string(),
            id: u.id.clone(),
        });
    }
    Ok(())
}

fn validate_curse(file: &str, c: &crate::schema::CurseDef) -> Result<(), ContentError> {
    let in_range = match c.effect {
        CurseEffect::FrictionMultiplier(m) => m > 0.0,
        CurseEffect::FlatDrawCost(v) => v > 0,
        CurseEffect::BetCap(v) => v > 0,
        CurseEffect::HandLimitOverride(v) => v > 0,
        _ => true,
    };
    if !in_range {
        return Err(ContentError::CurseValueOutOfRange {
            file: file.to_string(),
            id: c.id.clone(),
        });
    }
    Ok(())
}

fn validate_enemy(
    file: &str,
    wheel_ids: &HashSet<String>,
    e: &crate::schema::EnemyDef,
) -> Result<(), ContentError> {
    if e.max_hp == 0 {
        return Err(ContentError::ZeroMaxHp { file: file.to_string(), id: e.id.clone() });
    }
    if !(0.0..=1.0).contains(&e.difficulty) {
        return Err(ContentError::DifficultyOutOfRange {
            file: file.to_string(),
            id: e.id.clone(),
            difficulty: e.difficulty,
        });
    }
    if !wheel_ids.contains(&e.wheel_id) {
        return Err(ContentError::UnknownWheel {
            file: file.to_string(),
            id: e.id.clone(),
            wheel_id: e.wheel_id.clone(),
        });
    }
    Ok(())
}

fn validate_event(
    file: &str,
    card_ids: &HashSet<String>,
    ev: &crate::schema::EventDef,
) -> Result<(), ContentError> {
    if ev.choices.is_empty() {
        return Err(ContentError::EventNoChoices { file: file.to_string(), id: ev.id.clone() });
    }
    for choice in &ev.choices {
        for effect in &choice.effects {
            if let EffectKind::GrantCard { card_id } = effect {
                if !card_ids.contains(card_id) {
                    return Err(ContentError::UnknownCardRef {
                        file: file.to_string(),
                        id: ev.id.clone(),
                        choice: choice.id.clone(),
                        card_id: card_id.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn validate_forge_op(file: &str, op: &crate::schema::ForgeOpDef) -> Result<(), ContentError> {
    if op.cost > 25 {
        return Err(ContentError::ForgeCost {
            file: file.to_string(),
            id: op.id.clone(),
            cost: op.cost,
        });
    }
    // §9.2: destructive ops must keep the wheel at >= 2 slots; a count of 0
    // would be a no-op offer.
    let positive = match op.effect {
        ForgeEffect::DestroyRandom { count }
        | ForgeEffect::RemoveColor { count, .. }
        | ForgeEffect::AddColor { count, .. }
        | ForgeEffect::ConvertColor { count, .. } => count > 0,
        _ => true,
    };
    if !positive {
        return Err(ContentError::UpgradeValueNonPositive {
            file: file.to_string(),
            id: op.id.clone(),
        });
    }
    Ok(())
}

/// Validates a content bundle (REQ-010): unique ids, sane costs, wheels >= 2
/// slots with unique fully-colored numbers, positive payouts, and known
/// cross-references. Fails fast with the first error found.
pub fn validate(content: &Content) -> Result<(), ContentError> {
    const FILE: &str = "content";
    if content.schema_version != crate::schema::SCHEMA_VERSION {
        return Err(ContentError::SchemaVersion {
            file: FILE.to_string(),
            found: content.schema_version,
            expected: crate::schema::SCHEMA_VERSION,
        });
    }

    check_ids(FILE, "cards", content.cards.iter().map(|c| c.id.clone()))?;
    check_ids(FILE, "wheels", content.wheels.iter().map(|w| w.id.clone()))?;
    check_ids(FILE, "board_upgrades", content.board_upgrades.iter().map(|u| u.id.clone()))?;
    check_ids(FILE, "curses", content.curses.iter().map(|c| c.id.clone()))?;
    check_ids(FILE, "enemies", content.enemies.iter().map(|e| e.id.clone()))?;
    check_ids(FILE, "events", content.events.iter().map(|e| e.id.clone()))?;
    check_ids(FILE, "forge_ops", content.forge_ops.iter().map(|o| o.id.clone()))?;

    let upgrade_ids: HashSet<String> =
        content.board_upgrades.iter().map(|u| u.id.clone()).collect();
    for wheel in &content.wheels {
        validate_wheel(FILE, &upgrade_ids, wheel)?;
    }
    for card in &content.cards {
        validate_card(FILE, card)?;
    }
    for upgrade in &content.board_upgrades {
        validate_board_upgrade(FILE, upgrade)?;
    }
    for curse in &content.curses {
        validate_curse(FILE, curse)?;
    }
    let wheel_ids: HashSet<String> = content.wheels.iter().map(|w| w.id.clone()).collect();
    for enemy in &content.enemies {
        validate_enemy(FILE, &wheel_ids, enemy)?;
    }
    let card_ids: HashSet<String> = content.cards.iter().map(|c| c.id.clone()).collect();
    for event in &content.events {
        validate_event(FILE, &card_ids, event)?;
    }
    for op in &content.forge_ops {
        validate_forge_op(FILE, op)?;
    }
    Ok(())
}
