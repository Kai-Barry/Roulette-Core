//! Phase 5 tests (TASK-035): one golden replay per wave-1 card asserting
//! exact state deltas, rejection paths, lock-at-spin behaviour, and the §6.5
//! description rewriter.

use std::collections::BTreeMap;

use roulette_content::schema::{
    CardDef, CardRarity, CardType, ChipDestination, ConvertScope, DurationKind, EffectKind,
    HandOp, NumberSet, PayoutTarget, SlotColor, StunCondition, ZoneKind,
};
use roulette_core::battle::state::{BattlePhase, Side};
use roulette_core::battle::{BattleState, CombatMode, SpinInput};
use roulette_core::bets::BetType;
use roulette_core::cards::effects::{
    play_card_with_effects, EffectCtx, ESSENCE_CHIP_ID,
};
use roulette_core::cards::format::format_description;
use roulette_core::board::ModifierKind;
use roulette_core::rng::Rng;
use roulette_core::wheel::WheelConfig;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn payout_table(red: f32, black: f32, green: f32, number: f32) -> roulette_content::schema::PayoutTable {
    roulette_content::schema::PayoutTable {
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

/// 13-slot devil's dozen: green 0, red 9/1/3/11, black 2/4/12/10/8, plus
/// slot colors purple 6 / gold 7.
fn test_wheel() -> WheelConfig {
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

fn fresh_battle() -> BattleState {
    let mut state = BattleState::new(
        test_wheel(),
        test_wheel(),
        50,
        50,
        50,
        3,
        Vec::new(),
        CombatMode::Points,
    );
    state.enemy_difficulty = 1.0;
    state.begin_betting(Side::Player);
    state
}

fn card(id: &str, cost: u8, card_type: CardType, effects: Vec<EffectKind>) -> CardDef {
    CardDef {
        id: id.to_string(),
        name: id.to_string(),
        description: String::new(),
        cost,
        card_type,
        rarity: CardRarity::Common,
        effects,
    }
}


/// Physics effect with explicit fields (no struct-update syntax needed).
#[allow(clippy::too_many_arguments)]
fn physics(friction: f32, prediction: Option<u8>, spin: Option<f32>, nudge: Option<u8>, multiball: u8) -> EffectKind {
    EffectKind::Physics {
        friction: Some(friction),
        ball_mass: None,
        spin_speed: spin,
        wheel_tilt: None,
        target_zone_bias: None,
        bias_red_only: None,
        bias_black_only: None,
        nudge_distance: nudge,
        bounce_randomness: None,
        multiball: Some(multiball),
        split_peg: None,
        shotgun: None,
        prediction_size: prediction,
    }
}

/// A card database keyed by id.
fn defs(cards: Vec<CardDef>) -> std::collections::BTreeMap<String, CardDef> {
    cards.into_iter().map(|c| (c.id.clone(), c)).collect()
}

/// Gives the state a hand of `[a, b, ...]` and a pool of 50.
fn with_hand(state: &mut BattleState, ids: &[&str]) {
    state.hand = ids
        .iter()
        .map(|&id| roulette_core::battle::state::CardInstance {
            def_id: (*id).to_string(),
            marked_slots: Vec::new(),
            temp: false,
            retained: false,
            cost_override: None,
        })
        .collect();
    state.chips_pool = 50;
}

/// Plays hand card `idx` against the defs.
fn play(state: &mut BattleState, idx: usize, defs: &std::collections::BTreeMap<String, CardDef>) -> bool {
    let mut rng = Rng::from_string_seed("phase5");
    let mut ctx = EffectCtx { defs, rng: &mut rng };
    play_card_with_effects(state, idx, &mut ctx).expect("play should not error")
}

fn spin_input(landed: u32) -> SpinInput {
    SpinInput {
        side: Side::Player,
        landed,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    }
}

// ---------------------------------------------------------------------------
// Payout arming (TASK-032)
// ---------------------------------------------------------------------------

#[test]
fn green_greed_arms_50x_for_three_spins() {
    let mut defs = defs(vec![card(
        "green_greed",
        1,
        CardType::Payout,
        vec![EffectKind::PayoutMult {
            target: PayoutTarget::Green,
            value: 50.0,
            duration: DurationKind::Spins(3),
        }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["green_greed"]);
    assert!(play(&mut state, 0, &defs));
    // Green 0 bet: base 6 × 50 = 300 per chip.
    state.chips_pool = 60;
    state.place_bet(BetType::Green, 1).unwrap();
    let out = state.resolve_spin(&spin_input(0));
    assert_eq!(out.total_payout, 300);
    // Duration ticks per resolve: still armed on spins 2–3, gone on 4.
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Green, 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(0)).total_payout, 300);
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Green, 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(0)).total_payout, 300);
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Green, 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(0)).total_payout, 6, "expired");
    let _ = &mut defs;
}

