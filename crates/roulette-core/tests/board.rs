//! Phase 3 tests (TASK-023): effective-color precedence, green ladder, bet win
//! matrix vs §4.2, customizer invariants (§4.8), board upgrades (§4.7) and the
//! PAT-002 modifier stack lifecycle.

use roulette_content::schema::{
    BoardUpgradeDef, BoardUpgradeEffect, PayoutTable, SlotColor, ZoneKind,
};
use roulette_core::bets::{Bet, BetError, BetType, BASE_COLUMN, BASE_DOZEN};
use roulette_core::board::{ModifierEntry, ModifierKind, ModifierScope, ModifierStack, MultTarget};
use roulette_core::wheel::{green_ladder_slots, ColorLevels, WheelConfig, MAX_COLOR_LEVEL};

fn payout_table(red: f32, black: f32, green: f32, number: f32) -> PayoutTable {
    PayoutTable {
        red,
        black,
        green,
        number,
        odd: 2.0,
        even: 2.0,
        gold: None,
        purple: None,
        cyan: None,
        crimson: None,
    }
}

/// Minimal 13-slot wheel (§4.6 dozen template) with slot 6 purple, 7 gold.
fn small_wheel() -> WheelConfig {
    WheelConfig {
        id: "dozen".into(),
        name: "The Devil's Dozen".into(),
        description: "13 slots".into(),
        numbers: vec![0, 9, 2, 7, 4, 5, 12, 1, 10, 3, 8, 11, 6],
        green_numbers: vec![0],
        red_numbers: Vec::new(),
        black_numbers: Vec::new(),
        slot_colors: [(6u32, SlotColor::Purple), (7, SlotColor::Gold)].into(),
        payout_multipliers: payout_table(2.0, 2.0, 6.0, 12.0),
        upgrades: Vec::new(),
        rarity: roulette_content::schema::WheelRarity::Common,
        points_cost: None,
        lucky_numbers: Default::default(),
        physics_mods: Default::default(),
        unlocked_abilities: Default::default(),
    }
}

fn levels() -> ColorLevels {
    ColorLevels::default()
}

// ---------------------------------------------------------------------------
// TASK-023a: effective color precedence (convert > paint > base)
// ---------------------------------------------------------------------------

#[test]
fn base_colors_follow_template_lists() {
    let wheel = small_wheel();
    assert_eq!(wheel.effective_color(0, 0, None), SlotColor::Green);
    assert_eq!(wheel.effective_color(9, 0, None), SlotColor::Red);
    assert_eq!(wheel.effective_color(2, 0, None), SlotColor::Black);
    // Specials override red/black base.
    assert_eq!(wheel.effective_color(7, 0, None), SlotColor::Gold);
    assert_eq!(wheel.effective_color(6, 0, None), SlotColor::Purple);
}

#[test]
fn paint_overrides_base_and_convert_overrides_paint() {
    let wheel = small_wheel();
    // Paint 9 → black (round scope).
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "paint_card".into(),
        marked_slots: vec![9],
        kind: ModifierKind::ConvertSlots { to: SlotColor::Black, numbers: vec![9] },
        scope: ModifierScope::Round,
    });
    let board = stack.snapshot();
    assert_eq!(wheel.effective_color(9, 0, Some(&board)), SlotColor::Black);

    // Fight-scope convert 9 → gold beats the paint.
    stack.push(ModifierEntry {
        source: "baptism".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::ConvertSlots { to: SlotColor::Red, numbers: vec![9] },
        scope: ModifierScope::Fight,
    });
    let board = stack.snapshot();
    assert_eq!(wheel.effective_color(9, 0, Some(&board)), SlotColor::Red);
}

#[test]
fn green_numbers_are_sticky_under_specials_and_converts_respect_them() {
    let wheel = small_wheel();
    assert_eq!(wheel.effective_color(0, 0, None), SlotColor::Green);
    // Ladder green claims plain red/black slots but not specials.
    let mut levels = ColorLevels::default();
    levels.level_up(SlotColor::Green, 1);
    assert_eq!(wheel.effective_color(32, levels.get(SlotColor::Green), None), SlotColor::Green);
    assert_eq!(wheel.effective_color(7, levels.get(SlotColor::Green), None), SlotColor::Gold);
}

