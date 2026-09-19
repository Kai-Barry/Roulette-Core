//! Game Device Abstraction & Outcome Slot Systems.
//!
//! Provides the [`GameDevice`] trait for randomizer hardware (Roulette Wheels, Slot Machines, Dice, Pachinko)
//! and concrete implementations like [`EuropeanWheel`], [`AmericanWheel`], and [`DiceDevice`].

use crate::rng::Rng;
use std::fmt;

/// Represents the color classification of an outcome slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotColor {
    /// Green zero / house slot.
    Green,
    /// Red slot.
    Red,
    /// Black slot.
    Black,
    /// Custom color classification (e.g., Blue, Gold, Void).
    Custom(&'static str),
}

impl fmt::Display for SlotColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotColor::Green => write!(f, "Green"),
            SlotColor::Red => write!(f, "Red"),
            SlotColor::Black => write!(f, "Black"),
            SlotColor::Custom(c) => write!(f, "{}", c),
        }
    }
}

/// Represents an individual outcome slot on a game device.
#[derive(Debug, Clone)]
pub struct OutcomeSlot {
    /// Numerical value assigned to slot (0 through 36, or custom number).
    pub number: u32,
    /// Color classification of slot.
    pub color: SlotColor,
    /// Additional damage/payout multiplier bonus.
    pub multiplier_bonus: f64,
    /// Custom metadata tags (e.g. "jackpot", "curse", "bonus").
    pub tags: Vec<&'static str>,
}

impl OutcomeSlot {
    /// Creates a new outcome slot with default zero bonus multiplier and empty tags.
    pub fn new(number: u32, color: SlotColor) -> Self {
        OutcomeSlot {
            number,
            color,
            multiplier_bonus: 0.0,
            tags: Vec::new(),
        }
    }
}

/// Abstract trait representing any randomizer outcome device (Wheel, Dice, Slot Machine, Pachinko).
pub trait GameDevice: fmt::Debug + Send + Sync {
    /// Returns the descriptive name of the device type.
    fn name(&self) -> &str;

    /// Deterministically spins/rolls the device using the PRNG and returns the landed slot.
    fn spin(&self, rng: &mut Rng) -> OutcomeSlot;

    /// Returns a slice of all slots present on the device.
    fn slots(&self) -> &[OutcomeSlot];

    /// Returns a mutable slice of all slots present on the device.
    fn slots_mut(&mut self) -> &mut [OutcomeSlot];

    /// Adds a new slot to the device (expanding board size and probability distribution).
    fn add_slot(&mut self, slot: OutcomeSlot);

    /// Recolors all slots whose numbers fall within the specified `[start, end]` inclusive range.
    fn recolor_range(&mut self, start: u32, end: u32, color: SlotColor);

    /// Clones the device into a boxed trait object.
    fn clone_box(&self) -> Box<dyn GameDevice>;
}

impl Clone for Box<dyn GameDevice> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Concrete European 37-slot Roulette Wheel implementation (0 Green, 1-36 Red/Black).
#[derive(Debug, Clone)]
pub struct EuropeanWheel {
    pub slots: Vec<OutcomeSlot>,
}

impl Default for EuropeanWheel {
    fn default() -> Self {
        Self::new()
    }
}

impl EuropeanWheel {
    /// Creates a standard 37-slot European Roulette Wheel.
    pub fn new() -> Self {
        let red_numbers = [
            1, 3, 5, 7, 9, 12, 14, 16, 18, 19, 21, 23, 25, 27, 30, 32, 34, 36,
        ];

        let mut slots = Vec::with_capacity(37);
        // Slot 0 is Green
        slots.push(OutcomeSlot::new(0, SlotColor::Green));

        for n in 1..=36 {
            let color = if red_numbers.contains(&n) {
                SlotColor::Red
            } else {
                SlotColor::Black
            };
            slots.push(OutcomeSlot::new(n, color));
        }

        EuropeanWheel { slots }
    }
}

impl GameDevice for EuropeanWheel {
    fn name(&self) -> &str {
        "European Roulette Wheel (37 Slots)"
    }

    fn spin(&self, rng: &mut Rng) -> OutcomeSlot {
        let index = (rng.next_u32() as usize) % self.slots.len();
        self.slots[index].clone()
    }

    fn slots(&self) -> &[OutcomeSlot] {
        &self.slots
    }

    fn slots_mut(&mut self) -> &mut [OutcomeSlot] {
        &mut self.slots
    }

