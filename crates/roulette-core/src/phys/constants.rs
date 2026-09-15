//! Physics constants, verbatim from blueprint §5.1/§5.3 (do not restate elsewhere).

/// Fixed integration step in seconds (§5.1: PHYSICS_STEP).
pub const PHYSICS_STEP: f64 = 1.0 / 120.0;

/// Headless completion cap: 60 s at 120 Hz (§10.6).
pub const MAX_STEPS: usize = 7200;

/// Wheel angular velocity decay per step, scaled by friction (§5.1: WHEEL_DECAY).
pub const WHEEL_DECAY: f64 = 0.45;

/// Ball angular velocity decay per step, scaled by friction (§5.1: BALL_DECAY).
pub const BALL_DECAY: f64 = 2.4;

/// Track outer radius, normalized (§5.1: R_OUTER).
pub const R_OUTER: f64 = 1.0;

/// Track inner radius, normalized (§5.1: R_INNER).
pub const R_INNER: f64 = 0.65;

/// Deflector pin ring radius (§5.1: R_PIN).
pub const R_PIN: f64 = 0.82;

/// Radial band in which deflector pins can be struck (§5.1).
pub const PIN_WINDOW: (f64, f64) = (0.75, 0.93);

/// Number of diamond deflector pins on the track (§5.1).
pub const DEFLECTOR_PINS: u32 = 8;

/// Number of slot separator pegs on a full wheel (§5.1).
pub const DIVIDER_PEGS: u32 = 37;

/// Baseline inward gravity fighting centrifugal ω²r (§5.1).
pub const GRAVITY_BASE: f64 = 9.0;

/// Reduced inward pull above r 0.92 (rim band) (§5.1).
pub const GRAVITY_RIM: f64 = 3.0;

/// Mid-band inward pull above r 0.78 (§5.1).
pub const GRAVITY_MID: f64 = 4.5;

/// Gravity threshold: rim band start radius (§5.1).
pub const GRAVITY_RIM_RADIUS: f64 = 0.92;

/// Gravity threshold: mid band start radius (§5.1).
pub const GRAVITY_MID_RADIUS: f64 = 0.78;

/// Settle criterion: radius ≤ R_INNER + 0.04 (§5.1).
pub const SETTLE_RADIUS: f64 = R_INNER + 0.04;

/// Settle criterion: height ≤ 0.021 (§5.1).
pub const SETTLE_HEIGHT: f64 = 0.021;

/// Settle criterion: relative speed < 0.8 (§5.1).
pub const SETTLE_REL_SPEED: f64 = 0.8;

/// Settle criterion: |radial velocity| < 0.15 (§5.1).
pub const SETTLE_RADIAL_SPEED: f64 = 0.15;

/// Height floor at r ≤ R_INNER (§5.1: ball rides higher at the rim).
pub const HEIGHT_FLOOR_INNER: f64 = 0.02;

/// Height at the rim (§5.1).
pub const HEIGHT_RIM: f64 = 0.15;

/// Pin-bounce ω restitution factor (§5.2: reverses ω × 0.45).
pub const PIN_BOUNCE_RESTITUTION: f64 = 0.45;

/// Pin-bounce random jitter scale: bounceRandomness × 18 (§5.2).
pub const PIN_BOUNCE_JITTER: f64 = 18.0;

/// Divider-peg restitution per unit ball mass: 0.4/√mass (§5.2).
pub const PEG_RESTITUTION: f64 = 0.4;

/// Launch wheel ω lower bound, × spinSpeed (§5.1).
pub const LAUNCH_WHEEL_OMEGA_MIN: f64 = 2.0;

/// Launch wheel ω upper bound, × spinSpeed (§5.1).
pub const LAUNCH_WHEEL_OMEGA_MAX: f64 = 3.5;

/// Launch ball ω magnitude lower bound: −10/√mass counter-rotating (§5.1).
pub const LAUNCH_BALL_OMEGA_MIN: f64 = 10.0;

/// Launch ball ω magnitude upper bound: −15/√mass (§5.1).
pub const LAUNCH_BALL_OMEGA_MAX: f64 = 15.0;

/// Minimum wheel ω while the ball is unsettled (§5.2 phase model).
pub const MIN_WHEEL_OMEGA: f64 = 0.15;

/// Shotgun timer: after this duration 4 extra balls spawn (§5.3).
pub const SHOTGUN_TIME: f64 = 1.5;

/// Extra balls spawned by the shotgun timer (§5.3).
pub const SHOTGUN_EXTRA_BALLS: u32 = 4;

/// Multiball ball-count cap (§5.1: 6–8; we pin the upper bound).
pub const MULTIBALL_CAP: u32 = 8;

/// Tilt directional gravity scale: sin(angleDiff) × tilt × 6.5 (§5.3).
pub const TILT_GRAVITY_SCALE: f64 = 6.5;

/// Magnet angular pull scale: ω × 25 × bias × dt (§5.3).
pub const BIAS_PULL_SCALE: f64 = 25.0;

/// Magnet inward gravity scale: ×8 × bias (§5.3).
pub const BIAS_GRAVITY_SCALE: f64 = 8.0;

/// Nudge cheat: shift distance bounds in slots, 1–5 (§5.3).
pub const NUDGE_MAX_DISTANCE: u32 = 5;
