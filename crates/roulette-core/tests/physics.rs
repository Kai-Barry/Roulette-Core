//! Physics tests (TASK-017, TEST-001..004). Statistical tests use large samples:
//! full determinism makes 5k spins reproducible bit-for-bit, so thresholds are
//! strict.

use roulette_core::phys::{self, PhysicsModifiers, Prediction, SimEvent, Simulator, WheelLayout};
use roulette_core::Rng;

/// Classic 37-slot European layout, single green 0 (§4.6 sequence).
fn classic_layout() -> WheelLayout {
    let ron = r#"
(
    schema_version: 1,
    wheels: [(
        id: "classic", name: "Classic", description: "European single-zero",
        numbers: [0, 32, 15, 19, 4, 21, 2, 25, 17, 34, 6, 27, 13, 36, 11, 30, 8, 23, 10, 5, 24, 16, 33, 1, 20, 14, 31, 9, 22, 18, 29, 7, 28, 12, 35, 3, 26],
        green_numbers: [0],
        red_numbers: [1, 3, 5, 7, 9, 12, 14, 16, 18, 19, 21, 23, 25, 27, 30, 32, 34, 36],
        black_numbers: [2, 4, 6, 8, 10, 11, 13, 15, 17, 20, 22, 24, 26, 28, 29, 31, 33, 35],
        payout_multipliers: (red: 2.0, black: 2.0, green: 14.0, number: 36.0, odd: 2.0, even: 2.0),
        rarity: common,
    )],
)
"#;
    let content = roulette_content::Content::parse(ron).expect("fixture parses");
    let def = content.wheels.iter().find(|w| w.id == "classic").expect("wheel");
    WheelLayout::from_def(def)
}

fn spin(seed: u32, layout: &WheelLayout, mods: &PhysicsModifiers) -> (Vec<SimEvent>, usize) {
    let rng = Rng::new(seed);
    let sim = Simulator::new(layout.clone(), mods.clone(), rng);
    let (events, result) = sim.run_to_completion(None);
    let slot = result.slots[0];
    assert!(slot < layout.len(), "slot {slot} out of range");
    (events, slot)
}

/// `PhysicsModifiers::default()` with a few cheat fields overridden (lint-clean).
fn mods_with(overrides: impl FnOnce(&mut PhysicsModifiers)) -> PhysicsModifiers {
    let mut m = PhysicsModifiers::default();
    overrides(&mut m);
    m
}

#[test]
fn settle_yields_valid_slot_every_seed() {
    let layout = classic_layout();
    let mods = PhysicsModifiers::default();
    for seed in 0..200 {
        let (_, slot) = spin(seed, &layout, &mods);
        assert!(slot < 37, "seed {seed} -> slot {slot}");
    }
}

#[test]
fn same_seed_same_result_and_event_log() {
    let layout = classic_layout();
    let mods = PhysicsModifiers::default();
    let (e1, s1) = spin(12345, &layout, &mods);
    let (e2, s2) = spin(12345, &layout, &mods);
    assert_eq!(s1, s2, "golden seed 12345 drifted");
    assert_eq!(e1, e2, "event log differs for same seed");
    let (_, s3) = spin(12346, &layout, &mods);
    assert!(s3 != s1 || true); // different seed *may* collide; only equality is asserted
}

#[test]
fn event_log_hash_is_seed_deterministic() {
    // The event stream is the click/bounce soundtrack; it must be reproducible
    // for golden replays (TEST-001).
    let layout = classic_layout();
    let mods = PhysicsModifiers::default();
    let hash = |seed: u32| -> u64 {
        let (events, _) = spin(seed, &layout, &mods);
        let mut h: u64 = 0xcbf29ce484222325;
        for e in &events {
            h = (h ^ u64::from(e.tag())).wrapping_mul(0x100000001b3);
        }
        h
    };
    assert_eq!(hash(7), hash(7));
    assert_ne!(hash(7), hash(8), "distinct seeds share event hash");
}

#[test]
fn spin_completes_within_step_cap() {
    let layout = classic_layout();
    let mods = PhysicsModifiers::default();
    for seed in 0..50 {
        let rng = Rng::new(seed);
        let sim = Simulator::new(layout.clone(), mods.clone(), rng);
        let (events, result) = sim.run_to_completion(None);
        assert!(result.steps <= phys::MAX_STEPS, "seed {seed} over cap");
        assert!(
            events.iter().any(|e| matches!(e, SimEvent::BallSettled { .. })),
            "seed {seed} never settled"
        );
    }
}

#[test]
fn lands_uniformly_across_slots() {
    // Fair physics: over 5k seeds, no slot should dominate (chi-square style sanity).
    let layout = classic_layout();
    let mods = PhysicsModifiers::default();
    let mut counts = vec![0usize; 37];
    for seed in 0..5000 {
        let (_, slot) = spin(seed, &layout, &mods);
        counts[slot] += 1;
    }
    let expected = 5000.0 / 37.0;
    for (slot, &n) in counts.iter().enumerate() {
        let dev = (n as f64 - expected).abs();
        assert!(
            dev < expected * 0.25,
            "slot {slot} count {n} deviates > 25% from uniform {expected:.1} — physics is biased without cheats"
        );
    }
}

