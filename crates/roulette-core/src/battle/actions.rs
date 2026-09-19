//! Player turn actions (TASK-025, §3.2): play cards, buy draws on the ladder,
//! place/remove/clear/rebet bets, sacrifice, spin.

use super::state::{ActionError, ActiveCard, BattlePhase, BattleState, CardInstance, Side};
use crate::bets::{Bet, BetType};

impl BattleState {
    fn require_player_turn(&self) -> Result<(), ActionError> {
        if self.turn != Side::Player || self.phase != BattlePhase::Betting {
            return Err(ActionError::NotYourTurn);
        }
        Ok(())
    }

    /// Plays a hand card: moves it off the hand (caller dispatches effects at
    /// the §6.4 hook points, Phase 5) and onto the felt face-up.
    pub fn play_card(&mut self, hand_index: usize) -> Result<CardInstance, ActionError> {
        self.require_player_turn()?;
        if hand_index >= self.hand.len() {
            return Err(ActionError::NoCardAt(hand_index));
        }
        Ok(self.hand.remove(hand_index))
    }

    /// Charges a card's ⚡ cost; free-card credits are consumed first.
    pub fn pay_card_cost(&mut self, cost: u8) -> Result<(), ActionError> {
        let cost = if self.free_cards_active > 0 {
            self.free_cards_active -= 1;
            0
        } else {
            cost as u16
        };
        if self.chips_pool < cost {
            return Err(ActionError::NotEnoughChips);
        }
        self.chips_pool -= cost;
        Ok(())
    }

    /// Moves a played card onto the felt face-up.
    pub fn commit_played_card(&mut self, instance: CardInstance, locked: bool) {
        self.active_played.push(ActiveCard { instance, locked_at_spin: locked });
    }

    /// Buys one draw at the ladder price (§3.1: free 1st, then 3/5/7/9/11…;
    /// Avarice flat 3). Reshuffles the discard pile into the draw pile when it
    /// empties.
    pub fn buy_draw(&mut self) -> Result<CardInstance, ActionError> {
        self.require_player_turn()?;
        if self.hand.len() >= self.hand_limit() {
            return Err(ActionError::HandFull);
        }
        if self.draw_pile.is_empty() {
            if self.discard_pile.is_empty() {
                return Err(ActionError::EmptyDrawPileAndDiscard);
            }
            std::mem::swap(&mut self.draw_pile, &mut self.discard_pile);
        }
        let cost = self.draw_cost();
        if self.chips_pool < cost {
            return Err(ActionError::NotEnoughChips);
        }
        self.chips_pool -= cost;
        self.draws_this_turn = self.draws_this_turn.saturating_add(1);
        let card = self.draw_pile.pop().ok_or(ActionError::EmptyDrawPileAndDiscard)?;
        self.hand.push(card.clone());
        Ok(card)
    }

    /// Card-effect draw (QUICK_DRAW etc., §6.4): free, bypasses the ladder but
    /// still reshuffles from the discard pile.
    pub fn draw_free(&mut self, count: u8) -> u8 {
        let mut drawn = 0;
        for _ in 0..count {
            if self.draw_pile.is_empty() {
                if self.discard_pile.is_empty() {
                    break;
                }
                std::mem::swap(&mut self.draw_pile, &mut self.discard_pile);
            }
            let Some(card) = self.draw_pile.pop() else {
                break;
            };
            self.hand.push(card);
            drawn += 1;
        }
        drawn
    }

    /// Places/merges a bet (§3.4: same cell+type merges; chip denominations
    /// 1/5/10 arrive as `amount`). Lead curse caps single slots at 5.
    pub fn place_bet(&mut self, bet_type: BetType, amount: u16) -> Result<(), ActionError> {
        self.require_player_turn()?;
        if amount == 0 {
            return Err(ActionError::InvalidBet("zero amount".into()));
        }
        if let Err(e) = Bet::new(bet_type, 1) {
            return Err(ActionError::InvalidBet(e.to_string()));
        }
        if let BetType::Number(n) = bet_type {
            if !self.player_wheel.numbers.contains(&n) {
                return Err(ActionError::InvalidBet(format!("number {n} not on wheel")));
            }
        }
        let cap = self.bet_cap();
        let existing = self.bets.iter_mut().find(|b| b.bet_type == bet_type);
        if let Some(b) = existing {
            let new_amount = b.amount + amount;
            if let Some(cap) = cap {
                if new_amount > cap {
                    return Err(ActionError::BetCapExceeded(cap));
                }
            }
            b.amount = new_amount;
        } else {
            if let Some(cap) = cap {
                if amount > cap {
                    return Err(ActionError::BetCapExceeded(cap));
                }
            }
            self.bets.push(Bet { bet_type, amount });
        }
        self.chips_pool = self.chips_pool.checked_sub(amount).ok_or(ActionError::NotEnoughChips)?;
        Ok(())
    }