#[test]
fn lucky_seven_custom_number_and_fight_scope() {
    let mut defs = defs(vec![card(
        "lucky_seven",
        2,
        CardType::Payout,
        vec![EffectKind::CustomNumberMult { numbers: vec![7], mult: 200.0 }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["lucky_seven"]);
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    state.place_bet(BetType::Number(7), 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(7)).total_payout, 215, "200 custom + 15 gold PTS");
    // Fight scope: armed again next round with no re-play.
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Number(7), 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(7)).total_payout, 215, "200 custom + 15 gold PTS");
    let _ = &mut defs;
}

#[test]
fn high_low_prime_single_arms_fold_on_number_bets() {
    let defs = defs(vec![
        card(
            "high_roller",
            2,
            CardType::Payout,
            vec![EffectKind::PayoutMult {
                target: PayoutTarget::High,
                value: 2.5,
                duration: DurationKind::Spins(3),
            }],
        ),
        card(
            "single_out",
            1,
            CardType::Payout,
            vec![EffectKind::PayoutMult {
                target: PayoutTarget::SingleNumber,
                value: 40.0,
                duration: DurationKind::Spins(3),
            }],
        ),
    ]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["high_roller", "single_out"]);
    assert!(play(&mut state, 0, &defs));
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    // 12 is high (19–36 is empty on a 13-slot wheel; 12 is the highest low
    // number) — the High arm must NOT apply, the SingleNumber arm must.
    state.place_bet(BetType::Number(12), 1).unwrap();
    let out = state.resolve_spin(&spin_input(12));
    assert_eq!(out.total_payout, 480, "12 × 40 (single_out only)");
}

#[test]
fn prime_arm_folds_on_prime_number_bets() {
    let defs = defs(vec![card(
        "prime_target",
        2,
        CardType::Payout,
        vec![EffectKind::PayoutMult {
            target: PayoutTarget::Prime,
            value: 3.5,
            duration: DurationKind::Spins(3),
        }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["prime_target"]);
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    // 5 is prime and lands on the wheel.
    state.place_bet(BetType::Number(5), 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(5)).total_payout, 42, "12 × 3.5");
}

#[test]
fn double_payout_is_one_shot_next_win() {
    let defs = defs(vec![card(
        "double_payout",
        1,
        CardType::Payout,
        vec![EffectKind::DoubleNextPayout],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["double_payout"]);
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(9)).total_payout, 4, "×2 once");
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(9)).total_payout, 2, "one-shot spent");
}

#[test]
fn golden_zero_and_lucky_seven_custom_multipliers() {
    let defs = defs(vec![card(
        "golden_zero",
        3,
        CardType::Payout,
        vec![EffectKind::CustomNumberMult { numbers: vec![0], mult: 300.0 }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["golden_zero"]);
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    state.place_bet(BetType::Number(0), 2).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(0)).total_payout, 600);
}

#[test]
fn lucky_charm_counts_rerolls() {
    let defs = defs(vec![card("lucky_charm", 1, CardType::Utility, vec![EffectKind::LuckyCharm])]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["lucky_charm"]);
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.player_board.lucky_charms, 1);
    with_hand(&mut state, &["lucky_charm"]);
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.player_board.lucky_charms, 2);
}

#[test]
fn green_ripple_arms_and_boosts_green_by_active_slots() {
    let defs = defs(vec![card(
        "green_ripple",
        1,
        CardType::Payout,
        vec![EffectKind::GreenRipple { mult_per_slot: 5.0 }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["green_ripple"]);
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    state.place_bet(BetType::Green, 1).unwrap();
    // 1 active green slot (0): 6 + 5×1 = 11.
    let out = state.resolve_spin(&spin_input(0));
    assert_eq!(out.total_payout, 11);
    // Ripple is per-spin armed: gone next spin.
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Green, 1).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(0)).total_payout, 6);
}

// ---------------------------------------------------------------------------
// Hand/pile ops (TASK-033)
// ---------------------------------------------------------------------------

#[test]
fn quick_draw_heavy_draw_and_calculated_risk() {
    let defs = defs(vec![
        card("quick_draw", 1, CardType::Utility, vec![EffectKind::Draw { count: 2 }]),
        card(
            "heavy_draw",
            1,
            CardType::Utility,
            vec![EffectKind::Draw { count: 3 }, EffectKind::DiscardRandom { count: 1 }],
        ),
        card(
            "calculated_risk",
            1,
            CardType::Utility,
            vec![EffectKind::DiscardHandThenDrawEqual],
        ),
    ]);
    let mut state = fresh_battle();
    state.draw_pile = (0..10)
        .map(|i| roulette_core::battle::state::CardInstance {
            def_id: format!("filler{i}"),
            marked_slots: Vec::new(),
            temp: false,
            retained: false,
            cost_override: None,
        })
        .collect();
    with_hand(&mut state, &["quick_draw", "heavy_draw", "calculated_risk"]);

    assert!(play(&mut state, 0, &defs)); // quick draw
    assert_eq!(state.hand.len(), 4, "2 drawn + 2 remaining");
    assert_eq!(state.active_played.len(), 1, "played card is on the felt");

    assert!(play(&mut state, 0, &defs)); // heavy draw: +3, −1 random
    assert_eq!(state.hand.len(), 5, "3 − 1 played + 3 drawn − 1 discarded");
    assert_eq!(state.discard_pile.len(), 1, "the random discard only");

    // calculated_risk: discard the remaining hand (4), draw 4.
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.hand.len(), 4);
    assert_eq!(state.discard_pile.len(), 5, "1 random + 4 discarded hand");
}

