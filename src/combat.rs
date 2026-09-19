//! Modular Combat State Machine and Turn Execution Pipeline.
//!
//! Orchestrates combat and point-round lifecycle using abstract [`GameDevice`], [`Target`], and [`CombatMode`] traits.

use crate::bet::Bet;
use crate::cards::{Card, CardEffect};
use crate::device::{EuropeanWheel, GameDevice, OutcomeSlot, SlotColor};
use crate::mode::{CombatMode, CombatOutcome, GameModeKind, Standard1v1Mode};
use crate::rng::Rng;
use crate::target::{Enemy, Target};

/// Represents player state during a combat or point-round encounter.
#[derive(Debug, Clone)]
pub struct Player {
    /// Optional health points (tracked in Combat/Horde modes, None in Point Round mode).
    pub hp: Option<i32>,
    /// Optional maximum health points cap.
    pub max_hp: Option<i32>,
    /// Available chip balance for placing bets.
    pub chips: u32,
    /// Draw pile of modifier cards.
    pub deck: Vec<Card>,
    /// Active playable cards currently in hand.
    pub hand: Vec<Card>,
    /// Discard pile for played or discarded cards.
    pub discard: Vec<Card>,
}

impl Player {
    /// Creates a new player for a specific game mode kind.
    pub fn new_for_mode(mode_kind: GameModeKind, max_hp: Option<i32>, starting_chips: u32) -> Self {
        Player {
            hp: max_hp,
            max_hp,
            chips: starting_chips,
            deck: Card::starter_deck_for_mode(mode_kind),
            hand: Vec::new(),
            discard: Vec::new(),
        }
    }

    /// Helper for standard 1v1 combat initialization.
    pub fn new(max_hp: i32, starting_chips: u32) -> Self {
        Self::new_for_mode(GameModeKind::Combat1v1, Some(max_hp), starting_chips)
    }

    /// Draws `count` cards from deck to hand, automatically shuffling the discard pile into the deck when depleted.
    pub fn draw_cards(&mut self, count: usize, rng: &mut Rng) {
        for _ in 0..count {
            if self.deck.is_empty() && !self.discard.is_empty() {
                self.deck.append(&mut self.discard);
                rng.shuffle(&mut self.deck);
            }
            if let Some(card) = self.deck.pop() {
                self.hand.push(card);
            }
        }
    }
}

/// Summary report returned after a turn/hand completes.
#[derive(Debug, Clone)]
pub struct TurnResult {
    /// The outcome slot landed on during spin/roll.
    pub slot_landed: OutcomeSlot,
    /// Total damage dealt to targets / points scored from winning bets.
    pub total_damage_dealt: i32,
    /// Total chips won by player from payouts.
    pub total_chips_won: u32,
    /// Damage dealt by enemies to player HP (0 in modes without enemy attacks).
    pub enemy_damage_dealt: i32,
    /// Current encounter outcome status.
    pub outcome: CombatOutcome,
    /// Sequential text log of events occurring during turn.
    pub log_messages: Vec<String>,
}

/// Complete modular battle / round encounter state.
#[derive(Debug, Clone)]
pub struct CombatState {
    /// Player state.
    pub player: Player,
    /// Active targets / opponents in combat (or score target dummy in Point Round mode).
    pub targets: Vec<Box<dyn Target>>,
    /// Active randomizer device (Wheel, Dice, Slot Machine, Pachinko).
    pub device: Box<dyn GameDevice>,
    /// Active combat mode strategy (1v1, Horde, Point Round).
    pub mode: Box<dyn CombatMode>,
    /// Current turn / hand counter.
    pub turn_number: u32,
    /// Seedable PRNG instance.
    pub rng: Rng,
}

impl CombatState {
    /// Creates a standard 1v1 battle encounter against "The Cursed Croupier" on a European Wheel.
    pub fn new(seed: &str) -> Self {
        let mut rng = Rng::from_str(seed);
        let mode: Box<dyn CombatMode> = Box::new(Standard1v1Mode::default());
        let mut player = Player::new_for_mode(mode.mode_kind(), Some(100), 50);
        player.draw_cards(3, &mut rng);

        let target: Box<dyn Target> = Box::new(Enemy::create_pit_boss());
        let device: Box<dyn GameDevice> = Box::new(EuropeanWheel::new());

        CombatState {
            player,
            targets: vec![target],
            device,
            mode,
            turn_number: 1,
            rng,
        }
    }

    /// Creates a fully custom encounter with custom device, targets, and game mode.
    pub fn new_custom(
        seed: &str,
        device: Box<dyn GameDevice>,
        targets: Vec<Box<dyn Target>>,
        mode: Box<dyn CombatMode>,
    ) -> Self {
        let mut rng = Rng::from_str(seed);
        let max_hp = if mode.has_player_hp() { Some(100) } else { None };
        let mut player = Player::new_for_mode(mode.mode_kind(), max_hp, 50);
        player.draw_cards(3, &mut rng);

        CombatState {
            player,
            targets,
            device,
            mode,
            turn_number: 1,
            rng,
        }
    }

    /// Convenience getter for first target.
    pub fn enemy(&self) -> &dyn Target {
        self.targets[0].as_ref()
    }

    /// Convenience mutable getter for first target.
    pub fn enemy_mut(&mut self) -> &mut dyn Target {
        self.targets[0].as_mut()
    }

