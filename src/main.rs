//! Interactive Terminal Engine for *Roulette of the Damned*.
//!
//! Provides a playable, turn-by-turn interactive CLI game mode supporting Point Round High-Score Attack (Hand-Limit),
//! custom devices, and custom combat modes with mode-specific card pools and HUDs.

#![allow(dead_code)]

use roulette_core::bet::{Bet, BetType};
use roulette_core::combat::CombatState;
use roulette_core::device::{AmericanWheel, DiceDevice, GameDevice};
use roulette_core::mode::{CombatOutcome, HordeMode, PointRoundMode, Standard1v1Mode};
use roulette_core::target::{Enemy, Target};
use std::env;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Support non-interactive mode for automated testing / CI
    if args.iter().any(|arg| arg == "--non-interactive" || arg == "--demo") {
        run_demo_mode();
        return;
    }

    run_interactive_cli();
}

/// Runs the full interactive terminal combat loop.
fn run_interactive_cli() {
    clear_screen();
    println!("================================================================================");
    println!("             R O U L E T T E   O F   T H E   D A M N E D                       ");
    println!("                     Headless Interactive Engine                                ");
    println!("================================================================================");

    loop {
        println!("\nSELECT GAME MODE:");
        println!("  1. Point Round Mode (Balatro Style: Hand-Limit Score Attack - No Player HP)");
        println!("  2. Standard 1v1 Survival vs The Cursed Croupier (European Wheel)");
        println!("  3. American Double-Zero Wheel (1v1 vs The Cursed Croupier)");
        println!("  4. Elemental D6 Die (Horde Survival vs 3 Fire Fiends)");
        println!("  5. Exit Game");
        print!("\nEnter choice (1-5): ");
        io::stdout().flush().ok();

        let choice = read_input_line();
        if choice == "5" || choice.eq_ignore_ascii_case("exit") {
            println!("\nThank you for playing Roulette of the Damned!");
            break;
        }

        let seed = generate_random_seed();
        let mut state = match choice.as_str() {
            "1" => {
                print!("   Enter maximum hands allowed for this round (default 4): ");
                io::stdout().flush().ok();
                let hands_input = read_input_line();
                let max_hands = hands_input.parse::<u32>().unwrap_or(4).max(1);

                print!("   Enter target score quota to beat (e.g. 200, or 0 for unlimited high score): ");
                io::stdout().flush().ok();
                let score_input = read_input_line();
                let target_score = score_input.parse::<u32>().unwrap_or(200);

                let dev: Box<dyn GameDevice> = Box::new(roulette_core::device::EuropeanWheel::new());
                let target: Box<dyn Target> = Box::new(Enemy::new("The Dealer's Vault", 10000));
                let mode = Box::new(PointRoundMode::new(max_hands, target_score));

                CombatState::new_custom(&seed, dev, vec![target], mode)
            }
            "3" => {
                let dev: Box<dyn GameDevice> = Box::new(AmericanWheel::new());
                let target: Box<dyn Target> = Box::new(Enemy::create_pit_boss());
                let mode = Box::new(Standard1v1Mode::default());
                CombatState::new_custom(&seed, dev, vec![target], mode)
            }
            "4" => {
                let dev: Box<dyn GameDevice> = Box::new(DiceDevice::new_standard_d6());
                let targets: Vec<Box<dyn Target>> = vec![
                    Box::new(Enemy::new("Fire Fiend #1", 30)),
                    Box::new(Enemy::new("Fire Fiend #2", 30)),
                    Box::new(Enemy::new("Fire Fiend #3", 30)),
                ];
                let mode = Box::new(HordeMode::new(3));
                CombatState::new_custom(&seed, dev, targets, mode)
            }
            _ => CombatState::new(&seed),
        };

        play_battle_encounter(&mut state);

        print!("\nWould you like to play another encounter? (y/n): ");
        io::stdout().flush().ok();
        let again = read_input_line();
        if !again.eq_ignore_ascii_case("y") && !again.eq_ignore_ascii_case("yes") {
            println!("\nFarewell, gambler...");
            break;
        }
        clear_screen();
    }
}

