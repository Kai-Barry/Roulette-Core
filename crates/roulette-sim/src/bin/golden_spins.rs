//! Golden-spin dump: canonical spin outcomes for cross-build determinism checks.
//!
//! Prints `seed slot` for a range of seeds so debug and release builds can be
//! diffed byte-for-byte (TASK-017 determinism gate). Also used as the seed-anchor
//! source for future regression tests: a change in this output is a physics
//! behavior change.

use roulette_core::phys::{PhysicsModifiers, Simulator, WheelLayout};
use roulette_core::Rng;

fn main() {
    let ron = std::fs::read_to_string("content/wheels.ron").expect("content/wheels.ron");
    let content = roulette_content::Content::parse(&ron).expect("wheels parse");
    let def = content.wheels.iter().find(|w| w.id == "classic").expect("classic wheel");
    let layout = WheelLayout::from_def(def);

    let mods = PhysicsModifiers::default();
    for seed in 0..64u32 {
        let sim = Simulator::new(layout.clone(), mods.clone(), Rng::new(seed));
        let (_, result) = sim.run_to_completion(None);
        println!("{seed} {}", result.slots[0]);
    }
}
