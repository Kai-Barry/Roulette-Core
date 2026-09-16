//! TEST-010 (TASK-008/024 gate): the telemetry side channel agrees with the
//! normative event log. For 100 consecutive spins, the final telemetry
//! frame's ball angles — mapped through the slot geometry carried in the
//! payload header — must equal the `ball_landed` numbers the engine emitted.

#![cfg(feature = "telemetry")]

use roulette_content::Content;
use roulette_core::api::{Command, Engine, SpinSampling};
use std::sync::Arc;

/// Little-endian f32 read.
fn f32_at(bytes: &[u8], idx: usize) -> f32 {
    f32::from_le_bytes(bytes[idx * 4..idx * 4 + 4].try_into().unwrap())
}

/// Header: frame_count u32, ball_count u32, slot_count u32, slot_width f32.
/// All indices below are in 4-byte words (f32_at units).
type ParsedTelemetry = (u32, u32, u32, f32, Vec<(f32, Vec<f32>)>);

fn parse(bytes: &[u8]) -> ParsedTelemetry {
    let frame_count = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
    let ball_count = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    let slot_count = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let slot_width = f32_at(bytes, 3);
    let stride = 1 + ball_count as usize;
    let mut frames = Vec::with_capacity(frame_count as usize);
    for f in 0..frame_count as usize {
        let base = 4 + f * stride;
        let wheel = f32_at(bytes, base);
        let balls = (1..stride).map(|b| f32_at(bytes, base + b)).collect();
        frames.push((wheel, balls));
    }
    (frame_count, ball_count, slot_count, slot_width, frames)
}

/// Slot index from the FINAL frame's angles, replicating
/// `Simulator::slot_at_angle` math (slot 0 spans [-w/2, w/2)).
fn settled_slot(last_wheel: f32, ball: f32, slot_count: u32, slot_width: f32) -> usize {
    const TAU: f64 = std::f64::consts::TAU;
    let rel = (((ball as f64 - last_wheel as f64) % TAU) + TAU) % TAU;
    let half = slot_width as f64 / 2.0;
    let n = slot_count as i64;
    let mut idx = ((rel + half) / slot_width as f64).floor() as i64;
    idx %= n;
    if idx < 0 {
        idx += n;
    }
    idx as usize
}

