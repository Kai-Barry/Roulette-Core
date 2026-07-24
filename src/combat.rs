use crate::cards::{Card, CardEffect};
use crate::rng::Rng;
use crate::wheel::{Bet, Slot, SlotColor, Wheel};

#[derive(Debug, Clone)]
pub struct Player {
    pub hp: i32,
    pub max_hp: i32,
    pub chips: u32,
    pub deck: Vec<Card>,
    pub hand: Vec<Card>,
    pub discard: Vec<Card>,
}

impl Player {
    pub fn new(max_hp: i32, starting_chips: u32) -> Self {
        Player {
            hp: max_hp,
            max_hp,
            chips: starting_chips,
            deck: Card::all_starter_cards(),
            hand: Vec::new(),
            discard: Vec::new(),
        }
    }

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

#[derive(Debug, Clone)]
pub enum EnemyIntent {
    Attack(i32),
    Block(i32),
    CorruptRed,
}

#[derive(Debug, Clone)]
pub struct Enemy {
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub intent: EnemyIntent,
}

impl Enemy {
    pub fn create_pit_boss() -> Self {
        Enemy {
            name: "The Cursed Croupier".to_string(),
            hp: 100,
            max_hp: 100,
            intent: EnemyIntent::Attack(15),
        }
    }

    pub fn roll_intent(&mut self, rng: &mut Rng) {
        let val = rng.range_i32(1, 3);
        self.intent = match val {
            1 => EnemyIntent::Attack(rng.range_i32(10, 20)),
            2 => EnemyIntent::Block(rng.range_i32(5, 15)),
            _ => EnemyIntent::CorruptRed,
        };
    }
}

#[derive(Debug, Clone)]
pub struct TurnResult {
    pub slot_landed: Slot,
    pub total_damage_dealt: i32,
    pub total_chips_won: u32,
    pub enemy_damage_dealt: i32,
    pub log_messages: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CombatState {
    pub player: Player,
    pub enemy: Enemy,
    pub wheel: Wheel,
    pub turn_number: u32,
    pub rng: Rng,
}

impl CombatState {
    pub fn new(seed: &str) -> Self {
        let mut rng = Rng::from_str(seed);
        let mut player = Player::new(100, 50);
        player.draw_cards(3, &mut rng);
        let enemy = Enemy::create_pit_boss();
        let wheel = Wheel::european();

        CombatState {
            player,
            enemy,
            wheel,
            turn_number: 1,
            rng,
        }
    }

    /// Execute a turn with selected active played cards and player bets.
    pub fn execute_turn(
        &mut self,
        played_card_indices: &[usize],
        bets: &[Bet],
    ) -> TurnResult {
        let mut logs = Vec::new();
        logs.push(format!("--- TURN {} START ---", self.turn_number));

        let mut red_payout_boost = 0.0;
        let mut reroll_on_loss = false;
        let mut double_down = false;

        // 1. Play Cards
        let mut active_cards = Vec::new();
        for &idx in played_card_indices.iter().rev() {
            if idx < self.player.hand.len() {
                let card = self.player.hand.remove(idx);
                logs.push(format!("Player played card: {}", card.name));
                
                // Apply wheel modifications
                card.apply_to_wheel(&mut self.wheel);

                // Handle other card effects
                match &card.effect {
                    CardEffect::BoostPayout { color, bonus_multiplier } => {
                        if *color == SlotColor::Red {
                            red_payout_boost += bonus_multiplier;
                        }
                    }
                    CardEffect::RerollOnLoss => reroll_on_loss = true,
                    CardEffect::DoubleDown => double_down = true,
                    CardEffect::BloodSacrifice { hp_cost, chips_gained } => {
                        self.player.hp -= hp_cost;
                        self.player.chips += chips_gained;
                        logs.push(format!("Sacrificed {} HP for {} chips!", hp_cost, chips_gained));
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

        // 2. Spin Wheel (Deterministic)
        let mut slot = self.wheel.spin(&mut self.rng);
        logs.push(format!("Roulette spun... Landed on Number {} ({})", slot.number, slot.color));

        // Evaluate Bets & Damage
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
                logs.push(format!("BET WON! Type: {:?}, Payout: {} dmg", bet.bet_type, payout));
            }
        }

        // Reroll logic
        if !win && reroll_on_loss {
            logs.push("RerollOnLoss triggered! Respinning wheel...".to_string());
            slot = self.wheel.spin(&mut self.rng);
            logs.push(format!("Respun wheel landed on Number {} ({})", slot.number, slot.color));
            for bet in bets {
                if bet.evaluates_win(&slot) {
                    let mult = bet.base_payout_multiplier();
                    let payout = (bet.amount as f64 * mult) as u32;
                    total_chips_won += payout;
                    total_damage += payout as f64;
                    logs.push(format!("REROLL BET WON! Payout: {} dmg", payout));
                }
            }
        }

        let damage_dealt = total_damage as i32;
        self.enemy.hp -= damage_dealt;
        self.player.chips += total_chips_won;

        // 3. Enemy Action
        let mut enemy_dmg = 0;
        if self.enemy.hp > 0 {
            match self.enemy.intent {
                EnemyIntent::Attack(dmg) => {
                    enemy_dmg = dmg;
                    self.player.hp -= enemy_dmg;
                    logs.push(format!("Enemy attacked for {} damage!", enemy_dmg));
                }
                EnemyIntent::Block(block) => {
                    self.enemy.hp += block;
                    logs.push(format!("Enemy blocked for {} shield!", block));
                }
                EnemyIntent::CorruptRed => {
                    self.wheel.recolor_range(1, 18, SlotColor::Green);
                    logs.push("Enemy corrupted slots 1-18 to Green!".to_string());
                }
            }
            self.enemy.roll_intent(&mut self.rng);
        } else {
            logs.push("VICTORY! Enemy defeated!".to_string());
        }

        // Cleanup turn hand
        self.player.discard.append(&mut active_cards);
        self.player.draw_cards(2, &mut self.rng);
        self.turn_number += 1;

        TurnResult {
            slot_landed: slot,
            total_damage_dealt: damage_dealt,
            total_chips_won,
            enemy_damage_dealt: enemy_dmg,
            log_messages: logs,
        }
    }
}
