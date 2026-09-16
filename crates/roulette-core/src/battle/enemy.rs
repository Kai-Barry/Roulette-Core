//! Enemy turn (TASK-028, §7): EV-based bet choice, spin resolution into
//! `enemy_chips_pool`, and intent execution on a winning spin. Deterministic
//! under the caller's RNG substream.

use roulette_content::schema::{EnemyAction, SlotColor};

use super::state::{BattlePhase, BattleState, Side, SpinInput, SpinOutcome};
use crate::bets::{Bet, BetType};
use crate::rng::Rng;
use crate::wheel::ColorLevels;

impl BattleState {
    /// Runs the enemy turn (§7.2): pick 1–3 bets, spin its wheel, resolve, and
    /// execute the queued intent on a winning spin. Stunned enemies skip the
    /// turn (§7.3).
    pub fn enemy_take_turn(&mut self, rng: &mut Rng, landed: u32) -> Option<SpinOutcome> {
        if self.enemy_stun_turns > 0 {
            self.enemy_stun_turns -= 1;
            self.begin_betting(Side::Player);
            return None;
        }
        self.last_executed_intent = None;
        self.begin_betting(Side::Enemy);
        let chosen = self.enemy_choose_bets(rng);
        // Stakes leave the house pool at placement, mirroring the player's
        // `place_bet` (§3.4; resolve.rs "losing stakes vanish (already
        // deducted at placement)"). Without this the house line is a free
        // roll: pools only ever grow and the pts race at the round limit is
        // unwinnable (design-audit B1 root cause).
        let committed: u16 = chosen.iter().map(|(_, a)| *a).sum();
        self.enemy_chips_pool = self.enemy_chips_pool.saturating_sub(committed);
        for (bet_type, amount) in &chosen {
            self.enemy_bets.push(Bet { bet_type: *bet_type, amount: *amount });
        }
        // Observability (B4): snapshot before resolve clears `enemy_bets`.
        self.last_enemy_bets =
            chosen.into_iter().map(|(bet_type, amount)| Bet { bet_type, amount }).collect();
        self.phase = BattlePhase::Spinning;
        let outcome = self.resolve_spin(&SpinInput {
            side: Side::Enemy,
            landed,
            extra_balls: Vec::new(),
            prediction: None,
            low_rarity_physics_played: false,
        });
        if outcome.total_payout > 0 {
            self.execute_enemy_intent();
        }
        self.advance_intent();
        Some(outcome)
    }