// ---------------------------------------------------------------------------
// TASK-023b: extra-green ladder (§4.4)
// ---------------------------------------------------------------------------

#[test]
fn green_ladder_thresholds() {
    assert!(green_ladder_slots(0).is_empty());
    assert_eq!(green_ladder_slots(1), &[32]);
    assert_eq!(green_ladder_slots(2), &[32, 11, 22]);
    assert_eq!(green_ladder_slots(3), &[32, 11, 22]);
    assert_eq!(green_ladder_slots(4), &[32, 11, 22, 5, 17, 29]);
    // Monotone: every slot at a lower rung stays green at higher rungs.
    for level in 5..=10u8 {
        assert_eq!(green_ladder_slots(level), green_ladder_slots(4));
    }
}

#[test]
fn ladder_greens_win_green_bets_and_exclude_parity() {
    let wheel = small_wheel();
    let mut levels = ColorLevels::default();
    levels.level_up(SlotColor::Green, 2); // 32, 11, 22 green
    let board = None;
    // 32 is red canonically; ladder makes the green bet win.
    assert!(BetType::Green.is_win_on(&wheel, 32, &levels, board));
    assert!(BetType::Green.is_win_on(&wheel, 11, &levels, board));
    // Parity bets skip ladder greens (11 odd, 22 even — both excluded).
    assert!(!BetType::Odd.is_win_on(&wheel, 11, &levels, board));
    assert!(!BetType::Even.is_win_on(&wheel, 22, &levels, board));
    // A red bet on 32 loses once the ladder claims it.
    assert!(!BetType::Red.is_win_on(&wheel, 32, &levels, board));
}

// ---------------------------------------------------------------------------
// TASK-023c: bet win matrix vs §4.2
// ---------------------------------------------------------------------------

#[test]
fn bet_win_matrix_matches_section_4_2() {
    let wheel = small_wheel();
    let lv = ColorLevels::default();
    let none: Option<&roulette_core::board::BoardModifiers> = None;
    let win = |t: &BetType, landed: u32| t.is_win_on(&wheel, landed, &lv, none);

    // Red/black vs effective color (specials override base).
    assert!(win(&BetType::Red, 9));
    assert!(win(&BetType::Black, 2));
    assert!(!win(&BetType::Red, 0));
    assert!(!win(&BetType::Black, 9));
    // Slot 7 is gold → not red.
    assert!(!win(&BetType::Red, 7));
    assert!(win(&BetType::Gold, 7));
    assert!(win(&BetType::Purple, 6));

    // Odd/even: 0 and greens excluded.
    assert!(win(&BetType::Odd, 9));
    assert!(win(&BetType::Even, 2));
    assert!(!win(&BetType::Odd, 0));
    assert!(!win(&BetType::Even, 0));

    // Number.
    assert!(win(&BetType::Number(7), 7));
    assert!(!win(&BetType::Number(7), 8));

    // Dozen: ceil(n/12), 1..=36 only.
    assert!(win(&BetType::Dozen(1), 1));
    assert!(win(&BetType::Dozen(1), 12));
    assert!(!win(&BetType::Dozen(1), 13));
    assert!(win(&BetType::Dozen(2), 13));
    assert!(win(&BetType::Dozen(2), 24));
    assert!(win(&BetType::Dozen(3), 25));
    assert!(win(&BetType::Dozen(3), 36));
    assert!(!win(&BetType::Dozen(1), 0));
    assert!(!win(&BetType::Dozen(4), 37));

    // Column: ((n−1) mod 3) + 1.
    assert!(win(&BetType::Column(1), 1));
    assert!(win(&BetType::Column(2), 2));
    assert!(win(&BetType::Column(3), 3));
    assert!(win(&BetType::Column(1), 4));
    assert!(!win(&BetType::Column(2), 4));
    assert!(!win(&BetType::Column(1), 0));

    // Green: base green + Emerald Forest primes.
    assert!(win(&BetType::Green, 0));
    assert!(!win(&BetType::Green, 3));
}

