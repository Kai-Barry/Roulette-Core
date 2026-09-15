//! The House Shop (§9.1, TASK-038): 6 cards + 2 wheel drafts + 1 heal, all
//! priced in ⚡. The offer is cached per shop node.

use roulette_content::schema::{CardDef, CardRarity, WheelDef, WheelRarity};

use crate::rng::Rng;

/// Blood Infusion: +25 HP for 12 ⚡ (§9.1, blocked under Fragile).
pub const BLOOD_INFUSION_HEAL: u16 = 25;
pub const BLOOD_INFUSION_PRICE: u16 = 12;

/// One priced shop item.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(bound = "", rename_all = "snake_case")]
pub enum ShopItem {
    Card { def: CardDef, price: u16 },
    Wheel { def: WheelDef, price: u16 },
    BloodInfusion { price: u16 },
}

impl ShopItem {
    pub fn price(&self) -> u16 {
        match self {
            ShopItem::Card { price, .. }
            | ShopItem::Wheel { price, .. }
            | ShopItem::BloodInfusion { price } => *price,
        }
    }
}

/// A cached shop offer (fixed once entered, cleared on leaving the node).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShopOffer {
    pub items: Vec<ShopItem>,
}

impl ShopOffer {
    /// Builds the §9.1 offer: 6 cards (rarity-priced), 2 wheels, Blood Infusion.
    pub fn generate(cards: &[CardDef], wheels: &[WheelDef], rng: &mut Rng) -> Self {
        let mut items = Vec::new();
        let mut used = std::collections::BTreeSet::new();
        for _ in 0..6 {
            let pool: Vec<&CardDef> = cards
                .iter()
                .filter(|c| !used.contains(&c.id))
                .collect();
            if let Some(def) = rng.pick(&pool) {
                let price = card_price(&def.rarity, rng);
                used.insert(def.id.clone());
                items.push(ShopItem::Card { def: (*def).clone(), price });
            }
        }
        // Two distinct wheel drafts of uncommon/rare/legendary rarity.
        let drafts: Vec<&WheelDef> = wheels
            .iter()
            .filter(|w| w.rarity != WheelRarity::Common)
            .collect();
        let mut wheel_used = std::collections::BTreeSet::new();
        for _ in 0..2 {
            let pool: Vec<&WheelDef> = drafts
                .iter()
                .copied()
                .filter(|w| !wheel_used.contains(&w.id))
                .collect();
            if let Some(def) = rng.pick(&pool) {
                wheel_used.insert(def.id.clone());
                let price = wheel_price(&def.rarity, rng);
                items.push(ShopItem::Wheel { def: (*def).clone(), price });
            }
        }
        items.push(ShopItem::BloodInfusion { price: BLOOD_INFUSION_PRICE });
        Self { items }
    }

    /// Index of the Blood Infusion item, if present.
    pub fn infusion_index(&self) -> Option<usize> {
        self.items
            .iter()
            .position(|i| matches!(i, ShopItem::BloodInfusion { .. }))
    }
}

/// Card price bands (§9.1, verbatim).
pub fn card_price(rarity: &CardRarity, rng: &mut Rng) -> u16 {
    let (min, max) = match rarity {
        CardRarity::Common => (8, 13),
        CardRarity::Uncommon => (14, 21),
        CardRarity::Rare => (25, 35),
        CardRarity::Legendary => (45, 60),
    };
    rng.range_usize(min as usize, max as usize) as u16
}

/// Wheel price bands (§9.1: ≈15–25 uncommon, ≈25–35 rare, ≈45–60 legendary).
pub fn wheel_price(rarity: &WheelRarity, rng: &mut Rng) -> u16 {
    let (min, max) = match rarity {
        WheelRarity::Common => (0, 0),
        WheelRarity::Uncommon => (15, 25),
        WheelRarity::Rare => (25, 35),
        WheelRarity::Legendary => (45, 60),
    };
    if max == 0 {
        return 0;
    }
    rng.range_usize(min as usize, max as usize) as u16
}