    /// Removes part of a bet (right-click; proportional refund, §3.4).
    pub fn remove_bet(&mut self, bet_type: BetType, amount: u16) -> Result<u16, ActionError> {
        self.require_player_turn()?;
        let Some(pos) = self.bets.iter().position(|b| b.bet_type == bet_type) else {
            return Err(ActionError::NoBetToRemove);
        };
        let removed = amount.min(self.bets[pos].amount);
        self.bets[pos].amount -= removed;
        if self.bets[pos].amount == 0 {
            self.bets.remove(pos);
        }
        self.chips_pool += removed;
        Ok(removed)
    }

    /// Clears all bets with a full refund and backs the layout up for REBET.
    pub fn clear_bets(&mut self) -> u16 {
        let refund: u16 = self.bets.iter().map(|b| b.amount).sum();
        self.rebet_backup = std::mem::take(&mut self.bets);
        self.chips_pool += refund;
        refund
    }

    /// Re-places the last cleared layout if affordable (§3.4 REBET).
    pub fn rebet(&mut self) -> Result<(), ActionError> {
        self.require_player_turn()?;
        let total: u16 = self.rebet_backup.iter().map(|b| b.amount).sum();
        if self.chips_pool < total {
            return Err(ActionError::NotEnoughChips);
        }
        let backup = self.rebet_backup.clone();
        for bet in backup {
            let existing = self.bets.iter_mut().find(|b| b.bet_type == bet.bet_type);
            if let Some(b) = existing {
                b.amount += bet.amount;
            } else {
                self.bets.push(bet);
            }
        }
        self.chips_pool -= total;
        Ok(())
    }

    /// Sacrifice (§3.2 step 5): −10 PTS (points mode) or −5 HP (damage mode)
    /// for +5 chips.
    pub fn sacrifice(&mut self) -> Result<(), ActionError> {
        self.require_player_turn()?;
        match self.combat_mode {
            super::state::CombatMode::Points => {
                if self.chips_pool < 10 + 5 {
                    return Err(ActionError::NotEnoughChips);
                }
                self.chips_pool -= 10;
            }
            super::state::CombatMode::Damage => {
                if self.player_hp <= 5 {
                    return Err(ActionError::InvalidBet("not enough HP".into()));
                }
                self.player_hp -= 5;
            }
        }
        self.chips_pool += 5;
        Ok(())
    }

    /// Spins (§3.2 step 6): with no bets this passes the turn (`Ok(None)`);
    /// otherwise the phase moves to Spinning and locked cards are flagged.
    pub fn spin(&mut self) -> Result<Option<()>, ActionError> {
        self.require_player_turn()?;
        if self.bets.is_empty() {
            return Ok(None); // pass turn (caller confirms)
        }
        self.phase = BattlePhase::Spinning;
        for card in &mut self.active_played {
            card.locked_at_spin = true;
        }
        Ok(Some(()))
    }

    /// Total staked this betting phase.
    pub fn total_stake(&self) -> u16 {
        self.bets.iter().map(|b| b.amount).sum()
    }

    /// Re-arms a fresh betting phase after a resolution (turn flow §3.2).
    /// Banked free-card credits from last turn become active.
    pub fn begin_betting(&mut self, side: Side) {
        self.turn = side;
        self.phase = BattlePhase::Betting;
        self.active_wheel_owner = side;
        self.draws_this_turn = 0;
        self.damage_this_turn = 0;
        self.turn_start_pool = self.pool(side);
        if side == Side::Player {
            self.free_cards_active += self.free_cards_next_turn;
            self.free_cards_next_turn = 0;
        }
    }

    /// Resets the spin-scoped physics cheats to the run baseline, with curse
    /// overlays (Rust: friction ×2, §8).
    pub fn reset_physics(&mut self) {
        self.physics = self.physics_baseline.clone();
        for curse in &self.curses {
            if let roulette_content::schema::CurseEffect::FrictionMultiplier(m) = curse.effect {
                self.physics.friction *= m;
            }
        }
    }
}
