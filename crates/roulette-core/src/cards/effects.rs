//! Effect dispatch (TASK-031..033, §6.2/§6.4).
//!
//! One switch over the typed `EffectKind` DSL, applied at the §6.4 hook
//! points. Content is data: cards carry `Vec<EffectKind>`; this module owns
//! the mechanics. `Ok(false)` = play rejected (Identity Shift with no hand
//! card); `Ok(true)` = applied.

use std::collections::BTreeMap;

use roulette_content::schema::{
    CardDef, CardType, ChipDestination, ConvertScope, DurationKind, EffectKind, HandOp, NumberSet,
    SlotColor,
};

use crate::battle::state::{ActionError, BattlePhase, BattleState, CardInstance, Side};
use crate::board::{ModifierEntry, ModifierKind, MultTarget};
use crate::rng::Rng;

/// The money card `TaxRefund`/`TempEssenceChips` operate on (§6.3).
pub const ESSENCE_CHIP_ID: &str = "money_essence_chip";

/// Play-time context: the card database and the run RNG.
pub struct EffectCtx<'a> {
    pub defs: &'a BTreeMap<String, CardDef>,
    pub rng: &'a mut Rng,
}

/// Effect dispatch errors (unknown defs / malformed effects).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EffectError {
    UnknownCard(String),
    Invalid(String),
}

impl From<ActionError> for EffectError {
    fn from(e: ActionError) -> Self {
        EffectError::Invalid(e.to_string())
    }
}

/// Plays a hand card end-to-end (§3.2 step 3): removes it from the hand,
/// charges its cost, dispatches `OnPlayCard` effects and — when not rejected —
/// commits it face-up on the felt. Rejected plays refund the cost and return
/// the card to its hand slot.
pub fn play_card_with_effects(
    state: &mut BattleState,
    hand_index: usize,
    ctx: &mut EffectCtx,
) -> Result<bool, EffectError> {
    let instance = state.play_card(hand_index)?;
    let def_id = instance.def_id.clone();
    let Some(def) = ctx.defs.get(&def_id) else {
        state.hand.insert(hand_index, instance);
        return Err(EffectError::UnknownCard(def_id));
    };
    let cost = instance.cost_override.unwrap_or(def.cost);
    if let Err(e) = state.pay_card_cost(cost) {
        state.hand.insert(hand_index, instance);
        return Err(EffectError::from(e));
    }
    let mut played = instance.clone();
    let applied = on_play_card(state, def, &mut played, ctx)?;
    if !applied {
        // Refund cost + return the card; the play never happened.
        state.chips_pool += u16::from(cost);
        state.hand.insert(hand_index, played);
        return Ok(false);
    }
    let locked = played.temp;
    state.commit_played_card(played, locked);
    Ok(true)
}