#[test]
fn target_zone_bias_shifts_landing_toward_target() {
    // With a strong magnet at slot 0 (angle 0), landings should cluster near it.
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.target_zone_bias = 0.9;
        m.bias_target_angle = Some(0.0);
    });
    let mut near_target = 0;
    let n = 2000;
    for seed in 0..n {
        let (_, slot) = spin(seed, &layout, &mods);
        // Slots 0, 36, 1 flank slot 0 (European order: slot0=0, slot1=32, slot36=26).
        if matches!(slot, 0 | 1 | 36) {
            near_target += 1;
        }
    }
    let uniform = 3.0 * n as f64 / 37.0;
    assert!(
        near_target as f64 > uniform * 1.5,
        "bias 0.9 gave {near_target} near-target landings vs uniform {uniform:.0} — magnet too weak"
    );
}

#[test]
fn friction_extremes_change_spin_duration() {
    let layout = classic_layout();
    let steps = |fr: f32| -> usize {
        let m = PhysicsModifiers { friction: fr, ..PhysicsModifiers::default() };
        let sim = Simulator::new(layout.clone(), m, Rng::new(99));
        let (_, r) = sim.run_to_completion(None);
        r.steps
    };
    let icy_steps = steps(0.3);
    let sandy_steps = steps(5.0);
    assert!(
        icy_steps > sandy_steps,
        "ICE_GLAZE (0.3): {icy_steps} steps vs SAND_TRAP (5.0): {sandy_steps} — friction must shorten spins"
    );
    assert!(sandy_steps > 0);
}

#[test]
fn nudge_moves_slot_by_at_most_distance() {
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.nudge_distance = 3;
        m.nudge_cheat_active = true;
    });
    let rng = Rng::new(4242);
    let sim = Simulator::new(layout.clone(), mods.clone(), rng);
    let (events, result) = sim.run_to_completion(None);
    let settled = result.slots[0];
    // Nudge toward a far slot: direction +5 requested, capped at distance 3.
    let sim2 = Simulator::new(layout.clone(), mods.clone(), Rng::new(4242));
    let (_, r2) = sim2.run_to_completion(Some(5));
    let nudged = r2.slots[0];
    let shift = (nudged as i64 - settled as i64).rem_euclid(layout.len() as i64);
    let shift = shift.min(shift.abs()).max(-shift.abs());
    assert!(shift.abs() <= 3, "nudge shifted {shift} slots > distance 3");
    let _ = events;
}

#[test]
fn multiball_produces_capped_ball_results() {
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.multiball_count = 3; // TRIPLE_THREAT
    });
    let sim = Simulator::new(layout.clone(), mods.clone(), Rng::new(77));
    let (events, result) = sim.run_to_completion(None);
    assert_eq!(result.slots.len(), 4, "1 + 3 multiball");
    for &slot in &result.slots {
        assert!(slot < 37);
    }
    assert_eq!(events.iter().filter(|e| matches!(e, SimEvent::BallSettled { .. })).count(), 4);
    // Cap: split_peg during multiball cannot exceed 8 balls total.
    let mut split = mods.clone();
    split.split_peg_active = true;
    split.multiball_count = 6;
    let sim2 = Simulator::new(layout.clone(), split, Rng::new(78));
    let (_, r2) = sim2.run_to_completion(None);
    assert!(r2.slots.len() <= 8, "ball cap violated: {}", r2.slots.len());
}

#[test]
fn shotgun_adds_four_balls_mid_spin() {
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.shotgun_time = 1.5;
    });
    let sim = Simulator::new(layout.clone(), mods.clone(), Rng::new(555));
    let (events, result) = sim.run_to_completion(None);
    assert!(
        events.iter().any(|e| matches!(e, SimEvent::ShotgunFired { .. })),
        "shotgun timer never fired"
    );
    assert_eq!(result.slots.len(), 5, "1 + 4 shotgun balls");
}

#[test]
fn prediction_never_matches_with_certainty() {
    // DEC-001: the oracle-honesty roll means a size-1 prediction must NOT hit
    // 100%, and at the default 90% accuracy it should cluster near 90%. This is
    // the core anti-dominant-strategy test.
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.prediction_size = 1;
    });
    let n = 2000;
    let mut hits = 0;
    for seed in 0..n {
        let master = Rng::from_string_seed(&format!("spin-{seed}"));
        let prediction =
            Simulator::predict_from_master(&layout, &mods, &master).expect("prediction armed");
        let live = Simulator::new(layout.clone(), mods.clone(), phys::spin_rng(&master));
        let (_, result) = live.run_to_completion(None);
        let landed = result.slots[0];
        let size = prediction.sector_size as usize;
        let hit = (0..size).any(|i| {
            let s = (prediction.sector_start + i) % layout.len();
            s == landed
        });
        if hit {
            hits += 1;
        }
    }
    let rate = hits as f64 / n as f64;
    assert!(
        rate < 0.999,
        "DEC-001 violated: size-1 prediction hit {rate:.3} — prediction is leaking certainty"
    );
    assert!(
        (rate - 0.90).abs() < 0.07,
        "accuracy knob broken: hit rate {rate:.3}, expected ≈ 0.90"
    );
}

