//! The 120 Hz wheel/ball simulator (§5.2 phase model, §5.3 cheat hooks).
//!
//! Deterministic: all randomness flows through the injected [`Rng`]; all trigonometry
//! goes through [`crate::deterministic_floats`] (REQ-007). Live spins and prediction
//! dry-runs construct simulators with **different** RNG substreams (DEC-001), so the
//! predicted sector is a probabilistic band, never certain knowledge.

use super::constants as c;
use super::events::SimEvent;
use super::layout::WheelLayout;
use super::modifiers::PhysicsModifiers;
use crate::deterministic_floats::sin_approx;
use crate::rng::Rng;
use std::collections::HashMap;
use std::f64::consts::TAU;

/// One ball in flight. `angle` is absolute (world frame); the slot index derives from
/// the angle *relative to the wheel's rotation* (§5.1 formula).
#[derive(Clone, Debug, PartialEq)]
struct Ball {
    id: u32,
    /// Absolute angular position (rad, clockwise positive).
    angle: f64,
    /// Angular velocity (rad/s; counter-rotating = negative).
    omega: f64,
    /// Normalized track radius, R_INNER..=R_OUTER.
    radius: f64,
    /// Radial velocity (normalized units/s).
    radial_vel: f64,
    /// Height above the track surface (§5.1 height floor).
    height: f64,
    settled: Option<usize>,
}

/// Result of a completed spin (per-ball slots, in spawn order).
#[derive(Clone, Debug, PartialEq)]
pub struct SpinResult {
    /// Slot index per ball, in spawn order.
    pub slots: Vec<usize>,
    /// Steps taken to reach completion (settle of last ball or the cap).
    pub steps: usize,
}

/// Prediction sector returned by the dry-run (§10.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prediction {
    /// Sector center slot index (the dry-run's first-ball landing).
    pub sector_start: usize,
    /// Sector width in slots (`predictionSize`; 1 = pinpoint).
    pub sector_size: u8,
}

/// The simulator. Construct one per spin (live or dry-run); drive with
/// [`Simulator::step`], or consume entirely via [`Simulator::run_to_completion`].
pub struct Simulator {
    layout: WheelLayout,
    mods: PhysicsModifiers,
    rng: Rng,

    // Wheel state
    wheel_omega: f64,
    wheel_angle: f64,

    // Balls
    balls: Vec<Ball>,
    next_ball_id: u32,

    // Per-ball divider-tick bookkeeping: last wheel-relative slot seen.
    slot_ticks: HashMap<usize, usize>,

    // Timing
    elapsed: f64,
    shotgun_fired: bool,

    // Post-settle cheat state
    nudged: bool,

    events: Vec<SimEvent>,
    steps_taken: usize,

    /// Per-step wheel/ball angles, recorded only under the `telemetry` feature
    /// (TASK-008 side channel; fixed dt by construction = PHYSICS_STEP).
    #[cfg(feature = "telemetry")]
    frames: Vec<TelemetryFrame>,
}

/// One telemetry frame: absolute wheel angle + per-ball absolute angles at a
/// fixed 1/120 s step (feature `telemetry` only).
#[cfg(feature = "telemetry")]
#[derive(Clone, Debug)]
pub struct TelemetryFrame {
    pub wheel_angle: f64,
    pub ball_angles: Vec<f64>,
}

impl Simulator {
    /// Launch a spin: wheel ω 2.0–3.5 × spinSpeed; balls at ω −10…−15/√mass
    /// counter-rotating (§5.2 phase 1). One ball plus `multiball_count` extras (§5.3).
    pub fn new(layout: WheelLayout, mods: PhysicsModifiers, rng: Rng) -> Self {
        let mut sim = Self {
            layout,
            mods,
            rng,
            wheel_omega: 0.0,
            wheel_angle: 0.0,
            balls: Vec::new(),
            next_ball_id: 0,
            slot_ticks: HashMap::new(),
            elapsed: 0.0,
            shotgun_fired: false,
            nudged: false,
            events: Vec::new(),
            steps_taken: 0,
            #[cfg(feature = "telemetry")]
            frames: Vec::new(),
        };
        sim.wheel_omega = sim.launch_wheel_omega();
        let count = sim.mods.ball_count();
        for _ in 0..count {
            sim.spawn_ball();
        }
        sim
    }