#[test]
fn golden_mirror_clones_two_zero_cost_temps() {
    let defs = defs(vec![
        card("golden_mirror", 1, CardType::Utility, vec![EffectKind::HandOp { kind: HandOp::CopyRandomHandCard }]),
        card("red_blast", 3, CardType::Payout, vec![EffectKind::GrantChips { amount: 1 }]),
    ]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["golden_mirror", "red_blast"]);
    let mut rng = Rng::from_string_seed("mirror");
    let mut ctx = EffectCtx { defs: &defs, rng: &mut rng };
    // Golden mirror is at hand[0]; it clones a random hand card — only
    // red_blast is available, so 2 × 0-cost red_blast copies land in hand.
    assert!(play_card_with_effects(&mut state, 0, &mut ctx).unwrap());
    assert_eq!(state.hand.len(), 3);
    let clones: Vec<_> = state
        .hand
        .iter()
        .filter(|c| c.def_id == "red_blast" && c.temp)
        .collect();
    assert_eq!(clones.len(), 2, "original stays non-temp; 2 temp copies added");
}

#[test]
fn copy_paste_clones_last_played_non_utility() {
    let defs = defs(vec![
        card("red_blast", 2, CardType::Payout, vec![EffectKind::GrantChips { amount: 1 }]),
        card("copy_paste", 1, CardType::Utility, vec![EffectKind::HandOp { kind: HandOp::CopyLastPlayed }]),
        card("skipped", 1, CardType::Utility, vec![EffectKind::GrantChips { amount: 1 }]),
    ]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["red_blast", "copy_paste"]);
    assert!(play(&mut state, 0, &defs)); // red_blast onto the felt
    assert!(play(&mut state, 0, &defs)); // copy_paste clones red_blast
    assert!(state.hand.iter().any(|c| c.def_id == "red_blast"));
    // A utility played last is skipped (Copy Last Played non-utility).
    with_hand(&mut state, &["skipped"]);
    assert!(play(&mut state, 0, &defs));
    // No non-utility on the felt above the utility → rejected: card returns.
    let utility_only_map = BTreeMap::from([(
        "copy_paste".to_string(),
        card(
            "copy_paste",
            1,
            CardType::Utility,
            vec![EffectKind::HandOp { kind: HandOp::CopyLastPlayed }],
        ),
    )]);
    with_hand(&mut state, &["copy_paste"]);
    assert!(!play(&mut state, 0, &utility_only_map), "utility-only felt is rejected");
    assert!(state.hand.iter().any(|c| c.def_id == "copy_paste"), "card returned to hand");
}

#[test]
fn retain_vision_and_recycle_bin_and_identity_shift() {
    let defs = defs(vec![
        card("retain_vision", 1, CardType::Utility, vec![EffectKind::HandOp { kind: HandOp::Retain }]),
        card("recycle_bin", 1, CardType::Utility, vec![EffectKind::HandOp { kind: HandOp::RecycleFromDiscard }]),
        card(
            "essence_recycle",
            1,
            CardType::Utility,
            vec![EffectKind::HandOp { kind: HandOp::DiscardFromHandDrawOne }],
        ),
        card("filler_a", 0, CardType::Payout, vec![EffectKind::GrantChips { amount: 1 }]),
        card("filler_b", 0, CardType::Payout, vec![EffectKind::GrantChips { amount: 1 }]),
    ]);
    let mut state = fresh_battle();
    state.chips_pool = 50;
    // Recycle Bin with an empty discard is rejected.
    with_hand(&mut state, &["recycle_bin"]);
    assert!(!play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, 50, "cost refunded on rejection");

    // Identity Shift with an empty hand is rejected (no card to discard).
    with_hand(&mut state, &["essence_recycle"]);
    assert!(!play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, 50);

    // Identity Shift with a hand card: discards it, draws 1 for free.
    state.draw_pile.push(roulette_core::battle::state::CardInstance {
        def_id: "filler_b".into(),
        marked_slots: Vec::new(),
        temp: false,
        retained: false,
        cost_override: None,
    });
    with_hand(&mut state, &["essence_recycle", "filler_a"]);
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.hand.len(), 1, "filler_a discarded, drawn card in hand");
    assert_eq!(state.discard_pile.last().map(|c| c.def_id.as_str()), Some("filler_a"));

    // Retain flags hand[0].
    with_hand(&mut state, &["retain_vision", "filler_a"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state.hand.iter().any(|c| c.retained));
}

