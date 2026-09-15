//! Card System, Game Mode Compatibility, and Modifier Effects for *Roulette of the Damned*.
//!
//! Models player modifier cards, categorizing them into Roulette Modifiers, Board Modifiers,
//! Physics Rerolls, and Risk/Utility powers, with explicit game mode compatibility filters.

use crate::device::{GameDevice, OutcomeSlot, SlotColor};
use crate::mode::GameModeKind;

/// Classification categories for cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardType {
    /// Modifies payouts, spin attributes, or color damage scaling.
    RouletteModifier,
    /// Modifies wheel layout, slot colors, or slot counts.
    BoardModifier,
    /// Influences spin physics or triggers re-spins on losses.
    RerollPhysics,
    /// High risk / reward utilities (health sacrifice, chip gain, double down, hand sacrifice).
    RiskUtility,
}

/// Concrete functional effect executed when playing a card.
#[derive(Debug, Clone)]
pub enum CardEffect {
    /// Boosts payout multiplier for a specific slot color (e.g. Red deals +0.5x payout).
    BoostPayout { color: SlotColor, bonus_multiplier: f64 },
    /// Adds an extra Green zero slot to the device.
    AddGreenSlot,
    /// Adds a special Gold Jackpot slot to the device.
    AddJackpotSlot,
    /// Recolors a range of numbers to a specified color.
    RecolorRange { start: u32, end: u32, color: SlotColor },
    /// Grants a free automatic re-spin if the initial spin results in a loss.
    RerollOnLoss,
    /// Doubles payouts on all winning bets for the current turn.
    DoubleDown,
    /// Instantly gains free chips.
    GainChips(u32),
    /// Sacrifices player health to instantly gain chips (Combat 1v1 / Survival modes).
    BloodSacrifice { hp_cost: i32, chips_gained: u32 },
    /// Sacrifices 1 Hand to instantly gain bonus chips (Point Round / Hand Limit mode).
    HandSacrifice { hands_cost: u32, chips_gained: u32 },
}

/// Represents a playable card in the player's deck.
#[derive(Debug, Clone)]
pub struct Card {
    /// Unique internal string identifier.
    pub id: &'static str,
    /// User-facing card title.
    pub name: &'static str,
    /// Functional category of the card.
    pub card_type: CardType,
    /// Energy / resource cost to play the card.
    pub cost: u32,
    /// Readable card rules text.
    pub description: &'static str,
    /// Game engine effect triggered on play.
    pub effect: CardEffect,
    /// Modes where this card is compatible (empty slice means compatible with ALL modes).
    pub supported_modes: Vec<GameModeKind>,
}