#[test]
fn telemetry_final_frame_agrees_with_ball_landed() {
    let content: Arc<Content> = Arc::new(Content::embedded().expect("embedded content"));
    let mut engine = Engine::new(content.clone(), "telemetry-agree");
    engine
        .apply(&Command::StartRun { difficulty: roulette_core::run::state::Difficulty::Short })
        .expect("start");
    // Fast-forward the loadout draft to reach combat.
    let offer = engine.run().and_then(|r| r.loadout_offer.clone()).expect("offer");
    for id in &offer.card_ids {
        let _ = engine.apply(&Command::DraftCard { card_id: id.clone() });
    }
    let _ = engine.apply(&Command::DraftWheel);
    let _ = engine.apply(&Command::FinishLoadout);
    let _ = engine.apply(&Command::PickNode { node_id: pickable_first(&engine) });

    // Drive spins until 100 player-spin telemetry checks pass. Battles end;
    // when the run transitions (map/victory/game over) we keep driving via
    // legal commands and restart the run on terminal states (same seed - the
    // agreement under test is the physics math, not run novelty).
    let mut spins_checked = 0usize;
    let mut run_restarts = 0u32;
    for _ in 0..6000 {
        if spins_checked >= 100 {
            break;
        }
        let gs = engine.run().map(|r| r.state);
        use roulette_core::run::state::GameState as G;
        match gs {
            Some(G::LoadoutStore) => {
                let offer = engine.run().and_then(|r| r.loadout_offer.clone()).expect("offer");
                for id in &offer.card_ids {
                    let _ = engine.apply(&Command::DraftCard { card_id: id.clone() });
                }
                let _ = engine.apply(&Command::DraftWheel);
                let _ = engine.apply(&Command::FinishLoadout);
            }
            Some(G::Map) | Some(G::Shop) | Some(G::Forge) => {
                // pick_node on a connected node exits shop/forge (§2.3 contract).
                let _ = engine.apply(&Command::PickNode { node_id: pickable_first(&engine) });
            }
            Some(G::Event) => {
                // Choice ids are content-owned; the harness uses 'a' (§9.4).
                let _ = engine.apply(&Command::EventChoose { choice_id: "a".into() });
            }
            Some(G::Victory) | Some(G::GameOver) => {
                assert!(run_restarts < 10, "too many run restarts");
                run_restarts += 1;
                engine
                    .apply(&Command::StartRun {
                        difficulty: roulette_core::run::state::Difficulty::Short,
                    })
                    .expect("restart");
            }
            Some(G::Combat) => {
                let battle = engine.battle().cloned().expect("battle");
                let numbers = battle.player_wheel.numbers.clone();
                if battle.phase == roulette_core::battle::state::BattlePhase::Betting
                    && battle.bets.is_empty()
                {
                    let _ = engine.apply(&Command::PlaceBet {
                        bet: roulette_core::bets::BetType::Red,
                        amount: 1,
                    });
                }
                let ev = engine.apply(&Command::Spin).expect("spin");
                let landed: Vec<u32> = ev
                    .iter()
                    .filter_map(|e| match e {
                        roulette_core::api::EngineEvent::BallLanded {
                            side: roulette_core::battle::state::Side::Player,
                            number,
                        } => Some(*number),
                        _ => None,
                    })
                    .collect();
                if landed.is_empty() {
                    continue; // pass (no bets) - no telemetry to check
                }
                let bytes = engine
                    .spin_telemetry_bytes(0)
                    .expect("player telemetry present after physics spin");
                let (_, ball_count, slot_count, slot_width, frames) = parse(&bytes);
                assert_eq!(ball_count as usize, landed.len(), "ball count matches landed events");
                assert!(slot_count >= 2);
                let (last_wheel, balls) = frames.last().expect("frames non-empty").clone();
                for (b, number) in balls.iter().zip(landed.iter()) {
                    let slot = settled_slot(last_wheel, *b, slot_count, slot_width);
                    assert_eq!(numbers[slot], *number, "telemetry slot {slot} != landed {number}");
                }
                assert!(engine.spin_telemetry_bytes(1).is_some(), "enemy telemetry present");
                spins_checked += 1;
            }
            _ => panic!("unexpected game state {gs:?}"),
        }
    }
    assert_eq!(spins_checked, 100, "checked 100 consecutive player spins");
}

/// First pickable map node id (mirrors engine `pickable_nodes` §2.3).
fn pickable_first(engine: &Engine) -> String {
    let run = engine.run().expect("run active");
    run.pickable_nodes()[0].clone()
}

#[test]
fn uniform_mode_has_no_telemetry() {
    let content: Arc<Content> = Arc::new(Content::embedded().expect("embedded content"));
    let mut engine = Engine::new(content, "telemetry-uniform");
    engine.set_spin_sampling(SpinSampling::Uniform);
    engine
        .apply(&Command::StartRun { difficulty: roulette_core::run::state::Difficulty::Short })
        .expect("start");
    let offer = engine.run().and_then(|r| r.loadout_offer.clone()).expect("offer");
    for id in &offer.card_ids {
        let _ = engine.apply(&Command::DraftCard { card_id: id.clone() });
    }
    let _ = engine.apply(&Command::DraftWheel);
    let _ = engine.apply(&Command::FinishLoadout);
    let _ = engine.apply(&Command::PickNode { node_id: "f0l0".into() });
    let _ = engine.apply(&Command::PlaceBet { bet: roulette_core::bets::BetType::Red, amount: 5 });
    engine.apply(&Command::Spin).expect("spin");
    assert!(engine.spin_telemetry_bytes(0).is_none(), "uniform path records no frames");
}
