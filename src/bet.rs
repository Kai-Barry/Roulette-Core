//! Betting System and Device-Agnostic Payout Logic.
//!
//! Provides [`BetType`] and [`Bet`] for placing bets against any [`GameDevice`] or [`OutcomeSlot`].

use crate::device::{OutcomeSlot, SlotColor};

/// Represents the target category of a placed bet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BetType {
    /// Bet on Red slots.
    Red,
    /// Bet on Black slots.
    Black,
    /// Bet on Green slots.
    Green,
    /// Bet on a single exact slot number (e.g. 7, 17, 36).
    ExactNumber(u32),
    /// Bet on even non-zero numbers.
    Even,
    /// Bet on odd non-zero numbers.
    Odd,
    /// High range numbers (19 through 36).
    High,
    /// Low range numbers (1 through 18).
    Low,
    /// Custom tagged bet (e.g. betting on "jackpot" or "bonus" tagged slots).
    CustomTag(&'static str),
}

/// Represents a chip bet placed by the player prior to spinning a device.
#[derive(Debug, Clone)]
pub struct Bet {
    /// Category/type of the bet.
    pub bet_type: BetType,
    /// Amount of chips gambled on this bet.
    pub amount: u32,
}

impl Bet {
    /// Returns the base payout multiplier for a winning bet of this type.
    ///
    /// - Red / Black / Even / Odd / High / Low -> 2.0x
    /// - Green -> 14.0x
    /// - Exact Number -> 36.0x
    /// - CustomTag -> 10.0x
    pub fn base_payout_multiplier(&self) -> f64 {
        match self.bet_type {
            BetType::Red | BetType::Black | BetType::Even | BetType::Odd | BetType::High | BetType::Low => 2.0,
            BetType::Green => 14.0,
            BetType::ExactNumber(_) => 36.0,
            BetType::CustomTag(_) => 10.0,
        }
    }

    /// Evaluates whether the landed slot fulfills the winning conditions of this bet.
    ///
    /// # Arguments
    /// * `slot` - Reference to the landed outcome slot.
    pub fn evaluates_win(&self, slot: &OutcomeSlot) -> bool {
        match &self.bet_type {
            BetType::Red => slot.color == SlotColor::Red,
            BetType::Black => slot.color == SlotColor::Black,
            BetType::Green => slot.color == SlotColor::Green,
            BetType::ExactNumber(n) => slot.number == *n,
            BetType::Even => slot.number != 0 && slot.number % 2 == 0,
            BetType::Odd => slot.number != 0 && slot.number % 2 != 0,
            BetType::High => slot.number >= 19 && slot.number <= 36,
            BetType::Low => slot.number >= 1 && slot.number <= 18,
            BetType::CustomTag(tag) => slot.tags.contains(tag),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bet_evaluation() {
        let red_slot = OutcomeSlot::new(7, SlotColor::Red);
        let red_bet = Bet { bet_type: BetType::Red, amount: 10 };
        assert!(red_bet.evaluates_win(&red_slot));
        assert_eq!(red_bet.base_payout_multiplier(), 2.0);

        let mut jackpot_slot = OutcomeSlot::new(77, SlotColor::Green);
        jackpot_slot.tags.push("jackpot");

        let tag_bet = Bet { bet_type: BetType::CustomTag("jackpot"), amount: 5 };
        assert!(tag_bet.evaluates_win(&jackpot_slot));
        assert_eq!(tag_bet.base_payout_multiplier(), 10.0);
    }
}
