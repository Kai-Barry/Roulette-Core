//! Cheat/physics hook parameters (§5.3, §16.3 `PhysicsModifiers`), with verbatim
//! defaults. Cards arm these through the effect DSL; the sim reads them each step.

use serde::{Deserialize, Serialize};

/// Every modifier cards can touch (§5.3 table). Defaults per §16.3.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsModifiers {
    /// Scales initial wheel ω (TURBO 2.0, CALM 0.5).
    pub spin_speed: f32,
    /// Heavier = shorter, stabler spin (LEAD_BALL 2.0, HEAVY_CORE 5.0, LIGHT_SHELL 0.5).
    pub ball_mass: f32,
    /// Scales both decay rates (ICE_GLAZE 0.3, SAND_TRAP 5.0, Rust curse ×2).
    pub friction: f32,
    /// Pin-bounce jitter scale (CHAOS_BOUNCE 0.3, STICKY_TRACK 0.0).
    pub bounce_randomness: f32,
    /// Directional gravity toward the low side, 0..0.35 (§5.3).
    pub wheel_tilt: f32,
    /// "Magnetism": angular pull toward the highest-value bet zone, 0..1
    /// (LODESTONE 0.90, EMP 1.0, WEAK 0.2).
    pub target_zone_bias: f32,
    /// Magnet targets red-only slots (Attraction Coil).
    pub bias_red_only: bool,
    /// Magnet targets black-only slots (Repulsion Coil).
    pub bias_black_only: bool,
    /// Predicted sector width in slots; 0 = no prediction shown (§5.3).
    pub prediction_size: u8,
    /// Oracle honesty: probability the predicted band is centered on the true
    /// landing rather than a decoy (DEC-001). Tunable accuracy knob; keep < 1.0
    /// so pinpoint prediction is never certain knowledge.
    pub prediction_accuracy: f32,
    /// Post-settle cheat: shift the landed slot ±n toward the nearest player bet.
    pub nudge_distance: u8,
    pub nudge_cheat_active: bool,
    /// Multiple simultaneous balls (TRIPLE_THREAT 3).
    pub multiball_count: u32,
    /// Ball duplicates on pin hit, capped (§5.3).
    pub split_peg_active: bool,
    /// Timer that spawns 4 extra balls mid-spin at 1.5 s (§5.3).
    pub shotgun_time: f64,
    /// Where the magnet pulls (angular target); set by the battle layer from bets.
    #[serde(default)]
    pub bias_target_angle: Option<f64>,
}

impl Default for PhysicsModifiers {
    fn default() -> Self {
        Self {
            spin_speed: 1.0,
            ball_mass: 1.0,
            friction: 1.0,
            bounce_randomness: 0.1,
            wheel_tilt: 0.0,
            target_zone_bias: 0.0,
            bias_red_only: false,
            bias_black_only: false,
            prediction_size: 0,
            prediction_accuracy: Self::DEFAULT_PREDICTION_ACCURACY,
            nudge_distance: 0,
            nudge_cheat_active: false,
            multiball_count: 0,
            split_peg_active: false,
            shotgun_time: 0.0,
            bias_target_angle: None,
        }
    }
}

impl PhysicsModifiers {
    /// DEC-001 default: size-1 predictions are right ~90% of the time — informative,
    /// never certain. Blueprint §10.1(4) prices prediction by *width*; accuracy
    /// scales mildly with width (the oracle lies less about a wider band).
    pub const DEFAULT_PREDICTION_ACCURACY: f32 = 0.90;

    /// Friction clamped away from zero so the sim can never freeze decay at
    /// friction = 0 (endless-spin exploits are ICE_GLAZE 0.3, never 0).
    pub fn safe_friction(&self) -> f64 {
        (self.friction as f64).max(0.05)
    }

    /// Ball mass clamped away from zero (division site).
    pub fn safe_mass(&self) -> f64 {
        (self.ball_mass as f64).max(0.1)
    }

    /// Effective ball count for this spin: 1 + multiball extras (§5.3).
    pub fn ball_count(&self) -> u32 {
        1 + self.multiball_count
    }
}
