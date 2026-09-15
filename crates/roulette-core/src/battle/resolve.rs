//! The §10.1 resolve pipeline (TASK-026) plus resolve-time side effects
//! (§10.2/§10.4/§10.5, TASK-027), as pure functions over [`SpinInput`].

use roulette_content::schema::{SlotColor, ZoneKind};

use super::state::{
    BallPayout, BattlePhase, BattleState, ResolvedBet, Side, SpinInput, SpinOutcome,
};
use crate::bets::BetType;
use crate::board::MultTarget;
use crate::wheel::{green_ladder_slots, ColorLevels};

/// Prediction-sector penalty ladder (§10.1 step 4): sector size 9/7 → ×0.3,
/// 5/3 → ×0.5, 1 → ×1.0 (pinpoint = no penalty). Even sizes interpolate down.
pub fn prediction_penalty(sector_size: u8) -> f32 {
    match sector_size {
        0 => 1.0,
        1 => 1.0,
        2 | 3 => 0.5,
        4 | 5 => 0.5,
        6 | 7 => 0.3,
        _ => 0.3,
    }
}

/// Is a landed slot inside a prediction sector? Sector spans `size` slots
/// starting at slot index `start` (physical wheel order).
fn in_sector(
    landed: u32,
    wheel: &crate::wheel::WheelConfig,
    prediction: Option<(usize, u8)>,
) -> bool {
    let Some((start, size)) = prediction else {
        return false;
    };
    if size == 0 {
        return false;
    }
    let Some(idx) = wheel.numbers.iter().position(|&n| n == landed) else {
        return false;
    };
    let len = wheel.len();
    let offset = (idx + len - start % len) % len;
    (offset as u32) < size as u32
}

impl BattleState {
    /// Whose side is losing on score (crimson consolation ×12, §10.1/§10.2).
    fn is_losing(&self, side: Side) -> bool {
        let own = self.score(side);
        let other = self.score(side.other());
        own < other
    }