/// `OnPlayCard` hook: dispatches every effect of `def` in declaration order.
/// `played` is the live instance (paint slots are resolved onto it).
pub fn on_play_card(
    state: &mut BattleState,
    def: &CardDef,
    played: &mut CardInstance,
    ctx: &mut EffectCtx,
) -> Result<bool, EffectError> {
    for effect in &def.effects {
        if !apply_effect(state, effect, played, def, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `AfterSpinResolve` hook (§6.4): per-spin arms expire, played cards move to
/// the discard pile (temp cards are exiled), spin-scoped stack entries clear.
pub fn after_spin_resolve(state: &mut BattleState) {
    let played = std::mem::take(&mut state.active_played);
    for active in played {
        if !active.instance.temp {
            state.discard_pile.push(active.instance);
        }
    }
    state.player_stack.clear_spin_scoped();
    // "Next N spins" card arms count down per spin resolution (§6.3).
    state.player_stack.tick_after_spin();
    // Per-spin board arms expire after one spin (turbo, omniscience, ripple).
    state.player_board.turbo_active = false;
    state.player_board.omniscience_active = false;
    state.player_board.green_ripple_active = false;
    // Physics cheats are spin-scoped.
    state.reset_physics();
}

/// `OnRoundEnd` hook: pays Capital Venture on a round win and files the
/// round's played cards (non-temp → discard, temp → exile).
pub fn on_round_end(state: &mut BattleState, player_won: bool) {
    let venture = state.player_board.capital_venture_amount;
    if venture > 0 && player_won {
        state.player_board.capital_venture_amount = 0;
        state.venture_bank += venture;
    }
    let played = std::mem::take(&mut state.active_played);
    for active in played {
        if !active.instance.temp {
            state.discard_pile.push(active.instance);
        }
    }
}

fn apply_effect(
    state: &mut BattleState,
    effect: &EffectKind,
    played: &mut CardInstance,
    def: &CardDef,
    ctx: &mut EffectCtx,
) -> Result<bool, EffectError> {
    match effect {
        EffectKind::Physics { .. } => arm_physics(state, effect),
        EffectKind::PayoutMult { target, value, duration } => {
            let scope = match duration {
                DurationKind::Fight => crate::board::ModifierScope::Fight,
                DurationKind::Spins(n) => crate::board::ModifierScope::Spins(*n),
                DurationKind::NextWin => crate::board::ModifierScope::NextWin,
            };
            state.player_stack.push(ModifierEntry {
                source: def.id.clone(),
                marked_slots: Vec::new(),
                kind: ModifierKind::PayoutMult { target: MultTarget::from(*target), mult: *value },
                scope,
            });
        }
        EffectKind::CustomNumberMult { numbers, mult } => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::CustomNumberMult { numbers: numbers.clone(), mult: *mult },
            scope: crate::board::ModifierScope::Fight,
        }),
        EffectKind::ConvertSlots { to, from, numbers, scope } => {
            let slots = resolve_numbers(numbers, state, *from, &played.marked_slots, ctx.rng);
            state.player_stack.push(ModifierEntry {
                source: def.id.clone(),
                marked_slots: slots.clone(),
                kind: ModifierKind::ConvertSlots { to: *to, numbers: slots },
                scope: convert_scope(*scope),
            });
        }
        EffectKind::SwapRedBlack { scope } => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::SwapRedBlack,
            scope: convert_scope(*scope),
        }),
        EffectKind::AddGreenSlot { number } => {
            // Only real wheel slots can become green (§6.3: "choose a number").
            if state.player_wheel.numbers.contains(number) {
                state.player_stack.push(ModifierEntry {
                    source: def.id.clone(),
                    marked_slots: Vec::new(),
                    kind: ModifierKind::ConvertSlots {
                        to: SlotColor::Green,
                        numbers: vec![*number],
                    },
                    scope: crate::board::ModifierScope::Fight,
                });
            }
        }
        EffectKind::ZoneMark { kind, slots, count, value } => {
            let picked = if !slots.is_empty() {
                slots.clone()
            } else if !played.marked_slots.is_empty() {
                played.marked_slots.clone()
            } else {
                pick_random_slots(state, usize::from(count.unwrap_or(1)), None, ctx.rng)
            };
            state.player_stack.push(ModifierEntry {
                source: def.id.clone(),
                marked_slots: picked.clone(),
                kind: ModifierKind::ZoneMark { kind: *kind, slots: picked, value: *value },
                scope: crate::board::ModifierScope::Fight,
            });
        }
        EffectKind::GlobalMult { mult, spins } => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::GlobalMult { mult: *mult },
            scope: crate::board::ModifierScope::Spins(*spins),
        }),
        EffectKind::GreenRipple { .. } => state.player_board.green_ripple_active = true,
        EffectKind::Draw { count } => {
            state.draw_free(*count);
        }
        EffectKind::DiscardHand => discard_hand(state),
        EffectKind::DiscardRandom { count } => discard_random(state, *count, ctx.rng),
        EffectKind::DiscardHandThenDrawEqual => {
            let n = u8::try_from(state.hand.len()).unwrap_or(u8::MAX);
            discard_hand(state);
            state.draw_free(n);
        }
        EffectKind::GrantChips { amount } => {
            state.chips_pool = state.chips_pool.saturating_add(*amount)
        }
        EffectKind::GrantChipsPercent { percent } => {
            let grant = state.chips_pool * (*percent) / 100;
            state.chips_pool = state.chips_pool.saturating_add(grant);
        }
        EffectKind::LossHp { amount } => {
            state.player_hp = state.player_hp.saturating_sub(*amount);
        }
        EffectKind::StunEnemy { turns, condition } => match condition {
            roulette_content::schema::StunCondition::OnPlay => {
                state.enemy_stun_turns += u16::from(*turns);
            }
            roulette_content::schema::StunCondition::OnDamageGE(_) => {
                state.player_board.stun_strike_armed = true;
            }
        },
        EffectKind::DoubleNextPayout => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::DoubleNextPayout,
            scope: crate::board::ModifierScope::Spin,
        }),
        EffectKind::Insurance => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::Insurance,
            scope: crate::board::ModifierScope::Fight,
        }),
        EffectKind::LuckyCharm => state.player_board.lucky_charms += 1,
        EffectKind::EmeraldForest => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::EmeraldForest,
            scope: crate::board::ModifierScope::Fight,
        }),
        EffectKind::GoldenHeist { amount } => state.player_stack.push(ModifierEntry {
            source: def.id.clone(),
            marked_slots: Vec::new(),
            kind: ModifierKind::GoldenHeist { amount: *amount },
            scope: crate::board::ModifierScope::Fight,
        }),
        EffectKind::RiskCapital { gain, drain_per_spin } => {
            state.chips_pool = state.chips_pool.saturating_add(*gain);
            state.player_board.risk_capital_active = true;
            state.player_board.risk_capital_drain += *drain_per_spin;
        }
        EffectKind::CapitalVenture { amount } => {
            state.player_board.capital_venture_amount += *amount;
        }
        EffectKind::DoubleDown => apply_double_down(state),
        EffectKind::TaxRefund { per_card } => {
            let chips =
                state.discard_pile.iter().filter(|c| c.def_id == ESSENCE_CHIP_ID).count() as u16;
            state.chips_pool = state.chips_pool.saturating_add(chips * per_card);
        }
        EffectKind::HandOp { kind } => return apply_hand_op(state, *kind, played, ctx),
        EffectKind::TempEssenceChips { count, destination } => {
            let chips = (0..*count)
                .map(|_| CardInstance {
                    def_id: ESSENCE_CHIP_ID.to_string(),
                    marked_slots: Vec::new(),
                    temp: true,
                    retained: false,
                    cost_override: None,
                })
                .collect::<Vec<_>>();
            match destination {
                ChipDestination::Hand => state.hand.extend(chips),
                ChipDestination::DrawPile => state.draw_pile.extend(chips),
                ChipDestination::DiscardPile => state.discard_pile.extend(chips),
            }
        }
        EffectKind::ActionSurge { chips } => {
            state.chips_pool = state.chips_pool.saturating_add(*chips);
        }
        EffectKind::FreeCards { this_turn, next_turn } => {
            state.free_cards_active += *this_turn;
            state.free_cards_next_turn += *next_turn;
        }
        EffectKind::Omniscience => state.player_board.omniscience_active = true,
        EffectKind::TurboSpin => state.player_board.turbo_active = true,
        EffectKind::HeavyNudge => state.player_board.heavy_nudge_armed = true,
        EffectKind::GrantCard { card_id } => {
            if !ctx.defs.contains_key(card_id) {
                return Err(EffectError::UnknownCard(card_id.clone()));
            }
            state.hand.push(CardInstance {
                def_id: card_id.clone(),
                marked_slots: Vec::new(),
                temp: false,
                retained: false,
                cost_override: None,
            });
        }
    }
    Ok(true)
}

