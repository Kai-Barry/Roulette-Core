use crate::rng::Rng;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotColor {
    Green,
    Red,
    Black,
}

impl fmt::Display for SlotColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotColor::Green => write!(f, "Green"),
            SlotColor::Red => write!(f, "Red"),
            SlotColor::Black => write!(f, "Black"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Slot {
    pub number: u32,
    pub color: SlotColor,
    pub multiplier_bonus: f64,
}

#[derive(Debug, Clone)]
pub struct Wheel {
    pub slots: Vec<Slot>,
}

impl Default for Wheel {
    fn default() -> Self {
        Self::european()
    }
}

impl Wheel {
    /// Create standard European 37-slot wheel (0 Green, 1-36 Red/Black).
    pub fn european() -> Self {
        let red_numbers = [
            1, 3, 5, 7, 9, 12, 14, 16, 18, 19, 21, 23, 25, 27, 30, 32, 34, 36,
        ];

        let mut slots = Vec::with_capacity(37);
        // Slot 0 is Green
        slots.push(Slot {
            number: 0,
            color: SlotColor::Green,
            multiplier_bonus: 0.0,
        });

        for n in 1..=36 {
            let color = if red_numbers.contains(&n) {
                SlotColor::Red
            } else {
                SlotColor::Black
            };
            slots.push(Slot {
                number: n,
                color,
                multiplier_bonus: 0.0,
            });
        }

        Wheel { slots }
    }

    /// Add an extra Green slot to the wheel (Card effect).
    pub fn add_green_slot(&mut self) {
        self.slots.push(Slot {
            number: 0,
            color: SlotColor::Green,
            multiplier_bonus: 0.0,
        });
    }

    /// Recolor slots in range [start, end] to a specific color (Card effect).
    pub fn recolor_range(&mut self, start: u32, end: u32, color: SlotColor) {
        for slot in &mut self.slots {
            if slot.number >= start && slot.number <= end {
                slot.color = color;
            }
        }
    }

    /// Spin the wheel using deterministic PRNG.
    pub fn spin(&self, rng: &mut Rng) -> Slot {
        let index = (rng.next_u32() as usize) % self.slots.len();
        self.slots[index].clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BetType {
    Red,
    Black,
    Green,
    ExactNumber(u32),
    Even,
    Odd,
    High, // 19-36
    Low,  // 1-18
}

#[derive(Debug, Clone)]
pub struct Bet {
    pub bet_type: BetType,
    pub amount: u32,
}

impl Bet {
    /// Calculate base payout multiplier for winning bet.
    pub fn base_payout_multiplier(&self) -> f64 {
        match self.bet_type {
            BetType::Red | BetType::Black | BetType::Even | BetType::Odd | BetType::High | BetType::Low => 2.0,
            BetType::Green => 14.0,
            BetType::ExactNumber(_) => 36.0,
        }
    }

    /// Evaluate if winning slot satisfies this bet.
    pub fn evaluates_win(&self, slot: &Slot) -> bool {
        match &self.bet_type {
            BetType::Red => slot.color == SlotColor::Red,
            BetType::Black => slot.color == SlotColor::Black,
            BetType::Green => slot.color == SlotColor::Green,
            BetType::ExactNumber(n) => slot.number == *n,
            BetType::Even => slot.number != 0 && slot.number % 2 == 0,
            BetType::Odd => slot.number != 0 && slot.number % 2 != 0,
            BetType::High => slot.number >= 19 && slot.number <= 36,
            BetType::Low => slot.number >= 1 && slot.number <= 18,
        }
    }
}