    fn launch_wheel_omega(&mut self) -> f64 {
        let min = c::LAUNCH_WHEEL_OMEGA_MIN * self.mods.spin_speed as f64;
        let max = c::LAUNCH_WHEEL_OMEGA_MAX * self.mods.spin_speed as f64;
        self.rng.range_f64(min, max)
    }

    fn spawn_ball(&mut self) -> u32 {
        let id = self.next_ball_id;
        self.next_ball_id += 1;
        let mass = self.mods.safe_mass();
        // Counter-rotating launch at the rim (§5.1).
        let omega_mag = self.rng.range_f64(c::LAUNCH_BALL_OMEGA_MIN, c::LAUNCH_BALL_OMEGA_MAX);
        let angle = self.rng.range_f64(0.0, TAU);
        let jitter = self.rng.range_f64(-0.05, 0.05);
        self.balls.push(Ball {
            id,
            angle,
            omega: -omega_mag / mass.sqrt(),
            radius: c::R_OUTER + jitter,
            radial_vel: 0.0,
            height: c::HEIGHT_RIM,
            settled: None,
        });
        id
    }

    /// Advance one fixed step (1/120 s), returning the events emitted this step.
    pub fn step(&mut self) -> Vec<SimEvent> {
        self.events.clear();
        self.steps_taken += 1;
        let dt = c::PHYSICS_STEP;
        self.elapsed += dt;

        // Shotgun timer (§5.3): spawn 4 extra balls mid-spin, capped.
        if !self.shotgun_fired
            && self.mods.shotgun_time > 0.0
            && self.elapsed >= self.mods.shotgun_time
        {
            self.shotgun_fired = true;
            for _ in 0..c::SHOTGUN_EXTRA_BALLS {
                if self.total_balls() < c::MULTIBALL_CAP {
                    self.spawn_ball();
                }
            }
            self.events.push(SimEvent::ShotgunFired { count: c::SHOTGUN_EXTRA_BALLS });
        }

        // Wheel decay, floored while any ball is unsettled (§5.2).
        let any_unsettled = self.balls.iter().any(|b| b.settled.is_none());
        let friction = self.mods.safe_friction();
        self.wheel_omega *= (1.0 - c::WHEEL_DECAY * friction * dt).max(0.0);
        if any_unsettled && self.wheel_omega < c::MIN_WHEEL_OMEGA {
            self.wheel_omega = c::MIN_WHEEL_OMEGA;
        }
        self.wheel_angle = (self.wheel_angle + self.wheel_omega * dt) % TAU;

        // Integrate each unsettled ball.
        for idx in 0..self.balls.len() {
            if self.balls[idx].settled.is_none() {
                self.step_ball(idx, dt);
            }
        }

        std::mem::take(&mut self.events)
    }

    fn total_balls(&self) -> u32 {
        self.next_ball_id
    }

