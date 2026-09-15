# Roulette Core (`roulette_core`)

> **Fast, deterministic, modular, headless engine & high-speed Monte Carlo simulator written in Rust.**

`roulette_core` serves as the underlying state machine, combat resolution engine, and balance simulation foundation for ***Roulette of the Damned***—a dark, atmospheric 3D roguelike deckbuilder (inspired by *Inscryption* and *Balatro*) where combat is driven by customized game devices instead of standard attacks.

---

## Key Features

- **Pluggable & Highly Modular**: Built around trait abstractions (`GameDevice`, `Target`, `CombatMode`), allowing future developments to add custom devices (wheels, dice, pachinko, slot machines), custom combat modes (1v1, horde survival, boss phases), and multi-target encounters without touching core turn logic.
- **Blazing Speed**: Runs Monte Carlo battle simulations at **over 1.3 Million turns per second** on a single thread even with dynamic trait dispatch.
- **Pure Cross-Platform Determinism**: Built with a custom, seedable **Mulberry32 PRNG** guaranteeing 100% identical spin outcomes, draw orders, and combat logs across platforms given identical seeds.
- **Zero External Dependencies**: Standard library (`std`) only for ultra-fast compilation, zero bloat, and easy embedding into WebAssembly (`wasm32`) or game engine FFI wrappers (Unity C#, Godot, Unreal C++).
- **Rich Card & Bet System**: Implements modifier card categories (*Roulette Modifiers*, *Board Modifiers*, *Physics Rerolls*, *Risk/Utility*) and flexible bet types (`Red`, `Black`, `Green`, `ExactNumber`, `Even`, `Odd`, `High`, `Low`, `CustomTag`).

---

## Modular Architecture Overview

```
src/
├── lib.rs          # Exposes modular library API
├── rng.rs          # Seedable Mulberry32 PRNG & Fisher-Yates shuffle
├── device.rs       # GameDevice trait, OutcomeSlot, EuropeanWheel, AmericanWheel, DiceDevice
├── bet.rs          # Bet & BetType device-agnostic payout evaluation
├── target.rs       # Target trait for enemy AI, boss phases, multi-target groups
├── cards.rs        # Cards, CardType, CardEffect, device modification hooks
├── mode.rs         # CombatMode strategy trait (Standard1v1Mode, HordeMode)
├── combat.rs       # CombatState orchestrator and turn execution pipeline
├── wheel.rs        # Backward-compatibility re-export wrappers
├── main.rs         # Modular CLI demo binary
└── bin/
    └── simulate.rs # Monte Carlo simulation binary for high-speed balancing
```

---

## Getting Started

### Running Interactive Modular CLI Demo

```bash
cargo run --bin roulette_core
```

### Running High-Speed Monte Carlo Simulator

```bash
cargo run --release --bin simulate
```

### Running Automated Tests

```bash
cargo test
```

---

## License

Created by Kai-Barry. All rights reserved.
