//! `simulate battles` — full-battle Monte Carlo (TASK-045/048): fixed
//! aggressive policy (play affordable cards, bet red 10, spin) over the real
//! Engine facade. Two spin modes: `physics` (real 120 Hz sim) and `uniform`
//! (statistically equivalent fast path, zero sim steps).

use std::sync::Arc;
use std::time::Instant;

use roulette_content::Content;

use roulette_core::api::{Command, Engine, EngineEvent, SpinSampling};
use roulette_core::bets::BetType;
use roulette_core::run::map::NodeType;
use roulette_core::run::state::GameState;

const USAGE: &str = "usage: simulate battles [--runs N] [--mode physics|uniform] [--seed-prefix s]";

fn arg(args: &[String], flag: &str) -> Option<String> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1).cloned())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("battles") {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }
    let rest = &args[1..];
    if rest.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return;
    }
    let runs: usize = arg(rest, "--runs").and_then(|v| v.parse().ok()).unwrap_or(200);
    let mode = arg(rest, "--mode").unwrap_or_else(|| "uniform".into());
    let seed_prefix = arg(rest, "--seed-prefix").unwrap_or_else(|| "sim".into());
    let sampling = match mode.as_str() {
        "uniform" => SpinSampling::Uniform,
        "physics" => SpinSampling::Physics,
        other => {
            eprintln!("{USAGE}");
            eprintln!("unknown mode `{other}`");
            std::process::exit(2);
        }
    };

    let content = match Content::load_dir("content") {
        Ok(c) => Arc::new(c),
        Err(_) => Arc::new(Content::embedded().expect("embedded content validates")),
    };

    let started = Instant::now();
    let mut victories = 0u32;
    let mut battles_won = 0u32;
    let mut battles_total = 0u32;
    let mut spins_total = 0u32;

    for r in 0..runs {
        let seed = format!("{seed_prefix}:{r}");
        let mut engine = Engine::new(content.clone(), &seed);
        engine.set_spin_sampling(sampling);

        if engine.apply(&Command::StartRun { difficulty: Default::default() }).is_err() {
            eprintln!("run {r}: start failed");
            continue;
        }
        // Draft the full loadout offer (5 cards + wheel) with a fixed policy.
        let offer = engine
            .run()
            .and_then(|run| run.loadout_offer.clone())
            .expect("start opens the loadout");
        for card_id in &offer.card_ids {
            let _ = engine.apply(&Command::DraftCard { card_id: card_id.clone() });
        }
        let _ = engine.apply(&Command::DraftWheel);
        if engine.apply(&Command::FinishLoadout).is_err() {
            eprintln!("run {r}: loadout failed");
            continue;
        }

        // Walk the map: prefer combat nodes, always the first pickable.
        let mut guard = 0usize;
        'run: while guard < 400 {
            guard += 1;
            let Some(state) = engine.run().map(|run| run.state) else { break };
            match state {
                GameState::Victory => {
                    victories += 1;
                    break;
                }
                GameState::GameOver => break,
                GameState::Map => {
                    let pickable = engine.run().expect("run").pickable_nodes();
                    if pickable.is_empty() {
                        break;
                    }
                    let node = engine
                        .run()
                        .map(|run| {
                            run.map
                                .floors
                                .iter()
                                .flatten()
                                .find(|n| {
                                    pickable.contains(&n.id) && n.node_type == NodeType::Combat
                                })
                                .map(|n| n.id.clone())
                                .unwrap_or_else(|| pickable[0].clone())
                        })
                        .expect("run");
                    if engine.apply(&Command::PickNode { node_id: node }).is_err() {
                        break;
                    }
                }
                GameState::Combat => {
                    let mut bguard = 0usize;
                    while engine.run().map(|r| r.state) == Some(GameState::Combat) {
                        bguard += 1;
                        if bguard > 200 {
                            break 'run;
                        }
                        // Play affordable hand cards (index 0 until failure).
                        // Rejected plays return the card to its hand slot with
                        // Ok(()), so attempts are bounded by the hand size.
                        let attempts = engine.battle().map(|b| b.hand.len()).unwrap_or(0);
                        for _ in 0..attempts {
                            if engine.apply(&Command::PlayCard { hand_index: 0 }).is_err() {
                                break;
                            }
                        }
                        // Bet red 10 when affordable.
                        if let Some(battle) = engine.battle() {
                            if battle.chips_pool >= 10 {
                                let _ = engine
                                    .apply(&Command::PlaceBet { bet: BetType::Red, amount: 10 });
                            }
                        }
                        let before = engine.events().len();
                        if engine.apply(&Command::Spin).is_err() {
                            break;
                        }
                        spins_total += engine.events()[before..]
                            .iter()
                            .filter(|e| matches!(e, EngineEvent::SpinStarted { .. }))
                            .count() as u32;
                    }
                    battles_total += 1;
                    if engine.run().map(|r| r.state == GameState::Map).unwrap_or(false) {
                        battles_won += 1;
                    }
                }
                // Shop/event/forge nodes resolve via a fixed policy: take the
                // first event choice; shop/forge stalls end the run walk.
                GameState::Event => {
                    let choice = content
                        .events
                        .first()
                        .and_then(|e| e.choices.first())
                        .map(|c| c.id.clone());
                    match choice {
                        Some(choice_id) => {
                            if engine.apply(&Command::EventChoose { choice_id }).is_err() {
                                break 'run;
                            }
                        }
                        None => break 'run,
                    }
                }
                GameState::Shop | GameState::Forge | GameState::LoadoutStore | GameState::Menu => {
                    break 'run
                }
            }
        }
    }

    let elapsed = started.elapsed().as_secs_f64();
    let win_rate = victories as f64 / runs.max(1) as f64 * 100.0;
    let battle_win_rate = battles_won as f64 / battles_total.max(1) as f64 * 100.0;
    println!("mode={mode} runs={runs} seed_prefix={seed_prefix}");
    println!("run victories      : {victories}/{runs} ({win_rate:.1}%)");
    println!("battles won        : {battles_won}/{battles_total} ({battle_win_rate:.1}%)");
    println!("total spins        : {spins_total}");
    println!("wall time          : {elapsed:.2}s");
    println!(
        "throughput         : {:.0} battles/s, {:.0} spins/s",
        battles_total as f64 / elapsed,
        spins_total as f64 / elapsed
    );
}