    /// Resolves one spin (§10.1) for the spinning side's bets against its own
    /// landed results, then applies resolve-time side effects (§10.2/§10.4/§10.5).
    pub fn resolve_spin(&mut self, input: &SpinInput) -> SpinOutcome {
        let side = input.side;
        let wheel = self.wheel(side).clone();
        let levels = match side {
            Side::Player => self.player_levels.clone(),
            Side::Enemy => self.enemy_levels.clone(),
        };
        // Player: stack snapshot + battle-owned state; enemy: plain board.
        let mut board = self.merged_board(side);
        let bets: Vec<crate::bets::Bet> = match side {
            Side::Player => self.bets.clone(),
            Side::Enemy => self.enemy_bets.clone(),
        };
        let losing = self.is_losing(side);
        let green_voided = self.green_voided();

        let mut balls = Vec::new();
        let mut resolved: Vec<ResolvedBet> = Vec::new();
        let mut total: f32 = 0.0;

        for landed in std::iter::once(input.landed).chain(input.extra_balls.iter().copied()) {
            let sector_hit = in_sector(landed, &wheel, input.prediction);
            let mut ball_total = 0.0f32;
            for bet in &bets {
                let Some(resolved_bet) = self.resolve_single_bet(
                    side,
                    bet,
                    landed,
                    &wheel,
                    &levels,
                    &board,
                    losing,
                    green_voided,
                ) else {
                    continue;
                };
                ball_total += resolved_bet.payout;
                resolved.push(resolved_bet);
            }
            // --- step 3: zone multipliers (§10.1 step 3) ---
            if ball_total > 0.0 {
                ball_total *= zone_multiplier(&board, &wheel, landed);
                // --- step 4: cheat penalties ---
                if sector_hit {
                    if let Some((_, size)) = input.prediction {
                        ball_total *= prediction_penalty(size);
                    }
                }
                if input.low_rarity_physics_played {
                    ball_total *= 0.3;
                }
                // --- step 5: post-sum modifiers ---
                let streak_mult = streak_multiplier(&board, &wheel, landed, &levels);
                ball_total *= streak_mult;
                ball_total *= board.global_multiplier.max(1.0);
                if board.double_next_payout {
                    ball_total *= 2.0;
                    board.double_next_payout = false;
                }
                if board.turbo_active {
                    ball_total *= 1.5;
                }
                if board.omniscience_active && sector_hit {
                    ball_total *= 3.0;
                }
            }
            let floored = ball_total.floor() as u16;
            balls.push(BallPayout { landed, damage: floored, in_prediction_sector: sector_hit });
            total += floored as f32;
        }

        let mut total_payout = total as u16;
        let all_lost = total_payout == 0;

        // --- insurance (§10.5): full stake refund on an all-lost spin ---
        let stake: u16 = bets.iter().map(|b| b.amount).sum();
        let mut insurance_refund = 0u16;
        if all_lost && board.insurance_active {
            insurance_refund = stake;
            board.insurance_active = false;
            // The armed Insurance stack entry is consumed with its payout.
            if side == Side::Player {
                self.player_stack
                    .entries_mut()
                    .retain(|e| !matches!(e.kind, crate::board::ModifierKind::Insurance));
            }
        }
        // Block-red intent is spent on the spin it guarded (§7.2).
        if side == Side::Player && board.block_red_active {
            board.block_red_active = false;
        }

        // --- §10.2 special-color side effects + §10.4 zone triggers ---
        let mut target_stunned = 0u16;
        let mut side_effects_pts = 0u16;
        for ball in &balls {
            let (pts, stun) = self.apply_landed_side_effects(side, ball.landed, &levels);
            side_effects_pts += pts;
            target_stunned += stun;
            self.apply_zone_triggers(side, ball.landed);
        }
        total_payout += side_effects_pts;

        // STUN_STRIKE: damage ≥ 5 this turn → opponent stunned +2 (§10.4).
        if board.stun_strike_armed {
            board.stun_strike_armed = false;
            if self.damage_this_turn + total_payout >= 5 {
                target_stunned += 2;
            }
        }
        // HEAVY_NUDGE: all balls dealt 0 → +15 chips (§10.4).
        if board.heavy_nudge_armed {
            board.heavy_nudge_armed = false;
            if all_lost {
                *self.pool_mut(side) += 15;
            }
        }

        // --- apply payouts: payout-as-chips added back to the pool (§10.4) ---
        *self.pool_mut(side) += total_payout + insurance_refund;
        if side == Side::Player {
            self.damage_this_turn = self.damage_this_turn.saturating_add(total_payout);
        }
        // Risk Capital drain (§6.3): −⚡ per spin while armed.
        if board.risk_capital_active && board.risk_capital_drain > 0 {
            let drain = board.risk_capital_drain.min(*self.pool_mut(side));
            *self.pool_mut(side) -= drain;
        }
        // Stuns from purple/cursed/strike land on the opponent (§7.3).
        if target_stunned > 0 {
            match side {
                Side::Player => self.enemy_stun_turns += target_stunned,
                Side::Enemy => self.player_stun_turns += target_stunned,
            }
        }

        // --- streak tracking (§10.3): a red streak survives only all-red
        // spins (any black/green resets); symmetric for black. ---
        let colors_of = |b: &BallPayout| {
            wheel.effective_color(b.landed, levels.get(SlotColor::Green), Some(&board))
        };
        let any_black = balls.iter().any(|b| colors_of(b) == SlotColor::Black);
        let any_green = balls.iter().any(|b| colors_of(b) == SlotColor::Green);
        let any_red = balls.iter().any(|b| colors_of(b) == SlotColor::Red);
        if board.red_streak_active {
            if !any_black && !any_green && any_red {
                board.red_streak_count = board.red_streak_count.saturating_add(1);
                board.black_streak_count = 0;
                // HEAT COMBO (§10.3): red streak ≥ 3 → red pays 3.5 + 0.5(L−1).
                if board.red_streak_count >= 3 {
                    let l = levels.get(SlotColor::Red);
                    let heat = 3.5 + 0.5 * l.saturating_sub(1) as f32;
                    // Battle-owned: survives the fold (applies to future spins).
                    match side {
                        Side::Player => {
                            self.player_board
                                .payout_multipliers
                                .insert(crate::board::MultTarget::Red, heat);
                            board.payout_multipliers.insert(crate::board::MultTarget::Red, heat);
                        }
                        Side::Enemy => {
                            self.enemy_board
                                .payout_multipliers
                                .insert(crate::board::MultTarget::Red, heat);
                        }
                    }
                }
            } else {
                board.red_streak_count = 0;
                // Streak gone: the heat multiplier reverts to the base.
                match side {
                    Side::Player => {
                        self.player_board.payout_multipliers.remove(&crate::board::MultTarget::Red);
                    }
                    Side::Enemy => {
                        self.enemy_board.payout_multipliers.remove(&crate::board::MultTarget::Red);
                    }
                }
            }
        }
        if board.black_streak_active {
            if !any_red && !any_green && !any_black {
                // impossible (no red and no black and no green = no balls)
            } else if !any_red && !any_green && any_black {
                board.black_streak_count = board.black_streak_count.saturating_add(1);
                board.red_streak_count = 0;
                // GLACIER SHIELD (§10.3): black streak ≥ 3 drains the enemy pool.
                if board.black_streak_count >= 3 {
                    let l = levels.get(SlotColor::Black);
                    let drain = 10 + 3 * (l.saturating_sub(1) as u16);
                    let drained = drain.min(self.pool(side.other()));
                    *self.pool_mut(side.other()) -= drained;
                }
            } else {
                board.black_streak_count = 0;
            }
        }

        // Persist battle-owned board state only. The folded snapshot carries
        // stack-derived values (card arms) that live on in the stack — writing
        // them back would double-apply on the next spin.
        let streaks = (
            board.red_streak_active,
            board.red_streak_count,
            board.black_streak_active,
            board.black_streak_count,
        );
        match side {
            Side::Player => {
                let pb = &mut self.player_board;
                pb.red_streak_active = streaks.0;
                pb.red_streak_count = streaks.1;
                pb.black_streak_active = streaks.2;
                pb.black_streak_count = streaks.3;
                pb.block_red_active = board.block_red_active;
                pb.stun_strike_armed = board.stun_strike_armed;
                pb.heavy_nudge_armed = board.heavy_nudge_armed;
                pb.insurance_active = board.insurance_active;
                pb.double_next_payout = board.double_next_payout;
            }
            Side::Enemy => self.enemy_board = board,
        }

        // Losing stakes vanish (already deducted at placement).
        match side {
            Side::Player => self.bets.clear(),
            Side::Enemy => self.enemy_bets.clear(),
        }

        // Synapse Link (§4.5 green unlock): any green landing on the player's
        // spin grants a free card next turn. Blueprint gives no exact numbers;
        // +1 per landing spin (tuning is a Phase 9 balance pass).
        if side == Side::Player && any_green && wheel.unlocked_abilities.contains(&SlotColor::Green)
        {
            self.free_cards_next_turn = self.free_cards_next_turn.saturating_add(1);
        }

        let outcome = SpinOutcome {
            side,
            landed: input.landed,
            balls,
            resolved,
            total_payout,
            all_lost,
            insurance_refund,
            opponent_stunned_turns: target_stunned,
            target_stunned_turns: 0,
        };
        self.last_spin_result = Some(outcome.clone());
        self.phase = BattlePhase::Resolved;
        // AfterSpinResolve hook (§6.4): played cards file to the discard pile
        // (temp cards exiled), per-spin arms expire, physics resets.
        if side == Side::Player {
            crate::cards::effects::after_spin_resolve(self);
        }
        outcome
    }

