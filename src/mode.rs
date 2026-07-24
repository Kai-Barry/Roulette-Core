//! Combat & Game Mode Strategies, Game Mode Classifications, and Outcome Evaluators.
//!
//! Provides the [`CombatMode`] trait enabling modular rule engines (1v1 Combat, Horde Mode, Point Attack / Hand Limit).

use crate::target::Target;
use std::fmt;

/// High-level classification of game mode types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameModeKind {
    /// 1v1 Survival Combat against enemy entity with HP and attacks.
    Combat1v1,
    /// Survival combat against multiple enemy targets.
    HordeSurvival,
    /// Round-based score attack with fixed hand limit (Balatro style, no enemy attacks/HP).
    PointRound,
}

/// Represents the overall resolution state of an encounter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatOutcome {
    /// Encounter is still ongoing.
    InProgress,
    /// Player defeated targets or achieved target score quota.
    PlayerVictory,
    /// Player HP reached 0 or failed to reach target score within hand limit.
    PlayerDefeat,
    /// Maximum turn/hand limit reached without resolution.
    TurnLimitReached,
}

/// Abstract trait defining rule set and victory/defeat evaluation for different game modes.
pub trait CombatMode: fmt::Debug + Send + Sync {
    /// Returns the descriptive name of the game mode.
    fn name(&self) -> &str;

    /// Returns the structural category classification of this mode.
    fn mode_kind(&self) -> GameModeKind;

    /// Indicates whether player HP exists and is tracked in this mode.
    fn has_player_hp(&self) -> bool {
        match self.mode_kind() {
            GameModeKind::Combat1v1 | GameModeKind::HordeSurvival => true,
            GameModeKind::PointRound => false,
        }
    }

    /// Indicates whether enemies take turns and attack player HP.
    fn has_enemy_attacks(&self) -> bool {
        match self.mode_kind() {
            GameModeKind::Combat1v1 | GameModeKind::HordeSurvival => true,
            GameModeKind::PointRound => false,
        }
    }

    /// Evaluates whether game mode is completed or ongoing.
    fn evaluate_outcome(&self, player_hp: Option<i32>, targets: &[Box<dyn Target>], turn_number: u32) -> CombatOutcome;

    /// Clones combat mode into a boxed trait object.
    fn clone_box(&self) -> Box<dyn CombatMode>;
}

impl Clone for Box<dyn CombatMode> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Standard 1v1 Single-Target Combat Mode.
#[derive(Debug, Clone)]
pub struct Standard1v1Mode {
    /// Optional turn limit (default 30 turns).
    pub max_turns: u32,
}

impl Default for Standard1v1Mode {
    fn default() -> Self {
        Standard1v1Mode { max_turns: 30 }
    }
}

impl Standard1v1Mode {
    pub fn new(max_turns: u32) -> Self {
        Standard1v1Mode { max_turns }
    }
}

impl CombatMode for Standard1v1Mode {
    fn name(&self) -> &str {
        "Standard 1v1 Survival Encounter"
    }

    fn mode_kind(&self) -> GameModeKind {
        GameModeKind::Combat1v1
    }

    fn evaluate_outcome(&self, player_hp: Option<i32>, targets: &[Box<dyn Target>], turn_number: u32) -> CombatOutcome {
        if let Some(hp) = player_hp {
            if hp <= 0 {
                return CombatOutcome::PlayerDefeat;
            }
        }

        let all_targets_dead = targets.iter().all(|t| !t.is_alive());
        if all_targets_dead {
            return CombatOutcome::PlayerVictory;
        }

        if turn_number > self.max_turns {
            return CombatOutcome::TurnLimitReached;
        }

        CombatOutcome::InProgress
    }

    fn clone_box(&self) -> Box<dyn CombatMode> {
        Box::new(self.clone())
    }
}

/// Horde Combat Mode (Survival against multiple targets/enemies).
#[derive(Debug, Clone)]
pub struct HordeMode {
    pub required_kills: usize,
    pub current_kills: usize,
}

impl HordeMode {
    pub fn new(required_kills: usize) -> Self {
        HordeMode {
            required_kills,
            current_kills: 0,
        }
    }
}

impl CombatMode for HordeMode {
    fn name(&self) -> &str {
        "Horde Survival Encounter"
    }

    fn mode_kind(&self) -> GameModeKind {
        GameModeKind::HordeSurvival
    }