fn convert_scope(scope: ConvertScope) -> crate::board::ModifierScope {
    match scope {
        ConvertScope::Round => crate::board::ModifierScope::Round,
        ConvertScope::Spin => crate::board::ModifierScope::Spin,
        ConvertScope::Fight => crate::board::ModifierScope::Fight,
    }
}

/// Arms `Some` physics fields for the next spin (§5.3); the pipeline resets to
/// the baseline after the spin resolves.
fn arm_physics(state: &mut BattleState, effect: &EffectKind) {
    let EffectKind::Physics {
        friction,
        ball_mass,
        spin_speed,
        wheel_tilt,
        target_zone_bias,
        bias_red_only,
        bias_black_only,
        nudge_distance,
        bounce_randomness,
        multiball,
        split_peg,
        shotgun,
        prediction_size,
    } = effect
    else {
        return;
    };
    let p = &mut state.physics;
    if let Some(v) = friction {
        p.friction = *v;
    }
    if let Some(v) = ball_mass {
        p.ball_mass = *v;
    }
    if let Some(v) = spin_speed {
        p.spin_speed = *v;
    }
    if let Some(v) = wheel_tilt {
        p.wheel_tilt = *v;
    }
    if let Some(v) = target_zone_bias {
        p.target_zone_bias = *v;
    }
    if let Some(v) = bias_red_only {
        p.bias_red_only = *v;
    }
    if let Some(v) = bias_black_only {
        p.bias_black_only = *v;
    }
    if let Some(v) = nudge_distance {
        p.nudge_distance = *v;
        p.nudge_cheat_active = true;
    }
    if let Some(v) = bounce_randomness {
        p.bounce_randomness = *v;
    }
    if let Some(v) = multiball {
        p.multiball_count = u32::from(*v);
    }
    if let Some(v) = split_peg {
        p.split_peg_active = *v;
    }
    if let Some(v) = shotgun {
        p.shotgun_time = if *v { 1.5 } else { 0.0 };
    }
    if let Some(v) = prediction_size {
        p.prediction_size = *v;
    }
}

