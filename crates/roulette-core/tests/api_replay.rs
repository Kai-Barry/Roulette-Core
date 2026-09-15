//! TASK-044 determinism contract (REQ-007 at API level): the same seed plus
//! the same scripted command sequence must produce a byte-identical
//! `EngineEvent` log and an identical final `RunState`.

use std::sync::Arc;

use roulette_content::Content;

use roulette_core::api::{Command, CustomizeOp, Engine};

/// Full command script covering the command surface: loadout, map, combat
/// (bets, play, prediction, spins), undo, and an erroring command (the error
/// must not perturb the event log either).
fn script() -> Vec<Command> {
    vec![
        Command::StartRun { difficulty: roulette_core::run::state::Difficulty::Short },
        Command::DraftCard { card_id: "low_sweep".into() },
        Command::DraftWheel,
        Command::FinishLoadout,
        Command::PickNode { node_id: "f0l0".into() },
        Command::PlayCard { hand_index: 0 },
        Command::PlayCard { hand_index: 1 },
        Command::PlaceBet { bet: roulette_core::bets::BetType::Red, amount: 10 },
        Command::Predict,
        Command::Spin,
        Command::PlaceBet { bet: roulette_core::bets::BetType::Number(17), amount: 5 },
        Command::Spin,
        Command::Undo,
        Command::PlaceBet { bet: roulette_core::bets::BetType::Crimson, amount: 5 },
        Command::Spin,
        Command::BuyDraw,
        Command::ClearBets,
        Command::Rebet,
        Command::Sacrifice,
        // Deterministic failure: bet is illegal in the post-spin phase.
        Command::PlaceBet { bet: roulette_core::bets::BetType::Red, amount: 10 },
        Command::Customize { op: CustomizeOp::CycleColor { slot: 0 } },
        Command::Undo,
    ]
}

fn replay(seed: &str) -> (String, roulette_core::run::state::RunState) {
    let content = Arc::new(Content::embedded().expect("embedded content"));
    let mut engine = Engine::new(content.clone(), seed);
    for cmd in script() {
        // Errors are part of the contract too; both replays error identically.
        let _ = engine.apply(&cmd);
    }
    let log = serde_json::to_string(engine.events()).expect("events serialize");
    let run = engine.run().expect("run active").clone();
    (log, run)
}

#[test]
fn replay_is_byte_identical_for_same_seed() {
    let (a, run_a) = replay("contract-seed-1");
    let (b, run_b) = replay("contract-seed-1");
    assert_eq!(a, b, "same seed must give byte-identical event logs");
    assert_eq!(run_a, run_b, "same seed must give identical final state");
}

#[test]
fn different_seeds_diverge() {
    let (a, _) = replay("contract-seed-1");
    let (b, _) = replay("contract-seed-2");
    assert_ne!(a, b, "different seeds must diverge");
}