#[test]
fn mirror_slot_wins_number_bets() {
    let wheel = small_wheel();
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "mirror_mark".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::ZoneMark {
            kind: roulette_content::schema::ZoneKind::Mirror,
            slots: vec![5],
            value: Some(9),
        },
        scope: ModifierScope::Fight,
    });
    let board = stack.snapshot();
    let lv = ColorLevels::default();
    // Landed on 5; mirror target 9 → a bet on 9 wins; unrelated numbers lose.
    assert!(BetType::Number(9).is_win_on(&wheel, 5, &lv, Some(&board)));
    assert!(BetType::Number(5).is_win_on(&wheel, 5, &lv, Some(&board))); // plain match
    assert!(!BetType::Number(8).is_win_on(&wheel, 5, &lv, Some(&board)));
}

#[test]
fn bet_construction_validates_inputs() {
    let wheel = small_wheel();
    assert!(matches!(Bet::new(BetType::Dozen(4), 1), Err(BetError::DozenRange(4))));
    assert!(matches!(Bet::new(BetType::Column(0), 1), Err(BetError::ColumnRange(0))));
    assert!(matches!(Bet::number_on(&wheel, 100, 1), Err(BetError::NotOnWheel(100))));
    assert!(matches!(Bet::new(BetType::Red, 0), Err(BetError::ZeroAmount)));
    assert!(Bet::number_on(&wheel, 7, 5).is_ok());
}

// ---------------------------------------------------------------------------
// TASK-023d: payout base table + §4.5 level scaling
// ---------------------------------------------------------------------------

#[test]
fn payout_base_table_is_data_driven() {
    let wheel = small_wheel(); // 2/2/6/12 table
    let lv = ColorLevels::default();
    assert_eq!(BetType::Red.base(&wheel, &lv), 2.0);
    assert_eq!(BetType::Black.base(&wheel, &lv), 2.0);
    assert_eq!(BetType::Green.base(&wheel, &lv), 6.0);
    assert_eq!(BetType::Number(3).base(&wheel, &lv), 12.0);
    assert_eq!(BetType::Odd.base(&wheel, &lv), 2.0);
    assert_eq!(BetType::Dozen(2).base(&wheel, &lv), BASE_DOZEN);
    assert_eq!(BetType::Column(3).base(&wheel, &lv), BASE_COLUMN);
    // Special defaults when the table omits them: 4/4/4/6 (§4.2, §10.1).
    assert_eq!(BetType::Gold.base(&wheel, &lv), 4.0);
    assert_eq!(BetType::Purple.base(&wheel, &lv), 4.0);
    assert_eq!(BetType::Cyan.base(&wheel, &lv), 4.0);
    assert_eq!(BetType::Crimson.base(&wheel, &lv), 6.0);
}

#[test]
fn color_levels_scale_payouts_per_section_4_5() {
    let wheel = small_wheel();
    let mut lv = ColorLevels::default();
    lv.level_up(SlotColor::Red, 3);
    lv.level_up(SlotColor::Green, 2);
    lv.level_up(SlotColor::Gold, 4);
    // red 2.0 + 0.2×3; green 6.0 + 2.0×2; gold 4.0 + 0.5×4.
    assert_eq!(BetType::Red.base(&wheel, &lv), 2.0 + 0.6);
    assert_eq!(BetType::Green.base(&wheel, &lv), 6.0 + 4.0);
    assert_eq!(BetType::Gold.base(&wheel, &lv), 4.0 + 2.0);
    // Odd/even/number/dozen are not level-scaled.
    assert_eq!(BetType::Odd.base(&wheel, &lv), 2.0);
    assert_eq!(BetType::Number(3).base(&wheel, &lv), 12.0);
    assert_eq!(lv.get(SlotColor::Black), 0);
    // Cap at 10.
    lv.level_up(SlotColor::Red, u8::MAX);
    assert_eq!(lv.get(SlotColor::Red), MAX_COLOR_LEVEL);
}