/// Plays out a single encounter turn-by-turn interactively.
fn play_battle_encounter(state: &mut CombatState) {
    println!("\n[+] Encounter Initialized!");
    println!("    Device: {}", state.device.name());
    println!("    Rules : {}", state.mode.name());

    let mut cumulative_score: u32 = 0;

    loop {
        println!("\n================================================================================");
        println!(" HAND {} | DEVICE: {}", state.turn_number, state.device.name());
        println!("--------------------------------------------------------------------------------");
        if let (Some(hp), Some(max_hp)) = (state.player.hp, state.player.max_hp) {
            println!(" PLAYER HP : [{}/{}] | CHIPS: {}", hp, max_hp, state.player.chips);
        } else {
            println!(" MODE      : POINT ROUND (HAND-LIMIT SCORE ATTACK) | CHIPS: {}", state.player.chips);
        }
        println!(" SCORE     : {} POINTS ACCUMULATED", cumulative_score);

        if state.mode.has_player_hp() {
            print!(" TARGETS   : ");
            for target in &state.targets {
                if target.is_alive() {
                    print!("{} (HP: {}/{})  ", target.name(), target.hp(), target.max_hp());
                }
            }
            println!();
        }
        println!("--------------------------------------------------------------------------------");

        // 1. Display Hand Cards
        println!(" YOUR HAND CARDS:");
        if state.player.hand.is_empty() {
            println!("   (Hand is empty)");
        } else {
            for (i, card) in state.player.hand.iter().enumerate() {
                println!("   {}. [{}] (Cost {}) - {}", i + 1, card.name, card.cost, card.description);
            }
        }

        // 2. Select Cards to Play
        println!("\n SELECT CARD(S) TO PLAY:");
        println!("   Enter card numbers separated by spaces (e.g. '1 2'), or press Enter to skip.");
        print!("   Cards to play: ");
        io::stdout().flush().ok();

        let card_input = read_input_line();
        let mut played_indices = Vec::new();
        for token in card_input.split_whitespace() {
            if let Ok(num) = token.parse::<usize>() {
                if num >= 1 && num <= state.player.hand.len() {
                    played_indices.push(num - 1);
                }
            }
        }

        // 3. Select Bet Type & Bet Amount
        println!("\n PLACE YOUR BET:");
        println!("   1. Red       (2.0x base payout)");
        println!("   2. Black     (2.0x base payout)");
        println!("   3. Green     (14.0x base payout)");
        println!("   4. Exact Num (36.0x base payout)");
        println!("   5. Even      (2.0x base payout)");
        println!("   6. Odd       (2.0x base payout)");
        println!("   7. High 19-36(2.0x base payout)");
        println!("   8. Low 1-18  (2.0x base payout)");
        print!("   Choose bet type (1-8, default 1 Red): ");
        io::stdout().flush().ok();

        let bet_choice = read_input_line();
        let bet_type = match bet_choice.as_str() {
            "2" => BetType::Black,
            "3" => BetType::Green,
            "4" => {
                print!("   Enter exact slot number (0-36): ");
                io::stdout().flush().ok();
                let num_str = read_input_line();
                let num = num_str.parse::<u32>().unwrap_or(7);
                BetType::ExactNumber(num)
            }
            "5" => BetType::Even,
            "6" => BetType::Odd,
            "7" => BetType::High,
            "8" => BetType::Low,
            _ => BetType::Red,
        };

        print!("   Enter chip amount to bet (Available: {} chips): ", state.player.chips);
        io::stdout().flush().ok();
        let amount_str = read_input_line();
        let bet_amount = amount_str.parse::<u32>().unwrap_or(10).min(state.player.chips).max(1);

        let bets = vec![Bet {
            bet_type,
            amount: bet_amount,
        }];

        // 4. Spin Device Animation
        print!("\n[+] Spinning {} ", state.device.name());
        for _ in 0..3 {
            print!(".");
            io::stdout().flush().ok();
            thread::sleep(Duration::from_millis(200));
        }
        println!();

        // 5. Execute Turn
        let result = state.execute_turn(&played_indices, &bets);
        cumulative_score += result.total_chips_won;

        println!("\n--------------------------------------------------------------------------------");
        println!(" === HAND RESULT LOG ===");
        for log in &result.log_messages {
            println!("   * {}", log);
        }
        println!("--------------------------------------------------------------------------------");
        println!(" LANDED SLOT  : Number {} ({})", result.slot_landed.number, result.slot_landed.color);
        println!(" HAND SCORE   : +{} POINTS | TOTAL SCORE: {} POINTS", result.total_chips_won, cumulative_score);

        match result.outcome {
            CombatOutcome::PlayerVictory => {
                println!("\n********************************************************************************");
                println!("          VICTORY! TARGET SCORE ACHIEVED / ALL ROUNDS COMPLETED!                ");
                println!("          FINAL CUMULATIVE SCORE: {} POINTS                             ", cumulative_score);
                println!("********************************************************************************");
                break;
            }
            CombatOutcome::PlayerDefeat => {
                println!("\n================================================================================");
                println!("          HAND LIMIT REACHED WITHOUT MEETING TARGET SCORE QUOTA!                ");
                println!("          FINAL CUMULATIVE SCORE: {} POINTS                             ", cumulative_score);
                println!("================================================================================");
                break;
            }
            CombatOutcome::TurnLimitReached => {
                println!("\n--------------------------------------------------------------------------------");
                println!("          ROUND ENDED! FINAL SCORE: {} POINTS                           ", cumulative_score);
                println!("--------------------------------------------------------------------------------");
                break;
            }
            CombatOutcome::InProgress => {}
        }
    }
}

/// Runs non-interactive script demo for CI / testing.
fn run_demo_mode() {
    println!("==================================================");
    println!("  ROULETTE OF THE DAMNED - DEMO SCRIPT RUN        ");
    println!("==================================================");

    let mut state = CombatState::new("demo_seed_101");
    let result = state.execute_turn(&[0], &[Bet { bet_type: BetType::Red, amount: 10 }]);

    for log in &result.log_messages {
        println!("  {}", log);
    }
    println!("Landed Slot: Number {} ({})", result.slot_landed.number, result.slot_landed.color);
    println!("Damage Dealt: {} | Chips Won: {}", result.total_damage_dealt, result.total_chips_won);
    println!("==================================================");
}

/// Reads a trimmed line of user input from stdin.
fn read_input_line() -> String {
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer).ok();
    buffer.trim().to_string()
}

/// Generates a random seed string from system timestamp.
fn generate_random_seed() -> String {
    let duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("seed_{}_{}", duration.as_secs(), duration.subsec_nanos())
}

/// Clears screen using ANSI terminal sequence.
fn clear_screen() {
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().ok();
}