    /// Steps 1–2 for one bet × one ball: win detection + base payout +
    /// bet-type specific multipliers (custom numbers, green boosters, crimson
    /// consolation, mirror wins).
    #[allow(clippy::too_many_arguments)]
    fn resolve_single_bet(
        &self,
        _side: Side,
        bet: &crate::bets::Bet,
        landed: u32,
        wheel: &crate::wheel::WheelConfig,
        levels: &ColorLevels,
        board: &crate::board::BoardModifiers,
        losing: bool,
        green_voided: bool,
    ) -> Option<ResolvedBet> {
        let bt = bet.bet_type;
        if !bt.is_win_on(wheel, landed, levels, Some(board)) {
            return None;
        }
        // Base multiplier (§10.1 step 1) + board custom numbers (step 1 note).
        let mut mult = bt.base(wheel, levels);
        if let BetType::Number(n) = bt {
            if let Some(custom) = board.custom_number_multipliers.get(&n) {
                mult = *custom;
            }
            // Card-armed single-number boosts fold on top: exact number, the
            // single-number family arm (SINGLE_OUT), and the High/Low/Prime
            // zone arms when the number qualifies (§10.1 step 3).
            mult *= board.payout_multiplier_for(&MultTarget::Number(n));
            mult *= board.payout_multiplier_for(&MultTarget::SingleNumber);
            if (1..=18).contains(&n) {
                mult *= board.payout_multiplier_for(&MultTarget::Low);
            }
            if (19..=36).contains(&n) {
                mult *= board.payout_multiplier_for(&MultTarget::High);
            }
            if crate::cards::effects::is_prime(n) {
                mult *= board.payout_multiplier_for(&MultTarget::Prime);
            }
        }
        match bt {
            BetType::Red => mult *= board.payout_multiplier_for(&MultTarget::Red),
            BetType::Black => mult *= board.payout_multiplier_for(&MultTarget::Black),
            BetType::Odd => mult *= board.payout_multiplier_for(&MultTarget::Odd),
            BetType::Even => mult *= board.payout_multiplier_for(&MultTarget::Even),
            BetType::Dozen(d) => mult *= board.payout_multiplier_for(&MultTarget::Dozen(d)),
            BetType::Column(c) => mult *= board.payout_multiplier_for(&MultTarget::Column(c)),
            BetType::Green => {
                if green_voided {
                    return Some(ResolvedBet {
                        bet_type: bt,
                        amount: bet.amount,
                        multiplier: 0.0,
                        payout: 0.0,
                    });
                }
                // Card-armed green multiplier folds with the base (§10.1).
                mult *= board.payout_multiplier_for(&MultTarget::Green);
                // Emerald Forest doubles the green multiplier (§10.1).
                if board.emerald_forest_active {
                    mult *= 2.0;
                }
                // GREEN_RIPPLE: +5 × (green slots + extra ladder greens).
                if board.green_ripple_active {
                    let greens = wheel.green_numbers.len() as f32
                        + green_ladder_slots(levels.get(SlotColor::Green)).len() as f32;
                    mult += 5.0 * greens;
                }
            }
            // Crimson consolation: ×2 while the bettor is losing (12 vs 6).
            BetType::Crimson if losing => mult *= 2.0,
            _ => {}
        }
        let payout = bet.amount as f32 * mult.max(0.0);
        Some(ResolvedBet { bet_type: bt, amount: bet.amount, multiplier: mult, payout })
    }