/// Double Down (§6.3): doubles the stakes on the felt (no extra charge) and
/// doubles the enemy's intent value; the pipeline punishes the miss.
fn apply_double_down(state: &mut BattleState) {
    for bet in &mut state.bets {
        bet.amount = bet.amount.saturating_mul(2);
    }
    if let Some(intent) = &mut state.enemy_intent {
        intent.value = intent.value.saturating_mul(2);
        intent.description.push_str(" (Doubled Down!)");
    }
}

fn apply_hand_op(
    state: &mut BattleState,
    kind: HandOp,
    played: &mut CardInstance,
    ctx: &mut EffectCtx,
) -> Result<bool, EffectError> {
    match kind {
        HandOp::CopyRandomHandCard => {
            if state.hand.is_empty() {
                return Ok(false);
            }
            let idx = ctx.rng.range_usize(0, state.hand.len().saturating_sub(1));
            let source = state.hand[idx].def_id.clone();
            for _ in 0..2 {
                state.hand.push(CardInstance {
                    def_id: source.clone(),
                    marked_slots: Vec::new(),
                    temp: true,
                    retained: false,
                    cost_override: Some(0),
                });
            }
        }
        HandOp::CopyLastPlayed => {
            let Some(source) = state
                .active_played
                .iter()
                .rev()
                .find(|active| {
                    ctx.defs
                        .get(&active.instance.def_id)
                        .is_some_and(|d| d.card_type != CardType::Utility)
                })
                .map(|active| active.instance.def_id.clone())
            else {
                return Ok(false);
            };
            state.hand.push(CardInstance {
                def_id: source,
                marked_slots: Vec::new(),
                temp: false,
                retained: false,
                cost_override: None,
            });
        }
        HandOp::RecycleFromDiscard => {
            if state.discard_pile.is_empty() {
                return Ok(false);
            }
            let card = state.discard_pile.pop().unwrap();
            state.hand.push(card);
        }
        HandOp::Retain => {
            let Some(card) = state.hand.first_mut() else {
                return Ok(false);
            };
            card.retained = true;
        }
        // The played card is exiled after resolving (no discard return).
        HandOp::ExileSelf => played.temp = true,
        HandOp::Reshuffle => {
            let mut reshuffled = std::mem::take(&mut state.discard_pile);
            ctx.rng.shuffle(&mut reshuffled);
            state.draw_pile.append(&mut reshuffled);
        }
        HandOp::DiscardFromHandDrawOne => {
            if state.hand.is_empty() {
                return Ok(false);
            }
            let discarded = state.hand.remove(0);
            state.discard_pile.push(discarded);
            state.draw_free(1);
        }
    }
    Ok(true)
}