    fn step_ball(&mut self, idx: usize, dt: f64) {
        let friction = self.mods.safe_friction();
        let mass = self.mods.safe_mass();

        // Ball decay (§5.2 phase 2), scaled by friction.
        let decay = (1.0 - c::BALL_DECAY * friction * dt).max(0.0);
        let mut omega = self.balls[idx].omega * decay;

        // Magnetism (§5.3): angular pull toward the bet zone, ω × 25 × bias × dt,
        // plus inward gravity ×8 × bias. Red/black-only filtering is applied by the
        // battle layer when it picks `bias_target_angle` (a slot of the allowed color,
        // Phase 4) — the sim itself only executes the pull.
        let mut bias_gravity = 0.0;
        if self.mods.target_zone_bias > 0.0 {
            if let Some(target) = self.mods.bias_target_angle {
                let pull =
                    omega.abs() * c::BIAS_PULL_SCALE * self.mods.target_zone_bias as f64 * dt;
                let diff = normalized_angle_diff(target, self.balls[idx].angle);
                omega += pull * diff.signum();
                bias_gravity =
                    -c::GRAVITY_BASE * c::BIAS_GRAVITY_SCALE * self.mods.target_zone_bias as f64;
            }
        }

        // Centrifugal vs gravity bands (§5.1).
        let radius = self.balls[idx].radius;
        let centrifugal = omega * omega * radius;
        let gravity = self.gravity_at(radius);
        let mut radial_acc = centrifugal - gravity + bias_gravity;

        // Tilt (§5.3): directional gravity sin(angleDiff) × tilt × 6.5 pulling the
        // ball to the low side. The low-side angle is the magnet target when armed,
        // else angle 0 (the blueprint fixes only the scale, not the side).
        if self.mods.wheel_tilt > 0.0 {
            let low = self.mods.bias_target_angle.unwrap_or(0.0);
            let diff = normalized_angle_diff(low, self.balls[idx].angle);
            radial_acc -=
                (sin_approx(diff) * self.mods.wheel_tilt as f64 * c::TILT_GRAVITY_SCALE).max(0.0);
        }

        let b = &mut self.balls[idx];
        b.omega = omega;
        b.radial_vel += radial_acc * dt;
        b.radius += b.radial_vel * dt;
        // Track walls: reflect radial velocity.
        if b.radius > c::R_OUTER {
            b.radius = c::R_OUTER;
            b.radial_vel = b.radial_vel.min(0.0);
        }
        if b.radius < c::R_INNER {
            b.radius = c::R_INNER;
            b.radial_vel = b.radial_vel.max(0.0);
        }
        // Height floor: the ball rides higher at the rim (§5.1).
        let rim_t = ((b.radius - c::R_INNER) / (c::R_OUTER - c::R_INNER)).clamp(0.0, 1.0);
        let floor = c::HEIGHT_FLOOR_INNER + (c::HEIGHT_RIM - c::HEIGHT_FLOOR_INNER) * rim_t;
        b.height = (b.height * (1.0 - c::BALL_DECAY * dt)).max(floor);
        b.angle = (b.angle + b.omega * dt) % TAU;
        let (radius, height, omega) = (b.radius, b.height, b.omega);
        let radial_speed = b.radial_vel.abs();
        let ball_id = b.id;

        // Pin/peg chaos (§5.2 phase 3): 8 deflector diamonds in the window band.
        if radius >= c::PIN_WINDOW.0 && radius <= c::PIN_WINDOW.1 {
            self.pin_chaos(idx, mass);
        }

        // Divider tick: emit when the ball crosses into a new wheel-relative slot (§5.4).
        let rel = self.ball_rel_angle(idx);
        let slot = self.layout.slot_at_angle(rel);
        let prev = self.slot_ticks.insert(idx, slot);
        if prev.is_some_and(|p| p != slot) {
            let id = self.balls[idx].id;
            self.events.push(SimEvent::DividerTick { ball: id });
        }

        // Settle detection (§5.1).
        let rel_speed = (omega - self.wheel_omega).abs();
        if radius <= c::SETTLE_RADIUS
            && height <= c::SETTLE_HEIGHT
            && rel_speed < c::SETTLE_REL_SPEED
            && radial_speed < c::SETTLE_RADIAL_SPEED
        {
            self.balls[idx].settled = Some(slot);
            self.events.push(SimEvent::BallSettled { ball: ball_id, slot });
        }
    }

    fn gravity_at(&self, radius: f64) -> f64 {
        if radius > c::GRAVITY_RIM_RADIUS {
            c::GRAVITY_RIM
        } else if radius > c::GRAVITY_MID_RADIUS {
            c::GRAVITY_MID
        } else {
            c::GRAVITY_BASE
        }
    }

    /// Deflector pin strike (§5.2): reverses ω × 0.45 + jitter × bounceRandomness × 18;
    /// divider-peg restitution 0.4/√mass bounds the bounce for heavy balls.
    fn pin_chaos(&mut self, idx: usize, mass: f64) {
        let pin_spacing = TAU / c::DEFLECTOR_PINS as f64;
        let phase = ((self.balls[idx].angle % pin_spacing) + pin_spacing) % pin_spacing;
        let step_travel = self.balls[idx].omega.abs() * c::PHYSICS_STEP;
        if phase < step_travel.max(0.01) {
            let id = self.balls[idx].id;
            let jitter = self.rng.range_f64(-1.0, 1.0)
                * c::PIN_BOUNCE_JITTER
                * self.mods.bounce_randomness as f64;
            let restitution = c::PIN_BOUNCE_RESTITUTION.min(c::PEG_RESTITUTION / mass.sqrt());
            let b = &mut self.balls[idx];
            b.omega = -b.omega * restitution + jitter;
            b.radial_vel -= 0.3;
            self.events.push(SimEvent::PegHit { ball: id });

            // Split peg: duplicate the ball on pin hit, capped (§5.3).
            if self.mods.split_peg_active && self.total_balls() < c::MULTIBALL_CAP {
                let new_id = self.spawn_ball();
                self.events.push(SimEvent::BallSplit { ball: new_id });
            }
        }
    }

