//! TASK-009 perf gate (TEST-008): `apply(Spin)` median ≤ 16 ms × 20 over 200
//! physics spins on the default wheel (native build). Fails the build if the
//! budget is exceeded. Runs with the `telemetry` feature (the wasm build
//! always has it on), so the measured path includes side-channel recording.

use roulette_content::Content;
use roulette_core::api::{Command, Engine, SpinSampling};
use std::sync::Arc;
use std::time::Instant;

/// Median of the samples.
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

#[test]
fn apply_spin_median_within_budget() {
    let content: Arc<Content> = Arc::new(Content::embedded().expect("embedded content"));
    let mut engine = Engine::new(content, "perf-gate");
    engine.set_spin_sampling(SpinSampling::Physics);

    // Boot to combat.
    engine
        .apply(&Command::StartRun { difficulty: roulette_core::run::state::Difficulty::Short })
        .expect("start");
    let offer = engine.run().and_then(|r| r.loadout_offer.clone()).expect("offer");
    for id in &offer.card_ids {
        let _ = engine.apply(&Command::DraftCard { card_id: id.clone() });
    }
    let _ = engine.apply(&Command::DraftWheel);
    let _ = engine.apply(&Command::FinishLoadout);
    let _ = engine.apply(&Command::PickNode { node_id: pickable_first(&engine) });

    let mut spin_times_ms: Vec<f64> = Vec::new();
    let mut run_restarts = 0u32;
    let mut spins = 0usize;
    for _ in 0..20000 {
        if spins >= 200 {
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
                let _ = engine.apply(&Command::PickNode { node_id: pickable_first(&engine) });
            }
            Some(G::Event) => {
                let _ = engine.apply(&Command::EventChoose { choice_id: "a".into() });
            }
            Some(G::Victory) | Some(G::GameOver) => {
                run_restarts += 1;
                assert!(run_restarts < 10, "too many run restarts");
                engine
                    .apply(&Command::StartRun {
                        difficulty: roulette_core::run::state::Difficulty::Short,
                    })
                    .expect("restart");
            }
            Some(G::Combat) => {
                let battle = engine.battle().cloned().expect("battle");
                if battle.phase == roulette_core::battle::state::BattlePhase::Betting
                    && battle.bets.is_empty()
                {
                    let _ = engine.apply(&Command::PlaceBet {
                        bet: roulette_core::bets::BetType::Red,
                        amount: 1,
                    });
                }
                let t0 = Instant::now();
                let ev = engine.apply(&Command::Spin).expect("spin");
                let elapsed = t0.elapsed().as_secs_f64() * 1000.0;
                if ev.iter().any(|e| {
                    matches!(
                        e,
                        roulette_core::api::EngineEvent::BallLanded {
                            side: roulette_core::battle::state::Side::Player,
                            ..
                        }
                    )
                }) {
                    spin_times_ms.push(elapsed);
                    spins += 1;
                }
            }
            _ => panic!("unexpected game state {gs:?}"),
        }
    }
    assert_eq!(spins, 200, "collected 200 timed spins");

    // Budget: median ≤ 16 ms × 20 = 320 ms (CON-005/TEST-008).
    let med = median(&mut spin_times_ms);
    let budget = 16.0 * 20.0;
    assert!(med <= budget, "apply(Spin) median {med:.2} ms exceeds budget {budget:.0} ms");
    println!("apply(Spin) median: {med:.2} ms (budget {budget:.0} ms)");
}

/// First pickable map node id (mirrors engine `pickable_nodes` §2.3).
fn pickable_first(engine: &Engine) -> String {
    let run = engine.run().expect("run active");
    run.pickable_nodes()[0].clone()
}
