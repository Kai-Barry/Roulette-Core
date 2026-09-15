fn main() {
    let content = std::sync::Arc::new(roulette_content::Content::load_dir("content").unwrap());
    let filler = content.cards.iter().find(|c| c.id == "crimson_double").unwrap();
    let wheel_def = content.wheels.iter().find(|w| w.id == "classic").unwrap();
    let wheel = roulette_core::wheel::WheelConfig::from_def(wheel_def);
    let layout = roulette_sim::ev::player_layout(&wheel, &Default::default(), 0);
    let deck: Vec<roulette_content::schema::CardDef> =
        std::iter::repeat_n(filler.clone(), 8).chain(Some(filler.clone())).collect();
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
    let mut rng = roulette_core::rng::Rng::new(roulette_sim::ev::djb2("ev:baseline:deal"));
    battle.deal_from_defs(&deck, &mut rng);
    battle.begin_betting(roulette_core::battle::Side::Player);
    battle.chips_pool = 30;
    println!("placed={:?}", battle.place_bet(roulette_core::bets::BetType::Red, 10));
    let spin_rng = roulette_core::rng::Rng::new(roulette_sim::ev::djb2("ev:baseline:spin:0"));
    let (_e, result) = roulette_core::phys::Simulator::uniform_run(&layout, 1, spin_rng);
    let landed = layout.numbers[result.slots[0]];
    println!(
        "dbg: slot={} numbers[{}]={} effective_color={:?} layout_red35={:?} wheel_red35={:?}",
        result.slots[0],
        result.slots[0],
        layout.numbers[result.slots[0]],
        wheel.effective_color(landed, 0, None),
        layout.colors[result.slots[0]] == roulette_content::schema::SlotColor::Red,
        wheel.effective_color(35, 0, None),
    );
    let input = roulette_core::battle::state::SpinInput {
        side: roulette_core::battle::Side::Player,
        landed,
        extra_balls: vec![],
        prediction: None,
        low_rarity_physics_played: false,
    };
    println!("bets_before_resolve={}", battle.bets.len());
    let outcome = battle.resolve_spin(&input);
    println!("landed={landed} payout={} bets_before={}", outcome.total_payout, battle.bets.len());
}