    /// True once every ball has settled (or the step cap was reached).
    pub fn is_complete(&self) -> bool {
        self.balls.iter().all(|b| b.settled.is_some()) || self.steps_taken >= c::MAX_STEPS
    }

    /// Post-settle nudge cheat (§5.3): shift the landed slot ±n toward the nearest
    /// player bet (direction computed by the battle layer, Phase 4). Applied once,
    /// after completion, before reading results; bounded by `nudge_distance`.
    pub fn apply_nudge(&mut self, nudge_slots: i32) {
        if self.nudged || self.mods.nudge_distance == 0 || nudge_slots == 0 {
            return;
        }
        self.nudged = true;
        let n = self.layout.len() as i32;
        let max = self.mods.nudge_distance as i32;
        let shift = nudge_slots.clamp(-max, max);
        for b in &mut self.balls {
            if let Some(slot) = b.settled {
                b.settled = Some((slot as i32 + shift).rem_euclid(n) as usize);
            }
        }
    }

    /// Telemetry snapshot (feature `telemetry`, TASK-008): the current wheel
    /// angle plus every ball's absolute angle, in ball spawn order. Additive
    /// read-only accessor — private fields stay private; the normative event
    /// log is untouched (REQ-008).
    #[cfg(feature = "telemetry")]
    pub fn snapshot(&self) -> (f64, Vec<f64>) {
        (self.wheel_angle, self.balls.iter().map(|b| b.angle).collect())
    }

    /// Collect the result. Unsettled balls (step cap) map to their current
    /// wheel-relative slot so results are always valid indices (TEST-002).
    pub fn result(&self) -> SpinResult {
        let slots = self
            .balls
            .iter()
            .map(|b| {
                b.settled.unwrap_or_else(|| self.layout.slot_at_angle(self.ball_rel_angle_of(b)))
            })
            .collect();
        SpinResult { slots, steps: self.steps_taken }
    }

    fn ball_rel_angle_of(&self, b: &Ball) -> f64 {
        (((b.angle - self.wheel_angle) % TAU) + TAU) % TAU
    }

    fn ball_rel_angle(&self, idx: usize) -> f64 {
        self.ball_rel_angle_of(&self.balls[idx])
    }

    /// Run headless to completion (step cap 7200 = 60 s, §10.6). `nudge_toward`
    /// (signed slot shift) applies the post-settle nudge when armed. Returns all
    /// emitted events plus the result; `BallSettled` events carry pre-nudge slots —
    /// the battle layer emits its own event for the shift (Phase 4).
    pub fn run_to_completion(mut self, nudge_toward: Option<i32>) -> (Vec<SimEvent>, SpinResult) {
        let mut all_events = Vec::new();
        while !self.is_complete() {
            let ev = self.step();
            all_events.extend(ev);
            #[cfg(feature = "telemetry")]
            self.frames.push(TelemetryFrame {
                wheel_angle: self.wheel_angle,
                ball_angles: self.balls.iter().map(|b| b.angle).collect(),
            });
        }
        if let Some(dir) = nudge_toward {
            self.apply_nudge(dir);
        }
        let result = self.result();
        (all_events, result)
    }

    /// Like [`Simulator::run_to_completion`] but also returns the recorded
    /// telemetry frames (feature `telemetry`, TASK-008). Frames are recorded
    /// pre-nudge (the nudge is a result-space slot shift, not an angle change),
    /// so for cheat-free spins the final frame + [`Simulator::slot_at_angle`]
    /// agrees with [`SpinResult::slots`] (TEST-010).
    #[cfg(feature = "telemetry")]
    pub fn run_to_completion_with_telemetry(
        self,
        nudge_toward: Option<i32>,
    ) -> (Vec<SimEvent>, SpinResult, Vec<TelemetryFrame>) {
        let (events, result, frames) = self.run_to_completion_with_telemetry_timed(nudge_toward);
        (events.into_iter().map(|(_, e)| e).collect(), result, frames)
    }