impl Card {
    /// Returns the complete expanded card library pool.
    pub fn expanded_card_library() -> Vec<Card> {
        vec![
            Card {
                id: "red_fever",
                name: "Red Fever",
                card_type: CardType::RouletteModifier,
                cost: 1,
                description: "+0.5x payout to Red bets this turn.",
                effect: CardEffect::BoostPayout {
                    color: SlotColor::Red,
                    bonus_multiplier: 0.5,
                },
                supported_modes: vec![],
            },
            Card {
                id: "black_fever",
                name: "Black Fever",
                card_type: CardType::RouletteModifier,
                cost: 1,
                description: "+0.5x payout to Black bets this turn.",
                effect: CardEffect::BoostPayout {
                    color: SlotColor::Black,
                    bonus_multiplier: 0.5,
                },
                supported_modes: vec![],
            },
            Card {
                id: "green_corruption",
                name: "Green Corruption",
                card_type: CardType::BoardModifier,
                cost: 0,
                description: "Add an extra Green slot to the device.",
                effect: CardEffect::AddGreenSlot,
                supported_modes: vec![],
            },
            Card {
                id: "jackpot_slot",
                name: "Jackpot Slot",
                card_type: CardType::BoardModifier,
                cost: 1,
                description: "Add a Gold Jackpot slot to the wheel.",
                effect: CardEffect::AddJackpotSlot,
                supported_modes: vec![],
            },
            Card {
                id: "red_shift",
                name: "Crimson Sector",
                card_type: CardType::BoardModifier,
                cost: 2,
                description: "Recolor numbers 1 through 12 to Red.",
                effect: CardEffect::RecolorRange {
                    start: 1,
                    end: 12,
                    color: SlotColor::Red,
                },
                supported_modes: vec![],
            },
            Card {
                id: "black_shift",
                name: "Obsidian Sector",
                card_type: CardType::BoardModifier,
                cost: 2,
                description: "Recolor numbers 13 through 24 to Black.",
                effect: CardEffect::RecolorRange {
                    start: 13,
                    end: 24,
                    color: SlotColor::Black,
                },
                supported_modes: vec![],
            },
            Card {
                id: "loaded_dice",
                name: "Second Chance",
                card_type: CardType::RerollPhysics,
                cost: 2,
                description: "If your spin loses, automatically respin once.",
                effect: CardEffect::RerollOnLoss,
                supported_modes: vec![],
            },
            Card {
                id: "chip_surge",
                name: "Chip Surge",
                card_type: CardType::RiskUtility,
                cost: 0,
                description: "Gain 15 Chips instantly.",
                effect: CardEffect::GainChips(15),
                supported_modes: vec![],
            },
            Card {
                id: "blood_pact",
                name: "Blood Pact",
                card_type: CardType::RiskUtility,
                cost: 0,
                description: "Sacrifice 10 HP to gain 20 Chips.",
                effect: CardEffect::BloodSacrifice {
                    hp_cost: 10,
                    chips_gained: 20,
                },
                supported_modes: vec![GameModeKind::Combat1v1, GameModeKind::HordeSurvival],
            },
            Card {
                id: "hand_pact",
                name: "Greed Pact",
                card_type: CardType::RiskUtility,
                cost: 0,
                description: "Sacrifice 1 Hand to gain 30 Chips.",
                effect: CardEffect::HandSacrifice {
                    hands_cost: 1,
                    chips_gained: 30,
                },
                supported_modes: vec![GameModeKind::PointRound],
            },
            Card {
                id: "double_down",
                name: "Double Down",
                card_type: CardType::RiskUtility,
                cost: 1,
                description: "Double your bet payout on win.",
                effect: CardEffect::DoubleDown,
                supported_modes: vec![],
            },
        ]
    }

    /// Returns the standard card pool filtered for a specific [`GameModeKind`].
    pub fn starter_deck_for_mode(mode: GameModeKind) -> Vec<Card> {
        Self::expanded_card_library()
            .into_iter()
            .filter(|c| c.supported_modes.is_empty() || c.supported_modes.contains(&mode))
            .collect()
    }

    /// Legacy starter deck accessor defaulting to Combat1v1 mode.
    pub fn all_starter_cards() -> Vec<Card> {
        Self::starter_deck_for_mode(GameModeKind::Combat1v1)
    }

    /// Checks if this card is compatible with a given game mode.
    pub fn is_compatible_with(&self, mode: GameModeKind) -> bool {
        self.supported_modes.is_empty() || self.supported_modes.contains(&mode)
    }

    /// Applies device-altering card effects directly to a [`GameDevice`] instance.
    pub fn apply_to_device(&self, device: &mut dyn GameDevice) {
        match &self.effect {
            CardEffect::AddGreenSlot => device.add_slot(OutcomeSlot::new(0, SlotColor::Green)),
            CardEffect::AddJackpotSlot => {
                let mut slot = OutcomeSlot::new(77, SlotColor::Custom("Gold"));
                slot.tags.push("jackpot");
                slot.multiplier_bonus = 50.0;
                device.add_slot(slot);
            }
            CardEffect::RecolorRange { start, end, color } => device.recolor_range(*start, *end, *color),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::EuropeanWheel;

    #[test]
    fn test_starter_deck_filtering_by_mode() {
        let combat_cards = Card::starter_deck_for_mode(GameModeKind::Combat1v1);
        assert!(combat_cards.iter().any(|c| c.id == "blood_pact"));
        assert!(!combat_cards.iter().any(|c| c.id == "hand_pact"));

        let point_cards = Card::starter_deck_for_mode(GameModeKind::PointRound);
        assert!(!point_cards.iter().any(|c| c.id == "blood_pact"));
        assert!(point_cards.iter().any(|c| c.id == "hand_pact"));
    }

    #[test]
    fn test_card_apply_to_device() {
        let mut wheel = EuropeanWheel::new();
        let card = Card {
            id: "green_corruption",
            name: "Green Corruption",
            card_type: CardType::BoardModifier,
            cost: 0,
            description: "Add extra green slot",
            effect: CardEffect::AddGreenSlot,
            supported_modes: vec![],
        };
        card.apply_to_device(&mut wheel);
        assert_eq!(wheel.slots().len(), 38);
    }
}
