//! Monte Carlo High-Speed Battle Simulator for *Roulette of the Damned*.
//!
//! Runs thousands of full battle iterations in milliseconds to benchmark engine throughput,
//! quantify win/loss rates, and balance card power levels across different game modes.

#![allow(dead_code)]

use roulette_core::bet::{Bet, BetType};
use roulette_core::combat::CombatState;
use roulette_core::mode::CombatOutcome;
use std::time::Instant;

fn main() {
    println!("==================================================");
    println!("  HIGH-SPEED MONTE CARLO ROULETTE SIMULATOR       ");
    println!("==================================================");

    let total_runs = 10_000;
    let start_time = Instant::now();

    let mut wins = 0;
    let mut losses = 0;
    let mut total_turns = 0;
    let mut total_damage = 0;

    for i in 0..total_runs {
        let seed = format!("sim_run_{}", i);
        let mut state = CombatState::new(&seed);

        while state.player.hp.unwrap_or(100) > 0 && state.enemy().hp() > 0 && state.turn_number <= 20 {
            // Play available cards if possible
            let mut cards_to_play = Vec::new();
            if !state.player.hand.is_empty() {
                cards_to_play.push(0);
            }

            // Strategy: Bet $10 on Red
            let bets = vec![Bet {
                bet_type: BetType::Red,
                amount: 10,
            }];

            let res = state.execute_turn(&cards_to_play, &bets);
            total_damage += res.total_damage_dealt;
            total_turns += 1;

            if res.outcome == CombatOutcome::PlayerVictory {
                break;
            }
        }

        if state.enemy().hp() <= 0 {
            wins += 1;
        } else {
            losses += 1;
        }
    }

    let elapsed = start_time.elapsed();
    let win_rate = (wins as f64 / total_runs as f64) * 100.0;
    let avg_turns = total_turns as f64 / total_runs as f64;
    let ops_per_sec = (total_turns as f64) / elapsed.as_secs_f64();

    println!("Simulated {} Full Fights", total_runs);
    println!("Time Elapsed      : {:.2?}", elapsed);
    println!("Turns Processed   : {} turns", total_turns);
    println!("Simulation Speed  : {:.0} turns/sec", ops_per_sec);
    println!("Player Win Rate   : {:.2}% ({}/{} wins, {} losses)", win_rate, wins, total_runs, losses);
    println!("Loss Rate         : {:.2}%", 100.0 - win_rate);
    println!("Average Turns/Game: {:.2}", avg_turns);
    println!("Total Damage Dealt: {} HP", total_damage);
    println!("==================================================");
}