    /// Executes a complete turn/hand lifecycle against active device and target entities.
    pub fn execute_turn(
        &mut self,
        played_card_indices: &[usize],
        bets: &[Bet],
    ) -> TurnResult {
        let mut logs = Vec::new();
        logs.push(format!("--- TURN/HAND {} START [{}] ---", self.turn_number, self.device.name()));

        let mut red_payout_boost = 0.0;
        let mut reroll_on_loss = false;
        let mut double_down = false;

        // 1. Play Cards
        let mut active_cards = Vec::new();
        for &idx in played_card_indices.iter().rev() {
            if idx < self.player.hand.len() {
                let card = self.player.hand.remove(idx);
                logs.push(format!("Player played card: {}", card.name));

                // Apply device modifications
                card.apply_to_device(self.device.as_mut());

                // Handle card effects
                match &card.effect {
                    CardEffect::BoostPayout { color, bonus_multiplier } => {
                        if *color == SlotColor::Red {
                            red_payout_boost += bonus_multiplier;
                        }
                    }
                    CardEffect::RerollOnLoss => reroll_on_loss = true,
                    CardEffect::DoubleDown => double_down = true,
                    CardEffect::GainChips(amount) => {
                        self.player.chips += amount;
                        logs.push(format!("Activated Chip Surge: Gained {} chips!", amount));
                    }
                    CardEffect::BloodSacrifice { hp_cost, chips_gained } => {
                        if let Some(ref mut hp) = self.player.hp {
                            *hp -= hp_cost;
                            logs.push(format!("Sacrificed {} HP for {} chips!", hp_cost, chips_gained));
                        }
                        self.player.chips += chips_gained;
                    }
                    CardEffect::HandSacrifice { hands_cost: _, chips_gained } => {
                        self.player.chips += chips_gained;
                        logs.push(format!("Activated Greed Pact: Gained {} chips!", chips_gained));
                    }
                    _ => {}
                }
                active_cards.push(card);
            }
        }

        // Deduct bet cost from player chips
        let total_bet_amount: u32 = bets.iter().map(|b| b.amount).sum();
        if self.player.chips >= total_bet_amount {
            self.player.chips -= total_bet_amount;
        }

        // 2. Spin/Roll Device (Deterministic)
        let mut slot = self.device.spin(&mut self.rng);
        logs.push(format!("Device spun... Landed on Number {} ({})", slot.number, slot.color));

        // Evaluate Bets & Damage / Points
        let mut win = false;
        let mut total_damage = 0.0;
        let mut total_chips_won = 0;

        for bet in bets {
            if bet.evaluates_win(&slot) {
                win = true;
                let mut mult = bet.base_payout_multiplier();
                if slot.color == SlotColor::Red {
                    mult += red_payout_boost;
                }
                if double_down {
                    mult *= 2.0;
                }
                let payout = (bet.amount as f64 * mult) as u32;
                total_chips_won += payout;
                total_damage += payout as f64;
                logs.push(format!("BET WON! Type: {:?}, Payout/Score: {} pts", bet.bet_type, payout));
            }
        }

        // Reroll logic
        if !win && reroll_on_loss {
            logs.push("RerollOnLoss triggered! Respinning device...".to_string());
            slot = self.device.spin(&mut self.rng);
            logs.push(format!("Respun device landed on Number {} ({})", slot.number, slot.color));
            for bet in bets {
                if bet.evaluates_win(&slot) {
                    let mult = bet.base_payout_multiplier();
                    let payout = (bet.amount as f64 * mult) as u32;
                    total_chips_won += payout;
                    total_damage += payout as f64;
                    logs.push(format!("REROLL BET WON! Payout/Score: {} pts", payout));
                }
            }
        }

        let damage_dealt = total_damage as i32;
        self.player.chips += total_chips_won;

        // Apply damage/score to primary target
        if let Some(target) = self.targets.iter_mut().find(|t| t.is_alive()) {
            target.take_damage(damage_dealt);
        }

        // 3. Enemy Action Phase (Skipped if mode has no enemy attacks, e.g. PointRoundMode)
        let mut total_enemy_dmg = 0;
        if self.mode.has_enemy_attacks() {
            for target in self.targets.iter_mut() {
                if target.is_alive() {
                    let (enemy_dmg, log_msg) = target.execute_intent(self.device.as_mut(), &mut self.rng);
                    if let Some(ref mut hp) = self.player.hp {
                        *hp -= enemy_dmg;
                    }
                    total_enemy_dmg += enemy_dmg;
                    logs.push(log_msg);
                }
            }
        }

        // 4. Evaluate Victory / Defeat Outcome via CombatMode Strategy
        let outcome = self.mode.evaluate_outcome(self.player.hp, &self.targets, self.turn_number);
        match outcome {
            CombatOutcome::PlayerVictory => logs.push("VICTORY! Target score / encounter completed!".to_string()),
            CombatOutcome::PlayerDefeat => logs.push("DEFEAT! Encounter failed!".to_string()),
            CombatOutcome::TurnLimitReached => logs.push("DRAW / END! Limit reached!".to_string()),
            CombatOutcome::InProgress => {}
        }

        // Cleanup turn hand
        self.player.discard.append(&mut active_cards);
        self.player.draw_cards(2, &mut self.rng);
        self.turn_number += 1;

        TurnResult {
            slot_landed: slot,
            total_damage_dealt: damage_dealt,
            total_chips_won,
            enemy_damage_dealt: total_enemy_dmg,
            outcome,
            log_messages: logs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modular_combat_initialization() {
        let state = CombatState::new("test_seed");
        assert_eq!(state.player.hp, Some(100));
        assert_eq!(state.enemy().hp(), 100);
        assert_eq!(state.turn_number, 1);
    }
}
