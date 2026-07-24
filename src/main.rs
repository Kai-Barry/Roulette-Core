#![allow(dead_code)]

mod cards;
mod combat;
mod rng;
mod wheel;

use combat::CombatState;
use wheel::{Bet, BetType};

fn main() {
    println!("==================================================");
    println!("  ROULETTE OF THE DAMNED - HEADLESS RUST ENGINE   ");
    println!("==================================================");

    let seed = "seed_test_run_101";
    let mut state = CombatState::new(seed);

    println!("Initialized combat with seed: '{}'", seed);
    println!("Player HP: {} | Chips: {}", state.player.hp, state.player.chips);
    println!("Enemy: {} (HP: {})", state.enemy.name, state.enemy.hp);

    println!("\n[Hand Cards]:");
    for (i, card) in state.player.hand.iter().enumerate() {
        println!("  {}. [{}] - {}", i + 1, card.name, card.description);
    }

    // Play card #1 (Red Fever) and place $10 bet on Red
    println!("\n--> Playing Card 1 (Red Fever) & Placing 10 Chips on RED...");
    let played_cards = vec![0];
    let bets = vec![Bet {
        bet_type: BetType::Red,
        amount: 10,
    }];

    let result = state.execute_turn(&played_cards, &bets);

    println!("\n=== Turn Execution Log ===");
    for log in &result.log_messages {
        println!("  {}", log);
    }

    println!("\n=== Updated Game State ===");
    println!("Landed Slot : Number {} ({})", result.slot_landed.number, result.slot_landed.color);
    println!("Damage Dealt: {} | Chips Won: {}", result.total_damage_dealt, result.total_chips_won);
    println!("Player HP   : {} | Enemy HP  : {}", state.player.hp, state.enemy.hp);
    println!("==================================================");
}
