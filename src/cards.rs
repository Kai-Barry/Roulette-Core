use crate::wheel::{SlotColor, Wheel};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardType {
    RouletteModifier,
    BoardModifier,
    RerollPhysics,
    RiskUtility,
}

#[derive(Debug, Clone)]
pub enum CardEffect {
    /// Boost payout for a specific slot color (e.g. Red deals 2.5x instead of 2.0x).
    BoostPayout { color: SlotColor, bonus_multiplier: f64 },
    /// Add extra Green slot to the wheel to increase risk/payout.
    AddGreenSlot,
    /// Recolor number range (e.g. convert 1-12 to Red).
    RecolorRange { start: u32, end: u32, color: SlotColor },
    /// Free re-spin if outcome is not winning.
    RerollOnLoss,
    /// Double down on all bets (doubles bet amount and payout).
    DoubleDown,
    /// Gain chips/blood at the cost of player HP.
    BloodSacrifice { hp_cost: i32, chips_gained: u32 },
}

#[derive(Debug, Clone)]
pub struct Card {
    pub id: &'static str,
    pub name: &'static str,
    pub card_type: CardType,
    pub cost: u32,
    pub description: &'static str,
    pub effect: CardEffect,
}

impl Card {
    pub fn all_starter_cards() -> Vec<Card> {
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
            },
            Card {
                id: "green_corruption",
                name: "Green Corruption",
                card_type: CardType::BoardModifier,
                cost: 0,
                description: "Add an extra Green slot to the wheel.",
                effect: CardEffect::AddGreenSlot,
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
            },
            Card {
                id: "loaded_dice",
                name: "Second Chance",
                card_type: CardType::RerollPhysics,
                cost: 2,
                description: "If your spin loses, automatically respin once.",
                effect: CardEffect::RerollOnLoss,
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
            },
            Card {
                id: "double_down",
                name: "Double Down",
                card_type: CardType::RiskUtility,
                cost: 1,
                description: "Double your bet payout on win.",
                effect: CardEffect::DoubleDown,
            },
        ]
    }

    /// Apply card effect directly to wheel or turn state.
    pub fn apply_to_wheel(&self, wheel: &mut Wheel) {
        match &self.effect {
            CardEffect::AddGreenSlot => wheel.add_green_slot(),
            CardEffect::RecolorRange { start, end, color } => wheel.recolor_range(*start, *end, *color),
            _ => {}
        }
    }
}