    /// §10.2 special-color effects + lucky-number heals for a landed ball.
    /// Returns (bonus PTS, stun turns dealt to the opponent).
    fn apply_landed_side_effects(
        &mut self,
        side: Side,
        landed: u32,
        levels: &ColorLevels,
    ) -> (u16, u16) {
        // Folded snapshot so stack-armed effects (Golden Heist) are visible.
        let board = if side == Side::Player {
            self.merged_board(side)
        } else {
            self.board_for(side).clone()
        };
        let color =
            self.wheel(side).effective_color(landed, levels.get(SlotColor::Green), Some(&board));
        let level = |levels: &ColorLevels, c: SlotColor| levels.get(c);
        let mut pts = 0u16;
        let mut stun = 0u16;
        match color {
            SlotColor::Gold => {
                // Landed slot + next adjacent slot permanently become gold.
                let idx = self.wheel(side).numbers.iter().position(|&n| n == landed);
                let adjacent = idx.and_then(|i| {
                    self.wheel(side).numbers.get((i + 1) % self.wheel(side).len()).copied()
                });
                let gold_level = level(levels, SlotColor::Gold);
                pts = 15 + (gold_level.saturating_sub(1) as u16) * 5;
                self.wheel_mut(side).set_slot_color(landed, SlotColor::Gold);
                if let Some(next) = adjacent {
                    self.wheel_mut(side).set_slot_color(next, SlotColor::Gold);
                }
                if board.golden_heist_active {
                    *self.pool_mut(side) += board.golden_heist_amount;
                }
            }
            SlotColor::Purple => {
                let l = level(levels, SlotColor::Purple);
                pts = 20 + (l.saturating_sub(1) as u16) * 5;
                stun = 1 + (l.saturating_sub(1) / 2) as u16;
            }
            SlotColor::Cyan => {
                let l = level(levels, SlotColor::Cyan);
                pts = 10 + (l.saturating_sub(1) as u16) * 3;
                // Chips pool refilled to turn-start value (max of current vs backup).
                let backup = self.turn_start_pool;
                let pool = self.pool_mut(side);
                *pool = (*pool).max(backup);
                // Draws 2 + (L−1) (player only; enemy has no hand).
                if side == Side::Player {
                    let draws = 2 + l.saturating_sub(1);
                    self.draw_free(draws);
                }
            }
            SlotColor::Crimson => {
                let l = level(levels, SlotColor::Crimson);
                // Consolation bonus PTS: losing 15+5(L−1), winning 5+2(L−1).
                if self.is_losing(side) {
                    pts = 15 + (l.saturating_sub(1) as u16) * 5;
                } else {
                    pts = 5 + (l.saturating_sub(1) as u16) * 2;
                }
            }
            _ => {}
        }
        (pts, stun)
    }