fn discard_hand(state: &mut BattleState) {
    let hand = std::mem::take(&mut state.hand);
    for card in hand {
        if !card.retained {
            state.discard_pile.push(card);
        }
    }
}

fn discard_random(state: &mut BattleState, count: u8, rng: &mut Rng) {
    for _ in 0..count {
        if state.hand.is_empty() {
            break;
        }
        let idx = rng.range_usize(0, state.hand.len() - 1);
        let card = state.hand.remove(idx);
        state.discard_pile.push(card);
    }
}

/// Picks `count` random wheel slots, optionally restricted to slots currently
/// of color `from`. Deterministic through the battle RNG.
fn pick_random_slots(
    state: &BattleState,
    count: usize,
    from: Option<SlotColor>,
    rng: &mut Rng,
) -> Vec<u32> {
    let wheel = &state.player_wheel;
    let mut pool: Vec<u32> = wheel
        .numbers
        .iter()
        .copied()
        .filter(|&n| match from {
            Some(color) => {
                wheel.effective_color(n, state.player_levels.get(SlotColor::Green), None) == color
            }
            None => true,
        })
        .collect();
    rng.shuffle(&mut pool);
    pool.truncate(count);
    pool
}

/// Resolves a DSL `NumberSet` to explicit wheel numbers at play time
/// (PAT-002: the stack holds concrete data only). `PlayerChoice` falls back to
/// the instance's `marked_slots`, then to a random pick.
fn resolve_numbers(
    set: &NumberSet,
    state: &BattleState,
    from: Option<SlotColor>,
    marked: &[u32],
    rng: &mut Rng,
) -> Vec<u32> {
    let wheel = &state.player_wheel;
    let in_color = |n: u32| match from {
        Some(color) => {
            wheel.effective_color(n, state.player_levels.get(SlotColor::Green), None) == color
        }
        None => true,
    };
    let base: Vec<u32> = wheel.numbers.to_vec();
    let mut selected: Vec<u32> = match set {
        NumberSet::All => base,
        NumberSet::Below(n) => base.into_iter().filter(|&x| x < *n).collect(),
        NumberSet::Above(n) => base.into_iter().filter(|&x| x > *n).collect(),
        NumberSet::Range(lo, hi) => base.into_iter().filter(|&x| x >= *lo && x <= *hi).collect(),
        NumberSet::List(list) => base.into_iter().filter(|x| list.contains(x)).collect(),
        NumberSet::Odd => base.into_iter().filter(|&x| x % 2 == 1).collect(),
        NumberSet::Even => base.into_iter().filter(|&x| x % 2 == 0 && x != 0).collect(),
        NumberSet::Prime => base.into_iter().filter(|&x| is_prime(x)).collect(),
        NumberSet::SingleDigit => base.into_iter().filter(|&x| (1..=9).contains(&x)).collect(),
        NumberSet::High => base.into_iter().filter(|&x| (19..=36).contains(&x)).collect(),
        NumberSet::Low => base.into_iter().filter(|&x| (1..=18).contains(&x)).collect(),
        NumberSet::RandomSlots(n) | NumberSet::PlayerChoice(n) => {
            let n = usize::from(*n);
            if marked.len() >= n {
                marked.to_vec()
            } else {
                pick_random_slots(state, n, from, rng)
            }
        }
    };
    selected.retain(|&n| in_color(n) && wheel.numbers.contains(&n));
    selected
}

/// Primality for `NumberSet::Prime` and the Prime payout arm (§10.1).
pub fn is_prime(n: u32) -> bool {
    if n < 2 {
        return false;
    }
    if n.is_multiple_of(2) {
        return n == 2;
    }
    let mut d = 3u32;
    while d * d <= n {
        if n.is_multiple_of(d) {
            return false;
        }
        d += 2;
    }
    true
}

/// Spin-phase gate for effect validation: cards may only be played while the
/// player is betting (rejected otherwise by the play flow).
pub fn require_betting(state: &BattleState) -> Result<(), EffectError> {
    if state.phase != BattlePhase::Betting || state.turn != Side::Player {
        return Err(EffectError::Invalid("not the player's betting phase".into()));
    }
    Ok(())
}
