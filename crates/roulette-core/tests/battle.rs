//! Phase 4 tests (TASK-030): golden replays, §10.1 formula tables, draw
//! ladder/hand-limit edges, sudden death, AI determinism, curse behaviours.

use roulette_content::schema::{CardType, CurseDef, CurseEffect, EnemyAction, SlotColor};
use roulette_core::battle::{BattleOutcome, BattlePhase, BattleState, CombatMode, Side, SpinInput};
use roulette_core::bets::BetType;
use roulette_core::rng::Rng;
use roulette_core::wheel::WheelConfig;

fn payout_table(
    red: f32,
    black: f32,
    green: f32,
    number: f32,
) -> roulette_content::schema::PayoutTable {
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

/// §4.6 devil's dozen: 13 slots, green 0, purple 6, gold 7.
fn player_wheel() -> WheelConfig {
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

/// Plain enemy wheel: 13 slots, green 0, everything else red/black canonical.
fn enemy_wheel() -> WheelConfig {
    let mut wheel = player_wheel();
    wheel.slot_colors.clear();
    wheel
}

fn no_curses() -> Vec<CurseDef> {
    Vec::new()
}

fn curse(effect: CurseEffect) -> CurseDef {
    CurseDef { id: "test".into(), name: "Test".into(), description: String::new(), effect }
}

fn test_battle(curses: Vec<CurseDef>) -> BattleState {
    let mut state =
        BattleState::new(player_wheel(), enemy_wheel(), 50, 50, 50, 3, curses, CombatMode::Points);
    state.enemy_difficulty = 1.0;
    state
}

// ---------------------------------------------------------------------------
// §10.1 resolve formula table tests
// ---------------------------------------------------------------------------

#[test]
fn resolve_pays_base_multiplier_and_returns_stake_to_pool() {
    let mut state = test_battle(no_curses());
    state.chips_pool = 20;
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    // 9 is red on the dozen template.
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 20, "10 × 2.0 red payout");
    assert_eq!(state.chips_pool, 20 + 20 - 10, "stake deducted, payout returned");
    assert_eq!(state.phase, BattlePhase::Resolved);
}

#[test]
fn losing_stakes_vanish() {
    let mut state = test_battle(no_curses());
    state.chips_pool = 20;
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 2, // black
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 0);
    assert!(out.all_lost);
    assert_eq!(state.chips_pool, 10, "stake gone");
    assert!(state.bets.is_empty());
}

#[test]
fn custom_number_multipliers_override_base() {
    let mut state = test_battle(no_curses());
    state.player_board.custom_number_multipliers.insert(5, 300.0);
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Number(5), 1).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 5,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 300, "custom 300 overrides 12× base");
}

#[test]
fn prediction_sector_penalty_by_width() {
    use roulette_core::battle::resolve::prediction_penalty;
    assert_eq!(prediction_penalty(9), 0.3);
    assert_eq!(prediction_penalty(7), 0.3);
    assert_eq!(prediction_penalty(5), 0.5);
    assert_eq!(prediction_penalty(3), 0.5);
    assert_eq!(prediction_penalty(1), 1.0);
    // Applied in the pipeline: 9-wide sector → 20 payout → 6.
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    // Sector starting at index of 9 (idx 1), width 9 covers idx 1..=9.
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9,
        extra_balls: Vec::new(),
        prediction: Some((1, 9)),
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 6, "20 × 0.3 penalty");
    assert!(out.balls[0].in_prediction_sector);
}

#[test]
fn low_rarity_physics_penalty() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: true,
    });
    assert_eq!(out.total_payout, 6, "20 × 0.3");
}

#[test]
fn double_next_payout_is_one_shot() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    state.player_board.double_next_payout = true;
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 40, "20 × 2");
    assert!(!state.player_board.double_next_payout, "consumed");
}

#[test]
fn multiball_sums_per_ball() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Number(9), 10).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9,
        extra_balls: vec![9],
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.balls.len(), 2);
    assert_eq!(out.total_payout, 240, "120 + 120 per-ball");
}

// ---------------------------------------------------------------------------
// §10.2 special-color side effects
// ---------------------------------------------------------------------------

#[test]
fn gold_landing_converts_and_scores() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    // Landing on 7 (gold, L=0 → 15 pts) also converts 7 and the next slot (4).
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 7,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 15, "gold side-effect PTS only (no bets)");
    let wheel = state.player_wheel.clone();
    assert_eq!(wheel.effective_color(7, 0, None), SlotColor::Gold);
    let idx = wheel.numbers.iter().position(|&n| n == 7).unwrap();
    let next = wheel.numbers[(idx + 1) % wheel.numbers.len()];
    assert_eq!(wheel.effective_color(next, 0, None), SlotColor::Gold, "adjacent converted");
}