    /// §10.4 zone triggers for a landed ball; returns HP healed.
    fn apply_zone_triggers(&mut self, side: Side, landed: u32) -> u16 {
        // The folded snapshot (stack + battle-owned) is authoritative: zone
        // marks armed through the card stack must trigger (§10.4).
        let board = if side == Side::Player {
            self.merged_board(side)
        } else {
            self.board_for(side).clone()
        };
        let mut healed = 0u16;
        if let Some(mine) = board.zone(ZoneKind::ChipMine) {
            if mine.contains(&landed) {
                *self.pool_mut(side) += board.zone_value(ZoneKind::ChipMine, landed);
            }
        }
        if let Some(fountain) = board.zone(ZoneKind::LifeFountain) {
            if fountain.contains(&landed) && self.healing_allowed() && side == Side::Player {
                let heal = board.zone_value(ZoneKind::LifeFountain, landed);
                self.player_hp = self.player_hp.saturating_add(heal).min(self.player_max_hp);
                healed += heal;
            }
        }
        if let Some(danger) = board.zone(ZoneKind::DangerZone) {
            if danger.contains(&landed) {
                match self.combat_mode {
                    super::state::CombatMode::Points => {
                        *self.pool_mut(side) += board.zone_value(ZoneKind::DangerZone, landed)
                    }
                    super::state::CombatMode::Damage => {
                        // Direct enemy damage in legacy mode.
                        if side == Side::Player {
                            self.enemy_hp = self
                                .enemy_hp
                                .saturating_sub(board.zone_value(ZoneKind::DangerZone, landed));
                        }
                    }
                }
            }
        }
        if let Some(cursed) = board.zone(ZoneKind::Cursed) {
            if cursed.contains(&landed) {
                // Opponent stunned +2 turns.
                match side {
                    Side::Player => self.enemy_stun_turns += 2,
                    Side::Enemy => self.player_stun_turns += 2,
                }
            }
        }
        // §4.7 lucky_seven upgrade: land on a lucky number → heal.
        if side == Side::Player && self.healing_allowed() {
            if let Some(&heal) = self.player_wheel.lucky_numbers.get(&landed) {
                self.player_hp = self.player_hp.saturating_add(heal).min(self.player_max_hp);
                healed += heal;
            }
        }
        healed
    }

    /// Board state used by the pipeline: battle-owned state as the base, then
    /// the player's modifier stack folded on top (card arms). Enemy: plain
    /// board (intents may arm flags).
    pub fn merged_board(&self, side: Side) -> crate::board::BoardModifiers {
        match side {
            Side::Player => {
                let mut board = self.player_board.clone();
                self.player_stack.fold_into(&mut board);
                board
            }
            Side::Enemy => self.enemy_board.clone(),
        }
    }

    fn board_for(&self, side: Side) -> &crate::board::BoardModifiers {
        match side {
            Side::Player => &self.player_board,
            Side::Enemy => &self.enemy_board,
        }
    }
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Player => Side::Enemy,
            Side::Enemy => Side::Player,
        }
    }
}

/// Zone multiplier for a landed slot from felt marks (§10.1 step 3).
fn zone_multiplier(
    board: &crate::board::BoardModifiers,
    wheel: &crate::wheel::WheelConfig,
    _landed: u32,
) -> f32 {
    // Zone marks are per-slot; lucky ×1.5, goldFoil ×10, copperPlate ×1.5.
    let _ = wheel;
    let mut mult = 1.0f32;
    if let Some(lucky) = board.zone(roulette_content::schema::ZoneKind::Lucky) {
        if lucky.contains(&_landed) {
            mult *= 1.5;
        }
    }
    if let Some(foil) = board.zone(roulette_content::schema::ZoneKind::GoldFoil) {
        if foil.contains(&_landed) {
            mult *= 10.0;
        }
    }
    if let Some(copper) = board.zone(roulette_content::schema::ZoneKind::CopperPlate) {
        if copper.contains(&_landed) {
            mult *= 1.5;
        }
    }
    mult
}

/// Streak damage multiplier (§10.1 step 5): min(4.0, 1 + 0.5 × streak count)
/// when the streak is active and the landed slot matches the streak color.
fn streak_multiplier(
    board: &crate::board::BoardModifiers,
    wheel: &crate::wheel::WheelConfig,
    landed: u32,
    levels: &ColorLevels,
) -> f32 {
    let color = wheel.effective_color(landed, levels.get(SlotColor::Green), Some(board));
    let (active, count) = match color {
        SlotColor::Red => (board.red_streak_active, board.red_streak_count),
        SlotColor::Black => (board.black_streak_active, board.black_streak_count),
        _ => (false, 0),
    };
    if !active || count == 0 {
        return 1.0;
    }
    (4.0f32).min(1.0 + 0.5 * count as f32)
}