// ---------------------------------------------------------------------------
// TASK-023e: customizer invariants (§4.8)
// ---------------------------------------------------------------------------

#[test]
fn customizer_enforces_two_slot_minimum_and_unique_numbers() {
    let mut wheel = small_wheel();
    // Remove down to 2 slots, then the next removal must fail.
    while wheel.len() > 2 {
        wheel.remove_slot(wheel.len() - 1).unwrap();
    }
    assert_eq!(wheel.len(), 2);
    assert!(matches!(wheel.remove_slot(0), Err(roulette_core::wheel::WheelError::TooFewSlots)));
    // Unique numbers enforced for add + renumber.
    assert!(matches!(wheel.add_slot(0), Err(roulette_core::wheel::WheelError::DuplicateNumber(0))));
    wheel.add_slot(99).unwrap();
    assert!(matches!(
        wheel.set_slot_number(2, 0),
        Err(roulette_core::wheel::WheelError::DuplicateNumber(0))
    ));
    wheel.set_slot_number(2, 77).unwrap();
    assert!(wheel.numbers.contains(&77));
    assert!(matches!(
        wheel.remove_slot(wheel.len()),
        Err(roulette_core::wheel::WheelError::SlotIndex(3, 3))
    ));
}

#[test]
fn cycle_slot_color_walks_the_seven_colors() {
    let mut wheel = small_wheel();
    // 9 is red canonically.
    let seq = [
        SlotColor::Black,
        SlotColor::Green,
        SlotColor::Gold,
        SlotColor::Purple,
        SlotColor::Cyan,
        SlotColor::Crimson,
        SlotColor::Red,
    ];
    let idx = wheel.numbers.iter().position(|&n| n == 9).unwrap();
    for expected in seq {
        assert_eq!(wheel.cycle_slot_color(idx).unwrap(), expected);
        assert_eq!(wheel.effective_color(9, 0, None), expected);
    }
    // Cycle wrote consistent bookkeeping: 9 ended red (explicit), no stray marks.
    assert!(wheel.red_numbers.contains(&9));
    assert!(!wheel.green_numbers.contains(&9));
    assert!(!wheel.slot_colors.contains_key(&9));
}

#[test]
fn forced_non_canonical_colors_are_honored() {
    // red_heavy-style wheel: 2 forced red though canonically black (§4.6).
    let mut wheel = small_wheel();
    wheel.set_slot_color(2, SlotColor::Red);
    assert_eq!(wheel.effective_color(2, 0, None), SlotColor::Red);
    assert!(BetType::Red.is_win_on(&wheel, 2, &levels(), None));
    assert!(!BetType::Black.is_win_on(&wheel, 2, &levels(), None));
    // A template special still outranks an explicit list entry.
    assert_eq!(wheel.effective_color(7, 0, None), SlotColor::Gold);
}

#[test]
fn customizer_bookkeeping_stays_consistent() {
    let mut wheel = small_wheel();
    // Paint 4 gold, then renumber its slot: color follows the number.
    let idx = wheel.numbers.iter().position(|&n| n == 4).unwrap();
    wheel.set_slot_color(4, SlotColor::Cyan);
    assert_eq!(wheel.effective_color(4, 0, None), SlotColor::Cyan);
    wheel.set_slot_number(idx, 44).unwrap();
    assert_eq!(wheel.effective_color(44, 0, None), SlotColor::Cyan);
    assert!(!wheel.slot_colors.contains_key(&4));
    // Removing a slot cleans up its color marks.
    let idx44 = wheel.numbers.iter().position(|&n| n == 44).unwrap();
    wheel.remove_slot(idx44).unwrap();
    assert!(!wheel.slot_colors.contains_key(&44));
    assert!(!wheel.numbers.contains(&44));
}

// ---------------------------------------------------------------------------
// TASK-023f: board upgrades (§4.7)
// ---------------------------------------------------------------------------

fn upgrade(id: &str, effect: BoardUpgradeEffect) -> BoardUpgradeDef {
    BoardUpgradeDef { id: id.into(), name: id.into(), description: String::new(), cost: 10, effect }
}