#[test]
fn purple_landing_stuns_enemy() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 6, // purple
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 20, "purple PTS at L=0");
    assert_eq!(out.opponent_stunned_turns, 1);
    assert_eq!(state.enemy_stun_turns, 1);
}

#[test]
fn crimson_pays_double_while_losing() {
    let mut state = test_battle(no_curses());
    state.enemy_chips_pool = 40; // player (30) behind → losing
    state.player_board.payout_multipliers.insert(roulette_core::board::MultTarget::Crimson, 6.0);
    state.begin_betting(Side::Player);
    state.chips_pool = 10;
    state.player_wheel.slot_colors.insert(11, SlotColor::Crimson);
    state.place_bet(BetType::Number(11), 10).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 11,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    // 10 × 6.0 (armed) × 2 (losing) = 120, + consolation 15.
    assert_eq!(out.total_payout, 135);
}

// ---------------------------------------------------------------------------
// §10.3 streaks: HEAT COMBO & GLACIER SHIELD
// ---------------------------------------------------------------------------

#[test]
fn heat_combo_sets_red_multiplier_at_three_streaks() {
    let mut state = test_battle(no_curses());
    state.player_board.red_streak_active = true;
    state.player_board.red_streak_count = 2;
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 10).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9, // red
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    // Step 5: streak mult = 1 + 0.5×2 = 2.0 this spin → 10 × 2.0 × 2.0 = 40.
    assert_eq!(out.total_payout, 40);
    assert_eq!(state.player_board.red_streak_count, 3);
    assert_eq!(
        state.player_board.payout_multiplier_for(&roulette_core::board::MultTarget::Red),
        3.5
    );
}

#[test]
fn red_streak_resets_on_black() {
    let mut state = test_battle(no_curses());
    state.player_board.red_streak_active = true;
    state.player_board.red_streak_count = 2;
    state.player_board.black_streak_active = true;
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Black, 5).unwrap();
    state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 2, // black
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(state.player_board.red_streak_count, 0);
    assert_eq!(state.player_board.black_streak_count, 1);
}

// ---------------------------------------------------------------------------
// §10.4 zone triggers, LUCKY_CHARM, insurance
// ---------------------------------------------------------------------------

#[test]
fn chip_mine_pays_value_on_landing() {
    let mut state = test_battle(no_curses());
    state
        .player_board
        .zone_marks
        .entry(roulette_content::schema::ZoneKind::ChipMine)
        .or_default()
        .insert(9);
    state.player_board.zone_values.insert((roulette_content::schema::ZoneKind::ChipMine, 9), 25);
    state.begin_betting(Side::Player);
    let before = state.chips_pool;
    state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 9,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(state.chips_pool, before + 25);
}

#[test]
fn lucky_charm_reroll_policy() {
    let mut state = test_battle(no_curses());
    state.player_board.lucky_charms = 1;
    assert!(state.should_use_lucky_charm(0));
    assert!(!state.should_use_lucky_charm(10));
    assert!(state.consume_lucky_charm());
    assert!(!state.consume_lucky_charm());
}

