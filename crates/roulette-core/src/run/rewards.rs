//! Combat reward rolls (§2.5, TASK-038).

use roulette_content::schema::{CardDef, CardRarity, EnemyTier};

use crate::rng::Rng;

/// Rolls one card rarity per §2.5's cumulative table:
/// legendary 3% / rare 9% (cum < 0.12) / uncommon 28% (cum < 0.40) / common 60%.
pub fn roll_rarity(rng: &mut Rng) -> CardRarity {
    let roll = rng.next_f64();
    if roll < 0.03 {
        CardRarity::Legendary
    } else if roll < 0.12 {
        CardRarity::Rare
    } else if roll < 0.40 {
        CardRarity::Uncommon
    } else {
        CardRarity::Common
    }
}

/// Number of reward picks presented for a won fight (§2.5: elite 2, boss 3).
pub fn reward_picks(tier: EnemyTier) -> usize {
    match tier {
        EnemyTier::Normal => 1,
        EnemyTier::Elite => 2,
        EnemyTier::Boss => 3,
    }
}

/// Rolls `picks` cards for a won fight: each pick rolls its own rarity, then a
/// random card of that rarity is drawn without repetition across the pick set.
pub fn reward_cards<'a>(cards: &'a [CardDef], tier: EnemyTier, rng: &mut Rng) -> Vec<&'a CardDef> {
    let picks = reward_picks(tier);
    let mut chosen: Vec<&CardDef> = Vec::with_capacity(picks);
    for _ in 0..picks {
        let rarity = roll_rarity(rng);
        let mut rarity_now = rarity;
        let mut pool: Vec<&CardDef> =
            cards.iter().filter(|c| c.rarity == rarity_now).collect();
        // Fall down the table when a band runs dry (common always ships).
        while pool.is_empty() && rarity_now != CardRarity::Common {
            rarity_now = lower_rarity(&rarity_now);
            pool = cards.iter().filter(|c| c.rarity == rarity_now).collect();
        }
        // Pick avoiding duplicates already chosen.
        let mut available: Vec<&CardDef> = pool
            .iter()
            .copied()
            .filter(|c| !chosen.iter().any(|c2| c2.id == c.id))
            .collect();
        if available.is_empty() {
            available = pool;
        }
        if let Some(card) = rng.pick(&available) {
            chosen.push(*card);
        }
    }
    chosen
}

fn lower_rarity(rarity: &CardRarity) -> CardRarity {
    match rarity {
        CardRarity::Legendary => CardRarity::Rare,
        CardRarity::Rare => CardRarity::Uncommon,
        _ => CardRarity::Common,
    }
}