#[test]
fn board_upgrades_apply_all_effect_kinds() {
    use roulette_content::schema::BoardUpgradeEffect as E;
    let mut wheel = small_wheel();
    let mut lv = ColorLevels::default();

    wheel
        .apply_upgrade(
            &mut lv,
            &upgrade("red_boost", E::MultiplierBoost { color: SlotColor::Red, amount: 0.5 }),
        )
        .unwrap();
    assert_eq!(wheel.payout_multipliers.red, 2.5);

    wheel.apply_upgrade(&mut lv, &upgrade("add_green", E::AddGreenSlot { number: 9 })).unwrap();
    assert!(wheel.green_numbers.contains(&9));
    assert_eq!(wheel.effective_color(9, 0, None), SlotColor::Green);

    wheel
        .apply_upgrade(
            &mut lv,
            &upgrade("baptism", E::ConvertToColor { number: 2, to: SlotColor::Crimson }),
        )
        .unwrap();
    assert_eq!(wheel.effective_color(2, 0, None), SlotColor::Crimson);

    wheel
        .apply_upgrade(&mut lv, &upgrade("lucky7", E::LuckyNumber { number: 7, heal: 6 }))
        .unwrap();
    assert_eq!(wheel.lucky_numbers.get(&7), Some(&6));

    wheel
        .apply_upgrade(
            &mut lv,
            &upgrade("ghost", E::PhysicsMod { mass_mult: Some(0.8), friction_mult: None }),
        )
        .unwrap();
    wheel
        .apply_upgrade(
            &mut lv,
            &upgrade("tar", E::PhysicsMod { mass_mult: None, friction_mult: Some(1.25) }),
        )
        .unwrap();
    assert!((wheel.physics_mods.ball_mass_mult - 0.8).abs() < 1e-6);
    assert!((wheel.physics_mods.friction_mult - 1.25).abs() < 1e-6);

    wheel
        .apply_upgrade(&mut lv, &upgrade("fever", E::UnlockAbility { color: SlotColor::Red }))
        .unwrap();
    assert!(wheel.unlocked_abilities.contains(&SlotColor::Red));

    wheel
        .apply_upgrade(
            &mut lv,
            &upgrade("planet_green", E::ColorLevel { color: SlotColor::Green, levels: 1 }),
        )
        .unwrap();
    assert_eq!(lv.get(SlotColor::Green), 1);
    // Ladder applied: 32 joined green_numbers on the wheel.
    assert!(wheel.green_numbers.contains(&32));
    // All upgrade ids recorded, in order.
    assert_eq!(
        wheel.upgrades,
        vec![
            "red_boost",
            "add_green",
            "baptism",
            "lucky7",
            "ghost",
            "tar",
            "fever",
            "planet_green"
        ]
    );
    // Max-level ColorLevel errors.
    lv.level_up(SlotColor::Green, u8::MAX);
    assert!(matches!(
        wheel.apply_upgrade(
            &mut lv,
            &upgrade("planet_green2", E::ColorLevel { color: SlotColor::Green, levels: 1 })
        ),
        Err(roulette_core::wheel::WheelError::MaxLevel(SlotColor::Green))
    ));
}

// ---------------------------------------------------------------------------
// TASK-023g: modifier stack lifecycle (PAT-002)
// ---------------------------------------------------------------------------

#[test]
fn stack_snapshot_folds_entries_into_board_state() {
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "lucky_index".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::GlobalMult { mult: 1.2 },
        scope: ModifierScope::Spins(3),
    });
    stack.push(ModifierEntry {
        source: "lucky_index2".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::GlobalMult { mult: 1.5 },
        scope: ModifierScope::Fight,
    });
    stack.push(ModifierEntry {
        source: "zone".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::ZoneMark { kind: ZoneKind::Lucky, slots: vec![3, 9], value: None },
        scope: ModifierScope::Fight,
    });
    stack.push(ModifierEntry {
        source: "mine".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::ZoneMark { kind: ZoneKind::ChipMine, slots: vec![5], value: Some(12) },
        scope: ModifierScope::Round,
    });
    let board = stack.snapshot();
    assert!((board.global_multiplier - 1.8).abs() < 1e-6);
    assert_eq!(board.zone(ZoneKind::Lucky).unwrap().len(), 2);
    assert_eq!(board.zone_value(ZoneKind::ChipMine, 5), 12);
    assert!(!board.double_next_payout);
    assert!(!board.emerald_forest_active);
}