#[test]
fn prediction_accuracy_knob_bounds_hit_rate() {
    // The honesty parameter directly bounds the hit rate: accuracy 0.5 → ~50%,
    // accuracy 1.0 (cheat-mode) → 100% for the sim itself, but no shipped card may
    // set it there (content validator policy, Phase 5).
    let layout = classic_layout();
    let hit_rate = |accuracy: f32| -> f64 {
        let mods = mods_with(|m| {
            m.prediction_size = 1;
            m.prediction_accuracy = accuracy;
        });
        let mut hits = 0;
        let n = 1000;
        for seed in 0..n {
            let master = Rng::from_string_seed(&format!("acc-{seed}"));
            let pred = Simulator::predict_from_master(&layout, &mods, &master).unwrap();
            let live = Simulator::new(layout.clone(), mods.clone(), phys::spin_rng(&master));
            let (_, r) = live.run_to_completion(None);
            let hit = (0..pred.sector_size as usize)
                .any(|i| (pred.sector_start + i) % layout.len() == r.slots[0]);
            hits += hit as usize;
        }
        hits as f64 / n as f64
    };
    let half = hit_rate(0.5);
    let sure = hit_rate(1.0);
    assert!((half - 0.5).abs() < 0.08, "accuracy 0.5 → {half:.3}");
    assert!(sure > 0.99, "accuracy 1.0 → {sure:.3} (sim internal only)");
}

#[test]
fn prediction_wide_sector_beats_pinpoint_hit_rate() {
    // With flat honesty a wide band gains only through lie-overlap: honest 90%
    // bands always contain the true landing (wide ⊇ narrow center), and lies cover
    // 9/37 vs 1/37 of the wheel — so expected rates are ≈0.92 vs ≈0.90. Assert the
    // ordering, not a big margin.
    let layout = classic_layout();
    let hit_rate = |size: u8| -> f64 {
        let mods = mods_with(|m| {
            m.prediction_size = size;
        });
        let mut hits = 0;
        let n = 2000;
        for seed in 0..n {
            let master = Rng::from_string_seed(&format!("p-{seed}"));
            let pred = Simulator::predict_from_master(&layout, &mods, &master).unwrap();
            let live = Simulator::new(layout.clone(), mods.clone(), phys::spin_rng(&master));
            let (_, r) = live.run_to_completion(None);
            let hit = (0..pred.sector_size as usize)
                .any(|i| (pred.sector_start + i) % layout.len() == r.slots[0]);
            hits += hit as usize;
        }
        hits as f64 / n as f64
    };
    let narrow = hit_rate(1);
    let wide = hit_rate(9);
    assert!(wide > narrow, "wide sector ({wide:.3}) should outperform pinpoint ({narrow:.3})");
    assert!(
        wide < 0.97 && narrow < 0.97,
        "neither width may approach certainty (DEC-001): wide {wide:.3} narrow {narrow:.3}"
    );
}

#[test]
fn prediction_returns_none_when_unarmed() {
    let layout = classic_layout();
    let mods = PhysicsModifiers::default();
    assert!(Simulator::predict(&layout, &mods, Rng::new(1), Rng::new(2)).is_none());
}

#[test]
fn prediction_type_sizes() {
    // The Prediction struct is part of the public API surface; assert its shape.
    let p = Prediction { sector_start: 3, sector_size: 9 };
    assert_eq!(p.sector_start, 3);
    assert_eq!(p.sector_size, 9);
}

#[test]
fn tilt_and_mass_cheats_still_settle_validly() {
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.wheel_tilt = 0.35;
        m.ball_mass = 5.0; // HEAVY_CORE
        m.spin_speed = 2.0; // TURBO
        m.bounce_randomness = 0.3;
    });
    for seed in 0..100 {
        let (_, slot) = spin(seed, &layout, &mods);
        assert!(slot < 37);
    }
}

#[test]
fn determinism_holds_for_multiball_event_streams() {
    let layout = classic_layout();
    let mods = mods_with(|m| {
        m.multiball_count = 2;
        m.split_peg_active = true;
    });
    let run = || {
        let sim = Simulator::new(layout.clone(), mods.clone(), Rng::new(2024));
        let (events, result) = sim.run_to_completion(None);
        (events, result.slots)
    };
    let (e1, s1) = run();
    let (e2, s2) = run();
    assert_eq!(e1, e2);
    assert_eq!(s1, s2);
}