    fn evaluate_outcome(&self, player_hp: Option<i32>, targets: &[Box<dyn Target>], _turn_number: u32) -> CombatOutcome {
        if let Some(hp) = player_hp {
            if hp <= 0 {
                return CombatOutcome::PlayerDefeat;
            }
        }

        let dead_count = targets.iter().filter(|t| !t.is_alive()).count();
        if dead_count >= self.required_kills || targets.iter().all(|t| !t.is_alive()) {
            return CombatOutcome::PlayerVictory;
        }

        CombatOutcome::InProgress
    }

    fn clone_box(&self) -> Box<dyn CombatMode> {
        Box::new(self.clone())
    }
}

/// Point Round / Hand-Limit High Score Mode (Balatro style).
///
/// Players get a fixed, adjustable number of hands/rounds (`max_hands`) to reach a target score quota,
/// or achieve the highest cumulative score possible.
#[derive(Debug, Clone)]
pub struct PointRoundMode {
    /// Number of hands/rounds available per game (e.g. 4 hands).
    pub max_hands: u32,
    /// Target score quota required to win (e.g. 200 points). Set to 0 for unlimited high score mode.
    pub target_score: u32,
}

impl PointRoundMode {
    /// Creates a new point round mode with adjustable hand count and target score.
    pub fn new(max_hands: u32, target_score: u32) -> Self {
        PointRoundMode {
            max_hands,
            target_score,
        }
    }
}

impl Default for PointRoundMode {
    fn default() -> Self {
        PointRoundMode {
            max_hands: 4,
            target_score: 200,
        }
    }
}

impl CombatMode for PointRoundMode {
    fn name(&self) -> &str {
        "Point Round Mode (Hand-Limit Score Attack)"
    }

    fn mode_kind(&self) -> GameModeKind {
        GameModeKind::PointRound
    }

    fn evaluate_outcome(&self, _player_hp: Option<i32>, targets: &[Box<dyn Target>], turn_number: u32) -> CombatOutcome {
        let current_score: u32 = targets.iter().map(|t| (t.max_hp() - t.hp()).max(0) as u32).sum();

        // Check target score achievement
        if self.target_score > 0 && current_score >= self.target_score {
            return CombatOutcome::PlayerVictory;
        }

        // Check hand limit (turn_number > max_hands means all hands have been played)
        if turn_number > self.max_hands {
            if self.target_score > 0 && current_score < self.target_score {
                return CombatOutcome::PlayerDefeat;
            } else {
                return CombatOutcome::PlayerVictory;
            }
        }

        CombatOutcome::InProgress
    }

    fn clone_box(&self) -> Box<dyn CombatMode> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target::Enemy;

    #[test]
    fn test_standard_mode_evaluation() {
        let mode = Standard1v1Mode::default();
        let target: Box<dyn Target> = Box::new(Enemy::create_pit_boss());

        assert_eq!(mode.evaluate_outcome(Some(100), &[target.clone()], 1), CombatOutcome::InProgress);
        assert_eq!(mode.evaluate_outcome(Some(0), &[target.clone()], 1), CombatOutcome::PlayerDefeat);

        let mut dead_target = target.clone();
        dead_target.set_hp(0);
        assert_eq!(mode.evaluate_outcome(Some(100), &[dead_target], 1), CombatOutcome::PlayerVictory);
    }

    #[test]
    fn test_point_round_mode_evaluation() {
        let mode = PointRoundMode::new(4, 200);
        let mut target: Box<dyn Target> = Box::new(Enemy::new("Score Target Dummy", 1000));

        // Turn 1: 50 pts scored (target HP 950)
        target.set_hp(950);
        assert_eq!(mode.evaluate_outcome(None, &[target.clone()], 1), CombatOutcome::InProgress);

        // Turn 2: 250 pts scored total (exceeds 200 quota)
        target.set_hp(750);
        assert_eq!(mode.evaluate_outcome(None, &[target.clone()], 2), CombatOutcome::PlayerVictory);

        // Hand limit reached without quota
        let mode2 = PointRoundMode::new(2, 500);
        let mut target2: Box<dyn Target> = Box::new(Enemy::new("Score Target Dummy", 1000));
        target2.set_hp(800); // 200 score < 500 quota
        assert_eq!(mode2.evaluate_outcome(None, &[target2.clone()], 3), CombatOutcome::PlayerDefeat);
    }
}