#[test]
fn stack_tick_expires_spins_per_spin_and_rounds_per_round() {
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "greed".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::PayoutMult { target: MultTarget::Green, mult: 50.0 },
        scope: ModifierScope::Spins(3),
    });
    stack.push(ModifierEntry {
        source: "paint".into(),
        marked_slots: vec![2, 9],
        kind: ModifierKind::ConvertSlots { to: SlotColor::Purple, numbers: vec![2, 9] },
        scope: ModifierScope::Round,
    });
    stack.push(ModifierEntry {
        source: "heist".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::GoldenHeist { amount: 30 },
        scope: ModifierScope::Fight,
    });

    stack.tick_at_round_end(); // Round cleared; Spins counts per-spin
    assert_eq!(stack.len(), 2);
    let board = stack.snapshot();
    assert!(board.paints.is_empty());
    assert!(board.golden_heist_active);

    stack.tick_after_spin(); // 3→2
    stack.tick_after_spin(); // 2→1
    stack.tick_after_spin(); // expired
    assert_eq!(stack.len(), 1);
    assert_eq!(stack.entries()[0].source, "heist");
    // Payout mult query folds only live entries.
    assert_eq!(stack.payout_mult(&MultTarget::Green), 1.0);
}

#[test]
fn one_shots_are_consumed_once() {
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "double_payout".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::DoubleNextPayout,
        scope: ModifierScope::NextWin,
    });
    stack.push(ModifierEntry {
        source: "insurance".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::Insurance,
        scope: ModifierScope::Spin,
    });
    let board = stack.snapshot();
    assert!(board.double_next_payout);
    assert!(board.insurance_active);
    let taken = stack.take_next_win_oneshots();
    assert_eq!(taken, vec![ModifierKind::DoubleNextPayout]);
    assert_eq!(stack.len(), 1); // Insurance (Spin) still live until spin end.
    stack.clear_spin_scoped();
    assert!(stack.is_empty());
    let board = stack.snapshot();
    assert!(!board.double_next_payout);
}

#[test]
fn custom_number_multipliers_fold_and_scope() {
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "lucky_seven".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::CustomNumberMult { numbers: vec![7], mult: 200.0 },
        scope: ModifierScope::Spins(2),
    });
    stack.push(ModifierEntry {
        source: "devil_trio".into(),
        marked_slots: Vec::new(),
        kind: ModifierKind::CustomNumberMult { numbers: vec![6, 7, 8], mult: 250.0 },
        scope: ModifierScope::Fight,
    });
    let board = stack.snapshot();
    // 7 gets both; 6 gets only the trio.
    assert!((board.custom_number_multipliers[&7] - 50_000.0).abs() < 1e-3);
    assert!((board.custom_number_multipliers[&6] - 250.0).abs() < 1e-3);
    stack.tick_after_spin(); // 2→1
    stack.tick_after_spin(); // lucky_seven expires
    let board = stack.snapshot();
    assert!((board.custom_number_multipliers[&7] - 250.0).abs() < 1e-3);
}

#[test]
fn paint_marked_slots_are_kept_on_entries() {
    let mut stack = ModifierStack::new();
    stack.push(ModifierEntry {
        source: "purple_paint".into(),
        marked_slots: vec![4, 5],
        kind: ModifierKind::ConvertSlots { to: SlotColor::Purple, numbers: vec![4, 5] },
        scope: ModifierScope::Round,
    });
    assert_eq!(stack.entries()[0].marked_slots, vec![4, 5]);
    let board = stack.snapshot();
    assert_eq!(board.color_of_converted(4, false), Some(SlotColor::Purple));
}