    /// §7.4: enumerate candidate bets, score by expected value, take the best
    /// line with probability `difficulty` (else one random viable bet); stakes
    /// split ~50/30/rest over up to 3 bets; risk tolerance scales with being
    /// behind on score.
    pub fn enemy_choose_bets(&self, rng: &mut Rng) -> Vec<(BetType, u16)> {
        let wheel = &self.enemy_wheel;
        let slots = wheel.len().max(1) as f32;
        let colors_of = |target: SlotColor| {
            wheel.numbers.iter().filter(|&&n| wheel.effective_color(n, 0, None) == target).count()
                as f32
        };
        let greens = colors_of(SlotColor::Green);
        let reds = colors_of(SlotColor::Red);
        let blacks = colors_of(SlotColor::Black);
        let parity = |even: bool| {
            wheel
                .numbers
                .iter()
                .filter(|&&n| {
                    n != 0
                        && !wheel.green_numbers.contains(&n)
                        && (n % 2 == if even { 0 } else { 1 })
                })
                .count() as f32
        };

        let mut candidates: Vec<(BetType, f32, f32)> = Vec::new(); // (type, p_win, payout)
        if reds > 0.0 {
            candidates.push((BetType::Red, reds / slots, self.bet_base_for(BetType::Red)));
        }
        if blacks > 0.0 {
            candidates.push((BetType::Black, blacks / slots, self.bet_base_for(BetType::Black)));
        }
        if greens > 0.0 {
            candidates.push((BetType::Green, greens / slots, self.bet_base_for(BetType::Green)));
        }
        candidates.push((BetType::Odd, parity(true) / slots, 2.0));
        candidates.push((BetType::Even, parity(false) / slots, 2.0));
        // Dozen/column coverage is computed from the actual felt (§ readable
        // odds): on 37/38-slot wheels a dozen is 12/slots (EV < 0 at 3×), and
        // on short wheels some groups are simply absent.
        let dozen_count = |lo: u32| -> f32 {
            wheel.numbers.iter().filter(|&&n| n >= lo && n < lo + 12).count() as f32
        };
        for d in 1..=3u8 {
            let lo = u32::from(d) * 12 - 11;
            candidates.push((BetType::Dozen(d), dozen_count(lo) / slots, 3.0));
        }
        let column_count = |rem: u8| {
            wheel.numbers.iter().filter(|&&n| n > 0 && n % 3 == u32::from(rem)).count() as f32
        };
        for c in 1..=3u8 {
            candidates.push((BetType::Column(c), column_count(c % 3) / slots, 3.0));
        }
        // Special colors the enemy wheel carries, at their default payouts.
        for (&n, &color) in wheel.slot_colors.iter() {
            let base = match color {
                SlotColor::Gold | SlotColor::Purple | SlotColor::Cyan => 4.0,
                SlotColor::Crimson => 6.0,
                _ => 0.0,
            };
            if base > 0.0 {
                candidates.push((BetType::Number(n), 1.0 / slots, base));
            }
        }

        let mut scored: Vec<(BetType, f32)> =
            candidates.into_iter().map(|(t, p, pay)| (t, p * (pay - 1.0) - (1.0 - p))).collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        // §3.2: the house always bets on its own felt. When no line is
        // positive-EV (true on honest wheels), it still plays its least-bad
        // candidates instead of folding — a silent house would starve the
        // pts race and never execute intents (design-audit B1 follow-up).
        let positive: Vec<(BetType, f32)> =
            scored.iter().copied().filter(|&(_, ev)| ev > 0.0).collect();
        let scored = if positive.is_empty() { scored } else { positive };

        // Risk tolerance (§7.4): scales toward 1.0 as the AI falls behind.
        let risk =
            (self.enemy_chips_pool.max(1) as f32 / self.chips_pool.max(1) as f32).clamp(0.1, 1.0);

        // Optimal line (top-2 when both positive-EV) with p=difficulty.
        let chosen: Vec<BetType> = if rng.next_f64() < self.enemy_difficulty as f64 {
            scored.iter().take(2).map(|(t, _)| *t).collect()
        } else {
            // `range_usize` is inclusive; `scored` is non-empty here.
            let idx = rng.range_usize(0, scored.len() - 1);
            vec![scored[idx].0]
        };

        // Committed chips: ~10% of pool at base risk up to ~50% at max risk.
        // Anti-snowball (design-audit B2): the fraction is taken of a
        // bounded bank — the §3.1 pool floor — so absolute commitment stops
        // growing once the house is ahead; compounding cannot outrun the
        // player's flat line forever.
        let stake_base = self.enemy_chips_pool.min(self.chips_pool);
        let commit = ((stake_base as f32) * (0.1 + 0.4 * risk)).floor() as u16;
        if commit == 0 {
            return Vec::new();
        }
        let count = chosen.len().min(3);
        let mut stakes = Vec::with_capacity(count);
        let mut remaining = commit;
        for (i, bet_type) in chosen.iter().enumerate() {
            let share = match (i, count) {
                (0, 1) => 1.0,
                (0, _) => 0.5,
                (1, _) => 0.6, // 30% of the pool = 60% of the remainder
                _ => 1.0,
            };
            let mut amount = ((remaining as f32) * share).floor() as u16;
            if i == count - 1 {
                amount = remaining;
            }
            amount = amount.min(remaining);
            if amount == 0 {
                continue;
            }
            remaining -= amount;
            stakes.push((*bet_type, amount));
            if remaining == 0 {
                break;
            }
        }
        stakes.retain(|(_, a)| *a > 0);
        stakes
    }

    fn bet_base_for(&self, bet_type: BetType) -> f32 {
        bet_type.base(&self.enemy_wheel, &ColorLevels::default())
    }

    /// §7.2/§7.3: execute the queued intent when the enemy wins its spin.
    pub fn execute_enemy_intent(&mut self) {
        let Some(intent) = self.enemy_intent.take() else {
            return;
        };
        // Observability (B5): keep the executed intent for the event payload.
        self.last_executed_intent = Some(intent.clone());
        match intent.action {
            EnemyAction::Attack => {
                let dmg = intent.value.min(self.player_hp);
                self.player_hp -= dmg;
                self.player_damage_taken += dmg;
            }
            EnemyAction::StealChips => {
                let drain = intent.value.min(self.chips_pool);
                self.chips_pool -= drain;
            }
            EnemyAction::PhysicsDebuff => {
                self.physics.friction *= 1.5;
                self.physics.target_zone_bias = 0.0;
            }
            EnemyAction::Shield => {
                self.enemy_board.global_multiplier *= 1.2;
            }
            EnemyAction::BlockRed => {
                self.player_board.block_red_active = true;
            }
            EnemyAction::Buff => {
                self.enemy_board.emerald_forest_active = true;
            }
        }
    }

    /// Advances the enemy intent pattern (4-move loop, §7.1).
    pub fn advance_intent(&mut self) {
        self.enemy_intent_index = (self.enemy_intent_index + 1) % 4;
    }
}