    fn add_slot(&mut self, slot: OutcomeSlot) {
        self.slots.push(slot);
    }

    fn recolor_range(&mut self, start: u32, end: u32, color: SlotColor) {
        for slot in &mut self.slots {
            if slot.number >= start && slot.number <= end {
                slot.color = color;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn GameDevice> {
        Box::new(self.clone())
    }
}

/// Concrete American 38-slot Roulette Wheel implementation (0 Green, 00 Green, 1-36 Red/Black).
#[derive(Debug, Clone)]
pub struct AmericanWheel {
    pub slots: Vec<OutcomeSlot>,
}

impl Default for AmericanWheel {
    fn default() -> Self {
        Self::new()
    }
}

impl AmericanWheel {
    /// Creates an American Roulette Wheel with 0 and 00 Green house slots.
    pub fn new() -> Self {
        let mut european = EuropeanWheel::new();
        // Add 00 Double Zero slot (represented as number 37 with Green color)
        let mut double_zero = OutcomeSlot::new(37, SlotColor::Green);
        double_zero.tags.push("double_zero");
        european.slots.push(double_zero);

        AmericanWheel {
            slots: european.slots,
        }
    }
}

impl GameDevice for AmericanWheel {
    fn name(&self) -> &str {
        "American Roulette Wheel (38 Slots)"
    }

    fn spin(&self, rng: &mut Rng) -> OutcomeSlot {
        let index = (rng.next_u32() as usize) % self.slots.len();
        self.slots[index].clone()
    }

    fn slots(&self) -> &[OutcomeSlot] {
        &self.slots
    }

    fn slots_mut(&mut self) -> &mut [OutcomeSlot] {
        &mut self.slots
    }

    fn add_slot(&mut self, slot: OutcomeSlot) {
        self.slots.push(slot);
    }

    fn recolor_range(&mut self, start: u32, end: u32, color: SlotColor) {
        for slot in &mut self.slots {
            if slot.number >= start && slot.number <= end {
                slot.color = color;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn GameDevice> {
        Box::new(self.clone())
    }
}

/// Custom Dice Game Device example (e.g. 6-sided elemental die).
#[derive(Debug, Clone)]
pub struct DiceDevice {
    pub sides: Vec<OutcomeSlot>,
}

impl DiceDevice {
    /// Creates a 6-sided elemental die.
    pub fn new_standard_d6() -> Self {
        let sides = vec![
            OutcomeSlot::new(1, SlotColor::Red),
            OutcomeSlot::new(2, SlotColor::Black),
            OutcomeSlot::new(3, SlotColor::Red),
            OutcomeSlot::new(4, SlotColor::Black),
            OutcomeSlot::new(5, SlotColor::Red),
            OutcomeSlot::new(6, SlotColor::Green),
        ];
        DiceDevice { sides }
    }
}

impl GameDevice for DiceDevice {
    fn name(&self) -> &str {
        "Elemental D6 Die"
    }

    fn spin(&self, rng: &mut Rng) -> OutcomeSlot {
        let index = (rng.next_u32() as usize) % self.sides.len();
        self.sides[index].clone()
    }

    fn slots(&self) -> &[OutcomeSlot] {
        &self.sides
    }

    fn slots_mut(&mut self) -> &mut [OutcomeSlot] {
        &mut self.sides
    }

    fn add_slot(&mut self, slot: OutcomeSlot) {
        self.sides.push(slot);
    }

    fn recolor_range(&mut self, start: u32, end: u32, color: SlotColor) {
        for slot in &mut self.sides {
            if slot.number >= start && slot.number <= end {
                slot.color = color;
            }
        }
    }

    fn clone_box(&self) -> Box<dyn GameDevice> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_european_wheel_device() {
        let wheel = EuropeanWheel::new();
        assert_eq!(wheel.slots().len(), 37);
        assert_eq!(wheel.slots()[0].color, SlotColor::Green);
    }

    #[test]
    fn test_american_wheel_device() {
        let wheel = AmericanWheel::new();
        assert_eq!(wheel.slots().len(), 38);
    }

    #[test]
    fn test_dice_device() {
        let dice = DiceDevice::new_standard_d6();
        assert_eq!(dice.slots().len(), 6);

        let mut rng = Rng::new(42);
        let outcome = dice.spin(&mut rng);
        assert!(outcome.number >= 1 && outcome.number <= 6);
    }
}
