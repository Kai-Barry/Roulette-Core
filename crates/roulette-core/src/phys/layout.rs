//! Runtime wheel layout for the physics sim (TASK-013), built from a
//! `roulette_content::WheelDef`.
//!
//! The layout captures only what the *physics* needs: angular slot positions, slot
//! width, and the effective color of each slot. Payouts and upgrades live in the
//! `wheel` module (Phase 3).

use roulette_content::{SlotColor, WheelDef};

/// Angular geometry of a wheel plus its slot colors, indexed by slot position.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelLayout {
    /// Physical slot order, verbatim from the `WheelDef` (§4.6).
    pub numbers: Vec<u32>,
    /// Effective color per slot position (special overrides applied, §10.1 step 1).
    pub colors: Vec<SlotColor>,
    /// Angular width of one slot in radians: `TAU / slot_count`.
    pub slot_width: f64,
}

impl WheelLayout {
    /// Build from a content definition; slots are evenly spaced around the wheel
    /// starting at angle 0 (slot 0 at 12 o'clock, clockwise per §11 wheel visuals).
    ///
    /// Effective color precedence (§10.1 step 1): `slot_colors` overrides the
    /// red/black/green base lists; greens override red/black base coloring.
    pub fn from_def(def: &WheelDef) -> Self {
        let n = def.numbers.len();
        assert!(n >= 2, "validator guarantees wheels with >= 2 slots");
        let mut colors = Vec::with_capacity(n);
        for &num in &def.numbers {
            let base = if def.green_numbers.contains(&num) {
                SlotColor::Green
            } else if def.red_numbers.contains(&num) {
                SlotColor::Red
            } else if def.black_numbers.contains(&num) {
                SlotColor::Black
            } else {
                // The validator rejects uncolored slots; be defensive in the sim.
                SlotColor::Black
            };
            let effective = def.slot_colors.get(&num).copied().unwrap_or(base);
            colors.push(effective);
        }
        Self { numbers: def.numbers.clone(), colors, slot_width: std::f64::consts::TAU / n as f64 }
    }

    /// Number of slots.
    pub fn len(&self) -> usize {
        self.numbers.len()
    }

    /// Always false: layouts have at least 2 slots (validator-enforced).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }

    /// Map an angle relative to the wheel's rotation into a slot index
    /// (§5.1: `floor((angleInWheel + halfSlot) / slotWidth)`, wrapped).
    pub fn slot_at_angle(&self, angle_in_wheel: f64) -> usize {
        let half = self.slot_width / 2.0;
        let mut idx = ((angle_in_wheel + half) / self.slot_width).floor() as i64;
        let n = self.len() as i64;
        idx %= n;
        if idx < 0 {
            idx += n;
        }
        idx as usize
    }
}