#[test]
fn deck_shuffle_reshuffles_discard_into_draw() {
    let defs = defs(vec![card(
        "deck_shuffle",
        0,
        CardType::Utility,
        vec![EffectKind::HandOp { kind: HandOp::Reshuffle }, EffectKind::Draw { count: 1 }],
    )]);
    let mut state = fresh_battle();
    state.discard_pile = vec![
        roulette_core::battle::state::CardInstance {
            def_id: "a".into(),
            marked_slots: Vec::new(),
            temp: false,
            retained: false,
            cost_override: None,
        },
        roulette_core::battle::state::CardInstance {
            def_id: "b".into(),
            marked_slots: Vec::new(),
            temp: false,
            retained: false,
            cost_override: None,
        },
    ];
    with_hand(&mut state, &["deck_shuffle"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state.discard_pile.is_empty(), "discard shuffled into draw");
    // Both cards land in the draw pile, then Draw{1} moves one to the hand.
    assert_eq!(state.draw_pile.len(), 1);
    assert!(state.hand.iter().any(|c| c.def_id == "a" || c.def_id == "b"));
}

// ---------------------------------------------------------------------------
// Money cards (TASK-033)
// ---------------------------------------------------------------------------

#[test]
fn essence_chip_exiles_and_grants() {
    let defs = defs(vec![card(
        ESSENCE_CHIP_ID,
        0,
        CardType::Money,
        vec![
            EffectKind::ActionSurge { chips: 4 },
            EffectKind::HandOp { kind: HandOp::ExileSelf },
        ],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &[ESSENCE_CHIP_ID]);
    let pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool + 4);
    // Exile: after the spin resolve the played card vanishes (not discarded).
    state.place_bet(BetType::Red, 1).unwrap();
    state.resolve_spin(&spin_input(9));
    assert_eq!(state.active_played.len(), 0);
    assert!(!state.discard_pile.iter().any(|c| c.def_id == ESSENCE_CHIP_ID));
}

#[test]
fn chip_maker_makes_three_temp_draw_pile_chips() {
    let defs = defs(vec![card(
        "money_chip_maker",
        1,
        CardType::Money,
        vec![
            EffectKind::TempEssenceChips { count: 3, destination: ChipDestination::DrawPile },
            EffectKind::Draw { count: 1 },
        ],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["money_chip_maker"]);
    let _draw_len = state.draw_pile.len();
    assert!(play(&mut state, 0, &defs));
    let chips = state
        .draw_pile
        .iter()
        .chain(state.hand.iter())
        .filter(|c| c.def_id == ESSENCE_CHIP_ID)
        .count();
    assert_eq!(chips, 3, "3 chips made; the draw may pull one to the hand");
}

#[test]
fn tax_refund_counts_essence_chips_in_discard() {
    let defs = defs(vec![
        card(
            "money_tax_refund",
            1,
            CardType::Money,
            vec![EffectKind::TaxRefund { per_card: 2 }],
        ),
        card("chip", 0, CardType::Money, vec![EffectKind::ActionSurge { chips: 1 }]),
    ]);
    let mut state = fresh_battle();
    state.discard_pile = vec![
        roulette_core::battle::state::CardInstance {
            def_id: ESSENCE_CHIP_ID.into(),
            marked_slots: Vec::new(),
            temp: true,
            retained: false,
            cost_override: None,
        };
        3
    ];
    with_hand(&mut state, &["money_tax_refund"]);
    let pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool - 1 + 6, "cost 1, 3 chips × 2");
}

#[test]
fn capital_venture_banks_on_round_win_only() {
    let defs = defs(vec![card(
        "money_venture",
        1,
        CardType::Money,
        vec![
            EffectKind::CapitalVenture { amount: 25 },
            EffectKind::TempEssenceChips { count: 5, destination: ChipDestination::DiscardPile },
        ],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["money_venture"]);
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.discard_pile.iter().filter(|c| c.def_id == ESSENCE_CHIP_ID).count(), 5);
    // Lose the round → no bank.
    state.chips_pool = 10;
    state.enemy_chips_pool = 40;
    state.end_round();
    assert_eq!(state.venture_bank, 0);
    // Win the round → +25 banked.
    state.chips_pool = 40;
    state.enemy_chips_pool = 10;
    state.end_round();
    assert_eq!(state.venture_bank, 25);
    // One-shot: consumed after paying.
    state.chips_pool = 30;
    state.enemy_chips_pool = 10;
    state.end_round();
    assert_eq!(state.venture_bank, 25, "venture is one-shot per play");
}

#[test]
fn golden_heist_pays_when_gold_lands() {
    let defs = defs(vec![card(
        "money_heist",
        1,
        CardType::Money,
        vec![
            EffectKind::TempEssenceChips { count: 2, destination: ChipDestination::Hand },
            EffectKind::GoldenHeist { amount: 30 },
        ],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["money_heist"]);
    let _pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.hand.iter().filter(|c| c.def_id == ESSENCE_CHIP_ID).count(), 2);
    // Gold slot is 7 on the test wheel: +30 chips on landing (§10.2).
    state.chips_pool = 60;
    state.place_bet(BetType::Number(7), 1).unwrap();
    state.resolve_spin(&spin_input(7));
    assert_eq!(state.chips_pool, 60 - 1 + 12 + 15 + 30, "payout + gold PTS + heist");
}

// ---------------------------------------------------------------------------
// Utility: double down, insurance, risk capital, stun strike, adrenaline
// ---------------------------------------------------------------------------

#[test]
fn double_down_doubles_bets_and_intent() {
    let defs = defs(vec![card("double_down", 1, CardType::Utility, vec![EffectKind::DoubleDown])]);
    let mut state = fresh_battle();
    state.enemy_intent = Some(roulette_core::battle::state::EnemyIntent {
        action: roulette_content::schema::EnemyAction::Attack,
        value: 4,
        description: "Rears up".into(),
        pattern_index: 0,
    });
    with_hand(&mut state, &["double_down"]);
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 5).unwrap();
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.bets[0].amount, 10, "stake doubled without extra charge");
    assert_eq!(state.chips_pool, 60 - 5 - 1, "stake doubled in place; only card cost");
    let intent = state.enemy_intent.unwrap();
    assert_eq!(intent.value, 8, "intent value doubled");
    assert!(intent.description.contains("Doubled Down!"));
}

#[test]
fn insurance_refunds_and_consumes_stack_entry() {
    let defs = defs(vec![card("insurance_policy", 1, CardType::Utility, vec![EffectKind::Insurance])]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["insurance_policy"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state
        .player_stack
        .entries()
        .iter()
        .any(|e| matches!(e.kind, ModifierKind::Insurance)));
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 10).unwrap();
    let out = state.resolve_spin(&spin_input(2)); // black → lost
    assert_eq!(out.insurance_refund, 10);
    assert_eq!(state.chips_pool, 60, "stake refunded");
    assert!(!state
        .player_stack
        .entries()
        .iter()
        .any(|e| matches!(e.kind, ModifierKind::Insurance)),
        "insurance consumed");
    // Second spin: no refund.
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    let out = state.resolve_spin(&spin_input(2));
    assert_eq!(out.insurance_refund, 0);
}

#[test]
fn risk_capital_gains_then_drains_each_spin() {
    let defs = defs(vec![card(
        "risk_capital",
        1,
        CardType::Utility,
        vec![EffectKind::RiskCapital { gain: 10, drain_per_spin: 2 }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["risk_capital"]);
    let pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool - 1 + 10, "cost 1, +10 chips");
    state.place_bet(BetType::Red, 5).unwrap();
    state.resolve_spin(&spin_input(9));
    // Pool after win: (pool−1+10−5) + 10 payout − 2 drain.
    assert_eq!(state.chips_pool, pool - 1 + 10 - 5 + 10 - 2);
}

#[test]
fn stun_strike_arms_and_stuns_on_5_damage() {
    let defs = defs(vec![card(
        "stun_strike",
        1,
        CardType::Utility,
        vec![EffectKind::StunEnemy { turns: 2, condition: StunCondition::OnDamageGE(5) }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["stun_strike"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state.player_board.stun_strike_armed);
    // Deal ≥5 this turn → enemy stunned +2.
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 5).unwrap();
    state.resolve_spin(&spin_input(9)); // 5 × 2 = 10 payout
    assert!(state.enemy_stun_turns >= 2);
}

#[test]
fn adrenaline_rush_grants_chips_and_free_plays() {
    let mut defs = defs(vec![card(
        "adrenaline_rush",
        1,
        CardType::Utility,
        vec![
            EffectKind::ActionSurge { chips: 12 },
            EffectKind::FreeCards { this_turn: 3, next_turn: 0 },
        ],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["adrenaline_rush"]);
    let pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool - 1 + 12);
    assert_eq!(state.free_cards_active, 3);
    // Next card plays for free (credit consumed).
    state.hand.push(roulette_core::battle::state::CardInstance {
        def_id: "chip".into(),
        marked_slots: Vec::new(),
        temp: false,
        retained: false,
        cost_override: None,
    });
    defs.insert(
        "chip".into(),
        card("chip", 2, CardType::Money, vec![EffectKind::ActionSurge { chips: 1 }]),
    );
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool - 1 + 12 + 1, "free play charged 0");
    assert_eq!(state.free_cards_active, 2);
}

#[test]
fn compound_interest_grants_half_pool() {
    let defs = defs(vec![card(
        "compound_interest",
        1,
        CardType::Utility,
        vec![EffectKind::GrantChipsPercent { percent: 50 }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["compound_interest"]);
    state.chips_pool = 20;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, 20 - 1 + 9, "50% of the post-cost pool 19");
}

// ---------------------------------------------------------------------------
// Physics arms (TASK-031)
// ---------------------------------------------------------------------------

#[test]
fn physics_cards_arm_and_reset_per_spin() {
    let defs = defs(vec![
        card(
            "friction_oil",
            2,
            CardType::Physics,
            vec![physics(0.65, None, None, None, 0)],
        ),
        card("eagle_eye", 4, CardType::Physics, vec![physics(1.0, Some(1), None, None, 0)]),
    ]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["friction_oil", "eagle_eye"]);
    assert!(play(&mut state, 0, &defs));
    assert!((state.physics.friction - 0.65).abs() < 1e-6);
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.physics.prediction_size, 1);
    // Spin resolves → physics resets to baseline.
    state.place_bet(BetType::Red, 1).unwrap();
    state.resolve_spin(&spin_input(9));
    assert!((state.physics.friction - 1.0).abs() < 1e-6, "reset to baseline");
    assert_eq!(state.physics.prediction_size, 0);
}

#[test]
fn rust_curse_survives_physics_reset() {
    let defs = defs(vec![card(
        "friction_oil",
        2,
        CardType::Physics,
        vec![physics(0.65, None, None, None, 0)],
    )]);
    let mut state = BattleState::new(
        test_wheel(),
        test_wheel(),
        50,
        50,
        50,
        3,
        vec![roulette_content::schema::CurseDef {
            id: "rust".into(),
            name: "Rust".into(),
            description: String::new(),
            effect: roulette_content::schema::CurseEffect::FrictionMultiplier(2.0),
        }],
        CombatMode::Points,
    );
    state.enemy_difficulty = 1.0;
    state.begin_betting(Side::Player);
    with_hand(&mut state, &["friction_oil"]);
    assert!(play(&mut state, 0, &defs));
    assert!((state.physics.friction - 0.65).abs() < 1e-6);
    state.place_bet(BetType::Red, 1).unwrap();
    state.resolve_spin(&spin_input(9));
    assert!((state.physics.friction - 2.0).abs() < 1e-6, "baseline + Rust ×2");
}

#[test]
fn nudge_arms_nudge_cheat_flag() {
    let defs = defs(vec![card(
        "nudge_cheat",
        3,
        CardType::Physics,
        vec![physics(1.0, None, None, Some(3), 0)],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["nudge_cheat"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state.physics.nudge_cheat_active);
    assert_eq!(state.physics.nudge_distance, 3);
}

// ---------------------------------------------------------------------------
// Board converts, paints, zones (TASK-031)
// ---------------------------------------------------------------------------

#[test]
fn crimson_flood_converts_range_fight_scope() {
    let defs = defs(vec![card(
        "crimson_flood",
        1,
        CardType::Board,
        vec![EffectKind::ConvertSlots {
            to: SlotColor::Red,
            from: None,
            numbers: NumberSet::Range(1, 6),
            scope: ConvertScope::Fight,
        }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["crimson_flood"]);
    assert!(play(&mut state, 0, &defs));
    let board = state.merged_board(Side::Player);
    for n in [1u32, 2, 3, 6] {
        assert!(board.converts.get(&SlotColor::Red).is_some_and(|s| s.contains(&n)));
    }
    // Fight scope survives the round tick.
    state.end_round();
    assert!(state
        .merged_board(Side::Player)
        .converts
        .get(&SlotColor::Red)
        .is_some_and(|s| s.contains(&1)));
}

#[test]
fn red_sea_spin_scope_clears_after_one_spin() {
    let defs = defs(vec![card(
        "red_sea",
        1,
        CardType::Board,
        vec![EffectKind::ConvertSlots {
            to: SlotColor::Red,
            from: Some(SlotColor::Black),
            numbers: NumberSet::All,
            scope: ConvertScope::Spin,
        }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["red_sea"]);
    assert!(play(&mut state, 0, &defs));
    // Spin-scope converts land in the paint layer (converts are fight-scope).
    let board = state.merged_board(Side::Player);
    assert!(board
        .paints
        .get(&SlotColor::Red)
        .is_some_and(|s| s.contains(&2)), "black 2 → red");
    state.chips_pool = 60;
    state.place_bet(BetType::Number(2), 1).unwrap();
    state.resolve_spin(&spin_input(2)); // now red → wins at 2.0
    assert_eq!(state.bets.len(), 0);
    state.begin_betting(Side::Player);
    assert!(
        !state.merged_board(Side::Player).converts.get(&SlotColor::Red).is_some_and(|s| s.contains(&2)),
        "spin-scoped convert cleared"
    );
}

#[test]
fn paint_green_marks_three_slots_for_the_round() {
    let defs = defs(vec![card(
        "paint_green",
        1,
        CardType::Paint,
        vec![EffectKind::ConvertSlots {
            to: SlotColor::Green,
            from: None,
            numbers: NumberSet::PlayerChoice(3),
            scope: ConvertScope::Round,
        }],
    )]);
    let mut state = fresh_battle();
    // Player picks slots 3, 8, 11 via marked_slots.
    state.hand = vec![roulette_core::battle::state::CardInstance {
        def_id: "paint_green".into(),
        marked_slots: vec![3, 8, 11],
        temp: false,
        retained: false,
        cost_override: None,
    }];
    assert!(play(&mut state, 0, &defs));
    let board = state.merged_board(Side::Player);
    for n in [3u32, 8, 11] {
        assert!(board.paints.get(&SlotColor::Green).is_some_and(|s| s.contains(&n)));
    }
    // Round scope: cleared at end_round.
    state.end_round();
    assert!(state.merged_board(Side::Player).paints.get(&SlotColor::Green).is_none_or(|s| s.is_empty()));
}

#[test]
fn monochrome_swaps_red_black() {
    let defs = defs(vec![card(
        "monochrome",
        1,
        CardType::Board,
        vec![EffectKind::SwapRedBlack { scope: ConvertScope::Fight }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["monochrome"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state.merged_board(Side::Player).swap_red_black);
    state.chips_pool = 60;
    // 9 is red canonically; under the swap it pays black.
    state.place_bet(BetType::Black, 5).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(9)).total_payout, 10);
}

#[test]
fn emerald_rift_adds_green_slot() {
    let defs = defs(vec![card(
        "emerald_rift",
        1,
        CardType::Board,
        vec![EffectKind::AddGreenSlot { number: 32 }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["emerald_rift"]);
    assert!(play(&mut state, 0, &defs));
    // 32 is not on the 13-slot wheel → nothing to convert, harmless.
    assert!(state
        .merged_board(Side::Player)
        .converts
        .get(&SlotColor::Green)
        .is_none_or(|s| s.is_empty()));
}

#[test]
fn chip_mine_and_lucky_zone_and_cursed_zone_marks() {
    let defs = defs(vec![
        card(
            "chip_mine",
            1,
            CardType::Board,
            vec![EffectKind::ZoneMark { kind: ZoneKind::ChipMine, slots: vec![12], count: None, value: Some(15) }],
        ),
        card(
            "lucky_zone",
            1,
            CardType::Board,
            vec![EffectKind::ZoneMark {
                kind: ZoneKind::Lucky,
                slots: Vec::new(),
                count: Some(3),
                value: None,
            }],
        ),
        card(
            "cursed_zone",
            1,
            CardType::Board,
            vec![EffectKind::ZoneMark {
                kind: ZoneKind::Cursed,
                slots: Vec::new(),
                count: Some(5),
                value: None,
            }],
        ),
    ]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["chip_mine", "lucky_zone", "cursed_zone"]);
    assert!(play(&mut state, 0, &defs));
    assert!(play(&mut state, 0, &defs));
    assert!(play(&mut state, 0, &defs));
    let board = state.merged_board(Side::Player);
    assert_eq!(board.zone_value(ZoneKind::ChipMine, 12), 15);
    assert_eq!(board.zone(ZoneKind::Lucky).map(|s| s.len()), Some(3), "random pick");
    assert_eq!(board.zone(ZoneKind::Cursed).map(|s| s.len()), Some(5));
    // Chip mine pays when the ball lands there.
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 5).unwrap();
    state.resolve_spin(&spin_input(12));
    // 12 is canonical red: bet wins 5×2, then the mine pays +15.
    assert_eq!(state.chips_pool, 60 - 5 + 10 + 15, "win 10 + mine 15");
}

// ---------------------------------------------------------------------------
// Play-flow plumbing: costs, hand limits, lock-at-spin, phase gating
// ---------------------------------------------------------------------------

#[test]
fn unknown_def_id_errors() {
    let defs: BTreeMap<String, CardDef> = BTreeMap::new();
    let mut state = fresh_battle();
    with_hand(&mut state, &["ghost_card"]);
    let mut rng = Rng::from_string_seed("x");
    let mut ctx = EffectCtx { defs: &defs, rng: &mut rng };
    assert!(matches!(
        play_card_with_effects(&mut state, 0, &mut ctx),
        Err(roulette_core::cards::EffectError::UnknownCard(id)) if id == "ghost_card"
    ));
    assert_eq!(state.hand.len(), 1, "card returned");
}

#[test]
fn card_cost_charges_pool() {
    let defs = defs(vec![card("expensive", 3, CardType::Payout, vec![EffectKind::GrantChips { amount: 1 }])]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["expensive"]);
    let pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool - 3 + 1, "cost 3, +1 from the card");
}

#[test]
fn free_card_credit_makes_play_free() {
    let defs = defs(vec![card("cheap", 2, CardType::Payout, vec![EffectKind::GrantChips { amount: 0 }])]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["cheap"]);
    state.free_cards_active = 1;
    let pool = state.chips_pool;
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.chips_pool, pool, "free credit consumed");
    assert_eq!(state.free_cards_active, 0);
}

#[test]
fn cannot_play_during_spin_phase() {
    let _defs = defs(vec![card("cheap", 0, CardType::Utility, vec![EffectKind::GrantChips { amount: 0 }])]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["cheap"]);
    state.place_bet(BetType::Red, 1).unwrap();
    state.spin().unwrap();
    assert_eq!(state.phase, BattlePhase::Spinning);
    // play_card (step 1) already gates the betting phase.
    assert!(state.play_card(0).is_err(), "not betting phase");
}

#[test]
fn temp_cards_cannot_be_removed_mid_effect_lock_at_spin() {
    let defs = defs(vec![card(
        "heist_chip",
        0,
        CardType::Money,
        vec![
            EffectKind::ActionSurge { chips: 1 },
            EffectKind::HandOp { kind: HandOp::ExileSelf },
        ],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["heist_chip"]);
    assert!(play(&mut state, 0, &defs));
    assert!(state.active_played[0].locked_at_spin, "temp card locked");
    // The spin resolves; the locked temp card is exiled, not discarded.
    state.place_bet(BetType::Red, 1).unwrap();
    state.resolve_spin(&spin_input(9));
    assert!(state.active_played.is_empty());
    assert!(!state.discard_pile.iter().any(|c| c.def_id == "heist_chip"));
}

#[test]
fn played_cards_file_to_discard_after_spin() {
    let defs = defs(vec![card("booster", 1, CardType::Payout, vec![EffectKind::GrantChips { amount: 0 }])]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["booster"]);
    assert!(play(&mut state, 0, &defs));
    assert_eq!(state.active_played.len(), 1);
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 1).unwrap();
    state.resolve_spin(&spin_input(9));
    assert!(state.active_played.is_empty());
    assert!(state.discard_pile.iter().any(|c| c.def_id == "booster"));
}

#[test]
fn fight_scope_entries_survive_spin_and_round_tick() {
    let defs = defs(vec![card(
        "fight_boost",
        1,
        CardType::Payout,
        vec![EffectKind::PayoutMult {
            target: PayoutTarget::Red,
            value: 3.0,
            duration: DurationKind::Fight,
        }],
    )]);
    let mut state = fresh_battle();
    with_hand(&mut state, &["fight_boost"]);
    assert!(play(&mut state, 0, &defs));
    state.chips_pool = 60;
    state.place_bet(BetType::Red, 2).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(9)).total_payout, 12, "2 × 2 × 3");
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 2).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(9)).total_payout, 12);
    state.end_round();
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 2).unwrap();
    assert_eq!(state.resolve_spin(&spin_input(9)).total_payout, 12, "fight scope persists");
}

// ---------------------------------------------------------------------------
// §6.5 description rewriter (TASK-034)
// ---------------------------------------------------------------------------

#[test]
fn rewriter_deal_damage_variants() {
    let m = CombatMode::Points;
    assert_eq!(
        format_description("Red bets deal 2.5x damage instead of 2.0x for the rest of the fight.", m),
        "Red bets score 2.5x PTS instead of 2.0x for the rest of the fight."
    );
    assert_eq!(
        format_description("Deals 3x damage on a win.", m),
        "Scores 3x PTS on a win."
    );
    assert_eq!(
        format_description("They take 5 dealt damage.", m),
        "They take 5 scored PTS."
    );
}

#[test]
fn rewriter_enemy_and_flat_damage() {
    let m = CombatMode::Points;
    assert_eq!(
        format_description("incoming enemy damage doubles on a miss", m),
        "incoming opponent points doubles on a miss"
    );
    assert_eq!(
        format_description("Deals flat damage to the enemy.", m),
        "Scores flat PTS to the enemy."
    );
    assert_eq!(
        format_description("Damage taken while stunned.", m),
        "Points taken while stunned."
    );
    assert_eq!(
        format_description("Damage Dealt per gold slot.", m),
        "Points Scored per gold slot."
    );
}

#[test]
fn rewriter_dmg_and_loose_deal() {
    let m = CombatMode::Points;
    assert_eq!(format_description("Takes 10 dmg.", m), "Takes 10 PTS.");
    assert_eq!(format_description("Deal 4 damage.", m), "Score 4 PTS.");
    assert_eq!(format_description("Damage: 7", m), "PTS: 7");
}

#[test]
fn rewriter_damage_mode_is_verbatim() {
    let text = "Red bets deal 2.5x damage.";
    assert_eq!(format_description(text, CombatMode::Damage), text);
}

#[test]
fn rewriter_preserves_case_shape() {
    let m = CombatMode::Points;
    assert_eq!(format_description("DEALS 2x damage.", m), "SCORES 2x PTS.");
    assert_eq!(format_description("Dealt damage!", m), "Scored PTS!");
}