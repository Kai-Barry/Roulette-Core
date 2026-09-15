// Debug: replicate scenario_ev verbatim, print (landed, bets, payout).
fn main() {
    let content = std::sync::Arc::new(roulette_content::Content::load_dir("content").unwrap());
    let wheel_def =
        content.wheels.iter().find(|w| w.id == roulette_sim::ev::SCENARIO_WHEEL_ID).unwrap();
    let wheel = roulette_core::wheel::WheelConfig::from_def(wheel_def);
    let layout = roulette_sim::ev::player_layout(&wheel, &Default::default(), 0);
    println!("layout numbers: {}", layout.numbers.len());

    for i in 0..6 {
        let mut battle = roulette_core::battle::BattleState::new(
            wheel.clone(),
            wheel.clone(),
            80,
            80,
            9999,
            99,
            vec![],
            roulette_core::battle::CombatMode::Points,
        );
        battle.begin_betting(roulette_core::battle::Side::Player);
        battle.chips_pool = 30;
        let placed = battle.place_bet(roulette_core::bets::BetType::Red, 10);
        println!("placed={placed:?} bets={}", battle.bets.len());
        let spin_rng =
            roulette_core::rng::Rng::new(roulette_sim::ev::djb2(&format!("ev:baseline:spin:{i}")));
        let (_events, result) = roulette_core::phys::Simulator::uniform_run(&layout, 1, spin_rng);
        let landed = layout.numbers[result.slots[0]];
        let input = roulette_core::battle::state::SpinInput {
            side: roulette_core::battle::Side::Player,
            landed,
            extra_balls: vec![],
            prediction: None,
            low_rarity_physics_played: false,
        };
        let outcome = battle.resolve_spin(&input);
        println!(
            "i={i} landed={landed:>2} slot={} payout={} bets_at_resolve={}",
            result.slots[0],
            outcome.total_payout,
            battle.bets.len()
        );
    }
}
