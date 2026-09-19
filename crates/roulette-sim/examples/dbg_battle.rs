fn main() {
    let content = std::sync::Arc::new(roulette_content::Content::load_dir("content").unwrap());
    let mut engine = roulette_core::api::Engine::new(content.clone(), "sim:0");
    engine.set_spin_sampling(roulette_core::api::SpinSampling::Uniform);
    engine
        .apply(&roulette_core::api::Command::StartRun { difficulty: Default::default() })
        .unwrap();
    let offer = engine.run().and_then(|r| r.loadout_offer.clone()).unwrap();
    for id in &offer.card_ids {
        let _ = engine.apply(&roulette_core::api::Command::DraftCard { card_id: id.clone() });
    }
    engine.apply(&roulette_core::api::Command::DraftWheel).unwrap();
    engine.apply(&roulette_core::api::Command::FinishLoadout).unwrap();
    let node = engine
        .run()
        .map(|run| run.map.floors.iter().flatten().next().map(|n| n.id.clone()).unwrap())
        .unwrap();
    engine.apply(&roulette_core::api::Command::PickNode { node_id: node }).unwrap();
    for i in 0..12 {
        let state = engine.run().unwrap().state;
        if state != roulette_core::run::state::GameState::Combat {
            println!("ended: {state:?} after {i} spins");
            break;
        }
        let battle = engine.battle().unwrap();
        println!(
            "iter {i}: round={} phase={:?} turn={:?} chips={}",
            battle.round, battle.phase, battle.turn, battle.chips_pool
        );
        let chips = battle.chips_pool;
        if chips >= 10 {
            let _ = engine.apply(&roulette_core::api::Command::PlaceBet {
                bet: roulette_core::bets::BetType::Red,
                amount: 10,
            });
        }
        match engine.apply(&roulette_core::api::Command::Spin) {
            Ok(_) => {}
            Err(e) => {
                println!("spin err: {e:?}");
                break;
            }
        }
    }
}
