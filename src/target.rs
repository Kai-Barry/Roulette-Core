//! Target & Entity System for Modular Combat Encounters.
//!
//! Defines the [`Target`] trait allowing custom enemy AI, boss multi-phase transitions,
//! shields, status effects, and multi-target combat setups.

use crate::device::GameDevice;
use crate::rng::Rng;
use std::fmt;

/// Abstract trait representing an opponent, boss, or target entity in combat.
pub trait Target: fmt::Debug + Send + Sync {
    /// Display name of target.
    fn name(&self) -> &str;

    /// Current health points.
    fn hp(&self) -> i32;

    /// Maximum health points.
    fn max_hp(&self) -> i32;

    /// Sets target health points directly.
    fn set_hp(&mut self, hp: i32);

    /// Checks if target is currently alive (`hp > 0`).
    fn is_alive(&self) -> bool {
        self.hp() > 0
    }

    /// Inflicts damage on target.
    fn take_damage(&mut self, amount: i32) {
        self.set_hp((self.hp() - amount).max(0));
    }

    /// Heals target health points.
    fn heal(&mut self, amount: i32) {
        self.set_hp((self.hp() + amount).min(self.max_hp()));
    }

    /// Rolls next random intent for the target.
    fn roll_intent(&mut self, rng: &mut Rng);

    /// Executes the current queued intent against the player / device state.
    /// Returns `(damage_dealt_to_player, descriptive_log_message)`.
    fn execute_intent(&mut self, device: &mut dyn GameDevice, rng: &mut Rng) -> (i32, String);

    /// Clones target into a boxed trait object.
    fn clone_box(&self) -> Box<dyn Target>;
}

impl Clone for Box<dyn Target> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Category of standard enemy actions.
#[derive(Debug, Clone)]
pub enum EnemyIntent {
    /// Deals direct attack damage to player HP.
    Attack(i32),
    /// Restores enemy HP or gains shield.
    Block(i32),
    /// Corrupts roulette wheel slots (e.g. converts 1-18 to Green).
    CorruptRed,
}

/// Concrete Standard Pit Boss / Enemy implementation.
#[derive(Debug, Clone)]
pub struct Enemy {
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub intent: EnemyIntent,
}

impl Enemy {
    /// Creates the standard default boss entity ("The Cursed Croupier").
    pub fn create_pit_boss() -> Self {
        Enemy {
            name: "The Cursed Croupier".to_string(),
            hp: 100,
            max_hp: 100,
            intent: EnemyIntent::Attack(15),
        }
    }

    /// Creates a custom named enemy entity.
    pub fn new(name: &str, hp: i32) -> Self {
        Enemy {
            name: name.to_string(),
            hp,
            max_hp: hp,
            intent: EnemyIntent::Attack(10),
        }
    }
}

impl Target for Enemy {
    fn name(&self) -> &str {
        &self.name
    }

    fn hp(&self) -> i32 {
        self.hp
    }

    fn max_hp(&self) -> i32 {
        self.max_hp
    }

    fn set_hp(&mut self, hp: i32) {
        self.hp = hp;
    }

    fn roll_intent(&mut self, rng: &mut Rng) {
        let val = rng.range_i32(1, 3);
        self.intent = match val {
            1 => EnemyIntent::Attack(rng.range_i32(10, 20)),
            2 => EnemyIntent::Block(rng.range_i32(5, 15)),
            _ => EnemyIntent::CorruptRed,
        };
    }

    fn execute_intent(&mut self, device: &mut dyn GameDevice, rng: &mut Rng) -> (i32, String) {
        let (dmg, msg) = match self.intent {
            EnemyIntent::Attack(d) => (d, format!("{} attacked for {} damage!", self.name, d)),
            EnemyIntent::Block(b) => {
                self.heal(b);
                (0, format!("{} healed for {} HP!", self.name, b))
            }
            EnemyIntent::CorruptRed => {
                device.recolor_range(1, 18, crate::device::SlotColor::Green);
                (0, format!("{} corrupted slots 1-18 to Green!", self.name))
            }
        };
        self.roll_intent(rng);
        (dmg, msg)
    }

    fn clone_box(&self) -> Box<dyn Target> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enemy_target_trait() {
        let mut enemy = Enemy::create_pit_boss();
        assert_eq!(enemy.hp(), 100);
        assert!(enemy.is_alive());

        enemy.take_damage(30);
        assert_eq!(enemy.hp(), 70);

        enemy.heal(15);
        assert_eq!(enemy.hp(), 85);
    }
}
