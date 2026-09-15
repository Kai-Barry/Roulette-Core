//! Round end & outcomes (TASK-029, §3.2/§10.3/§2.5).

use roulette_content::schema::EnemyIntentDef;

use super::state::{BattleOutcome, BattleState, Side};

impl BattleState {
    /// Arms the enemy's current intent from its 4-move pattern.
    pub fn load_intent(&mut self, pattern: &[EnemyIntentDef; 4]) {
        let def = &pattern[self.enemy_intent_index as usize];
        self.enemy_intent = Some(super::state::EnemyIntent::from_def(def, self.enemy_intent_index));
    }

    /// LUCKY_CHARM policy (§10.5): reroll one all-zero spin when a charm is
    /// available. Callers re-run the spin and resolve again.
    pub fn should_use_lucky_charm(&self, payout: u16) -> bool {
        payout == 0 && self.player_board.lucky_charms > 0
    }

    pub fn consume_lucky_charm(&mut self) -> bool {
        if self.player_board.lucky_charms > 0 {
            self.player_board.lucky_charms -= 1;
            true
        } else {
            false
        }
    }

    /// Ends the round (after both sides spun): tick modifier durations, apply
    /// Curse of Blood, advance the round, and evaluate the outcome. Sudden
    /// death grants one extra round per tie (§3.5).
    pub fn end_round(&mut self) -> BattleOutcome {
        // OnRoundEnd hook (§6.4): Capital Venture pays on a round win and the
        // round's played cards are filed (temp cards exiled).
        crate::cards::effects::on_round_end(self, self.chips_pool > self.enemy_chips_pool);

        // Tick Fight-scope modifiers at the round boundary.
        self.player_stack.tick_at_round_end();

        // Curse of Blood: −HP at the start of every round (§8).
        let blood = self.hp_loss_per_round();
        if blood > 0 && self.round > 1 || blood > 0 {
            self.player_hp = self.player_hp.saturating_sub(blood);
        }
        if self.player_hp == 0 {
            return BattleOutcome::PlayerDefeat;
        }

        if self.round >= self.max_rounds && !self.is_sudden_death {
            return self.evaluate_round_limit();
        }
        if self.round >= self.max_rounds && self.is_sudden_death {
            // Sudden-death ties repeat: evaluate again each extra round.
            return self.evaluate_round_limit();
        }
        self.round += 1;
        BattleOutcome::InProgress
    }

    fn evaluate_round_limit(&mut self) -> BattleOutcome {
        match self.chips_pool.cmp(&self.enemy_chips_pool) {
            std::cmp::Ordering::Greater => BattleOutcome::PlayerVictory,
            std::cmp::Ordering::Less => BattleOutcome::PlayerDefeat,
            std::cmp::Ordering::Equal => {
                if self.is_sudden_death {
                    // Repeated tie: another sudden-death round.
                    self.round += 1;
                    BattleOutcome::SuddenDeath
                } else {
                    self.is_sudden_death = true;
                    self.round += 1;
                    BattleOutcome::SuddenDeath
                }
            }
        }
    }

    /// Turn handoff (§3.2): after the player's spin resolves, the enemy acts;
    /// after the enemy's spin the round ends.
    pub fn advance_after_spin(&mut self) {
        match self.turn {
            Side::Player => {
                if self.player_stun_turns > 0 {
                    self.player_stun_turns -= 1;
                }
                // Enemy acts next.
                self.begin_betting(Side::Enemy);
            }
            Side::Enemy => {
                self.begin_betting(Side::Player);
            }
        }
    }

    /// Full battle loop helper for headless tests: plays both sides of one
    /// round with pre-rolled landings and returns the outcome.
    pub fn play_round_headless(
        &mut self,
        enemy_rng: &mut crate::rng::Rng,
        player_landed: u32,
        enemy_landed: u32,
    ) -> BattleOutcome {
        self.begin_betting(Side::Player);
        let _ = enemy_rng;
        let input = super::state::SpinInput {
            side: Side::Player,
            landed: player_landed,
            extra_balls: Vec::new(),
            prediction: None,
            low_rarity_physics_played: false,
        };
        self.resolve_spin(&input);
        self.advance_after_spin();
        self.enemy_take_turn(enemy_rng, enemy_landed);
        self.end_round()
    }
}