    /// Like [`Self::run_to_completion_with_telemetry`], but each event is
    /// paired with the telemetry frame index at which it fired (§5.4 sound
    /// hooks: the renderer/SoundManager syncs clicks and bounces to the
    /// playback cursor). Additive under the `telemetry` feature (CON-001).
    #[cfg(feature = "telemetry")]
    pub fn run_to_completion_with_telemetry_timed(
        mut self,
        nudge_toward: Option<i32>,
    ) -> (Vec<(u32, SimEvent)>, SpinResult, Vec<TelemetryFrame>) {
        let mut all_events = Vec::new();
        while !self.is_complete() {
            let ev = self.step();
            let frame = self.frames.len() as u32;
            all_events.extend(ev.into_iter().map(|e| (frame, e)));
            self.frames.push(TelemetryFrame {
                wheel_angle: self.wheel_angle,
                ball_angles: self.balls.iter().map(|b| b.angle).collect(),
            });
        }
        if let Some(dir) = nudge_toward {
            self.apply_nudge(dir);
        }
        let result = self.result();
        let frames = std::mem::take(&mut self.frames);
        (all_events, result, frames)
    }

    /// Uniform fast path (§15/TASK-045): samples each ball's slot uniformly
    /// with zero sim steps. Statistically equivalent to a fair-physics spin
    /// for bulk Monte Carlo runs; ignores cheat hooks (physics cards are
    /// excluded from repricing per §15.2). Deterministic via the same live
    /// stream the physics path consumes.
    pub fn uniform_run(
        layout: &WheelLayout,
        ball_count: u32,
        mut rng: crate::rng::Rng,
    ) -> (Vec<SimEvent>, SpinResult) {
        let last = layout.numbers.len().saturating_sub(1);
        let slots = (0..ball_count.max(1)).map(|_| rng.range_usize(0, last)).collect();
        (Vec::new(), SpinResult { slots, steps: 0 })
    }

    /// Prediction dry-run (§10.6, DEC-001).
    ///
    /// Two RNG streams flow in: `live_rng` is the *master* stream the live spin will
    /// run on (the caller passes `phys::spin_rng(master)` semantics — see
    /// [`predict_with_live`]); `prediction_rng` drives the oracle's lies. With
    /// probability `prediction_accuracy` the band is centered on the **true** landing
    /// (a full dry-run on the live stream); otherwise it is centered on a plausible
    /// decoy landing from the independent prediction stream. Deterministic, and the
    /// hit rate is a direct function of `prediction_accuracy` — always < 100% when
    /// accuracy < 1.0, so pinpoint prediction is never certain knowledge.
    pub fn predict(
        layout: &WheelLayout,
        mods: &PhysicsModifiers,
        live_rng: Rng,
        mut prediction_rng: Rng,
    ) -> Option<Prediction> {
        let size = mods.prediction_size;
        if size == 0 {
            return None;
        }
        let n = layout.len();
        // Oracle honesty roll (independent stream).
        let honest = prediction_rng.next_f64() < mods.prediction_accuracy.clamp(0.0, 1.0) as f64;
        let center = if honest {
            // Truth: dry-run on the live stream reproduces the actual landing.
            let truth_sim = Simulator::new(layout.clone(), mods.clone(), live_rng);
            let (_, result) = truth_sim.run_to_completion(None);
            result.slots.first().copied()?
        } else {
            // Lie: a plausible landing from the independent prediction stream.
            let decoy_sim = Simulator::new(layout.clone(), mods.clone(), prediction_rng);
            let (_, result) = decoy_sim.run_to_completion(None);
            result.slots.first().copied()?
        };
        let half = (size as usize).saturating_sub(1) / 2;
        let start = (center + n - half) % n;
        Some(Prediction { sector_start: start, sector_size: size })
    }

    /// Convenience wrapper for the battle layer: derive both streams from one
    /// master (GUD-004 labels; DEC-001 keeps them independent).
    pub fn predict_from_master(
        layout: &WheelLayout,
        mods: &PhysicsModifiers,
        master: &Rng,
    ) -> Option<Prediction> {
        let live = super::spin_rng(master);
        let prediction = super::prediction_rng(master);
        Simulator::predict(layout, mods, live, prediction)
    }
}

/// Normalized signed angular difference `target - angle` into (−π, π].
fn normalized_angle_diff(target: f64, angle: f64) -> f64 {
    let diff = (target - angle) % TAU;
    if diff > std::f64::consts::PI {
        diff - TAU
    } else if diff < -std::f64::consts::PI {
        diff + TAU
    } else {
        diff
    }
}