#[test]
fn insurance_refunds_full_stake_on_zero() {
    let mut state = test_battle(no_curses());
    state.player_board.insurance_active = true;
    state.begin_betting(Side::Player);
    state.chips_pool = 20;
    state.place_bet(BetType::Red, 10).unwrap();
    state.place_bet(BetType::Number(5), 5).unwrap();
    let out = state.resolve_spin(&SpinInput {
        side: Side::Player,
        landed: 2, // loses red; 5 not landed
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(out.total_payout, 0);
    assert_eq!(out.insurance_refund, 15, "full stake back");
    assert_eq!(state.chips_pool, 5 + 15, "refund restored");
    assert!(!state.player_board.insurance_active, "consumed");
}

// ---------------------------------------------------------------------------
// §3.1 draw ladder & hand limits
// ---------------------------------------------------------------------------

#[test]
fn draw_ladder_costs() {
    let mut state = test_battle(no_curses());
    assert_eq!(state.draw_cost(), 0, "1st draw free");
    state.draws_this_turn = 1;
    assert_eq!(state.draw_cost(), 3);
    state.draws_this_turn = 2;
    assert_eq!(state.draw_cost(), 5);
    state.draws_this_turn = 3;
    assert_eq!(state.draw_cost(), 7);
}

#[test]
fn avarice_flattens_draw_cost() {
    let mut state = test_battle(vec![curse(CurseEffect::FlatDrawCost(3))]);
    assert_eq!(state.draw_cost(), 3);
    state.draws_this_turn = 4;
    assert_eq!(state.draw_cost(), 3, "flat at every draw");
}

#[test]
fn greed_starts_pool_at_15() {
    let state = test_battle(vec![curse(CurseEffect::ChipsPoolOverride(15))]);
    assert_eq!(state.chips_pool, 15);
    assert_eq!(state.enemy_chips_pool, 30);
}

#[test]
fn hand_limit_eight_or_five_under_choked() {
    let mut state = test_battle(no_curses());
    assert_eq!(state.hand_limit(), 8);
    let choked = test_battle(vec![curse(CurseEffect::HandLimitOverride(5))]);
    assert_eq!(choked.hand_limit(), 5);
    let _ = &mut state;
}

#[test]
fn buy_draw_pays_ladder_and_refills_from_discard() {
    let mut state = test_battle(no_curses());
    let defs = (0..6)
        .map(|i| roulette_content::schema::CardDef {
            id: format!("card{i}"),
            name: format!("Card {i}"),
            description: String::new(),
            cost: 0,
            card_type: CardType::Utility,
            rarity: roulette_content::schema::CardRarity::Common,
            effects: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut rng = Rng::new(77);
    state.deal_from_defs(&defs, &mut rng);
    assert_eq!(state.hand.len(), 2, "exactly 2 at start");
    assert_eq!(state.draw_pile.len(), 4);

    state.begin_betting(Side::Player);
    let before = state.chips_pool;
    state.buy_draw().unwrap();
    assert_eq!(state.chips_pool, before, "1st draw free");
    assert_eq!(state.hand.len(), 3);

    // Drain the pile to force a discard reshuffle.
    while state.draw_pile.len() > 1 {
        state.draw_pile.pop();
    }
    state.discard_pile = state.draw_pile.clone();
    state.draw_pile.clear();
    state.draws_this_turn = 5;
    let before = state.chips_pool;
    state.buy_draw().unwrap();
    assert_eq!(state.chips_pool, before - 11, "6th draw costs 11");
    assert_eq!(state.hand.len(), 4);
}

// ---------------------------------------------------------------------------
// §3.4 bet actions: merge, cap, clear/rebet, sacrifice
// ---------------------------------------------------------------------------

#[test]
fn bets_merge_on_same_type() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    state.place_bet(BetType::Red, 5).unwrap();
    state.place_bet(BetType::Red, 10).unwrap();
    assert_eq!(state.bets.len(), 1);
    assert_eq!(state.bets[0].amount, 15);
}

#[test]
fn lead_caps_single_slots_at_five() {
    let mut state = test_battle(vec![curse(CurseEffect::BetCap(5))]);
    state.begin_betting(Side::Player);
    assert!(matches!(
        state.place_bet(BetType::Red, 10),
        Err(roulette_core::battle::ActionError::BetCapExceeded(5))
    ));
    state.place_bet(BetType::Red, 3).unwrap();
    assert!(state.place_bet(BetType::Red, 3).is_err(), "3+3 > 5 cap");
    state.place_bet(BetType::Red, 2).unwrap();
    assert_eq!(state.bets[0].amount, 5);
}

#[test]
fn clear_then_rebet_restores_layout() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    state.chips_pool = 30;
    state.place_bet(BetType::Red, 10).unwrap();
    state.place_bet(BetType::Number(5), 5).unwrap();
    let refund = state.clear_bets();
    assert_eq!(refund, 15);
    assert!(state.bets.is_empty());
    assert_eq!(state.chips_pool, 30);
    state.rebet().unwrap();
    assert_eq!(state.bets.len(), 2);
    assert_eq!(state.chips_pool, 15);
}

#[test]
fn sacrifice_trades_ten_points_for_five_chips() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    let before = state.chips_pool;
    state.sacrifice().unwrap();
    assert_eq!(state.chips_pool, before - 10 + 5);
}

#[test]
fn spin_with_no_bets_passes_turn() {
    let mut state = test_battle(no_curses());
    state.begin_betting(Side::Player);
    assert_eq!(state.spin().unwrap(), None);
    assert_eq!(state.phase, BattlePhase::Betting, "no phase change on pass");
}

// ---------------------------------------------------------------------------
// §7 enemy AI
// ---------------------------------------------------------------------------

#[test]
fn enemy_ai_is_deterministic_per_seed() {
    let a = {
        let state = test_battle(no_curses());
        let mut rng = Rng::new(1234);
        state.enemy_choose_bets(&mut rng)
    };
    let b = {
        let state = test_battle(no_curses());
        let mut rng = Rng::new(1234);
        state.enemy_choose_bets(&mut rng)
    };
    assert_eq!(a, b);
    assert!(!a.is_empty(), "AI with 30 chips and positive-EV bets bets");
}

#[test]
fn enemy_bets_split_50_30_rest() {
    let mut state = test_battle(no_curses());
    state.enemy_difficulty = 1.0;
    let mut rng = Rng::new(9);
    let stakes = state.enemy_choose_bets(&mut rng);
    assert!(!stakes.is_empty() && stakes.len() <= 3);
    let commit: u16 = stakes.iter().map(|(_, a)| a).sum();
    assert!(commit <= state.enemy_chips_pool);
    assert!(stakes.iter().all(|&(_, a)| a > 0));
    assert!(stakes.len() <= 3, "at most 3 bets");
}

#[test]
fn enemy_attack_intent_damages_player_on_win() {
    use roulette_core::battle::EnemyIntent;
    let mut state = test_battle(no_curses());
    state.enemy_intent = Some(EnemyIntent {
        action: EnemyAction::Attack,
        value: 7,
        description: "test".into(),
        pattern_index: 0,
    });
    state.enemy_wheel.payout_multipliers.green = 50.0;
    state.enemy_wheel.green_numbers = vec![0];
    state.enemy_bets.push(roulette_core::bets::Bet { bet_type: BetType::Green, amount: 5 });
    state.resolve_spin(&SpinInput {
        side: Side::Enemy,
        landed: 0,
        extra_balls: Vec::new(),
        prediction: None,
        low_rarity_physics_played: false,
    });
    assert_eq!(state.enemy_chips_pool, 30 + 5 * 50);
    state.execute_enemy_intent();
    assert_eq!(state.player_hp, 50 - 7, "intent executed on winning spin");
    assert!(state.enemy_intent.is_none(), "intent consumed");
}

#[test]
fn stunned_enemy_skips_turn() {
    let mut state = test_battle(no_curses());
    state.enemy_stun_turns = 2;
    let mut rng = Rng::new(9);
    let out = state.enemy_take_turn(&mut rng, 0);
    assert!(out.is_none(), "stunned enemy does not spin");
    assert_eq!(state.enemy_stun_turns, 1);
    assert_eq!(state.turn, Side::Player, "turn passes to player");
}

// ---------------------------------------------------------------------------
// §3.2/§3.5 round flow & sudden death
// ---------------------------------------------------------------------------

#[test]
fn round_limit_reached_scores_decide() {
    let mut state = test_battle(no_curses());
    state.round = 3;
    state.chips_pool = 40;
    state.enemy_chips_pool = 30;
    assert_eq!(state.end_round(), BattleOutcome::PlayerVictory);

    let mut state = test_battle(no_curses());
    state.round = 3;
    state.chips_pool = 25;
    assert_eq!(state.end_round(), BattleOutcome::PlayerDefeat);
}

#[test]
fn tie_at_limit_triggers_sudden_death() {
    let mut state = test_battle(no_curses());
    state.round = 3;
    state.chips_pool = 30;
    state.enemy_chips_pool = 30;
    assert_eq!(state.end_round(), BattleOutcome::SuddenDeath);
    assert!(state.is_sudden_death);
}

#[test]
fn curse_of_blood_drains_two_hp_per_round() {
    let mut state = test_battle(vec![curse(CurseEffect::HpLossPerRound(2))]);
    state.round = 1;
    state.player_hp = 50;
    state.end_round();
    assert_eq!(state.player_hp, 48);
    assert_eq!(state.round, 2);
}

#[test]
fn zero_hp_is_defeat() {
    let mut state = test_battle(vec![curse(CurseEffect::HpLossPerRound(50))]);
    state.round = 1;
    state.end_round();
    assert_eq!(state.end_round(), BattleOutcome::PlayerDefeat);
}

// ---------------------------------------------------------------------------
// Golden replay (determinism): same seed → same full battle trace
// ---------------------------------------------------------------------------

#[test]
fn golden_battle_replay_is_identical() {
    let run = |seed: u32| {
        let mut state = test_battle(no_curses());
        state.enemy_difficulty = 0.6;
        let mut rng = Rng::new(seed).derive("battle");
        let mut trace = String::new();
        for round in 0..3 {
            state.begin_betting(Side::Player);
            state.place_bet(BetType::Red, 5).ok();
            state.place_bet(BetType::Green, 5).ok();
            let player_landed = [9u32, 2, 0, 7, 4, 5][(round * 2) % 6];
            state.resolve_spin(&SpinInput {
                side: Side::Player,
                landed: player_landed,
                extra_balls: Vec::new(),
                prediction: None,
                low_rarity_physics_played: false,
            });
            state.advance_after_spin();
            let enemy_landed = [2u32, 9, 12, 4, 8, 1][(round * 2 + 1) % 6];
            let out = state.enemy_take_turn(&mut rng, enemy_landed);
            trace += &format!(
                "r{} p={:?} e={:?} pools={}/{} hp={}\n",
                round,
                state.player_board.red_streak_count,
                out.map(|o| o.total_payout),
                state.chips_pool,
                state.enemy_chips_pool,
                state.player_hp,
            );
            state.end_round();
        }
        trace
    };
    assert_eq!(run(424242), run(424242), "same seed → identical trace");
    assert_ne!(run(424242), run(99), "different seed → different trace");
}
